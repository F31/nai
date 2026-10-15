use nai_contracts::remote_knowledge::{
    RemoteCapability, RemoteDocument, RemoteOperation, RemotePolicy, RemoteReceipt,
    RemoteReceiptStatus,
};
use nai_contracts::v1::RetrievalStatus;
use nai_knowledge_core::{RemoteError, RemoteGateway, RemoteKnowledgeClient, rerank_evidence};
use nai_security::{Operation, Secret};
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

// ---------------------------------------------------------------------------
// W114：远端网关构造期与输入边界闭世界守卫
// ---------------------------------------------------------------------------
// §25.3 把 policy 身份字段 / unknown_revision_ttl_ms、客户端 endpoint/timeout/
// max_retries、sync 文档边界、capabilities 首操作守卫的「已声明不变量」钉成显式
// 构造期与输入边界闭世界。既有 `gateway()` 辅助函数只用合法 policy `.unwrap()`、
// `Mock::client()` 只传合法参数，下列四条此前零判据。全部零生产代码改动。

#[tokio::test]
async fn remote_gateway_new_rejects_invalid_policy() {
    // remote.rs:518-526：构造期闭世界 —— policy 必填身份字段与
    // unknown_revision_ttl_ms 取值边界。构造本身不触网、也不应 panic。
    let mock = Mock::start(|_| Some(json(404, serde_json::json!({"message": "unused"}))));

    let mut bad = policy();
    bad.source_id = "".into();
    assert_eq!(
        RemoteGateway::new(mock.client(), bad, None).err(),
        Some(RemoteError::Invalid)
    );

    let mut bad = policy();
    bad.policy_ref = "".into();
    assert_eq!(
        RemoteGateway::new(mock.client(), bad, None).err(),
        Some(RemoteError::Invalid)
    );

    let mut bad = policy();
    bad.permission_revision = "".into();
    assert_eq!(
        RemoteGateway::new(mock.client(), bad, None).err(),
        Some(RemoteError::Invalid)
    );

    let mut bad = policy();
    bad.allowed_kb_ids = vec![];
    assert_eq!(
        RemoteGateway::new(mock.client(), bad, None).err(),
        Some(RemoteError::Invalid)
    );

    // unknown_revision_ttl_ms == 0 ⇒ 拒
    let mut bad = policy();
    bad.unknown_revision_ttl_ms = 0;
    assert_eq!(
        RemoteGateway::new(mock.client(), bad, None).err(),
        Some(RemoteError::Invalid)
    );

    // unknown_revision_ttl_ms > 5000 ⇒ 拒
    let mut bad = policy();
    bad.unknown_revision_ttl_ms = 5001;
    assert_eq!(
        RemoteGateway::new(mock.client(), bad, None).err(),
        Some(RemoteError::Invalid)
    );

    // 合法 policy 仍应构造成功（守卫不误伤正常路径）
    assert!(RemoteGateway::new(mock.client(), policy(), None).is_ok());
}

#[tokio::test]
async fn remote_knowledge_client_new_rejects_invalid_args() {
    // remote.rs:67-74：构造期闭世界 —— endpoint / timeout / max_retries 取值边界。
    // 纯构造测试，不触网（校验在建 http client 之前）。`.err()` 而非 `.unwrap_err()`
    // 因为 `RemoteKnowledgeClient` 未实现 `Debug`（`.unwrap_err()` 要求 Ok 值可 Debug）。
    // 空 endpoint
    assert_eq!(
        RemoteKnowledgeClient::new("".into(), None, Duration::from_secs(5), 1).err(),
        Some(RemoteError::Invalid)
    );
    // endpoint 过长（> 2048）
    assert_eq!(
        RemoteKnowledgeClient::new("x".repeat(2049), None, Duration::from_secs(5), 1).err(),
        Some(RemoteError::Invalid)
    );
    // timeout 为零
    assert_eq!(
        RemoteKnowledgeClient::new("http://example".into(), None, Duration::from_secs(0), 1).err(),
        Some(RemoteError::Invalid)
    );
    // timeout 超过 600s
    assert_eq!(
        RemoteKnowledgeClient::new("http://example".into(), None, Duration::from_secs(601), 1)
            .err(),
        Some(RemoteError::Invalid)
    );
    // max_retries 超过 10
    assert_eq!(
        RemoteKnowledgeClient::new("http://example".into(), None, Duration::from_secs(5), 11).err(),
        Some(RemoteError::Invalid)
    );
    // 合法参数仍应构造成功
    assert!(
        RemoteKnowledgeClient::new("http://example".into(), None, Duration::from_secs(5), 1)
            .is_ok()
    );
}

