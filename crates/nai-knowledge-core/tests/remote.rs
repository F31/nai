use nai_contracts::remote_knowledge::{
    RemoteCapability, RemoteDocument, RemoteOperation, RemotePolicy, RemoteReceipt,
    RemoteReceiptStatus,
};
use nai_contracts::v1::RetrievalStatus;
use nai_knowledge_core::{RemoteError, RemoteGateway, RemoteKnowledgeClient, rerank_evidence};
use nai_security::Operation;
use std::{
    collections::BTreeMap,
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use tiny_http::{Header, Request, Response, Server};

struct Mock {
    _server: Arc<Server>,
    port: u16,
}
impl Mock {
    fn start(
        handler: impl Fn(&Request) -> Option<Response<std::io::Cursor<Vec<u8>>>> + Send + Sync + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = Arc::new(Server::from_listener(listener, None).unwrap());
        let worker = server.clone();
        let handler = Arc::new(handler);
        thread::spawn(move || {
            while let Ok(request) = worker.recv() {
                let handler = handler.clone();
                thread::spawn(move || {
                    if let Some(response) = handler(&request) {
                        let _ = request.respond(response);
                    }
                });
            }
        });
        Self {
            _server: server,
            port,
        }
    }
    fn client(&self) -> RemoteKnowledgeClient {
        RemoteKnowledgeClient::new(
            format!("http://127.0.0.1:{}", self.port),
            None,
            Duration::from_secs(5),
            1,
        )
        .unwrap()
    }
}
fn json(status: u16, value: serde_json::Value) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut response =
        Response::from_data(serde_json::to_vec(&value).unwrap()).with_status_code(status);
    response
        .add_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
    response
}
fn capabilities_body() -> serde_json::Value {
    serde_json::json!({
        "capabilities": ["retrieval_with_evidence", "answer_with_evidence"],
        "operations": ["search", "answer", "sync", "delete"],
        "retention": {"policy_ref": "retention-v1", "retention_seconds": 86400, "supports_delete": true}
    })
}
fn policy() -> RemotePolicy {
    RemotePolicy {
        source_id: "kb-adapter".into(),
        policy_ref: "policy-v1".into(),
        allowed_kb_ids: vec!["kb-allowed".into()],
        allowed_operations: vec![
            RemoteOperation::Search,
            RemoteOperation::Answer,
            RemoteOperation::Sync,
            RemoteOperation::Delete,
        ],
        allow_query_upload: true,
        allow_document_upload: true,
        allow_third_party_retention: true,
        max_retention_seconds: 3600,
        require_remote_delete: true,
        expires_at_utc_ms: (now_ms() + 600_000).to_string(),
        permission_revision: "permission-1".into(),
        unknown_revision_ttl_ms: 300,
    }
}
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[tokio::test]
async fn capabilities_health_and_search_map_to_contract_status() {
    let mock = Mock::start(|request| {
        let path = request.url().to_string();
        Some(match path.as_str() {
            "/capabilities" => json(200, capabilities_body()),
            "/health" => json(200, serde_json::json!({"status": "ok"})),
            "/search" => json(
                200,
                serde_json::json!({
                    "status": "complete",
                    "provider_id": "kb-test",
                    "evidence": [{"evidence_id": "ev-1", "document_id": "doc-1", "revision": "rev-2", "chunk_id": "c-1", "text": "保修条款", "source_ref": "doc-1#c-1", "rank": 1}]
                }),
            ),
            _ => json(404, serde_json::json!({"message": "unknown"})),
        })
    });
    let client = mock.client();
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let caps = client.capabilities(&op).await.unwrap();
    assert!(
        caps.capabilities
            .iter()
            .any(|c| c == &RemoteCapability::RetrievalWithEvidence)
    );
    assert!(caps.retention.supports_delete);
    assert!(client.health(&op).await.unwrap());
    let (status, evidence) = client
        .search(
            &op,
            "req-1",
            "session-1",
            &["kb".into()],
            "保修",
            &BTreeMap::new(),
            5,
        )
        .await
        .unwrap();
    assert_eq!(status, RetrievalStatus::RetrievalComplete);
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].evidence_id, "ev-1");
}

