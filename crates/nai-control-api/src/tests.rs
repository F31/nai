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
// ---------------------------------------------------------------------------
// §42.3/§42.4 记忆面（W110）
// ---------------------------------------------------------------------------
use std::sync::atomic::{AtomicUsize, Ordering};

/// 路由喂给后端的**结果剧本**：三态各一（`Ok` / `Rejected` / `Fault`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemoryScript {
    Ok,
    Rejected(MemoryRejection),
    Fault(ErrorCode),
}

/// 一个**记录调用**的测试实现。
///
/// ⚠️ 非空洞的关键：接线成立 MUST 由「调用真的发生了」证明。若用恒真桩（一律返回空
/// outcome 且不记账），「接线」可以是装饰——路由返回 `200` 而**从未**调到后端，
/// 测试照样绿（R1）。故本实现既记账，也留下路由构造出的**入参**（作用域/类别/配额），
/// 让「路由填了什么」本身可被断言。
struct RecordingSurface {
    reads: AtomicUsize,
    forgets: AtomicUsize,
    seen_scopes: Mutex<Vec<(String, String)>>,
    seen_kinds: Mutex<Vec<Vec<MemoryKind>>>,
    seen_budgets: Mutex<Vec<usize>>,
    /// W111: 第三方留存是**事实**，MUST 由路由原样转交到接缝（不许在控制面被改写）。
    seen_retention: Mutex<Vec<Option<String>>>,
    script: MemoryScript,
}

impl RecordingSurface {
    fn new(script: MemoryScript) -> Self {
        Self {
            reads: AtomicUsize::new(0),
            forgets: AtomicUsize::new(0),
            seen_scopes: Mutex::new(Vec::new()),
            seen_kinds: Mutex::new(Vec::new()),
            seen_budgets: Mutex::new(Vec::new()),
            seen_retention: Mutex::new(Vec::new()),
            script,
        }
    }
    fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
    fn forgets(&self) -> usize {
        self.forgets.load(Ordering::SeqCst)
    }
}

impl MemorySurface for RecordingSurface {
    fn read(&self, query: &MemoryQuery) -> MemoryControlResult<MemoryReadOutcome> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.seen_scopes.lock().unwrap().push((
            query.scope.workspace_id.clone(),
            query.scope.session_id.clone(),
        ));
        self.seen_kinds.lock().unwrap().push(query.kinds.clone());
        self.seen_budgets.lock().unwrap().push(query.budget);
        match self.script {
            MemoryScript::Ok => MemoryControlResult::Ok(MemoryReadOutcome {
                records: vec![],
                refused: vec![],
            }),
            MemoryScript::Rejected(reason) => MemoryControlResult::Rejected(reason),
            MemoryScript::Fault(code) => MemoryControlResult::Fault(code),
        }
    }
    fn forget(&self, request: &MemoryForgetRequest) -> MemoryControlResult<MemoryPurgeReport> {
        self.forgets.fetch_add(1, Ordering::SeqCst);
        self.seen_retention
            .lock()
            .unwrap()
            .push(request.third_party_retention.clone());
        match self.script {
            MemoryScript::Ok => MemoryControlResult::Ok(MemoryPurgeReport {
                record_id: request.record_id.clone(),
                revoked_at_ms: 1_700_000_000_000,
                payload_purged_at_ms: 1_700_000_000_001,
                copy_sites_cleared: 0,
                purge_state: "complete",
                payload_cleared: true,
                // W111: 实现端 MUST NOT 报 `true`——本仓未做介质级擦除。
                physical_bytes_erased: false,
            }),
            MemoryScript::Rejected(reason) => MemoryControlResult::Rejected(reason),
            MemoryScript::Fault(code) => MemoryControlResult::Fault(code),
        }
    }
}

struct MemoryHarness {
    url: String,
    client: reqwest::Client,
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<std::io::Result<()>>,
}
impl Drop for MemoryHarness {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
        self.task.abort();
    }
}

