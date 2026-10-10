//! Substitution integration test (A02 "仅改配置换 Provider").
//!
//! Two vendors, two wire protocols, **one driver**. `generate_once` and
//! `speak_once` below are the caller: they hold a capability client from
//! `provider_core::binding` and never name a vendor. The only thing that differs
//! between the two halves of each test is the [`ProviderProfile`] they are given
//! — a configuration value, not code.
//!
//! What is asserted, and why each assertion is the one that matters:
//!
//! * the observed wire really differed (path, credential header, body shape), so
//!   the test cannot pass by accidentally talking to the same protocol twice;
//! * the caller-visible result is the same shape from both vendors, so a swap
//!   does not leak into the pipeline;
//! * usage lands in the closed canonical units for both vendors even though the
//!   vendors report different field names;
//! * speech is synthesized one sentence per call with no sequence number, buffer
//!   reference or sample offset carried across calls, which is what forbids
//!   stitching two sentences into one continuous buffer.
//!
//! No external network: both endpoints are local `tiny_http` servers.
use nai_contracts::v1::{
    AudioFormat, ErrorCode, LlmChunk, LlmRequest, StreamEnd, TtsChunk, TtsRequest,
    UsageLedgerEntry, VoiceRef,
};
use nai_provider_core::{
    AuthScheme, Capability, InMemoryGateway, LlmClient, Protocol, ProviderProfile, SpeechClient,
};
use nai_security::{Operation, Secret};
use serde_json::Value;
use std::{
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use tiny_http::{Header, Response, Server};

const SECRET: &str = "vendor-secret-not-for-production";

// ---------------------------------------------------------------- mock server

/// One request as the mock endpoint saw it.
#[derive(Debug, Clone, Default)]
struct Recorded {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Recorded {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).expect("mock endpoint received non-JSON body")
    }
}

struct Mock {
    _server: Arc<Server>,
    port: u16,
    seen: Arc<Mutex<Vec<Recorded>>>,
}