#[tokio::test]
async fn unauthorized_never_retries() {
    let attempts = Arc::new(Mutex::new(0));
    let mock = Mock::start({
        let attempts = attempts.clone();
        move |_| {
            let mut count = attempts.lock().unwrap();
            *count += 1;
            Some(json(401, serde_json::json!({"message": "bad key"})))
        }
    });
    let client = mock.client();
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    assert_eq!(
        client
            .search(&op, "req", "s", &["kb".into()], "q", &BTreeMap::new(), 5)
            .await
            .unwrap()
            .0,
        RetrievalStatus::RetrievalUnauthorized
    );
    assert_eq!(*attempts.lock().unwrap(), 1);
}
#[tokio::test]
async fn rate_limit_retries_once_then_succeeds_and_empty_vs_unavailable_are_distinct() {
    let attempts = Arc::new(Mutex::new(0));
    let limited = Mock::start({
        let attempts = attempts.clone();
        move |_| {
            let mut count = attempts.lock().unwrap();
            *count += 1;
            if *count == 1 {
                let mut r = json(429, serde_json::json!({"message": "limited"}));
                r.add_header(Header::from_bytes(&b"Retry-After"[..], &b"0"[..]).unwrap());
                Some(r)
            } else {
                Some(json(
                    200,
                    serde_json::json!({"status": "complete", "evidence": []}),
                ))
            }
        }
    });
    let client = limited.client();
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let status = client
        .search(&op, "r1", "s", &["kb".into()], "q", &BTreeMap::new(), 5)
        .await
        .unwrap()
        .0;
    assert_eq!(status, RetrievalStatus::RetrievalComplete);
    assert_eq!(*attempts.lock().unwrap(), 2);

    let empty = Mock::start(|_| {
        Some(json(
            200,
            serde_json::json!({"status": "empty", "evidence": []}),
        ))
    });
    let client = empty.client();
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let status = client
        .search(&op, "r2", "s", &["kb".into()], "q", &BTreeMap::new(), 5)
        .await
        .unwrap()
        .0;
    assert_eq!(status, RetrievalStatus::RetrievalEmpty);

    let failing = Mock::start(|_| Some(json(500, serde_json::json!({"message": "boom"}))));
    let client = failing.client();
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let status = client
        .search(&op, "r3", "s", &["kb".into()], "q", &BTreeMap::new(), 5)
        .await
        .unwrap()
        .0;
    assert_eq!(status, RetrievalStatus::RetrievalUnavailable);
}

#[tokio::test]
async fn deadline_and_cancel_abort_without_an_empty_result() {
    let mock = Mock::start(|_| {
        thread::sleep(Duration::from_secs(2));
        Some(json(
            200,
            serde_json::json!({"status": "complete", "evidence": []}),
        ))
    });
    let client = RemoteKnowledgeClient::new(
        format!("http://127.0.0.1:{}", mock.port),
        None,
        Duration::from_secs(5),
        0,
    )
    .unwrap();
    let op = Operation::new(Duration::from_millis(200)).unwrap();
    let status = client
        .search(&op, "r", "s", &["kb".into()], "q", &BTreeMap::new(), 5)
        .await
        .unwrap()
        .0;
    assert_eq!(status, RetrievalStatus::RetrievalTimeout);
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let cancel = op.clone();
    let started = std::time::Instant::now();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        cancel.cancel();
    });
    assert_eq!(
        client
            .search(&op, "cancel", "s", &["kb".into()], "q", &BTreeMap::new(), 5)
            .await
            .unwrap()
            .0,
        RetrievalStatus::RetrievalTimeout
    );
    assert!(started.elapsed() < Duration::from_millis(500));
}

#[tokio::test]
async fn permanent_client_error_does_not_retry_and_unknown_verifiability_is_false() {
    let attempts = Arc::new(Mutex::new(0));
    let mock = Mock::start({
        let attempts = attempts.clone();
        move |_| {
            *attempts.lock().unwrap() += 1;
            Some(json(400, serde_json::json!({"message": "bad"})))
        }
    });
    assert_eq!(
        mock.client()
            .search(
                &Operation::new(Duration::from_secs(2)).unwrap(),
                "r",
                "s",
                &["kb".into()],
                "q",
                &BTreeMap::new(),
                1
            )
            .await
            .unwrap()
            .0,
        RetrievalStatus::RetrievalUnavailable
    );
    assert_eq!(*attempts.lock().unwrap(), 1);
    let mock = Mock::start(|_| {
        Some(json(
            200,
            serde_json::json!({"status":"complete","evidence":[{"evidence_id":"e1","text":"unverified"}]}),
        ))
    });
    assert!(
        !mock
            .client()
            .search(
                &Operation::new(Duration::from_secs(2)).unwrap(),
                "r",
                "s",
                &["kb".into()],
                "q",
                &BTreeMap::new(),
                1
            )
            .await
            .unwrap()
            .1[0]
            .original_verifiable
    );
}