async fn memory_harness(
    permissions: &[&str],
    surface: Option<Arc<dyn MemorySurface>>,
) -> MemoryHarness {
    let access =
        Arc::new(Access::new(TOKEN, "device", permissions, Duration::from_secs(10)).unwrap());
    let mut api = ControlApi::new(
        Session::new("s", "w").unwrap(),
        EmergencyHandle::default(),
        access,
    )
    .unwrap();
    if let Some(surface) = surface {
        api = api.with_memory(surface);
    }
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (stop, shutdown) = watch::channel(false);
    let task = tokio::spawn(serve(listener, Arc::new(api), shutdown));
    MemoryHarness {
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

async fn memory_post(h: &MemoryHarness, route: &str, body: &Value) -> reqwest::Response {
    h.client
        .post(format!("{}{route}", h.url))
        .bearer_auth(TOKEN)
        .json(body)
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn memory_backend_without_surface_reports_not_configured() {
    // 未注入记忆后端 MUST NOT 假成功：空结果与「没这能力」不得不可区分。
    let h = memory_harness(&["memory.read", "memory.forget"], None).await;
    for route in ["/api/v1/memory/read", "/api/v1/memory/forget"] {
        let response = memory_post(&h, route, &json!({"record_id": "r"})).await;
        assert_eq!(response.status(), 503, "{route}");
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["error"]["code"], "CAPABILITY_NOT_READY", "{route}");
    }
}

#[tokio::test]
async fn memory_seam_is_really_invoked_over_loopback() {
    // 「接线存在」MUST 由「调用真的发生了」证明，而不是由「响应是 200 」证明。
    let surface = Arc::new(RecordingSurface::new(MemoryScript::Ok));
    let h = memory_harness(&["memory.read", "memory.forget"], Some(surface.clone())).await;
    let read = memory_post(&h, "/api/v1/memory/read", &json!({})).await;
    assert_eq!(read.status(), 200);
    let forget = memory_post(&h, "/api/v1/memory/forget", &json!({"record_id": "r"})).await;
    assert_eq!(forget.status(), 200);
    assert_eq!(surface.reads(), 1, "读取接缝 MUST 被调用恰好一次");
    assert_eq!(surface.forgets(), 1, "遗忘接缝 MUST 被调用恰好一次");
    // 「没给输入」≠「空集」：kinds 缺失 ⇒ 全部三类，不是一个空查询。
    assert_eq!(
        surface.seen_kinds.lock().unwrap().as_slice(),
        &[MemoryKind::ALL.to_vec()]
    );
    // budget 缺失 ⇒ 常量默认。
    assert_eq!(surface.seen_budgets.lock().unwrap().as_slice(), &[16]);
}

#[tokio::test]
async fn memory_explicit_empty_kinds_is_an_empty_set_not_all_three() {
    let surface = Arc::new(RecordingSurface::new(MemoryScript::Ok));
    let h = memory_harness(&["memory.read"], Some(surface.clone())).await;
    assert_eq!(
        memory_post(&h, "/api/v1/memory/read", &json!({"kinds": []}))
            .await
            .status(),
        200
    );
    assert_eq!(
        surface.seen_kinds.lock().unwrap().as_slice(),
        &[Vec::<MemoryKind>::new()]
    );
}

// W112：§42.5 跨会话边界——作用域来自控制面绑定，请求伪造被忽略（3.3 归属）。
#[tokio::test]
async fn memory_scope_comes_from_the_control_surface_and_the_request_cannot_pick_it() {
    let surface = Arc::new(RecordingSurface::new(MemoryScript::Ok));
    let h = memory_harness(&["memory.read"], Some(surface.clone())).await;
    // 请求里硬塞作用域 MUST 被忽略：作用域是控制面自己的绑定，不是请求参数。
    assert_eq!(
        memory_post(
            &h,
            "/api/v1/memory/read",
            &json!({"workspace_id": "attacker", "session_id": "attacker"})
        )
        .await
        .status(),
        200
    );
    assert_eq!(
        surface.seen_scopes.lock().unwrap().as_slice(),
        &[("w".to_owned(), "s".to_owned())]
    );
}

#[tokio::test]
async fn memory_read_requires_a_token_and_the_named_permission() {
    let surface = Arc::new(RecordingSurface::new(MemoryScript::Ok));
    let h = memory_harness(&["memory.read", "memory.forget"], Some(surface.clone())).await;
    let anonymous = h
        .client
        .post(format!("{}/api/v1/memory/read", h.url))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(anonymous.status(), 401);
    // 只有 `memory.read` 时，遗忘 MUST 是 403（两类权限可分别吊销）。
    let read_only = memory_harness(&["memory.read"], Some(surface.clone())).await;
    assert_eq!(
        memory_post(
            &read_only,
            "/api/v1/memory/forget",
            &json!({"record_id": "r"})
        )
        .await
        .status(),
        403
    );
    assert_eq!(surface.forgets(), 0, "被拒的请求 MUST NOT 到达后端");
}

#[tokio::test]
async fn memory_request_level_refusal_is_not_an_empty_read() {
    // 判定拒绝 MUST 具名返回，MUST NOT 折成 200 加空 records——
    // 否则「本次读取不成立」与「读出来零条」不可区分。
    let surface = Arc::new(RecordingSurface::new(MemoryScript::Rejected(
        MemoryRejection::ViewerMemoryDisabled,
    )));
    let h = memory_harness(&["memory.read"], Some(surface.clone())).await;
    let response = memory_post(&h, "/api/v1/memory/read", &json!({})).await;
    assert_eq!(response.status(), 422);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["message"], "viewer_memory_disabled");
    assert!(body.get("records").is_none(), "MUST NOT 返回空 records");
}

#[tokio::test]
async fn memory_fault_is_not_reported_as_a_refusal() {
    // 路径自身的故障 MUST 与判定拒绝分开：锁竞争/存储故障不得记在门槛名下。
    let surface = Arc::new(RecordingSurface::new(MemoryScript::Fault(
        ErrorCode::Conflict,
    )));
    let h = memory_harness(&["memory.read"], Some(surface.clone())).await;
    let response = memory_post(&h, "/api/v1/memory/read", &json!({})).await;
    assert_eq!(response.status(), 409);
    assert_ne!(response.status(), 422, "故障 MUST NOT 报成判定拒绝");
}

#[tokio::test]
async fn memory_request_parse_failures_are_each_named() {
    let surface = Arc::new(RecordingSurface::new(MemoryScript::Ok));
    let h = memory_harness(&["memory.read", "memory.forget"], Some(surface.clone())).await;
    let bad_reads: &[Value] = &[
        json!({"kinds": "semantic"}),
        json!({"kinds": ["nonsense"]}),
        json!({"kinds": [1]}),
        json!({"budget": 4096}),
        json!([]),
    ];
    for body in bad_reads {
        assert_eq!(
            memory_post(&h, "/api/v1/memory/read", body).await.status(),
            400,
            "{body}"
        );
    }
    let bad_forgets: &[Value] = &[
        json!({}),
        json!({"record_id": "   "}),
        json!({"record_id": 1}),
        json!({"record_id": "r", "trigger": 1}),
        json!({"record_id": "r", "trigger": "nonsense"}),
        json!({"record_id": "r", "purge_state": "nonsense"}),
        json!([]),
    ];
    for body in bad_forgets {
        assert_eq!(
            memory_post(&h, "/api/v1/memory/forget", body)
                .await
                .status(),
            400,
            "{body}"
        );
    }
    assert_eq!(surface.reads(), 0, "入参不合法 MUST NOT 到达后端");
    assert_eq!(surface.forgets(), 0);
}

#[tokio::test]
async fn memory_oversized_body_is_rejected_before_the_seam() {
    let surface = Arc::new(RecordingSurface::new(MemoryScript::Ok));
    let h = memory_harness(&["memory.read"], Some(surface.clone())).await;
    let huge = json!({"purpose": "x".repeat(8192)});
    assert_eq!(
        memory_post(&h, "/api/v1/memory/read", &huge).await.status(),
        413
    );
    assert_eq!(surface.reads(), 0);
}

#[tokio::test]
async fn memory_forget_labels_round_trip_exhaustively() {
    // 非空洞守卫：每个在册标签都 MUST 被路由认得。加一个变体而此处没跟上 ⇒ 红。
    let surface = Arc::new(RecordingSurface::new(MemoryScript::Ok));
    let h = memory_harness(&["memory.forget"], Some(surface.clone())).await;

    for trigger in ForgetTrigger::ALL {
        let response = memory_post(
            &h,
            "/api/v1/memory/forget",
            &json!({"record_id": "r", "trigger": trigger.label()}),
        )
        .await;
        assert_eq!(response.status(), 200, "trigger {}", trigger.label());
    }
    assert_eq!(surface.forgets(), ForgetTrigger::ALL.len());

    // W111: `purge_state` 从**事实**降级为**结论**，因而不再是请求字段。
    // 它 MUST 被**拒**，MUST NOT 被静默忽略——静默忽略会让调用方以为自己的宣称生效了。
    for state in nai_kernel::evaluate::memory::PurgeState::ALL {
        let response = memory_post(
            &h,
            "/api/v1/memory/forget",
            &json!({"record_id": "r", "purge_state": state.label()}),
        )
        .await;
        assert_eq!(
            response.status(),
            400,
            "purge_state 是结论而非请求字段，MUST 被拒：{}",
            state.label()
        );
    }
    assert_eq!(
        surface.forgets(),
        ForgetTrigger::ALL.len(),
        "被拒的请求 MUST NOT 到达后端"
    );
}

#[tokio::test]
async fn memory_forget_defaults_take_the_safe_direction() {
    // 缺省 MUST 取安全方向：不宣称已清除、不宣称第三方无留存。
    // 这里只钉**路由不报错且真调到后端**；「缺省值是什么」由 runtime 侧用例钉。
    let surface = Arc::new(RecordingSurface::new(MemoryScript::Ok));
    let h = memory_harness(&["memory.forget"], Some(surface.clone())).await;
    assert_eq!(
        memory_post(&h, "/api/v1/memory/forget", &json!({"record_id": "r"}))
            .await
            .status(),
        200
    );
    assert_eq!(surface.forgets(), 1);
}

// ---------------------------------------------------------------------------
// W111：清除结论由**系统**给出，调用方 MUST NOT 参与构造
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_forget_response_carries_the_systems_own_purge_conclusion() {
    let surface = Arc::new(RecordingSurface::new(MemoryScript::Ok));
    let h = memory_harness(&["memory.forget"], Some(surface.clone())).await;

    let response = memory_post(
        &h,
        "/api/v1/memory/forget",
        &json!({"record_id": "memory-1", "trigger": "user_request"}),
    )
    .await;
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();

    // 清除的完成状态、清除时刻、副本清点结果都 MUST 在响应里，且由实现端给出。
    assert_eq!(body["record_ref"], "memory-1");
    assert_eq!(body["purge_state"], "complete");
    assert_eq!(body["copy_sites_cleared"], 0);
    assert!(
        body["payload_purged_at_utc_ms"].is_string(),
        "清除时刻 MUST 出现在响应里：否则「可验证」无从谈起"
    );
    // 口径写进形状：逻辑擦除，**恒不**宣称介质级擦除。
    assert_eq!(body["erased"]["scope"], "logical");
    assert_eq!(body["erased"]["payload_cleared"], true);
    assert_eq!(
        body["erased"]["physical_bytes_erased"], false,
        "本仓未做介质级擦除，响应 MUST NOT 把它报成 true"
    );
    assert_eq!(surface.forgets(), 1, "接线 MUST 真被调用");
}

#[tokio::test]
async fn a_caller_supplied_conclusion_field_is_refused_before_the_seam() {
    // W111 的核心修复点：结论字段从请求体**删除**。若只是「不再使用」而静默忽略，
    // 调用方会以为自己的宣称生效了——那正是 W110 交付面与实际能力矛盾的地方。
    let surface = Arc::new(RecordingSurface::new(MemoryScript::Ok));
    let h = memory_harness(&["memory.forget"], Some(surface.clone())).await;

    for (field, value) in [
        ("claims_fully_erased", json!(true)),
        ("purge_state", json!("complete")),
        ("claims_fully_erased", json!(false)),
    ] {
        let response = memory_post(
            &h,
            "/api/v1/memory/forget",
            &json!({"record_id": "r", field: value}),
        )
        .await;
        assert_eq!(response.status(), 400, "{field} MUST 被拒");
        let body: Value = response.json().await.unwrap();
        assert!(
            body["error"]["message"].as_str().unwrap().contains(field),
            "错误消息 MUST 点名那个字段：{body}"
        );
    }
    assert_eq!(
        surface.forgets(),
        0,
        "被拒的请求 MUST NOT 到达后端——否则「拒」只是装饰"
    );
}

// W112：§42.5 跨会话边界——授权不跨场继承（3.1 新增判据）。
#[tokio::test]
async fn memory_route_rejects_a_token_issued_by_another_control_surface() {
    // §42.5：授权不跨场继承。别的场（不同 Access token）MUST NOT 读/忘本场的记忆。
    let surface = Arc::new(RecordingSurface::new(MemoryScript::Ok));
    let h = memory_harness(&["memory.read", "memory.forget"], Some(surface.clone())).await;

    let foreign = h
        .client
        .post(format!("{}/api/v1/memory/read", h.url))
        .bearer_auth("token-issued-by-another-control-surface")
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(foreign.status(), 401, "跨场 token MUST 被拒（W112：§42.5）");

    let foreign_forget = h
        .client
        .post(format!("{}/api/v1/memory/forget", h.url))
        .bearer_auth("token-issued-by-another-control-surface")
        .json(&json!({"record_id": "r"}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        foreign_forget.status(),
        401,
        "跨场 token MUST 被拒（W112：§42.5）"
    );

    assert_eq!(surface.reads(), 0, "跨场 token MUST NOT 到达后端");
    assert_eq!(surface.forgets(), 0);
}

#[tokio::test]
async fn the_third_party_retention_fact_is_forwarded_verbatim() {
    // 留存是**事实**（只有调用方知道），与被删掉的两个**结论**字段不同层：
    // 控制面 MUST 原样转交，MUST NOT 改写、也不 MUST NOT 丢弃。
    let surface = Arc::new(RecordingSurface::new(MemoryScript::Ok));
    let h = memory_harness(&["memory.forget"], Some(surface.clone())).await;

    let response = memory_post(
        &h,
        "/api/v1/memory/forget",
        &json!({"record_id": "r", "third_party_retention": "provider-retains-30d"}),
    )
    .await;
    assert_eq!(response.status(), 200);
    assert_eq!(
        surface.seen_retention.lock().unwrap().as_slice(),
        &[Some("provider-retains-30d".to_owned())]
    );
}

// ---------------------------------------------------------------------------
// §1330 O05：升级成本的**控制面来源**（W137）
//
// 这一组判的是「成本怎么从控制面出来」，不是「成本模型对不对」（那在
// `nai-upgrade-core` 的 tests/gates.rs）。三条边界在这里各有判据：
//   ① 权限：没有 `runtime.upgrade` 就到不了成本模型；
//   ② 形状：响应必须满足**它自己声明的契约**（`UpgradePlanView`）；
//   ③ 表示性：币种/金额超出呈现范围 ⇒ 具名拒绝，MUST NOT 发一个契约不合格的 200。
// ---------------------------------------------------------------------------

/// 一份**结构合规**的包清单。只让 `cost` 变化，其余字段固定——
/// 于是「成本怎么呈现」的每条判据都只钉一个变量。
fn package_manifest(cost: Value) -> Value {
    json!({
        "manifest_schema": 1,
        "package_id": "plugin.demo",
        "package_version": "1.2.3",
        "kind": "plugin",
        "kernel_range": ">=1.0.0, <2.0.0",
        "requires": [{ "ref": "cap.one", "range": ">=1.0.0, <2.0.0" }],
        "hosts": ["inproc"],
        "os": ["linux"],
        "architectures": ["x86_64"],
        "runtime": [{ "id": "rt-cpu", "accel": "cpu" }],
        "files": [{ "path": "artifacts/one.bin", "sha256": "0".repeat(64), "bytes": "1024" }],
        "install_bytes": "1024",
        "migrated_bytes": "2048",
        "signer_ref": "signer.demo",
        "license_ref": "license.demo",
        "migration_ref": "migration.forward",
        "rollback_range": ">=1.0.0, <1.2.0",
        "cost": cost,
    })
}

fn declared_cost(currency: &str, amounts: [Option<&str>; 4]) -> Value {
    json!({
        "currency": currency,
        "cloud": amounts[0],
        "download": amounts[1],
        "storage": amounts[2],
        "runtime_delta": amounts[3],
    })
}

async fn plan(h: &Harness, manifest: Value) -> (u16, Value) {
    let response = post(h, "/api/v1/upgrades/plan", &json!({ "manifest": manifest })).await;
    let status = response.status().as_u16();
    (status, response.json().await.unwrap())
}

#[tokio::test]
async fn the_plan_route_needs_the_upgrade_permission_and_says_nothing_else() {
    let h = harness(&["runtime.read"], Duration::from_secs(10)).await;
    let (status, body) = plan(
        &h,
        package_manifest(declared_cost("CNY", [Some("10"), None, None, None])),
    )
    .await;
    assert_eq!(status, 403);
    assert_eq!(body["error"]["code"], "PERMISSION_DENIED");
    // 「被拒」与「算出来是 0」在类型上分开：拒绝的响应里 MUST 没有成本面。
    assert!(body.get("cost").is_none(), "{body}");
}

#[tokio::test]
async fn the_plan_view_satisfies_the_contract_it_declares() {
    let h = harness(&["runtime.upgrade"], Duration::from_secs(10)).await;
    let (status, body) = plan(
        &h,
        package_manifest(declared_cost(
            "CNY",
            [Some("10"), Some("20"), Some("30"), Some("40")],
        )),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    // 响应交给**它自己声明的** schema（additionalProperties:false ⇒ 多一个 total 也会红）。
    validates("UpgradePlanView", &body);
    assert_eq!(body["package_id"], "plugin.demo");
    assert_eq!(body["package_version"], "1.2.3");
    assert_eq!(body["cost"]["currency"], "CNY");
    assert_eq!(body["cost"]["components"]["cloud"], json!({ "known": 10 }));
    assert_eq!(
        body["cost"]["components"]["runtime_delta"],
        json!({ "known": 40 })
    );
}

#[tokio::test]
async fn a_component_the_manifest_leaves_out_is_a_reason_and_never_zero() {
    let h = harness(&["runtime.upgrade"], Duration::from_secs(10)).await;
    let (status, body) = plan(
        &h,
        package_manifest(declared_cost("CNY", [Some("10"), None, None, None])),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    validates("UpgradePlanView", &body);
    let components = &body["cost"]["components"];
    assert_eq!(components["cloud"], json!({ "known": 10 }));
    for name in ["download", "storage", "runtime_delta"] {
        let line = &components[name];
        // 未声明的分项 MUST 是**原因**，MUST NOT 被补成 0——0 是「这块免费」的断言。
        assert!(line.get("known").is_none(), "{name}: {line}");
        let reason = line["unknown"].as_str().unwrap_or_default();
        assert!(!reason.trim().is_empty(), "{name}: {line}");
    }
}

#[tokio::test]
async fn an_explicit_zero_is_reported_as_zero() {
    // 与上一条互为反向：显式的 0 是一个**断言**，被如实呈现为 0，MUST NOT 被当成「未知」。
    let h = harness(&["runtime.upgrade"], Duration::from_secs(10)).await;
    let (status, body) = plan(
        &h,
        package_manifest(declared_cost(
            "CNY",
            [Some("0"), Some("0"), Some("0"), Some("0")],
        )),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["cost"]["components"]["cloud"], json!({ "known": 0 }));
    assert_eq!(body["cost"]["components"]["storage"], json!({ "known": 0 }));
}

#[tokio::test]
async fn a_manifest_that_fails_its_schema_is_named_and_yields_no_estimate() {
    let h = harness(&["runtime.upgrade"], Duration::from_secs(10)).await;
    let mut broken = package_manifest(declared_cost("CNY", [Some("10"), None, None, None]));
    // 缺 `cost.runtime_delta`：清单 schema 要求四个分项**都在**（可以为 null）。
    broken["cost"]
        .as_object_mut()
        .unwrap()
        .remove("runtime_delta");
    let (status, body) = plan(&h, broken).await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["code"], "INVALID_ARGUMENT");
    let message = body["error"]["message"].as_str().unwrap_or_default();
    assert!(message.contains("package manifest rejected"), "{message}");
    // 失败 MUST NOT 退化成「全未知」「全 0」或空成本面。
    assert!(body.get("cost").is_none(), "{body}");
}

#[tokio::test]
async fn a_request_carrying_anything_beside_the_manifest_is_refused() {
    let h = harness(&["runtime.upgrade"], Duration::from_secs(10)).await;
    let manifest = package_manifest(declared_cost("CNY", [Some("10"), None, None, None]));
    // 多带一个顶层键：调用方若想「顺手给个覆盖价」，那是把判定输入交给调用方。
    let response = post(
        &h,
        "/api/v1/upgrades/plan",
        &json!({ "manifest": manifest, "override": { "cloud": "0" } }),
    )
    .await;
    assert_eq!(response.status(), 400);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "INVALID_ARGUMENT");
    assert!(body.get("cost").is_none(), "{body}");

    // 连 manifest 都不给 ⇒ 同样是具名拒绝，不是 200 加空成本。
    let response = post(
        &h,
        "/api/v1/upgrades/plan",
        &json!({ "cost": { "currency": "CNY" } }),
    )
    .await;
    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn a_currency_that_cannot_be_presented_is_refused_rather_than_widened() {
    // 清单 schema 允许 1..8 个字符，呈现在契约里是三字母大写码。
    // 两条口径**不**在这里合并：能做的是拒绝这个包，MUST NOT 放宽呈现口径。
    let h = harness(&["runtime.upgrade"], Duration::from_secs(10)).await;
    let (status, body) = plan(
        &h,
        package_manifest(declared_cost("cny", [Some("10"), None, None, None])),
    )
    .await;
    assert_eq!(status, 400, "{body}");
    let message = body["error"]["message"].as_str().unwrap_or_default();
    assert!(message.contains("three-letter"), "{message}");
    assert!(body.get("cost").is_none(), "{body}");
}

#[tokio::test]
async fn an_amount_past_the_representable_range_is_refused() {
    // 2^53：JS 里 `Number.isSafeInteger` 为假 ⇒ 消费端会悄悄取整。
    // 契约把上界钉在 2^53-1，于是发一个更大的数是发一个自己契约不合格的 200。
    let h = harness(&["runtime.upgrade"], Duration::from_secs(10)).await;
    let (status, body) = plan(
        &h,
        package_manifest(declared_cost(
            "CNY",
            [Some("9007199254740992"), None, None, None],
        )),
    )
    .await;
    assert_eq!(status, 400, "{body}");
    let message = body["error"]["message"].as_str().unwrap_or_default();
    assert!(message.contains("representable"), "{message}");
}

#[tokio::test]
async fn the_plan_body_limit_follows_the_manifest_limit_not_the_command_limit() {
    // 反向判据：命令体上限是 4096，包清单允许到 65536。
    // 若上限照抄命令那一份，这里会变成静默的 413，而不是把一个合法清单算出来。
    let h = harness(&["runtime.upgrade"], Duration::from_secs(10)).await;
    let mut large = package_manifest(declared_cost("CNY", [Some("10"), None, None, None]));
    let files: Vec<Value> = (0..300)
        .map(|index| {
            json!({
                "path": format!("artifacts/asset_{index:04}.bin"),
                "sha256": "0".repeat(64),
                "bytes": "1024",
            })
        })
        .collect();
    large["files"] = Value::Array(files);
    let encoded = serde_json::to_vec(&large).unwrap();
    assert!(encoded.len() > COMMAND_BODY_LIMIT, "{}", encoded.len());
    assert!(encoded.len() < MANIFEST_LIMIT, "{}", encoded.len());

    let (status, body) = plan(&h, large).await;
    assert_eq!(status, 200, "{body}");

    // 另一侧：超过 plan 体上限 ⇒ 具名的 413，MUST NOT 悄悄算一半。
    // 体量故意只超出一点点（不是几倍）：服务端拒绝后只排空有界的一段，
    // 过大的体仍会让对端先看到 RST —— 那是排空的边界，不是判据的边界。
    let mut count = 300;
    let mut oversized = package_manifest(declared_cost("CNY", [Some("10"), None, None, None]));
    loop {
        let files: Vec<Value> = (0..count)
            .map(|index| {
                json!({
                    "path": format!("artifacts/asset_{index:04}.bin"),
                    "sha256": "0".repeat(64),
                    "bytes": "1024",
                })
            })
            .collect();
        oversized["files"] = Value::Array(files);
        let size = serde_json::to_vec(&json!({ "manifest": oversized }))
            .unwrap()
            .len();
        if size > PLAN_BODY_LIMIT {
            assert!(
                size < PLAN_BODY_LIMIT + 4096,
                "fixture overshoots the drain window"
            );
            break;
        }
        count += 20;
    }
    let response = post(
        &h,
        "/api/v1/upgrades/plan",
        &json!({ "manifest": oversized }),
    )
    .await;
    assert_eq!(response.status(), 413);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "RESOURCE_EXHAUSTED");
    assert!(body.get("cost").is_none(), "{body}");
}

// ---------------------------------------------------------------------------
// 作业状态（W138）：`GET /api/v1/jobs/{id}` 读的是 `upgrade_journal` 本身。
//   ① 三态各有其名：「查到了」「没有这条」「这个面没接」。
//   ② 步骤与终态由枚举派生，控制面不另判一次。
//   ③ 执行/回滚缺平台安装器 ⇒ 具名拒绝，且**不铸** job_id。
// ---------------------------------------------------------------------------

/// 一个 journal 行：`(transaction_id, step, backup_ref, error_code)`。
type JournalRow<'a> = (
    &'a str,
    nai_storage::UpgradeStep,
    Option<&'a str>,
    Option<&'a str>,
);

fn seed_journal_store(rows: &[JournalRow<'_>]) -> (tempfile::TempDir, nai_storage::Store) {
    let dir = tempfile::tempdir().unwrap();
    let op = Operation::new(nai_storage::BRINGUP_OPERATION).unwrap();
    let paths = nai_storage::DataPaths::at(dir.path()).unwrap();
    let mut store = nai_storage::Store::open(paths, &op).unwrap();
    for (id, step, backup, error) in rows {
        store
            .record_upgrade(
                &op,
                &nai_storage::UpgradeRecord {
                    transaction_id: (*id).into(),
                    from_manifest: "manifest-from".into(),
                    to_manifest: "manifest-to".into(),
                    step: *step,
                    backup_ref: backup.map(Into::into),
                    error_code: error.map(Into::into),
                },
            )
            .unwrap();
    }
    (dir, store)
}

async fn jobs_harness(
    permissions: &[&str],
    rows: &[JournalRow<'_>],
) -> (Harness2, Arc<Mutex<nai_storage::Store>>) {
    let (dir, store) = seed_journal_store(rows);
    let store = Arc::new(Mutex::new(store));
    let access =
        Arc::new(Access::new(TOKEN, "device", permissions, Duration::from_secs(10)).unwrap());
    let api = Arc::new(
        ControlApi::new(
            Session::new("s", "w").unwrap(),
            EmergencyHandle::default(),
            access,
        )
        .unwrap()
        .with_jobs(Arc::new(JobBackend::new(store.clone()))),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (stop, shutdown) = watch::channel(false);
    let task = tokio::spawn(serve(listener, api.clone(), shutdown));
    (
        Harness2 {
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
        },
        store,
    )
}

async fn job(h: &Harness2, id: &str) -> (u16, Value) {
    let response = h
        .client
        .get(format!("{}/api/v1/jobs/{id}", h.url))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    (status, response.json().await.unwrap())
}

#[tokio::test]
async fn a_job_surface_that_is_not_wired_is_not_ready_not_absent() {
    // 默认 harness 没有作业后端。此时 MUST 答「没接」，MUST NOT 答 404——
    // 404 会把「这个面没接」说成「这条作业不存在」。
    let h = harness(&["job.read"], Duration::from_secs(10)).await;
    let response = h
        .client
        .get(format!("{}/api/v1/jobs/anything", h.url))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 503);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "CAPABILITY_NOT_READY");
    assert_eq!(body["error"]["category"], "job");
    let message = body["error"]["message"].as_str().unwrap_or_default();
    // 内容锚点：拒绝必须说得出**缺的是 journal 读取方**，不是「未知错误」。
    assert!(message.contains("journal"), "{message}");
}

#[tokio::test]
async fn a_job_the_journal_never_recorded_is_not_found() {
    let (h, _store) = jobs_harness(&["job.read"], &[]).await;
    let (status, body) = job(&h, "tx-that-never-was").await;
    assert_eq!(status, 404, "{body}");
    assert_eq!(body["error"]["code"], "NOT_FOUND");
    // 「没有这条」MUST NOT 被一个空步骤冒充成「查到了」。
    assert!(body.get("step").is_none(), "{body}");
}

#[tokio::test]
async fn a_job_is_reported_as_the_journal_recorded_it_and_satisfies_the_contract() {
    let (h, _store) = jobs_harness(
        &["job.read"],
        &[(
            "tx-1",
            nai_storage::UpgradeStep::Migrated,
            Some("backup-1"),
            None,
        )],
    )
    .await;
    let (status, body) = job(&h, "tx-1").await;
    assert_eq!(status, 200, "{body}");
    // 响应交给**它自己声明的** schema（两层 additionalProperties:false）。
    validates("JobView", &body);
    assert_eq!(body["job_id"], "tx-1");
    assert_eq!(body["kind"], "upgrade");
    assert_eq!(body["step"], "migrated");
    assert_eq!(body["from_manifest"], "manifest-from");
    assert_eq!(body["to_manifest"], "manifest-to");
    assert_eq!(body["backup_ref"], "backup-1");
}

#[tokio::test]
async fn every_step_the_journal_can_hold_is_reported_verbatim() {
    const IDS: [&str; 5] = ["tx-0", "tx-1", "tx-2", "tx-3", "tx-4"];
    let steps = nai_storage::UpgradeStep::ALL;
    let rows: Vec<JournalRow> = steps
        .iter()
        .zip(IDS)
        .map(|(step, id)| (id, *step, None, None))
        .collect();
    let (h, _store) = jobs_harness(&["job.read"], &rows).await;
    for (step, id) in steps.iter().zip(IDS) {
        let (status, body) = job(&h, id).await;
        assert_eq!(status, 200, "{body}");
        // 逐字：控制面 MUST NOT 改写、缩写或翻译步骤名。
        assert_eq!(body["step"], step.as_str(), "step vocabulary drift");
        // `terminal` MUST 由枚举派生——这里逐态对照它**自己的** `terminal()`，
        // 于是「控制面另写一份判定」会在某个态上与枚举不一致。
        assert_eq!(
            body["terminal"],
            json!(step.terminal()),
            "{}",
            step.as_str()
        );
        validates("JobView", &body);
    }
}

#[tokio::test]
async fn the_contract_step_vocabulary_equals_the_journal_enum() {
    let schema: Value = serde_json::from_str(SCHEMAS_JSON).unwrap();
    let declared = schema["$defs"]["JobView"]["properties"]["step"]["enum"]
        .as_array()
        .expect("JobView.step must declare its vocabulary");
    let declared: Vec<&str> = declared
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    let from_enum: Vec<&str> = nai_storage::UpgradeStep::ALL
        .iter()
        .map(|step| step.as_str())
        .collect();
    // 顺序也钉住：一条 Requirement 的值域是**序列**，不是集合。
    assert_eq!(
        declared, from_enum,
        "the contract and the journal must hold the same steps in the same order"
    );
    // 反向：契约里出现的每个值都必须能被枚举解出来（改名会在这里红）。
    for name in &declared {
        assert!(
            nai_storage::UpgradeStep::parse(name).is_some(),
            "{name} is not a journal step"
        );
    }
}

#[tokio::test]
async fn the_backup_reference_appears_only_after_it_was_taken() {
    // 「可选」不是「可以是空串」：没备份过就 MUST 缺这个键，而不是给一个空值。
    let (h, _store) = jobs_harness(
        &["job.read"],
        &[("tx-early", nai_storage::UpgradeStep::Prepared, None, None)],
    )
    .await;
    let (status, body) = job(&h, "tx-early").await;
    assert_eq!(status, 200, "{body}");
    validates("JobView", &body);
    assert!(body.get("backup_ref").is_none(), "{body}");
}

#[tokio::test]
async fn the_jobs_route_needs_the_job_permission() {
    let (h, _store) = jobs_harness(
        &["runtime.read"],
        &[("tx-1", nai_storage::UpgradeStep::Prepared, None, None)],
    )
    .await;
    let (status, body) = job(&h, "tx-1").await;
    assert_eq!(status, 403);
    assert_eq!(body["error"]["code"], "PERMISSION_DENIED");
    // 被拒的响应里 MUST NOT 有步骤：403 与「查到了但是空的」必须分得开。
    assert!(body.get("step").is_none(), "{body}");
}

#[tokio::test]
async fn execute_and_rollback_refuse_by_name_and_mint_no_job_id() {
    let h = harness(&["runtime.upgrade"], Duration::from_secs(10)).await;
    let command = json!({
        "command_id": "c1",
        "expected_state_revision": "0",
        "ttl_ms": 10000,
        "resource_ref": "pkg.demo",
    });
    for route in ["/api/v1/upgrades/execute", "/api/v1/upgrades/tx-1/rollback"] {
        let response = post(&h, route, &command).await;
        let status = response.status().as_u16();
        let body: Value = response.json().await.unwrap();
        assert_eq!(status, 503, "{route}: {body}");
        assert_eq!(body["error"]["code"], "CAPABILITY_NOT_READY");
        assert_eq!(body["error"]["category"], "upgrade");
        // **不铸** job_id：一个指向永远不会被写的 journal 行的 id 是伪造可观测面。
        assert!(body.get("job_id").is_none(), "{route}: {body}");
        // 内容锚点：拒绝必须点名缺**平台安装器**，只判「非空」等于没判。
        let message = body["error"]["message"].as_str().unwrap_or_default();
        assert!(message.contains("UpgradeHost"), "{route}: {message}");
        assert!(message.contains("installer"), "{route}: {message}");
    }
}

#[tokio::test]
async fn an_id_that_cannot_name_a_transaction_is_a_bad_argument() {
    // 超长 id 连**命名**都做不到（存储自己拒绝）。这不是「没有这条」，是请求不合法。
    let (h, _store) = jobs_harness(&["job.read"], &[]).await;
    let (status, body) = job(&h, &"x".repeat(600)).await;
    assert_eq!(status, 400, "{body}");
    assert_eq!(body["error"]["code"], "INVALID_ARGUMENT");
}

#[tokio::test]
async fn a_locked_journal_is_a_conflict_not_a_missing_job() {
    // 读不到 ≠ 没有。「存储正忙」被答成 404 会让运维去查一条根本不存在的记录。
    let (h, store) = jobs_harness(
        &["job.read"],
        &[("tx-1", nai_storage::UpgradeStep::Prepared, None, None)],
    )
    .await;
    let held = store.try_lock();
    assert!(held.is_ok(), "the test must hold the journal lock");
    let (status, body) = job(&h, "tx-1").await;
    assert_eq!(status, 409, "{body}");
    assert_eq!(body["error"]["code"], "CONFLICT");
}
