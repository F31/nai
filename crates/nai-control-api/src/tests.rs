use super::*;
use nai_media_core::{MediaError, MediaGate};

const TOKEN: &str = "test_only_credential_01234567890123456789";
struct Harness {
    api: Arc<ControlApi>,
    gate: MediaGate,
    url: String,
    client: reqwest::Client,
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<std::io::Result<()>>,
}
impl Drop for Harness {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
        self.task.abort();
    }
}
async fn harness(permissions: &[&str], ttl: Duration) -> Harness {
    let mut gate = MediaGate::new("s", 1);
    gate.arm(
        PlaybackLease {
            lease_id: "test-lease".into(),
            session_id: "s".into(),
            generation_epoch: 1,
            allowed_utterance_ids: vec!["u".into()],
            mode: SessionMode::Assisted as i32,
            policy_revision: "test-policy".into(),
            expires_at_utc_ms: 1600,
            ..Default::default()
        },
        1000,
        Instant::now(),
    )
    .unwrap();
    let access = Arc::new(Access::new(TOKEN, "device", permissions, ttl).unwrap());
    let api = Arc::new(
        ControlApi::new(Session::new("s", "w").unwrap(), gate.emergency(), access).unwrap(),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (stop, shutdown) = watch::channel(false);
    let task = tokio::spawn(serve(listener, api.clone(), shutdown));
    Harness {
        api,
        gate,
        url,
        client: reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap(),
        stop,
        task,
    }
}
fn body(id: &str, seq: u64, epoch: u64, revision: u64) -> Value {
    json!({"command_id":id,"seq":seq.to_string(),"device_id":"device","session_id":"s","generation_epoch":epoch.to_string(),"expected_state_revision":revision.to_string(),"ttl_ms":10000})
}
async fn post(h: &Harness, route: &str, body: &Value) -> reqwest::Response {
    h.client
        .post(format!("{}{route}", h.url))
        .bearer_auth(TOKEN)
        .json(body)
        .send()
        .await
        .unwrap()
}
fn validates(name: &str, value: &Value) {
    let mut schema: Value = serde_json::from_str(SCHEMAS_JSON).unwrap();
    schema["$ref"] = json!(format!("#/$defs/{name}"));
    assert!(
        jsonschema::validator_for(&schema).unwrap().is_valid(value),
        "{name}: {value}"
    );
}

#[tokio::test]
async fn expired_body_budget_does_not_execute_media_signal() {
    let mut h = harness(&["media.mute"], Duration::from_secs(10)).await;
    let response = h.api.command(
        &serde_json::to_vec(&body("expired", 1, 0, 0)).unwrap(),
        CommandAction::Mute,
        Instant::now() - Duration::from_secs(11),
    );
    assert_eq!(response.status(), 409);
    assert!(h.gate.poll(Instant::now()).is_ok());
}

#[tokio::test]
async fn real_http_status_is_authenticated_and_matches_generated_schema() {
    let h = harness(&["runtime.read", "session.read"], Duration::from_secs(10)).await;
    for route in [
        "/api/v1/health",
        "/api/v1/runtime/status",
        "/api/v1/sessions/s",
    ] {
        let response = h
            .client
            .get(format!("{}{route}", h.url))
            .bearer_auth(TOKEN)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let value: Value = response.json().await.unwrap();
        validates("ResourceView", &value);
        assert_eq!(value["session"]["lifecycle"], "created");
        assert_eq!(value["session"]["generation_epoch"], "0");
        assert_eq!(value["session"]["media_state"], "standby");
    }
}

#[tokio::test]
async fn identity_permissions_and_scope_fail_without_muting() {
    let mut h = harness(&["runtime.read"], Duration::from_secs(10)).await;
    for token in ["", "wrong"] {
        let response = h
            .client
            .get(format!("{}/api/v1/runtime/status", h.url))
            .bearer_auth(token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 401);
        validates("ErrorResponse", &response.json().await.unwrap());
    }
    assert_eq!(
        post(&h, "/api/v1/sessions/s/mute", &body("a", 1, 0, 0))
            .await
            .status(),
        403
    );
    assert_eq!(
        post(&h, "/api/v1/sessions/other/mute", &body("a", 1, 0, 0))
            .await
            .status(),
        403
    );
    assert!(h.gate.poll(Instant::now()).is_ok());
    h.api.access.revoke();
    assert_eq!(
        h.client
            .get(format!("{}/api/v1/health", h.url))
            .bearer_auth(TOKEN)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
}

#[tokio::test]
async fn mute_takeover_seq_cas_and_retries_execute_once() {
    let mut h = harness(&["media.mute", "session.takeover"], Duration::from_secs(10)).await;
    let command = body("mute", 1, 0, 0);
    let first = post(&h, "/api/v1/sessions/s/mute", &command).await;
    assert_eq!(first.status(), 200);
    let ack: Value = first.json().await.unwrap();
    validates("CommandAck", &ack);
    assert_eq!(ack["generation_epoch"], "1");
    assert_eq!(ack["executed"], true);
    assert_eq!(h.gate.poll(Instant::now()), Err(MediaError::Muted));
    let retry: Value = post(&h, "/api/v1/sessions/s/mute", &command)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(ack, retry);
    assert_eq!(
        post(&h, "/api/v1/sessions/s/mute", &body("mute", 2, 1, 1))
            .await
            .status(),
        409
    );
    assert_eq!(
        post(&h, "/api/v1/sessions/s/mute", &body("old-seq", 1, 1, 1))
            .await
            .status(),
        409
    );
    assert_eq!(
        post(&h, "/api/v1/sessions/s/mute", &body("old-state", 2, 0, 0))
            .await
            .status(),
        409
    );
    let takeover = post(
        &h,
        "/api/v1/sessions/s/takeover",
        &body("takeover", 2, 1, 1),
    )
    .await;
    assert_eq!(takeover.status(), 200);
    assert_eq!(
        takeover.json::<Value>().await.unwrap()["generation_epoch"],
        "2"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn busy_state_and_full_ledger_do_not_prevent_media_revocation() {
    let mut h = harness(&["media.mute"], Duration::from_secs(10)).await;
    let api = h.api.clone();
    {
        let (ready, acquired) = std::sync::mpsc::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let blocked = api.clone();
        let holder = std::thread::spawn(move || {
            let _busy = blocked.state.lock().unwrap();
            ready.send(()).unwrap();
            let _ = wait.recv();
        });
        acquired.recv().unwrap();
        assert_eq!(
            post(&h, "/api/v1/sessions/s/mute", &body("busy", 1, 0, 0))
                .await
                .status(),
            409
        );
        assert_eq!(h.gate.poll(Instant::now()), Err(MediaError::Muted));
        release.send(()).unwrap();
        holder.join().unwrap();
    }
    api.state.lock().unwrap().commands = CommandLedger::new(0);
    let response = post(&h, "/api/v1/sessions/s/mute", &body("full", 1, 0, 0)).await;
    assert_eq!(response.status(), 409);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "RESOURCE_EXHAUSTED"
    );
    assert_eq!(api.session.lock().unwrap().snapshot().generation_epoch, 0);
}

#[tokio::test]
async fn malformed_unknown_oversized_and_foreign_device_commands_are_rejected() {
    let mut h = harness(&["media.mute"], Duration::from_secs(10)).await;
    for (field, value, status) in [
        ("secret", json!("not-allowed"), 400),
        ("seq", json!("18446744073709551616"), 400),
        ("device_id", json!("other"), 403),
    ] {
        let mut command = body("bad", 1, 0, 0);
        command[field] = value;
        assert_eq!(
            post(&h, "/api/v1/sessions/s/mute", &command).await.status(),
            status
        );
    }
    let response = h
        .client
        .post(format!("{}/api/v1/sessions/s/mute", h.url))
        .bearer_auth(TOKEN)
        .header("content-type", "application/json")
        .body(vec![b' '; 4097])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 413);
    assert!(h.gate.poll(Instant::now()).is_ok());
}

#[tokio::test]
async fn cors_queries_unimplemented_routes_and_expired_access_are_explicit() {
    let h = harness(&["runtime.read"], Duration::from_secs(10)).await;
    let response = h
        .client
        .get(format!("{}/api/v1/health", h.url))
        .bearer_auth(TOKEN)
        .header("origin", ORIGIN)
        .send()
        .await
        .unwrap();
    assert_eq!(response.headers()["access-control-allow-origin"], ORIGIN);
    assert_eq!(
        h.client
            .get(format!("{}/api/v1/health", h.url))
            .bearer_auth(TOKEN)
            .header("origin", "https://untrusted.invalid")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        h.client
            .get(format!("{}/api/v1/health?token=not-allowed", h.url))
            .bearer_auth(TOKEN)
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    let response = h
        .client
        .get(format!("{}/api/v1/plugins", h.url))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "CAPABILITY_ABSENT"
    );
    let expired = harness(&["runtime.read"], Duration::from_millis(1)).await;
    tokio::time::sleep(Duration::from_millis(5)).await;
    assert_eq!(
        expired
            .client
            .get(format!("{}/api/v1/health", expired.url))
            .bearer_auth(TOKEN)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
}

#[tokio::test]
async fn server_rejects_external_binding_and_shutdown_closes_listener() {
    let h = harness(&[], Duration::from_secs(10)).await;
    let external = TcpListener::bind("0.0.0.0:0").await.unwrap();
    let (_tx, rx) = watch::channel(false);
    assert_eq!(
        serve(external, h.api.clone(), rx).await.unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    h.stop.send(true).unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(
        h.client
            .get(format!("{}/api/v1/health", h.url))
            .send()
            .await
            .is_err()
    );
}

fn approve_body(id: &str, seq: u64, epoch: u64, revision: u64, hash: &str) -> Value {
    json!({"command_id":id,"seq":seq.to_string(),"device_id":"device","session_id":"s","generation_epoch":epoch.to_string(),"expected_state_revision":revision.to_string(),"ttl_ms":10000,"candidate_hash":hash,"hash_algorithm":"sha256-canonical-v1"})
}

fn seed_approval_store() -> (tempfile::TempDir, nai_storage::Store, String) {
    let dir = tempfile::tempdir().unwrap();
    // `Store::open` on a fresh data root runs the whole schema migration chain --
    // that is bring-up work, not a per-call read, so it uses the bring-up budget.
    let op = Operation::new(nai_storage::BRINGUP_OPERATION).unwrap();
    let paths = nai_storage::DataPaths::at(dir.path()).unwrap();
    let mut store = nai_storage::Store::open(paths.clone(), &op).unwrap();
    store.create_workspace(&op, "w", "test only").unwrap();
    for permission in [
        "session.manage",
        "approval.read",
        "approval.write",
        "knowledge.read",
        "knowledge.publish",
    ] {
        store
            .set_permission(&op, "w", "operator", permission, "*", true)
            .unwrap();
    }
    // A candidate needs a resolved fact dependency before it can be recorded.
    let cue = nai_contracts::v1::PerformanceCue {
        utterance_id: "approval-candidate".into(),
        session_id: "s".into(),
        speech_text: "test".into(),
        display_text: "待审批文本：满 300 减 50".into(),
        risk: 2,
        expires_at_utc_ms: 4_294_967_296_000,
        ..Default::default()
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    store.create_session(&op, "w", "operator", "s").unwrap();
    store
        .publish_fact(
            &op,
            "operator",
            &nai_contracts::v1::AuthoritativeFact {
                workspace_id: "w".into(),
                product_id: "sku".into(),
                field_key: "price".into(),
                authority_ref: "test-authority".into(),
                value: Some(nai_contracts::v1::authoritative_fact::Value::Money(
                    nai_contracts::v1::Money {
                        minor_units: 129900,
                        currency: "CNY".into(),
                        scale: 2,
                    },
                )),
                valid_from_utc_ms: Some(1),
                valid_until_utc_ms: Some(now + 60_000),
                revision: "revision-1".into(),
                publisher_ref: "supplier".into(),
                ..Default::default()
            },
            None,
        )
        .unwrap();
    let hash = store
        .record_candidate(&op, "w", "operator", &cue, &[("sku", "price", "1")])
        .unwrap();
    (dir, store, hash)
}

async fn approval_harness(permissions: &[&str]) -> (Harness2, String) {
    let (dir, store, hash) = seed_approval_store();
    let access =
        Arc::new(Access::new(TOKEN, "device", permissions, Duration::from_secs(10)).unwrap());
    let backend =
        Arc::new(ApprovalBackend::new(Arc::new(Mutex::new(store)), "w", "operator").unwrap());
    let api = Arc::new(
        ControlApi::with_approvals(
            Session::new("s", "w").unwrap(),
            EmergencyHandle::default(),
            access,
            Some(backend),
        )
        .unwrap(),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (stop, shutdown) = watch::channel(false);
    let task = tokio::spawn(serve(listener, api.clone(), shutdown));
    let harness = Harness2 {
        api,
        url,
        client: reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap(),
        stop,
        task,
        _dir: dir,
    };
    (harness, hash)
}

struct Harness2 {
    api: Arc<ControlApi>,
    url: String,
    client: reqwest::Client,
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<std::io::Result<()>>,
    _dir: tempfile::TempDir,
}
impl Drop for Harness2 {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
        self.task.abort();
    }
}

#[tokio::test]
async fn approval_backend_without_store_reports_not_configured() {
    // Default harness has no approval backend; it must not fabricate results.
    let h = harness(&["approval.read"], Duration::from_secs(10)).await;
    let response = h
        .client
        .get(format!("{}/api/v1/approvals", h.url))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 503);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "CAPABILITY_NOT_READY"
    );
}

#[tokio::test]
async fn approval_list_and_real_reject_workflow_over_loopback() {
    let (h, hash) = approval_harness(&["approval.read", "approval.write"]).await;
    let response = h
        .client
        .get(format!("{}/api/v1/approvals", h.url))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let value: Value = response.json().await.unwrap();
    validates("ResourceView", &value);
    let items = value["items"].as_array().unwrap();
    assert_eq!(
        items.len(),
        1,
        "seeded candidate should be awaiting approval"
    );
    assert_eq!(items[0]["id"], hash);
    assert_eq!(items[0]["status"], "awaiting_approval");
    // The operator approves content, not a hash: the candidate's own text rides
    // along, and it is never the identity.
    assert_eq!(items[0]["display_text"], "待审批文本：满 300 减 50");
    assert_ne!(items[0]["display_text"], items[0]["id"]);
    // The rest of the candidate payload stays out of the list.
    for absent in [
        "speech_text",
        "candidate_hash",
        "performance",
        "evidence_refs",
    ] {
        assert!(items[0].get(absent).is_none(), "list item leaked {absent}");
    }
    // Reject the real candidate through the API.
    let reject = h
        .client
        .post(format!("{}/api/v1/approvals/{hash}/reject", h.url))
        .bearer_auth(TOKEN)
        .json(&approve_body("reject-1", 1, 0, 0, &hash))
        .send()
        .await
        .unwrap();
    assert_eq!(reject.status(), 200);
    assert_eq!(reject.json::<Value>().await.unwrap()["executed"], true);
    // The rejected candidate disappears from the pending list.
    let after: Value = h
        .client
        .get(format!("{}/api/v1/approvals", h.url))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(after["items"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn approval_unauthorized_and_scope_rejected() {
    let h = harness(&["approval.read"], Duration::from_secs(10)).await;
    let hash64 = "a".repeat(64);
    let response = h
        .client
        .post(format!("{}/api/v1/approvals/{}/reject", h.url, hash64))
        .bearer_auth(TOKEN)
        .json(&approve_body("reject-1", 1, 0, 0, &hash64))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "PERMISSION_DENIED"
    );
    // Valid write permission does not authorize a different session.
    let (h, hash) = approval_harness(&["approval.write"]).await;
    let mut foreign = approve_body("foreign", 1, 0, 0, &hash);
    foreign["session_id"] = json!("other-session");
    assert_eq!(
        approval_post(&h, &hash, "reject", &foreign).await.status(),
        403
    );
    // A candidate URL without an action is not a registered operation.
    let response = h
        .client
        .post(format!(
            "{}/api/v1/approvals/{}",
            h.url,
            hash.replace(&hash[..8], "11111111")
        ))
        .bearer_auth(TOKEN)
        .json(&approve_body("reject-2", 1, 0, 0, &hash))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
}

async fn approval_post(h: &Harness2, hash: &str, action: &str, body: &Value) -> reqwest::Response {
    h.client
        .post(format!("{}/api/v1/approvals/{hash}/{action}", h.url))
        .bearer_auth(TOKEN)
        .json(body)
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn approval_approve_retry_conflict_and_reject_are_scoped() {
    let (h, hash) = approval_harness(&["approval.read", "approval.write"]).await;
    let command = approve_body("approve-1", 1, 0, 0, &hash);
    let response = approval_post(&h, &hash, "approve", &command).await;
    assert_eq!(response.status(), 200);
    let ack: Value = response.json().await.unwrap();
    validates("CommandAck", &ack);
    let retry = approval_post(&h, &hash, "approve", &command).await;
    assert_eq!(retry.status(), 200);
    assert_eq!(retry.json::<Value>().await.unwrap(), ack);
    assert_eq!(
        approval_post(&h, &hash, "reject", &command).await.status(),
        409
    );
    assert_eq!(
        approval_post(
            &h,
            &hash,
            "reject",
            &approve_body("reject-2", 2, 0, 0, &hash)
        )
        .await
        .status(),
        409
    );
    let backend = h.api.approvals.as_ref().unwrap();
    let store = backend.store.try_lock().unwrap();
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    assert!(store.approval_valid(&op, "w", "s", 0, &hash).unwrap());
    assert_eq!(
        store
            .read_approval(&op, "w", "operator", "s", 0, &hash)
            .unwrap()
            .approval_id
            .as_deref(),
        Some("approve-1")
    );
    store.verify_decision_chain(&op, "w").unwrap();
}

#[tokio::test]
async fn approval_invalid_schema_epoch_revision_hash_and_expired_ttl_do_not_execute() {
    let (h, hash) = approval_harness(&["approval.write"]).await;
    for field in ["candidate_hash", "hash_algorithm", "device_id", "seq"] {
        let mut command = approve_body("invalid", 1, 0, 0, &hash);
        command.as_object_mut().unwrap().remove(field);
        assert_eq!(
            approval_post(&h, &hash, "approve", &command).await.status(),
            400
        );
    }
    assert_eq!(
        approval_post(
            &h,
            &hash,
            "approve",
            &approve_body("wrong-epoch", 1, 1, 0, &hash)
        )
        .await
        .status(),
        409
    );
    assert_eq!(
        approval_post(
            &h,
            &hash,
            "approve",
            &approve_body("wrong-revision", 1, 0, 1, &hash)
        )
        .await
        .status(),
        409
    );
    let mut wrong_hash = approve_body("wrong-hash", 1, 0, 0, &hash);
    wrong_hash["candidate_hash"] = json!("a".repeat(64));
    assert_eq!(
        approval_post(&h, &hash, "approve", &wrong_hash)
            .await
            .status(),
        400
    );
    let command = approve_body("expired", 1, 0, 0, &hash);
    assert_eq!(
        h.api
            .approval_action(
                &hash,
                true,
                &serde_json::to_vec(&command).unwrap(),
                Instant::now() - Duration::from_secs(11)
            )
            .status(),
        409
    );
    assert_eq!(
        approval_post(&h, &hash, "reject", &approve_body("valid", 1, 0, 0, &hash))
            .await
            .status(),
        200
    );
}

#[tokio::test]
async fn approval_busy_backend_and_full_ledger_fail_bounded() {
    let (h, hash) = approval_harness(&["approval.read", "approval.write"]).await;
    {
        let _owner = h.api.approvals.as_ref().unwrap().store.try_lock().unwrap();
        assert_eq!(h.api.approvals_list().status(), 409);
        assert_eq!(
            h.api
                .approval_action(
                    &hash,
                    true,
                    &serde_json::to_vec(&approve_body("busy", 1, 0, 0, &hash)).unwrap(),
                    Instant::now()
                )
                .status(),
            409
        );
    }
    h.api.state.try_lock().unwrap().commands = CommandLedger::new(0);
    let response = approval_post(&h, &hash, "approve", &approve_body("full", 1, 0, 0, &hash)).await;
    assert_eq!(response.status(), 409);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "RESOURCE_EXHAUSTED"
    );
}

#[tokio::test]
async fn approval_reject_retry_survives_backend_age_and_storage_permissions_are_checked() {
    let (h, hash) = approval_harness(&["approval.read", "approval.write"]).await;
    tokio::time::sleep(Duration::from_millis(5100)).await;
    let command = approve_body("reject-aged", 1, 0, 0, &hash);
    let response = approval_post(&h, &hash, "reject", &command).await;
    assert_eq!(response.status(), 200);
    let ack: Value = response.json().await.unwrap();
    validates("CommandAck", &ack);
    assert_eq!(
        approval_post(&h, &hash, "reject", &command)
            .await
            .json::<Value>()
            .await
            .unwrap(),
        ack
    );
    let backend = h.api.approvals.as_ref().unwrap();
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    backend
        .store
        .try_lock()
        .unwrap()
        .set_permission(&op, "w", "operator", "approval.read", "*", false)
        .unwrap();
    let response = h
        .client
        .get(format!("{}/api/v1/approvals", h.url))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
}