#[tokio::test]
async fn malformed_or_unknown_reply_is_invalid_not_empty() {
    let unknown_field = Mock::start(|_| {
        Some(json(
            200,
            serde_json::json!({"status": "complete", "evidence": [], "unexpected": true}),
        ))
    });
    assert_eq!(
        unknown_field
            .client()
            .search(
                &Operation::new(Duration::from_secs(2)).unwrap(),
                "r",
                "s",
                &["kb".into()],
                "q",
                &BTreeMap::new(),
                1
            )
            .await
            .unwrap_err(),
        RemoteError::Invalid
    );
    let missing_required =
        Mock::start(|_| Some(json(200, serde_json::json!({"status": "complete"}))));
    assert_eq!(
        missing_required
            .client()
            .search(
                &Operation::new(Duration::from_secs(2)).unwrap(),
                "r",
                "s",
                &["kb".into()],
                "q",
                &BTreeMap::new(),
                1
            )
            .await
            .unwrap_err(),
        RemoteError::Invalid
    );
}

fn gateway(mock: &Mock, policy: RemotePolicy) -> RemoteGateway {
    RemoteGateway::new(mock.client(), policy, None).unwrap()
}

#[tokio::test]
async fn policy_gates_operations_kb_and_query_upload() {
    let mock = Mock::start(|request| {
        let path = request.url().to_string();
        Some(match path.as_str() {
            "/capabilities" => json(200, capabilities_body()),
            "/search" => json(
                200,
                serde_json::json!({"status": "complete", "evidence": [{"evidence_id": "e1", "text": "t"}]}),
            ),
            _ => json(404, serde_json::json!({"message": "unknown"})),
        })
    });
    let allowed = gateway(&mock, policy());
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    assert_eq!(
        allowed
            .search(
                &op,
                "r",
                "s",
                &["kb-allowed".into()],
                "q",
                &BTreeMap::new(),
                1
            )
            .await
            .unwrap()
            .0,
        RetrievalStatus::RetrievalComplete
    );

    let mut no_upload = policy();
    no_upload.allow_query_upload = false;
    assert_eq!(
        gateway(&mock, no_upload)
            .search(
                &op,
                "r",
                "s",
                &["kb-allowed".into()],
                "q",
                &BTreeMap::new(),
                1
            )
            .await
            .unwrap_err(),
        RemoteError::Unauthorized
    );

    let mut no_search = policy();
    no_search.allowed_operations = vec![RemoteOperation::Answer];
    assert_eq!(
        gateway(&mock, no_search)
            .search(
                &op,
                "r",
                "s",
                &["kb-allowed".into()],
                "q",
                &BTreeMap::new(),
                1
            )
            .await
            .unwrap_err(),
        RemoteError::Unauthorized
    );

    let disallowed_kb = policy();
    assert_eq!(
        gateway(&mock, disallowed_kb)
            .search(
                &op,
                "r",
                "s",
                &["kb-other".into()],
                "q",
                &BTreeMap::new(),
                1
            )
            .await
            .unwrap_err(),
        RemoteError::Unauthorized
    );
}

#[tokio::test]
async fn expired_policy_and_answer_only_stay_human_reference() {
    let mock = Mock::start(|request| {
        let path = request.url().to_string();
        Some(match path.as_str() {
            "/answer" => json(
                200,
                serde_json::json!({"status": "complete", "capability": "answer_only", "evidence": [{"evidence_id": "e1", "text": "t"}], "answer_draft": "draft"}),
            ),
            "/search" => json(
                200,
                serde_json::json!({"status": "complete", "evidence": []}),
            ),
            _ => json(404, serde_json::json!({"message": "unknown"})),
        })
    });
    let mut expired = policy();
    expired.expires_at_utc_ms = (now_ms() - 1).to_string();
    assert_eq!(
        gateway(&mock, expired)
            .search(
                &Operation::new(Duration::from_secs(2)).unwrap(),
                "r",
                "s",
                &["kb-allowed".into()],
                "q",
                &BTreeMap::new(),
                1
            )
            .await
            .unwrap_err(),
        RemoteError::Unauthorized
    );

    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let (status, evidence, draft, answer_only) = gateway(&mock, policy())
        .answer(
            &op,
            "r",
            "s",
            &["kb-allowed".into()],
            "q",
            &BTreeMap::new(),
            1,
        )
        .await
        .unwrap();
    assert_eq!(status, RetrievalStatus::RetrievalComplete);
    assert_eq!(evidence.len(), 1);
    assert_eq!(draft.as_deref(), Some("draft"));
    assert!(answer_only);
}

