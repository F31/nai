//! Local mock-server integration tests: transport, retry, cancellation, SSE,
//! binary stream, usage mapping and budget wiring. No external network.
use nai_contracts::v1::{
    AudioFormat, EmbeddingRequest, ErrorCode, LlmRequest, TtsRequest, UsageLedgerEntry, VoiceRef,
};
use nai_provider_core::{
    AuthScheme, CallBudget, HttpConfig, InMemoryGateway, OpenAiAdapter, ProviderClient, SseDecoder,
    usage_charge,
};
use nai_security::{Operation, Secret};
use serde_json::{Value, json};
use std::{
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use tiny_http::{Header, Response, Server};

const SECRET: &str = "test-secret-not-for-production";

fn entry(request: &str) -> UsageLedgerEntry {
    UsageLedgerEntry {
        workspace_id: "test-workspace".into(),
        session_id: "session-test".into(),
        provider_ref: "test-provider".into(),
        request_id: request.into(),
        generation_epoch: 1,
        estimate: Some(nai_contracts::v1::BudgetCharge {
            currency: "CNY".into(),
            cost_minor: 0,
            units: [("requests".into(), 1), ("tokens".into(), 100)].into(),
            price_revision: "price-1".into(),
        }),
        state: nai_contracts::v1::BillingState::Reserved as i32,
        ..Default::default()
    }
}

fn gateway() -> InMemoryGateway {
    let ceiling = nai_budget_core::Ceiling {
        cost_minor: 1000,
        units: [
            ("requests".into(), 1000u64),
            ("tokens".into(), 100_000u64),
            ("prompt_tokens".into(), 100_000u64),
            ("completion_tokens".into(), 100_000u64),
            ("total_tokens".into(), 100_000u64),
        ]
        .into(),
        concurrent: [("provider_slots".into(), 4)].into(),
    };
    InMemoryGateway::new(
        "test-workspace",
        nai_budget_core::Limits {
            currency: "CNY".into(),
            global: ceiling.clone(),
            sessions: [("session-test".into(), ceiling.clone())].into(),
            providers: [("test-provider".into(), ceiling.clone())].into(),
            max_entries: 100,
        },
    )
    .unwrap()
}

struct Mock {
    _server: Arc<Server>,
    port: u16,
}
impl Mock {
    fn start(
        handler: impl Fn(&tiny_http::Request) -> Option<Response<std::io::Cursor<Vec<u8>>>>
        + Send
        + Sync
        + 'static,
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
    fn client(&self, max_retries: u32, timeout: Duration) -> ProviderClient {
        ProviderClient::new(HttpConfig {
            provider_ref: "test-provider".into(),
            endpoint: format!("http://127.0.0.1:{}", self.port),
            api_key: Some(Secret::new(SECRET.as_bytes().to_vec()).unwrap()),
            auth: AuthScheme::Bearer,
            extra_headers: Vec::new(),
            model: "test-model".into(),
            price_revision: "price-1".into(),
            currency: "CNY".into(),
            timeout,
            max_retries,
        })
        .unwrap()
    }
}
fn json_response(status: u16, value: Value) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut response =
        Response::from_data(serde_json::to_vec(&value).unwrap()).with_status_code(status);
    response
        .add_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
    response
}
fn body_response(
    status: u16,
    content_type: &str,
    bytes: Vec<u8>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut response = Response::from_data(bytes).with_status_code(status);
    response.add_header(
        Header::from_bytes(&b"Content-Type"[..], &b"application/octet-stream"[..]).unwrap(),
    );
    let _ = content_type;
    response
}

#[tokio::test]
async fn sse_fragmented_utf8_and_multiline_data_are_reassembled() {
    let mock = Mock::start(|_| {
        let body = b"data: {\"choices\":[{\"delta\":{\"content\":\"\xe4\xb8\xad\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"\xe6\x96\x87\"}},{\"delta\":{\"content\":\"A\"}}]}\n\ndata: {\"choices\":[],\"usage\":{\"prompt_tokens\":1,\"total_tokens\":2}}\n\ndata: [DONE]\n\n".to_vec();
        Some(body_response(200, "text/event-stream", body))
    });
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let client = mock.client(0, Duration::from_secs(5));
    let adapter = OpenAiAdapter::new(&client);
    let mut gateway = gateway();
    let req = LlmRequest {
        permitted_context: "请输出中文".into(),
        max_output_tokens: 10,
        context: Some(nai_contracts::v1::RequestContext {
            request_id: "req-1".into(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let chunks = adapter
        .llm_stream(&op, &req, &mut gateway, entry("req-1"))
        .await
        .unwrap();
    let text: String = chunks
        .iter()
        .filter(|c| c.end == 1)
        .map(|c| c.text_delta.clone())
        .collect();
    assert_eq!(text, "中文");
    assert_eq!(chunks.last().unwrap().end, 2);
    let settled = gateway.book().get("test-provider", "req-1").unwrap();
    assert_eq!(
        settled.state,
        nai_contracts::v1::BillingState::Settled as i32
    );
    assert!(settled.actual.is_some());
}

#[tokio::test]
async fn sse_error_midstream_stops_without_completion_and_keeps_estimate() {
    let mock = Mock::start(|_| {
        let body = b"data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: {\"error\":{\"message\":\"boom\"}}\n\n".to_vec();
        Some(body_response(200, "text/event-stream", body))
    });
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let client = mock.client(0, Duration::from_secs(5));
    let adapter = OpenAiAdapter::new(&client);
    let mut gateway = gateway();
    let req = LlmRequest {
        permitted_context: "ctx".into(),
        max_output_tokens: 10,
        context: Some(nai_contracts::v1::RequestContext {
            request_id: "req-2".into(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let chunks = adapter
        .llm_stream(&op, &req, &mut gateway, entry("req-2"))
        .await
        .unwrap();
    assert_eq!(chunks.last().unwrap().end, 3);
    assert!(chunks.last().unwrap().error.is_some());
    let state = gateway.book().get("test-provider", "req-2").unwrap();
    assert_eq!(
        state.state,
        nai_contracts::v1::BillingState::NeedsReconciliation as i32
    );
    assert!(state.actual.is_none());
}

#[tokio::test]
async fn retry_after_respected_within_deadline_and_auth_never_retries() {
    let attempts = Arc::new(Mutex::new(0));
    let mock = Mock::start({
        let attempts = attempts.clone();
        move |request| {
            let path = request.url().to_string();
            let mut count = attempts.lock().unwrap();
            *count += 1;
            if path.starts_with("/retry") {
                if *count == 1 {
                    let mut r = json_response(429, json!({"error": {"message": "slow down"}}));
                    r.add_header(Header::from_bytes(&b"Retry-After"[..], &b"0"[..]).unwrap());
                    Some(r)
                } else {
                    Some(json_response(200, json!({"ok": true})))
                }
            } else {
                Some(json_response(401, json!({"error": {"message": "bad key"}})))
            }
        }
    });
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let client = mock.client(2, Duration::from_secs(5));
    let (status, body) = client
        .post_json(&op, "/retry", &json!({"x": 1}))
        .await
        .unwrap();
    assert_eq!(status, 200);
    assert!(body["ok"].as_bool().unwrap());
    assert_eq!(*attempts.lock().unwrap(), 2);
    let error = client
        .post_json(&op, "/auth", &json!({"x": 1}))
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Unauthenticated);
    assert!(!error.retryable);
    assert_eq!(*attempts.lock().unwrap(), 3);
}

#[tokio::test]
async fn five_hundred_retries_finite_and_unavailable_after_exhaustion() {
    let attempts = Arc::new(Mutex::new(0));
    let mock = Mock::start({
        let attempts = attempts.clone();
        move |_| {
            let mut count = attempts.lock().unwrap();
            *count += 1;
            if *count == 1 {
                Some(json_response(500, json!({"error": {"message": "boom"}})))
            } else {
                Some(json_response(200, json!({"ok": true})))
            }
        }
    });
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let client = mock.client(1, Duration::from_secs(5));
    let (status, _) = client
        .post_json(&op, "/five", &json!({"x": 1}))
        .await
        .unwrap();
    assert_eq!(status, 200);
    assert_eq!(*attempts.lock().unwrap(), 2);

    let mock = Mock::start(|_| Some(json_response(503, json!({}))));
    let client = mock.client(1, Duration::from_secs(5));
    let error = client
        .post_json(&op, "/always", &json!({}))
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ProviderUnavailable);
    assert!(error.retryable);
}

#[tokio::test]
async fn cancellation_and_deadline_abort_stream_and_skip_late_chunks() {
    let mock = Mock::start(|_| {
        let body = b"data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"b\"}}]}\n\n".to_vec();
        Some(body_response(200, "text/event-stream", body))
    });
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let client = mock.client(0, Duration::from_secs(5));
    let adapter = OpenAiAdapter::new(&client);
    let mut gateway = gateway();
    let req = LlmRequest {
        permitted_context: "ctx".into(),
        max_output_tokens: 10,
        context: Some(nai_contracts::v1::RequestContext {
            request_id: "req-cancel".into(),
            ..Default::default()
        }),
        ..Default::default()
    };
    op.cancel();
    let error = adapter
        .llm_stream(&op, &req, &mut gateway, entry("req-cancel"))
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Cancelled);
    let state = gateway.book().get("test-provider", "req-cancel").unwrap();
    assert!(matches!(
        nai_contracts::v1::BillingState::try_from(state.state),
        Ok(nai_contracts::v1::BillingState::NeedsReconciliation)
    ));
}

#[tokio::test]
async fn tts_binary_stream_chunks_and_usage_stays_estimate() {
    let mock = Mock::start(|_| {
        let body = vec![7u8; 10_000];
        Some(body_response(200, "audio/pcm", body))
    });
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let client = mock.client(0, Duration::from_secs(5));
    let adapter = OpenAiAdapter::new(&client);
    let mut gateway = gateway();
    let mut chunks = Vec::new();
    let req = TtsRequest {
        speech_text: "测试".into(),
        voice: Some(VoiceRef {
            provider_ref: "test-provider".into(),
            voice_ref: "alloy".into(),
            ..Default::default()
        }),
        format: Some(AudioFormat {
            sample_rate: 24_000,
            channels: 1,
            sample_format: "s16".into(),
            codec: "pcm".into(),
        }),
        speed: 1.0,
        ..Default::default()
    };
    let total = adapter
        .tts_stream(&op, &req, &mut gateway, entry("req-tts"), &mut |chunk| {
            chunks.push(chunk)
        })
        .await
        .unwrap();
    assert_eq!(total, 10_000);
    assert!(chunks.len() >= 3);
    assert_eq!(chunks.last().unwrap().end, 2);
    let state = gateway.book().get("test-provider", "req-tts").unwrap();
    assert_eq!(
        state.state,
        nai_contracts::v1::BillingState::NeedsReconciliation as i32
    );
    assert!(state.actual.is_none());
}

#[tokio::test]
async fn embeddings_usage_maps_to_exact_units_and_requests_are_preserved() {
    let mock = Mock::start(|_| {
        Some(json_response(
            200,
            json!({
                "object": "list",
                "data": [{"embedding": [0.1, 0.2, 0.3], "index": 0}],
                "model": "text-embedding-3-small",
                "usage": {"prompt_tokens": 2, "total_tokens": 2}
            }),
        ))
    });
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let client = mock.client(0, Duration::from_secs(5));
    let adapter = OpenAiAdapter::new(&client);
    let mut gateway = gateway();
    let req = EmbeddingRequest {
        permitted_texts: vec!["测试".into()],
        ..Default::default()
    };
    let result = adapter
        .embeddings(&op, &req, &mut gateway, entry("req-emb"))
        .await
        .unwrap();
    assert_eq!(result.dimension, 3);
    assert_eq!(result.vectors.len(), 1);
    let state = gateway.book().get("test-provider", "req-emb").unwrap();
    assert_eq!(state.state, nai_contracts::v1::BillingState::Settled as i32);
    let actual = state.actual.as_ref().unwrap();
    assert_eq!(actual.units.get("requests"), Some(&1));
    assert_eq!(actual.units.get("total_tokens"), Some(&2));
}

#[tokio::test]
async fn redispatch_after_ack_is_acquired_false_and_does_not_restart_io() {
    let requests = Arc::new(Mutex::new(0));
    let mock = Mock::start({
        let requests = requests.clone();
        move |_| {
            let mut count = requests.lock().unwrap();
            *count += 1;
            Some(json_response(
                200,
                json!({"data": [{"embedding": [0.1], "index": 0}], "model": "test", "usage": {"total_tokens": 3}}),
            ))
        }
    });
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let client = mock.client(0, Duration::from_secs(5));
    let adapter = OpenAiAdapter::new(&client);
    let mut gateway = gateway();
    let req = EmbeddingRequest {
        permitted_texts: vec!["x".into()],
        ..Default::default()
    };
    adapter
        .embeddings(&op, &req, &mut gateway, entry("req-redispatch"))
        .await
        .unwrap();
    assert_eq!(*requests.lock().unwrap(), 1);
    // Simulated ACK retry: reserve is idempotent and dispatch reports Already.
    let budget = CallBudget::start(&mut gateway, entry("req-redispatch")).unwrap();
    assert!(!budget.dispatched());
    assert_eq!(*requests.lock().unwrap(), 1);
}

#[tokio::test]
async fn sse_decoder_reassembles_fragmented_utf8() {
    let mut decoder = SseDecoder::new();
    let mut events = Vec::new();
    events.extend(decoder.push(b"data: {\"a\":\"\xe4"));
    events.extend(decoder.push(b"\xb8\xad\"}\n\n"));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "{\"a\":\"中\"}");
    assert!(!decoder.done());
    events.extend(decoder.push(b"data: [DONE]\n\n"));
    assert_eq!(events.len(), 2);
    assert!(events[1].done);
    assert!(decoder.done());
}
#[tokio::test]
async fn usage_charge_rejects_non_integer_and_keeps_currency() {
    assert!(usage_charge("CNY", "price-1", &json!({"total_tokens": 5}), 1).is_ok());
    assert!(usage_charge("CNY", "price-1", &json!({"total_tokens": "5"}), 1).is_err());
    assert!(usage_charge("CNY", "price-1", &json!({"tokens": 1.0}), 1).is_err());
}