#[tokio::test]
async fn remote_sync_rejects_malformed_documents_without_network() {
    // §25.3 / §24.4 对称缺口：sync 输入边界闭世界（remote.rs:639-647）。
    // 既有 `sync_and_delete_are_gated_*` 只钉了 allow_document_upload /
    // allow_third_party_retention 两条开关；documents 的空集 / >16 / content>65536
    // 边界此前零判据。下面钉死，并显式证明被拒调用未触网。
    let attempts = Arc::new(Mutex::new(0));
    let mock = Mock::start({
        let attempts = attempts.clone();
        move |request| {
            if request.url() == "/sync" {
                let mut count = attempts.lock().unwrap();
                *count += 1;
            }
            Some(json(
                200,
                serde_json::json!({"request_id": "r", "status": "accepted", "policy_ref": "p"}),
            ))
        }
    });
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let gw = gateway(&mock, policy());

    // 空集 ⇒ 拒（不触网）
    assert_eq!(
        gw.sync(&op, "kb-allowed", "r", "i", vec![])
            .await
            .unwrap_err(),
        RemoteError::Unauthorized
    );
    // 超过 16 个 ⇒ 拒
    let too_many: Vec<RemoteDocument> = (0..17)
        .map(|n| RemoteDocument {
            document_id: format!("d{n}"),
            revision: "r".into(),
            content: "t".into(),
        })
        .collect();
    assert_eq!(
        gw.sync(&op, "kb-allowed", "r", "i", too_many)
            .await
            .unwrap_err(),
        RemoteError::Unauthorized
    );
    // 单文档 content 超过 65536 字节 ⇒ 拒
    let oversized = vec![RemoteDocument {
        document_id: "big".into(),
        revision: "r".into(),
        content: "x".repeat(65537),
    }];
    assert_eq!(
        gw.sync(&op, "kb-allowed", "r", "i", oversized)
            .await
            .unwrap_err(),
        RemoteError::Unauthorized
    );
    // 合法单文档仍应成功（守卫不误伤正常路径）
    let ok = gw
        .sync(
            &op,
            "kb-allowed",
            "r",
            "i",
            vec![RemoteDocument {
                document_id: "ok".into(),
                revision: "r".into(),
                content: "t".into(),
            }],
        )
        .await
        .unwrap();
    assert_eq!(ok.status, RemoteReceiptStatus::Accepted);
    assert_eq!(
        *attempts.lock().unwrap(),
        1,
        "only the well-formed call must reach the network"
    );
}