#[tokio::test]
async fn sync_and_delete_are_gated_single_attempt_and_return_receipts() {
    let attempts = Arc::new(Mutex::new(0));
    let mock = Mock::start({
        let attempts = attempts.clone();
        move |request| {
            let path = request.url().to_string();
            *attempts.lock().unwrap() += 1;
            Some(match path.as_str() {
                "/sync" => json(
                    200,
                    serde_json::json!({"request_id": "r", "status": "accepted", "policy_ref": "p"}),
                ),
                "/delete" => json(
                    200,
                    serde_json::json!({"request_id": "r", "status": "deleted", "policy_ref": "p"}),
                ),
                _ => json(404, serde_json::json!({"message": "unknown"})),
            })
        }
    });
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let gw = gateway(&mock, policy());
    let document = RemoteDocument {
        document_id: "doc".into(),
        revision: "r1".into(),
        content: "text".into(),
    };
    let receipt = gw
        .sync(&op, "kb-allowed", "req-sync", "idem-1", vec![document])
        .await
        .unwrap();
    assert_eq!(
        receipt.status,
        nai_contracts::remote_knowledge::RemoteReceiptStatus::Accepted
    );
    let receipt = gw
        .delete(&op, "kb-allowed", "req-del", "idem-2", vec!["doc".into()])
        .await
        .unwrap();
    assert_eq!(
        receipt.status,
        nai_contracts::remote_knowledge::RemoteReceiptStatus::Deleted
    );

    let mut no_upload = policy();
    no_upload.allow_document_upload = false;
    assert_eq!(
        gateway(&mock, no_upload)
            .sync(
                &op,
                "kb-allowed",
                "r",
                "i",
                vec![RemoteDocument {
                    document_id: "d".into(),
                    revision: "r".into(),
                    content: "t".into()
                }]
            )
            .await
            .unwrap_err(),
        RemoteError::Unauthorized
    );

    let mut no_retention = policy();
    no_retention.allow_third_party_retention = false;
    assert_eq!(
        gateway(&mock, no_retention)
            .sync(
                &op,
                "kb-allowed",
                "r",
                "i",
                vec![RemoteDocument {
                    document_id: "d".into(),
                    revision: "r".into(),
                    content: "t".into()
                }]
            )
            .await
            .unwrap_err(),
        RemoteError::Unauthorized
    );

    let mut no_delete = policy();
    no_delete.require_remote_delete = false;
    assert_eq!(
        gateway(&mock, no_delete)
            .delete(&op, "kb-allowed", "r", "i", vec!["d".into()])
            .await
            .unwrap_err(),
        RemoteError::Unauthorized
    );

    assert_eq!(
        *attempts.lock().unwrap(),
        2,
        "gated calls must not reach the network"
    );
}

// ---------------------------------------------------------------------------
// W113：§24.4 远端留存清除 —— `RemoteGateway::delete` 传输契约判定面闭合
// ---------------------------------------------------------------------------
// 既有 `sync_and_delete_are_gated_single_attempt_and_return_receipts` 只钉了
// `require_remote_delete == false` 与 `allow_third_party_retention == false`
// 两条门控开关；delete 的**输入边界**与「回执透传 ≠ 第三方擦除证明」这两条
// §24.4 不变量此前零判据。下面两个测试钉死它们（零生产代码改动）。

#[tokio::test]
async fn remote_delete_rejects_malformed_document_ids() {
    // 远端始终返回 deleted；本测试钉 delete 的**输入边界**闭世界
    // （remote.rs:670-672），与既有门控开关测试互补。
    let mock = Mock::start(|_| {
        Some(json(
            200,
            serde_json::json!({"request_id": "r", "status": "deleted", "policy_ref": "p"}),
        ))
    });
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let gw = gateway(&mock, policy());

    // 空集 ⇒ 拒（不触网）
    assert_eq!(
        gw.delete(&op, "kb-allowed", "r", "i", vec![])
            .await
            .unwrap_err(),
        RemoteError::Unauthorized
    );
    // 超过 16 个 ⇒ 拒
    let too_many: Vec<String> = (0..17).map(|n| format!("d{n}")).collect();
    assert_eq!(
        gw.delete(&op, "kb-allowed", "r", "i", too_many)
            .await
            .unwrap_err(),
        RemoteError::Unauthorized
    );
    // 含空 id ⇒ 拒
    assert_eq!(
        gw.delete(&op, "kb-allowed", "r", "i", vec!["".into()])
            .await
            .unwrap_err(),
        RemoteError::Unauthorized
    );
}

