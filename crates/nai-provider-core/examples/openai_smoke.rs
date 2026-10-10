//! Minimal real integration smoke test against api.openai.com. Runs only when
//! OPENAI_API_KEY is present; never prints or stores the key. CI does not run
//! this example. Consumes a tiny amount of paid quota for one TTS + one
//! embeddings request.
use nai_budget_core::{Ceiling, Limits};
use nai_contracts::v1::{AudioFormat, EmbeddingRequest, TtsRequest, UsageLedgerEntry, VoiceRef};
use nai_provider_core::{AuthScheme, HttpConfig, InMemoryGateway, OpenAiAdapter, ProviderClient};
use nai_security::{Operation, Secret};
use std::time::Duration;

fn entry(session: &str, request: &str) -> UsageLedgerEntry {
    UsageLedgerEntry {
        workspace_id: "smoke-workspace".into(),
        session_id: session.into(),
        provider_ref: "openai-smoke".into(),
        request_id: request.into(),
        generation_epoch: 1,
        estimate: Some(nai_contracts::v1::BudgetCharge {
            currency: "USD".into(),
            cost_minor: 0,
            units: [("requests".into(), 1u64), ("tokens".into(), 128u64)].into(),
            price_revision: "smoke-price-1".into(),
        }),
        state: nai_contracts::v1::BillingState::Reserved as i32,
        ..Default::default()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = std::env::var("OPENAI_API_KEY").map_err(|_| "OPENAI_API_KEY not set")?;
    if key.is_empty() {
        return Err("OPENAI_API_KEY is empty".into());
    }
    let client = ProviderClient::new(HttpConfig {
        provider_ref: "openai-smoke".into(),
        endpoint: "https://api.openai.com/v1".into(),
        api_key: Some(Secret::new(key.into_bytes()).unwrap()),
        auth: AuthScheme::Bearer,
        extra_headers: Vec::new(),
        model: "text-embedding-3-small".into(),
        price_revision: "smoke-price-1".into(),
        currency: "USD".into(),
        timeout: Duration::from_secs(30),
        max_retries: 1,
    })?;
    let adapter = OpenAiAdapter::new(&client);
    let ceiling = Ceiling {
        cost_minor: u64::MAX,
        units: [
            ("requests".into(), 10u64),
            ("tokens".into(), 100_000u64),
            ("prompt_tokens".into(), 100_000u64),
            ("completion_tokens".into(), 100_000u64),
            ("total_tokens".into(), 100_000u64),
        ]
        .into(),
        concurrent: [("provider_slots".into(), 2u64)].into(),
    };
    let mut gateway = InMemoryGateway::new(
        "smoke-workspace",
        Limits {
            currency: "USD".into(),
            global: ceiling.clone(),
            sessions: [("smoke-session".into(), ceiling.clone())].into(),
            providers: [("openai-smoke".into(), ceiling)].into(),
            max_entries: 100,
        },
    )
    .map_err(|error| format!("budget: {error:?}"))?;

    let mut any_failed = false;

    let op = Operation::new(Duration::from_secs(30)).unwrap();
    let embedding = EmbeddingRequest {
        permitted_texts: vec!["测试价格".into()],
        ..Default::default()
    };
    match adapter
        .embeddings(
            &op,
            &embedding,
            &mut gateway,
            entry("smoke-session", "smoke-emb"),
        )
        .await
    {
        Ok(result) => {
            let tokens = result
                .usage
                .first()
                .map(|u| u.quantity_decimal.clone())
                .unwrap_or_else(|| "0".into());
            println!(
                "embeddings ok: model_revision={} dimension={} vectors={} tokens={}",
                result.provider_model_revision,
                result.dimension,
                result.vectors.len(),
                tokens
            );
        }
        Err(error) => {
            println!("embeddings failed: {error}");
            any_failed = true;
        }
    }

    let op = Operation::new(Duration::from_secs(30)).unwrap();
    let tts = TtsRequest {
        speech_text: "你好".into(),
        voice: Some(VoiceRef {
            provider_ref: "openai-smoke".into(),
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
    match adapter
        .tts_stream(
            &op,
            &tts,
            &mut gateway,
            entry("smoke-session", "smoke-tts"),
            &mut |_| {},
        )
        .await
    {
        Ok(bytes) => {
            let state = gateway
                .book()
                .get("openai-smoke", "smoke-tts")
                .map(|e| e.state)
                .unwrap_or(0);
            println!("tts ok: bytes={bytes} ledger_state={state}");
        }
        Err(error) => {
            println!("tts failed: {error}");
            any_failed = true;
        }
    }

    // LLM path is intentionally not smoke-tested (token cost); it shares the
    // same transport/retry/cancellation path covered by unit tests.
    if any_failed {
        println!("real integration smoke: partial failure");
        std::process::exit(1);
    }
    println!("real integration smoke passed");
    Ok(())
}