#[tokio::test]
async fn remote_capabilities_rejects_when_first_allowed_op_not_search_or_answer() {
    // remote.rs:571-581：capabilities 首操作守卫。policy 首个 allowed op 必须是
    // Search 或 Answer（检索/问答才是机器消费），否则即便远端可用也不得暴露给上层。
    // 既有测试首 op 都是 Search，零判据。
    let mock = Mock::start(|_| {
        Some(json(
            200,
            serde_json::json!({
                "capabilities": ["retrieval_with_evidence", "answer_with_evidence"],
                "operations": ["search", "answer", "sync", "delete"],
                "retention": {"policy_ref": "retention-v1", "retention_seconds": 86400, "supports_delete": true}
            }),
        ))
    });
    let op = Operation::new(Duration::from_secs(5)).unwrap();

    // 首 op 是 Sync ⇒ 拒（不触网，远端即便可用也不被查询）
    let mut p = policy();
    p.allowed_operations = vec![RemoteOperation::Sync, RemoteOperation::Delete];
    assert_eq!(
        RemoteGateway::new(mock.client(), p, None)
            .unwrap()
            .capabilities(&op)
            .await
            .unwrap_err(),
        RemoteError::Unauthorized
    );

    // 首 op 是 Answer ⇒ 放行（合法正常路径）
    let mut p = policy();
    p.allowed_operations = vec![RemoteOperation::Answer, RemoteOperation::Search];
    let caps = RemoteGateway::new(mock.client(), p, None)
        .unwrap()
        .capabilities(&op)
        .await
        .unwrap();
    assert!(caps.retention.supports_delete);
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

// ---------------------------------------------------------------------------
// W115：检索证据闭世界守卫（rerank_evidence + to_evidence 的已声明不变量）
// ---------------------------------------------------------------------------
// §25.3 / §25.4 把「检索证据集合的闭世界」钉成显式判据：空 base 短路、重复 id
// 去重、reranker 故障→Invalid、空 evidence_id 丢弃。上述不变量此前在 remote.rs
// 中「已声明、零判据」。全部零生产代码改动，确定性离线（前三条直调
// rerank_evidence，第四条经公共 search 路径 + localhost Mock）。与既有 901 区
// rerank 测试（reorder / unknown-id drop / budget-expiry partial）互不重复。

#[tokio::test]
async fn rerank_empty_base_returns_empty_without_calling_reranker() {
    // remote.rs:463-468：空 base 短路守卫。base 为空时必须在调用 reranker 之前
    // 直接返回 RetrievalEmpty，绝不触达 reranker。用「会 panic 的 reranker」证明：
    // 若早期返回未生效、reranker 被调用，则 spawn_blocking 抛 JoinError →
    // RemoteError::Invalid → .unwrap() 会让本测试失败。
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let boom: Arc<
        dyn Fn(Vec<nai_contracts::v1::Evidence>) -> Vec<nai_contracts::v1::Evidence> + Send + Sync,
    > = Arc::new(|_| panic!("reranker must not be invoked for an empty base"));
    let outcome = rerank_evidence(
        &op,
        RetrievalStatus::RetrievalComplete,
        Vec::new(),
        Duration::from_secs(2),
        &boom,
    )
    .await
    .unwrap();
    assert_eq!(outcome.status, RetrievalStatus::RetrievalEmpty);
    assert!(outcome.evidence.is_empty());
}

#[tokio::test]
async fn rerank_deduplicates_repeated_ids() {
    // remote.rs:490-495：闭世界去重守卫。base 内出现重复 evidence_id 时，
    // `seen.insert` 必须把后续同名项丢弃（已知集合 `known` 也只认一次）。
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let base = vec![
        nai_contracts::v1::Evidence {
            evidence_id: "a".into(),
            text: "a1".into(),
            ..Default::default()
        },
        nai_contracts::v1::Evidence {
            evidence_id: "a".into(),
            text: "a2".into(),
            ..Default::default()
        },
        nai_contracts::v1::Evidence {
            evidence_id: "b".into(),
            text: "b".into(),
            ..Default::default()
        },
    ];
    // identity reranker：保持顺序，仅供去重判据观察
    let identity: Arc<
        dyn Fn(Vec<nai_contracts::v1::Evidence>) -> Vec<nai_contracts::v1::Evidence> + Send + Sync,
    > = Arc::new(|items| items);
    let outcome = rerank_evidence(
        &op,
        RetrievalStatus::RetrievalComplete,
        base,
        Duration::from_secs(2),
        &identity,
    )
    .await
    .unwrap();
    // 已知集合 {a,b}；a1 入、a2 重复被 seen 丢弃、b 入 ⇒ 仅 2 条
    assert_eq!(outcome.evidence.len(), 2);
    assert_eq!(outcome.evidence[0].evidence_id, "a");
    assert_eq!(outcome.evidence[1].evidence_id, "b");
}

#[tokio::test]
async fn rerank_reranker_failure_maps_to_invalid() {
    // remote.rs:476-478：reranker 在 spawn_blocking 内 panic ⇒ JoinError ⇒
    // 必须映射为 RemoteError::Invalid（不是 Cancelled、也不是静默吞掉）。
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let base = vec![nai_contracts::v1::Evidence {
        evidence_id: "a".into(),
        text: "a".into(),
        ..Default::default()
    }];
    let boom: Arc<
        dyn Fn(Vec<nai_contracts::v1::Evidence>) -> Vec<nai_contracts::v1::Evidence> + Send + Sync,
    > = Arc::new(|_| panic!("reranker exploded"));
    let err = rerank_evidence(
        &op,
        RetrievalStatus::RetrievalComplete,
        base,
        Duration::from_secs(2),
        &boom,
    )
    .await
    .err();
    assert_eq!(err, Some(RemoteError::Invalid));
}

#[tokio::test]
async fn search_drops_evidence_with_empty_id() {
    // remote.rs:413-416 / 404-411：to_evidence 闭世界守卫。远端返回的 evidence
    // 若 evidence_id 为空，必须被 filter_map 丢弃，绝不进入上层消费集合。
    // 经公共 search 路径（私有 to_evidence 不可从集成测试直调）确定性离线验证。
    let mock = Mock::start(|request| {
        let path = request.url().to_string();
        match path.as_str() {
            "/capabilities" => Some(json(200, capabilities_body())),
            "/search" => Some(json(
                200,
                serde_json::json!({
                    "status": "complete",
                    "evidence": [
                        {"evidence_id": "good", "text": "keep"},
                        {"evidence_id": "", "text": "drop-me"}
                    ]
                }),
            )),
            _ => Some(json(404, serde_json::json!({"message": "unknown"}))),
        }
    });
    let gw = gateway(&mock, policy());
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let (_status, evidence) = gw
        .search(
            &op,
            "r",
            "s",
            &["kb-allowed".into()],
            "q",
            &BTreeMap::new(),
            10,
        )
        .await
        .unwrap();
    assert_eq!(
        evidence.len(),
        1,
        "empty evidence_id must be dropped by to_evidence"
    );
    assert_eq!(evidence[0].evidence_id, "good");
}

// W117：检索结果 top_k 截断闭世界守卫
// remote.rs:395-411 parse_reply 的 evidence 处理顺序为
// `filter_map(to_evidence).take(top_k)`，此前零判据（既有测试无一条断言 top_k）。
// 两条非显然语义：
//   (a) top_k 统计的是「已丢弃空 id 后的有效项」数量（filter 先于 take）；
//   (b) take 保留序列前 N 项（顺序不变），top_k=0 ⇒ 空，top_k>有效项数 ⇒ 不补齐。
// parse_reply 私有，经公共 gateway.search 路径 + Mock 确定性离线覆盖。
fn search_mock(evidence: serde_json::Value) -> Mock {
    Mock::start(move |request| {
        let path = request.url().to_string();
        match path.as_str() {
            "/capabilities" => Some(json(200, capabilities_body())),
            "/search" => Some(json(
                200,
                serde_json::json!({"status": "complete", "evidence": evidence.clone()}),
            )),
            _ => Some(json(404, serde_json::json!({"message": "unknown"}))),
        }
    })
}

async fn gateway_search_ids(top_k: u32) -> Vec<String> {
    let mock = search_mock(serde_json::json!([
        {"evidence_id": "e0", "text": "t0"},
        {"evidence_id": "e1", "text": "t1"},
        {"evidence_id": "e2", "text": "t2"},
        {"evidence_id": "e3", "text": "t3"},
        {"evidence_id": "e4", "text": "t4"}
    ]));
    let gw = gateway(&mock, policy());
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let (_status, evidence) = gw
        .search(
            &op,
            "r",
            "s",
            &["kb-allowed".into()],
            "q",
            &BTreeMap::new(),
            top_k,
        )
        .await
        .unwrap();
    evidence.into_iter().map(|e| e.evidence_id).collect()
}

#[tokio::test]
async fn search_truncates_evidence_to_top_k_in_order() {
    // 5 个有效项配 top_k=2 ⇒ 恰好前 2 项，且顺序不变（e0, e1）。
    let ids = gateway_search_ids(2).await;
    assert_eq!(ids, ["e0", "e1"], "top_k must keep the first N in order");
}

#[tokio::test]
async fn search_applies_top_k_after_dropping_empty_ids() {
    // 非显然顺序：filter_map(to_evidence) 先于 take(top_k)——top_k 统计有效项。
    // reply=[e0, "", e1, e2]（3 有效）配 top_k=2 ⇒ [e0, e1]（len 2）。
    // 若回归成 take-then-filter，take(2) 得 [e0, ""]，filter 后只剩 [e0]（len 1）⇒ 本断言变红。
    let mock = search_mock(serde_json::json!([
        {"evidence_id": "e0", "text": "t0"},
        {"evidence_id": "", "text": "drop-me"},
        {"evidence_id": "e1", "text": "t1"},
        {"evidence_id": "e2", "text": "t2"}
    ]));
    let gw = gateway(&mock, policy());
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let (_status, evidence) = gw
        .search(
            &op,
            "r",
            "s",
            &["kb-allowed".into()],
            "q",
            &BTreeMap::new(),
            2,
        )
        .await
        .unwrap();
    let ids: Vec<&str> = evidence.iter().map(|e| e.evidence_id.as_str()).collect();
    assert_eq!(
        ids,
        ["e0", "e1"],
        "top_k counts valid evidence (filter precedes take)"
    );
}

#[tokio::test]
async fn search_top_k_zero_yields_empty_evidence() {
    // top_k=0 ⇒ take(0) ⇒ 空集合（MUST 被显式清点，绝不静默变「未截断」）。
    let ids = gateway_search_ids(0).await;
    assert!(ids.is_empty(), "top_k=0 must truncate to an empty set");
}

#[tokio::test]
async fn search_top_k_exceeding_reply_keeps_all_evidence() {
    // top_k>有效项数 ⇒ 保留全部，不补齐占位、不报错。
    let ids = gateway_search_ids(10).await;
    assert_eq!(
        ids,
        ["e0", "e1", "e2", "e3", "e4"],
        "top_k greater than evidence count must keep all, never pad"
    );
}

// W118：parse_reply 的 RemoteStatus → RetrievalStatus 闭世界映射守卫
// remote.rs:396-403 把 6 个 RemoteStatus 变体钉成 RetrievalStatus。既有测试经 parse_reply 只覆盖
// Complete/Empty 两臂；其余四臂（partial/timeout/unavailable/unauthorized）零判据——既有
// RetrievalTimeout/RetrievalUnavailable/RetrievalUnauthorized 断言全部来自 search 的 **HTTP 内联
// 错误分支**（RemoteError → status），而非 **body status 映射**（parse_reply）。故把 parse_reply 的
// 映射换错（如 unavailable ↔ unauthorized）完全不可见。经公共 gateway.search + Mock（HTTP 200 body
// status）逐臂钉死。
fn status_mock(status: &str) -> Mock {
    let status = status.to_string();
    Mock::start(move |request| {
        let path = request.url().to_string();
        match path.as_str() {
            "/capabilities" => Some(json(200, capabilities_body())),
            "/search" => Some(json(
                200,
                serde_json::json!({"status": status.clone(), "evidence": []}),
            )),
            _ => Some(json(404, serde_json::json!({"message": "unknown"}))),
        }
    })
}

async fn gateway_search_status(remote_status: &str) -> RetrievalStatus {
    let mock = status_mock(remote_status);
    let gw = gateway(&mock, policy());
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    gw.search(
        &op,
        "r",
        "s",
        &["kb-allowed".into()],
        "q",
        &BTreeMap::new(),
        5,
    )
    .await
    .unwrap()
    .0
}

#[tokio::test]
async fn search_maps_partial_remote_status_to_retrieval_partial() {
    assert_eq!(
        gateway_search_status("partial").await,
        RetrievalStatus::RetrievalPartial
    );
}

#[tokio::test]
async fn search_maps_timeout_remote_status_to_retrieval_timeout() {
    assert_eq!(
        gateway_search_status("timeout").await,
        RetrievalStatus::RetrievalTimeout
    );
}

#[tokio::test]
async fn search_maps_unavailable_remote_status_to_retrieval_unavailable() {
    assert_eq!(
        gateway_search_status("unavailable").await,
        RetrievalStatus::RetrievalUnavailable
    );
}

#[tokio::test]
async fn search_maps_unauthorized_remote_status_to_retrieval_unauthorized() {
    assert_eq!(
        gateway_search_status("unauthorized").await,
        RetrievalStatus::RetrievalUnauthorized
    );
}

// W119：出站请求构造闭世界守卫（remote.rs:100-112 `request()`）
// 既有测试经 `Mock::client()` 恒传 `api_key: None`、且 endpoint 无尾斜杠 ⇒ 下列不变量零判据：
//   (a) api_key = Some ⇒ 每个出站请求 MUST 带 `Authorization: Bearer <key>`（凭证真的被发出去，
//       若该头被静默丢弃，请求会以「未认证」身份发出而无人察觉）；
//   (b) api_key = None ⇒ MUST 不带 Authorization 头（闭世界：不凭空造凭证）；
//   (c) endpoint 尾斜杠 MUST 被 `trim_end_matches('/')` 规范化，拼出的路径不是 `//search`。
// 经真实出站请求观测（Mock 记录 url + Authorization 头），确定性离线。
#[derive(Clone, Default)]
struct ObservedRequest {
    url: String,
    authorization: Option<String>,
}

async fn observe_outgoing_request(endpoint: String, api_key: Option<Secret>) -> ObservedRequest {
    let observed = Arc::new(Mutex::new(ObservedRequest::default()));
    let mock = Mock::start({
        let observed = observed.clone();
        move |request| {
            let mut authorization = None;
            for header in request.headers() {
                // tiny_http 的 HeaderField/Header value 都是 `AsciiStr`；先经 AsRef<str>
                // 转成 &str 再做大小写无关比较（AsciiStr::eq_ignore_ascii_case 只接受 &AsciiStr）。
                let name: &str = header.field.as_str().as_ref();
                if name.eq_ignore_ascii_case("authorization") {
                    // `HeaderField::as_str()` 返回 &AsciiStr（需 as_ref 转 &str），
                    // 而 `Header.value`（AsciiString）的 as_str() 本身就返回 &str。
                    let value: &str = header.value.as_str();
                    authorization = Some(value.to_string());
                }
            }
            *observed.lock().unwrap() = ObservedRequest {
                url: request.url().to_string(),
                authorization,
            };
            Some(json(
                200,
                serde_json::json!({"status": "complete", "evidence": []}),
            ))
        }
    });
    let endpoint = endpoint.replace("{port}", &mock.port.to_string());
    let client = RemoteKnowledgeClient::new(endpoint, api_key, Duration::from_secs(5), 0).unwrap();
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    client
        .search(&op, "r", "s", &["kb".into()], "q", &BTreeMap::new(), 5)
        .await
        .unwrap();
    observed.lock().unwrap().clone()
}

#[tokio::test]
async fn remote_request_sends_bearer_authorization_header_when_api_key_present() {
    let observed = observe_outgoing_request(
        "http://127.0.0.1:{port}".into(),
        Some(Secret::new(b"test-api-key".to_vec()).unwrap()),
    )
    .await;
    assert_eq!(
        observed.authorization.as_deref(),
        Some("Bearer test-api-key"),
        "api_key must be sent as an exact `Authorization: Bearer <key>` header"
    );
}

#[tokio::test]
async fn remote_request_omits_authorization_header_when_api_key_absent() {
    let observed = observe_outgoing_request("http://127.0.0.1:{port}".into(), None).await;
    assert_eq!(
        observed.authorization, None,
        "no api_key must mean no Authorization header at all"
    );
}

#[tokio::test]
async fn remote_request_normalizes_trailing_slash_in_endpoint() {
    let observed = observe_outgoing_request("http://127.0.0.1:{port}/".into(), None).await;
    assert_eq!(
        observed.url, "/search",
        "a trailing slash in endpoint must be trimmed, never produce `//search`"
    );
}

#[tokio::test]
async fn remote_request_normalizes_repeated_trailing_slashes_in_endpoint() {
    let observed = observe_outgoing_request("http://127.0.0.1:{port}///".into(), None).await;
    assert_eq!(
        observed.url, "/search",
        "repeated trailing slashes in endpoint must be trimmed as well"
    );
}

// W116：RemoteError::status() 闭世界映射守卫
// remote.rs:37-46 把 RemoteError 各变体钉成 RetrievalStatus，此前「已声明、零判据」。
// 两条非显然坍缩：Invalid→RetrievalUnavailable、Timeout+Cancelled→RetrievalTimeout。
// 纯函数、确定性离线、零生产代码改动；覆盖全映射以封闭「已声明≠已执行」的缝。
#[test]
fn remote_error_status_unauthorized_maps_to_retrieval_unauthorized() {
    assert_eq!(
        RemoteError::Unauthorized.status(),
        RetrievalStatus::RetrievalUnauthorized
    );
}

#[test]
fn remote_error_status_unavailable_maps_to_retrieval_unavailable() {
    assert_eq!(
        RemoteError::Unavailable.status(),
        RetrievalStatus::RetrievalUnavailable
    );
}

#[test]
fn remote_error_status_invalid_maps_to_retrieval_unavailable() {
    // 非显然坍缩：Invalid 不映射到独立的 RetrievalInvalid，而是并入 RetrievalUnavailable。
    assert_eq!(
        RemoteError::Invalid.status(),
        RetrievalStatus::RetrievalUnavailable
    );
}

#[test]
fn remote_error_status_timeout_and_cancelled_map_to_retrieval_timeout() {
    // 非显然坍缩：Timeout 与 Cancelled 都并入 RetrievalTimeout。
    assert_eq!(
        RemoteError::Timeout.status(),
        RetrievalStatus::RetrievalTimeout
    );
    assert_eq!(
        RemoteError::Cancelled.status(),
        RetrievalStatus::RetrievalTimeout
    );
}