#[tokio::test]
async fn remote_delete_receipt_is_passed_through_and_never_claims_erasure() {
    // §24.4：远端回执透传，绝不声称第三方副本已擦除（remote.rs:658-659）。
    let mock = Mock::start(|request| {
        let path = request.url().to_string();
        Some(match path.as_str() {
            "/delete" => json(
                200,
                serde_json::json!({"request_id": "r", "status": "deleted", "policy_ref": "p"}),
            ),
            _ => json(404, serde_json::json!({"message": "unknown"})),
        })
    });
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let receipt: RemoteReceipt = gateway(&mock, policy())
        .delete(&op, "kb-allowed", "req-del", "idem", vec!["doc".into()])
        .await
        .unwrap();
    // 透传：回执原样返回，网关不做任何「已彻底擦除」转换
    assert_eq!(receipt.status, RemoteReceiptStatus::Deleted);
    assert_eq!(receipt.request_id, "r");
    assert_eq!(receipt.policy_ref, "p");
    // 闭世界守卫：RemoteReceiptStatus 必须恰好是这四种，且无一表示「第三方已擦除」。
    // 若契约新增 erased/purged 之类变体，下面的穷尽 match 会编译失败，强制就 §24.4
    // 不变量评审一次（防止有人把远端回执谎称为擦除证明）。
    match receipt.status {
        RemoteReceiptStatus::Accepted
        | RemoteReceiptStatus::Pending
        | RemoteReceiptStatus::Deleted
        | RemoteReceiptStatus::Retained => {}
    }
}

#[tokio::test]
async fn rerank_preserves_base_on_budget_expiry_and_drops_unknown_ids() {
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let evidence = vec![
        nai_contracts::v1::Evidence {
            evidence_id: "a".into(),
            text: "a".into(),
            ..Default::default()
        },
        nai_contracts::v1::Evidence {
            evidence_id: "b".into(),
            text: "b".into(),
            ..Default::default()
        },
    ];
    let fast: Arc<
        dyn Fn(Vec<nai_contracts::v1::Evidence>) -> Vec<nai_contracts::v1::Evidence> + Send + Sync,
    > = Arc::new(|items| items.into_iter().rev().collect());
    let outcome = rerank_evidence(
        &op,
        RetrievalStatus::RetrievalComplete,
        evidence.clone(),
        Duration::from_secs(2),
        &fast,
    )
    .await
    .unwrap();
    assert_eq!(outcome.status, RetrievalStatus::RetrievalComplete);
    assert_eq!(outcome.evidence[0].evidence_id, "b");
    assert_eq!(outcome.evidence.len(), 2);

    let injecting: Arc<
        dyn Fn(Vec<nai_contracts::v1::Evidence>) -> Vec<nai_contracts::v1::Evidence> + Send + Sync,
    > = Arc::new(|mut items| {
        items.push(nai_contracts::v1::Evidence {
            evidence_id: "unknown".into(),
            text: "injected".into(),
            ..Default::default()
        });
        items
    });
    let outcome = rerank_evidence(
        &op,
        RetrievalStatus::RetrievalComplete,
        evidence.clone(),
        Duration::from_secs(2),
        &injecting,
    )
    .await
    .unwrap();
    assert_eq!(outcome.evidence.len(), 2);
    assert!(
        !outcome
            .evidence
            .iter()
            .any(|item| item.evidence_id == "unknown")
    );

    let slow: Arc<
        dyn Fn(Vec<nai_contracts::v1::Evidence>) -> Vec<nai_contracts::v1::Evidence> + Send + Sync,
    > = Arc::new(|items| {
        thread::sleep(Duration::from_millis(300));
        items
    });
    let outcome = rerank_evidence(
        &op,
        RetrievalStatus::RetrievalComplete,
        evidence,
        Duration::from_millis(50),
        &slow,
    )
    .await
    .unwrap();
    assert_eq!(outcome.status, RetrievalStatus::RetrievalPartial);
    assert_eq!(outcome.evidence.len(), 2);
}