impl Mock {
    fn start(
        handler: impl Fn(&Recorded) -> Response<std::io::Cursor<Vec<u8>>> + Send + Sync + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = Arc::new(Server::from_listener(listener, None).unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let worker = server.clone();
        let records = seen.clone();
        let handler = Arc::new(handler);
        thread::spawn(move || {
            while let Ok(mut request) = worker.recv() {
                let mut body = Vec::new();
                // Read through the trait explicitly: the reader is a
                // `&mut dyn Read`, so naming the trait avoids relying on what
                // happens to be in scope here.
                let _ = std::io::Read::read_to_end(&mut request.as_reader(), &mut body);
                let recorded = Recorded {
                    method: request.method().as_str().to_string(),
                    path: request.url().to_string(),
                    headers: request
                        .headers()
                        .iter()
                        .map(|header| {
                            (
                                header.field.as_str().as_str().to_ascii_lowercase(),
                                header.value.as_str().to_string(),
                            )
                        })
                        .collect(),
                    body,
                };
                records.lock().unwrap().push(recorded.clone());
                let handler = handler.clone();
                let response = handler(&recorded);
                let _ = request.respond(response);
            }
        });
        Self {
            _server: server,
            port,
            seen,
        }
    }

    fn endpoint(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn requests(&self) -> Vec<Recorded> {
        self.seen.lock().unwrap().clone()
    }
}

fn sse_response(frames: &[&str]) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut body = String::new();
    for data in frames {
        body.push_str("data: ");
        body.push_str(data);
        body.push_str("\n\n");
    }
    let mut response = Response::from_data(body.into_bytes()).with_status_code(200);
    response
        .add_header(Header::from_bytes(&b"Content-Type"[..], &b"text/event-stream"[..]).unwrap());
    response
}

fn binary_response(
    bytes: Vec<u8>,
    extra: Option<(&str, String)>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut response = Response::from_data(bytes).with_status_code(200);
    response.add_header(
        Header::from_bytes(&b"Content-Type"[..], &b"application/octet-stream"[..]).unwrap(),
    );
    if let Some((name, value)) = extra {
        response.add_header(Header::from_bytes(name.as_bytes(), value.as_bytes()).unwrap());
    }
    response
}

// ------------------------------------------------------------------ the caller

fn profile(
    provider_ref: &str,
    endpoint: String,
    protocol: Protocol,
    model: &str,
) -> ProviderProfile {
    ProviderProfile {
        provider_ref: provider_ref.into(),
        endpoint,
        model: model.into(),
        price_revision: "price-1".into(),
        currency: "CNY".into(),
        timeout: Duration::from_secs(10),
        max_retries: 0,
        protocol,
    }
}

fn gateway(provider_ref: &str) -> InMemoryGateway {
    let ceiling = nai_budget_core::Ceiling {
        cost_minor: 1_000,
        units: [
            ("requests".into(), 1_000u64),
            ("tokens".into(), 1_000_000u64),
            ("prompt_tokens".into(), 1_000_000u64),
            ("completion_tokens".into(), 1_000_000u64),
            ("total_tokens".into(), 1_000_000u64),
            ("characters".into(), 1_000_000u64),
            ("audio_seconds".into(), 1_000_000u64),
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
            providers: [(provider_ref.to_string(), ceiling)].into(),
            max_entries: 100,
        },
    )
    .unwrap()
}

fn usage_entry(
    provider_ref: &str,
    request_id: &str,
    unit: &str,
    quantity: u64,
) -> UsageLedgerEntry {
    UsageLedgerEntry {
        workspace_id: "test-workspace".into(),
        session_id: "session-test".into(),
        provider_ref: provider_ref.into(),
        request_id: request_id.into(),
        generation_epoch: 1,
        estimate: Some(nai_contracts::v1::BudgetCharge {
            currency: "CNY".into(),
            cost_minor: 0,
            units: [("requests".into(), 1), (unit.into(), quantity)].into(),
            price_revision: "price-1".into(),
        }),
        state: nai_contracts::v1::BillingState::Reserved as i32,
        ..Default::default()
    }
}

/// The caller under test for text: provider-agnostic by construction.
async fn generate_once(
    llm: &LlmClient<'_>,
    op: &Operation,
    request: &LlmRequest,
    gateway: &mut dyn nai_provider_core::UsageGateway,
    entry: UsageLedgerEntry,
) -> Vec<LlmChunk> {
    llm.generate(op, request, gateway, entry)
        .await
        .expect("generation through a bound provider")
}

/// The caller under test for speech: provider-agnostic by construction. It
/// synthesizes exactly one sentence and returns that sentence's own chunks.
async fn speak_once(
    speech: &SpeechClient<'_>,
    op: &Operation,
    request: &TtsRequest,
    gateway: &mut dyn nai_provider_core::UsageGateway,
    entry: UsageLedgerEntry,
) -> Vec<TtsChunk> {
    let mut chunks = Vec::new();
    speech
        .tts_stream(op, request, gateway, entry, &mut |chunk| chunks.push(chunk))
        .await
        .expect("synthesis through a bound provider");
    chunks
}

fn text_of(chunks: &[LlmChunk]) -> String {
    chunks
        .iter()
        .filter(|chunk| chunk.end == StreamEnd::StreamData as i32)
        .map(|chunk| chunk.text_delta.clone())
        .collect()
}

fn terminal(chunks: &[LlmChunk]) -> &LlmChunk {
    chunks
        .last()
        .filter(|chunk| chunk.end == StreamEnd::StreamComplete as i32)
        .expect("every provider stream ends in one terminal chunk")
}

fn llm_request(text: &str) -> LlmRequest {
    LlmRequest {
        permitted_context: text.into(),
        max_output_tokens: 32,
        context: Some(nai_contracts::v1::RequestContext {
            request_id: "req-llm".into(),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn tts_request(sentence: &str, voice: &str, sample_rate: u32) -> TtsRequest {
    TtsRequest {
        context: Some(nai_contracts::v1::RequestContext {
            request_id: format!("req-{sentence}"),
            ..Default::default()
        }),
        speech_text: sentence.into(),
        voice: Some(VoiceRef {
            voice_ref: voice.into(),
            ..Default::default()
        }),
        format: Some(AudioFormat {
            codec: "pcm".into(),
            sample_rate,
            channels: 1,
            sample_format: "s16".into(),
        }),
        ..Default::default()
    }
}

// --------------------------------------------------------- vendor A: OpenAI wire

fn openai_llm_mock() -> Mock {
    Mock::start(|_| {
        sse_response(&[
            r#"{"choices":[{"delta":{"content":"甲"}}]}"#,
            r#"{"choices":[{"delta":{"content":"乙"}}]}"#,
            r#"{"choices":[],"usage":{"prompt_tokens":5,"completion_tokens":2,"total_tokens":7}}"#,
            "[DONE]",
        ])
    })
}

fn openai_speech_mock() -> Mock {
    Mock::start(|_| binary_response(vec![1u8; 32], None))
}

// ------------------------------------------------------ vendor B: Anthropic wire

fn anthropic_mock() -> Mock {
    Mock::start(|_| {
        sse_response(&[
            r#"{"type":"message_start","message":{"usage":{"input_tokens":5,"output_tokens":0}}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"甲"}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"乙"}}"#,
            r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":2}}"#,
            r#"{"type":"message_stop"}"#,
        ])
    })
}

// ------------------------------------------------------ vendor B: ElevenLabs wire

fn elevenlabs_mock() -> Mock {
    Mock::start(|_| binary_response(vec![2u8; 48], Some(("X-Character-Count", "9".into()))))
}

/// The same endpoint, but without the character-cost header: the adapter must
/// fall back to the deterministic count of the text it sent.
fn elevenlabs_mock_without_cost() -> Mock {
    Mock::start(|_| binary_response(vec![2u8; 48], None))
}

// ----------------------------------------------------------------- the assertions

#[tokio::test]
async fn the_same_caller_generates_text_through_two_different_wire_protocols() {
    let openai = openai_llm_mock();
    let anthropic = anthropic_mock();

    let profiles = [
        profile(
            "vendor-a",
            openai.endpoint(),
            Protocol::OpenAiCompatible,
            "gpt-x",
        ),
        profile(
            "vendor-b",
            anthropic.endpoint(),
            Protocol::AnthropicMessages,
            "claude-x",
        ),
    ];

    let mut texts = Vec::new();
    for vendor in &profiles {
        let client = vendor
            .client(Secret::new(SECRET.as_bytes().to_vec()).unwrap())
            .unwrap();
        let op = Operation::new(Duration::from_secs(5)).unwrap();
        let mut gateway = gateway(&vendor.provider_ref);
        let llm = vendor.llm(&client).unwrap();
        let chunks = generate_once(
            &llm,
            &op,
            &llm_request("请介绍商品"),
            &mut gateway,
            usage_entry(&vendor.provider_ref, "req-llm", "total_tokens", 64),
        )
        .await;
        // Same caller-visible result shape from both vendors.
        assert_eq!(terminal(&chunks).seq, 2);
        texts.push(text_of(&chunks));
    }
    assert_eq!(texts[0], "甲乙");
    assert_eq!(
        texts[1], "甲乙",
        "the caller must not observe which vendor answered"
    );

    // The wires really were different, so the test is not two runs of one path.
    let a = &openai.requests()[0];
    let b = &anthropic.requests()[0];
    assert_eq!((a.method.as_str(), b.method.as_str()), ("POST", "POST"));
    assert_eq!(a.path, "/chat/completions");
    assert_eq!(b.path, "/messages");
    assert_eq!(
        a.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    assert!(a.header("x-api-key").is_none());
    assert_eq!(b.header("x-api-key"), Some(SECRET));
    assert!(
        b.header("authorization").is_none(),
        "the credential header is the protocol's, not a default"
    );
    assert_eq!(b.header("anthropic-version"), Some("2023-06-01"));
    // Body shape differs: the OpenAI-compatible body carries a stream_options
    // flag; the Messages body does not. The role does NOT differ — the permitted
    // context is one user turn under both protocols, because a chat request with
    // no user turn is not universally valid (a strict gateway answers 400
    // `No user query found in messages`).
    assert_eq!(a.json()["messages"][0]["role"], "user");
    assert_eq!(a.json()["stream_options"]["include_usage"], true);
    assert_eq!(b.json()["messages"][0]["role"], "user");
    assert!(b.json().get("stream_options").is_none());
    assert_eq!(b.json()["max_tokens"], 32);
    assert_eq!(a.json()["max_tokens"], 32);
}

#[tokio::test]
async fn vendor_specific_usage_fields_land_in_the_same_canonical_units() {
    let openai = openai_llm_mock();
    let anthropic = anthropic_mock();

    for (endpoint, protocol, provider_ref) in [
        (openai.endpoint(), Protocol::OpenAiCompatible, "vendor-a"),
        (
            anthropic.endpoint(),
            Protocol::AnthropicMessages,
            "vendor-b",
        ),
    ] {
        let vendor = profile(provider_ref, endpoint, protocol, "model-x");
        let client = vendor
            .client(Secret::new(SECRET.as_bytes().to_vec()).unwrap())
            .unwrap();
        let op = Operation::new(Duration::from_secs(5)).unwrap();
        let mut gateway = gateway(provider_ref);
        let llm = vendor.llm(&client).unwrap();
        generate_once(
            &llm,
            &op,
            &llm_request("请介绍商品"),
            &mut gateway,
            usage_entry(provider_ref, "req-llm", "total_tokens", 64),
        )
        .await;

        // Anthropic reports input_tokens/output_tokens; OpenAI reports
        // prompt_tokens/completion_tokens/total_tokens. Both must be recorded in
        // the closed canonical vocabulary, per provider, not by request id.
        let stored = gateway
            .book()
            .get(provider_ref, "req-llm")
            .expect("usage must be finished for every vendor");
        let units = stored
            .actual
            .as_ref()
            .expect("an actual charge must be recorded")
            .units
            .clone();
        assert_eq!(units.get("prompt_tokens"), Some(&5), "{provider_ref}");
        assert_eq!(units.get("completion_tokens"), Some(&2), "{provider_ref}");
        assert_eq!(units.get("total_tokens"), Some(&7), "{provider_ref}");
        assert_eq!(units.get("requests"), Some(&1), "{provider_ref}");
        assert_eq!(
            stored.provider_ref, provider_ref,
            "the charge must be keyed to the vendor that answered"
        );
    }
}

#[tokio::test]
async fn the_same_caller_speaks_through_two_different_speech_protocols() {
    let openai = openai_speech_mock();
    let eleven = elevenlabs_mock();

    let profiles = [
        profile(
            "vendor-a",
            openai.endpoint(),
            Protocol::OpenAiCompatible,
            "tts-x",
        ),
        profile(
            "vendor-b",
            eleven.endpoint(),
            Protocol::ElevenLabs,
            "eleven-x",
        ),
    ];

    let mut chunk_counts = Vec::new();
    for vendor in &profiles {
        let client = vendor
            .client(Secret::new(SECRET.as_bytes().to_vec()).unwrap())
            .unwrap();
        let op = Operation::new(Duration::from_secs(5)).unwrap();
        let mut gateway = gateway(&vendor.provider_ref);
        let speech = vendor.speech(&client).unwrap();
        let chunks = speak_once(
            &speech,
            &op,
            &tts_request("第一句", "voice-abc", 24_000),
            &mut gateway,
            usage_entry(&vendor.provider_ref, "req-tts", "characters", 64),
        )
        .await;
        // Identical caller-visible shape: ordered data chunks then one terminal.
        assert_eq!(chunks.last().unwrap().end, StreamEnd::StreamComplete as i32);
        assert!(
            chunks
                .iter()
                .all(|chunk| { chunk.format.as_ref().map(|f| f.sample_rate) == Some(24_000) })
        );
        chunk_counts.push(chunks.len());
        let stored = gateway.book().get(&vendor.provider_ref, "req-tts").unwrap();
        // The reservation happened for both vendors, so neither call can have
        // bypassed the budget.
        assert!(stored.estimate.is_some(), "{}", vendor.provider_ref);
        match vendor.protocol {
            // This vendor reports the character count it charged, so the actual
            // charge is built from the provider's number — in the same closed
            // canonical units the text path uses.
            Protocol::ElevenLabs => {
                let units = &stored
                    .actual
                    .as_ref()
                    .unwrap_or_else(|| panic!("{} must have settled a charge", vendor.provider_ref))
                    .units;
                assert_eq!(units.get("requests"), Some(&1));
                assert_eq!(
                    units.get("characters"),
                    Some(&9),
                    "the provider-reported character cost must be the recorded one"
                );
            }
            // This vendor returns no usage for speech at all. The attempt is
            // still reserved and the charge stays at the estimate; inventing an
            // actual here would be fabricating a billable number.
            _ => assert!(
                stored.actual.is_none(),
                "a vendor that reports no usage must not have one invented for it"
            ),
        }
    }
    assert!(chunk_counts.iter().all(|count| *count >= 2));

    let a = &openai.requests()[0];
    let b = &eleven.requests()[0];
    assert_eq!((a.method.as_str(), b.method.as_str()), ("POST", "POST"));
    assert_eq!(a.path, "/audio/speech");
    assert_eq!(
        b.path,
        "/v1/text-to-speech/voice-abc?output_format=pcm_24000"
    );
    assert_eq!(
        a.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    assert_eq!(b.header("xi-api-key"), Some(SECRET));
    assert!(b.header("authorization").is_none());
    // The voice lives in the body for one vendor and in the path for the other.
    assert_eq!(a.json()["voice"], "voice-abc");
    assert_eq!(a.json()["input"], "第一句");
    assert_eq!(b.json()["text"], "第一句");
    assert!(b.json().get("voice").is_none());
}

#[tokio::test]
async fn each_sentence_is_synthesized_into_its_own_sequence() {
    let eleven = elevenlabs_mock();
    let vendor = profile(
        "vendor-b",
        eleven.endpoint(),
        Protocol::ElevenLabs,
        "eleven-x",
    );
    let client = vendor
        .client(Secret::new(SECRET.as_bytes().to_vec()).unwrap())
        .unwrap();
    let speech = vendor.speech(&client).unwrap();
    let op = Operation::new(Duration::from_secs(5)).unwrap();

    let mut sequences = Vec::new();
    let mut books = Vec::new();
    for (index, sentence) in ["第一句", "第二句"].iter().enumerate() {
        let mut gateway = gateway("vendor-b");
        let chunks = speak_once(
            &speech,
            &op,
            &tts_request(sentence, "voice-abc", 24_000),
            &mut gateway,
            usage_entry("vendor-b", &format!("req-{index}"), "characters", 64),
        )
        .await;
        sequences.push(chunks);
        // Each sentence settles its own charge, keyed by its own request id, so
        // the ledger never merges two sentences into one billable call.
        assert!(
            gateway
                .book()
                .get("vendor-b", &format!("req-{index}"))
                .is_some(),
            "sentence {index} must settle its own usage entry"
        );
        books.push(format!("req-{index}"));
    }

    // The second sentence restarts at sequence zero instead of continuing the
    // first: nothing about the first call leaks into the second, so downstream
    // cannot concatenate them into one buffer.
    assert_eq!(sequences[0][0].seq, 0);
    assert_eq!(sequences[1][0].seq, 0);
    for chunks in &sequences {
        let data: Vec<&TtsChunk> = chunks
            .iter()
            .filter(|chunk| chunk.end == StreamEnd::StreamData as i32)
            .collect();
        assert!(!data.is_empty());
        for chunk in data {
            assert_eq!(
                chunk.sample_start, 0,
                "each sentence's samples are its own, not offset into a shared timeline"
            );
        }
    }
    assert_ne!(books[0], books[1]);
    assert_eq!(eleven.requests().len(), 2);
    assert_eq!(eleven.requests()[0].json()["text"], "第一句");
    assert_eq!(eleven.requests()[1].json()["text"], "第二句");
}

#[tokio::test]
async fn a_provider_that_reports_no_character_cost_still_bills_a_real_count() {
    let eleven = elevenlabs_mock_without_cost();
    let vendor = profile(
        "vendor-b",
        eleven.endpoint(),
        Protocol::ElevenLabs,
        "eleven-x",
    );
    let client = vendor
        .client(Secret::new(SECRET.as_bytes().to_vec()).unwrap())
        .unwrap();
    let speech = vendor.speech(&client).unwrap();
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let mut gateway = gateway("vendor-b");
    speak_once(
        &speech,
        &op,
        &tts_request("第一句", "voice-abc", 24_000),
        &mut gateway,
        usage_entry("vendor-b", "req-tts", "characters", 64),
    )
    .await;
    let stored = gateway.book().get("vendor-b", "req-tts").unwrap();
    // Three characters were sent, so three are billed: a deterministic count of
    // the request, never zero and never an invented number.
    assert_eq!(
        stored.actual.as_ref().unwrap().units.get("characters"),
        Some(&3)
    );
    assert_eq!(
        stored.actual.as_ref().unwrap().units.get("requests"),
        Some(&1)
    );
}

#[tokio::test]
async fn a_rate_the_provider_cannot_produce_is_refused_before_any_request() {
    let eleven = elevenlabs_mock();
    let vendor = profile(
        "vendor-b",
        eleven.endpoint(),
        Protocol::ElevenLabs,
        "eleven-x",
    );
    let client = vendor
        .client(Secret::new(SECRET.as_bytes().to_vec()).unwrap())
        .unwrap();
    let speech = vendor.speech(&client).unwrap();
    let op = Operation::new(Duration::from_secs(5)).unwrap();
    let mut gateway = gateway("vendor-b");
    // 48 kHz is a rate the caller may legitimately want and this provider does
    // not name: the binding must refuse rather than quietly serve another rate,
    // which is what keeps a caller from publishing audio at the wrong speed.
    let mut chunks = Vec::new();
    let error = speech
        .tts_stream(
            &op,
            &tts_request("第一句", "voice-abc", 48_000),
            &mut gateway,
            usage_entry("vendor-b", "req-tts", "characters", 64),
            &mut |chunk| chunks.push(chunk),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert!(
        chunks.is_empty(),
        "no audio may be produced at the wrong rate"
    );
    assert!(
        eleven.requests().is_empty(),
        "no request may have been sent"
    );
    // The refusal released the reservation rather than leaving it hanging. An
    // entry that is absent entirely also satisfies this: what must not survive
    // is a *reserved* one.
    if let Some(stored) = gateway.book().get("vendor-b", "req-tts") {
        assert!(
            !matches!(
                nai_contracts::v1::BillingState::try_from(stored.state),
                Ok(nai_contracts::v1::BillingState::Reserved)
            ),
            "a refused call must not leave its reservation reserved"
        );
    }
}

#[tokio::test]
async fn a_provider_is_refused_for_a_capability_it_does_not_have() {
    let eleven = elevenlabs_mock();
    let vendor = profile(
        "vendor-b",
        eleven.endpoint(),
        Protocol::ElevenLabs,
        "eleven-x",
    );
    let client = vendor
        .client(Secret::new(SECRET.as_bytes().to_vec()).unwrap())
        .unwrap();
    // Asking a speech-only endpoint for text is an error, not a fallback.
    let error = match vendor.llm(&client) {
        Ok(_) => panic!("a speech-only protocol must not be bound as an LLM provider"),
        Err(error) => error,
    };
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert!(
        eleven.requests().is_empty(),
        "no request may have been sent"
    );

    let anthropic = anthropic_mock();
    let vendor = profile(
        "vendor-c",
        anthropic.endpoint(),
        Protocol::AnthropicMessages,
        "claude-x",
    );
    let client = vendor
        .client(Secret::new(SECRET.as_bytes().to_vec()).unwrap())
        .unwrap();
    let error = match vendor.speech(&client) {
        Ok(_) => panic!("a text-only protocol must not be bound as a speech provider"),
        Err(error) => error,
    };
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert!(anthropic.requests().is_empty());
}

#[test]
fn the_transport_shape_is_a_function_of_the_protocol_alone() {
    // A configuration swap changes only these derived values; the caller's code
    // is the same object either way.
    let secret = || Secret::new(SECRET.as_bytes().to_vec()).unwrap();
    let openai = profile(
        "a",
        "http://127.0.0.1:1".into(),
        Protocol::OpenAiCompatible,
        "m",
    )
    .http_config(secret());
    let anthropic = profile(
        "b",
        "http://127.0.0.1:1".into(),
        Protocol::AnthropicMessages,
        "m",
    )
    .http_config(secret());
    assert_eq!(openai.auth, AuthScheme::Bearer);
    assert_eq!(
        anthropic.auth,
        AuthScheme::Header {
            name: "x-api-key".into()
        }
    );
    assert_eq!(Capability::Llm, Protocol::OpenAiCompatible.capability());
    assert_eq!(Capability::Speech, Protocol::ElevenLabs.capability());
}
