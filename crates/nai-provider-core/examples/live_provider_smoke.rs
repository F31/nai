//! Real-endpoint smoke through **this crate's own transport, binding and
//! adapters**.
//!
//! The sibling `openai_smoke` example hardcodes `api.openai.com` and
//! deliberately skips the generation path. This one is endpoint-agnostic and
//! covers both capabilities, so it answers the question the other one leaves
//! open: does a *configured* OpenAI-compatible gateway work when driven through
//! `ProviderProfile` -> `ProviderClient` -> adapter, with the budget gateway,
//! deadline and usage mapping in the path?
//!
//! Credentials come from the environment and are never printed or stored. Any
//! missing variable stops the run with `CAPABILITY_NOT_READY`; a rehearsal stub
//! is not substituted, because a stub that makes a live smoke green is exactly
//! what §30.1 forbids.
//!
//!     NAI_LLM_BASE_URL=... NAI_LLM_API_KEY=... NAI_LLM_MODEL=... \
//!     NAI_TTS_BASE_URL=... NAI_TTS_API_KEY=... NAI_TTS_MODEL=... NAI_TTS_VOICE=... \
//!     cargo run -p nai-provider-core --example live_provider_smoke
//!
//! Exit codes: 1 a variable is missing (no call is made); 2 a call ran but did
//! not succeed.
use nai_budget_core::{Ceiling, Limits};
use nai_contracts::v1::{
    AudioFormat, BillingState, BudgetCharge, LlmChunk, LlmRequest, RequestContext, StreamEnd,
    TtsChunk, TtsRequest, UsageLedgerEntry, VoiceRef,
};
use nai_provider_core::{InMemoryGateway, Protocol, ProviderProfile};
use nai_security::{Operation, Secret};
use serde_json::json;
use std::path::Path;
use std::time::Duration;

const REQUIRED: [&str; 7] = [
    "NAI_LLM_BASE_URL",
    "NAI_LLM_API_KEY",
    "NAI_LLM_MODEL",
    "NAI_TTS_BASE_URL",
    "NAI_TTS_API_KEY",
    "NAI_TTS_MODEL",
    "NAI_TTS_VOICE",
];

/// An unset and a blank variable are equally absent: a half-filled `.env` must
/// not produce a run that "half works".
fn read(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn credential(name: &str) -> Secret {
    Secret::new(read(name).expect("checked before any call").into_bytes())
        .expect("a non-empty credential")
}

fn entry(provider_ref: &str, request_id: &str, unit: &str, quantity: u64) -> UsageLedgerEntry {
    UsageLedgerEntry {
        workspace_id: "live-smoke-workspace".into(),
        session_id: "live-smoke-session".into(),
        provider_ref: provider_ref.into(),
        request_id: request_id.into(),
        generation_epoch: 1,
        estimate: Some(BudgetCharge {
            currency: "CNY".into(),
            cost_minor: 0,
            units: [("requests".into(), 1u64), (unit.into(), quantity)].into(),
            price_revision: "live-smoke-price".into(),
        }),
        state: BillingState::Reserved as i32,
        ..Default::default()
    }
}

fn ceiling() -> Ceiling {
    Ceiling {
        cost_minor: 10_000,
        units: [
            ("requests".into(), 10u64),
            ("tokens".into(), 1_000_000u64),
            ("prompt_tokens".into(), 1_000_000u64),
            ("completion_tokens".into(), 1_000_000u64),
            ("total_tokens".into(), 1_000_000u64),
            ("characters".into(), 1_000_000u64),
            ("audio_seconds".into(), 1_000_000u64),
        ]
        .into(),
        concurrent: [("provider_slots".into(), 2u64)].into(),
    }
}

/// One profile per endpoint. Both are `openai` because a compatible gateway is
/// configuration, not a protocol: no enum arm and no adapter is added here.
fn profile(provider_ref: &str, base: &str, model: &str) -> ProviderProfile {
    ProviderProfile {
        provider_ref: provider_ref.into(),
        endpoint: base.into(),
        model: model.into(),
        price_revision: "live-smoke-price".into(),
        currency: "CNY".into(),
        timeout: OPERATION_CEILING,
        max_retries: 1,
        protocol: Protocol::OpenAiCompatible,
    }
}

/// The security layer refuses any operation longer than thirty seconds, so a
/// smoke cannot simply allow itself a longer deadline. The request timeout and
/// the operation deadline are kept equal here rather than having the transport
/// silently cut the call short.
const OPERATION_CEILING: Duration = Duration::from_secs(30);

fn text_of(chunks: &[LlmChunk]) -> String {
    chunks
        .iter()
        .filter(|chunk| chunk.end == StreamEnd::StreamData as i32)
        .map(|chunk| chunk.text_delta.clone())
        .collect()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let missing: Vec<&str> = REQUIRED
        .iter()
        .copied()
        .filter(|name| read(name).is_none())
        .collect();
    if !missing.is_empty() {
        // Variable names only; the values are never touched.
        println!("CAPABILITY_NOT_READY: missing {}", missing.join(", "));
        println!("no call was made; a rehearsal stub cannot stand in for a live endpoint");
        std::process::exit(1);
    }

    let text_profile = profile(
        "live-text",
        &read("NAI_LLM_BASE_URL").expect("checked"),
        &read("NAI_LLM_MODEL").expect("checked"),
    );
    let speech_profile = profile(
        "live-speech",
        &read("NAI_TTS_BASE_URL").expect("checked"),
        &read("NAI_TTS_MODEL").expect("checked"),
    );

    let mut gateway = InMemoryGateway::new(
        "live-smoke-workspace",
        Limits {
            currency: "CNY".into(),
            global: ceiling(),
            sessions: [("live-smoke-session".into(), ceiling())].into(),
            providers: [
                ("live-text".into(), ceiling()),
                ("live-speech".into(), ceiling()),
            ]
            .into(),
            max_entries: 100,
        },
    )
    .map_err(|error| format!("budget: {error:?}"))?;

    // Facts only: the credential is never an input to any record written below.
    let mut facts: Vec<serde_json::Value> = Vec::new();
    let mut failed = false;

    // ---- text: configured gateway -> ProviderClient -> OpenAI-compatible adapter
    let text_client = text_profile.client(credential("NAI_LLM_API_KEY"))?;
    let llm = text_profile.llm(&text_client)?;
    let op =
        Operation::new(OPERATION_CEILING).expect("the smoke stays inside the security ceiling");
    let request = LlmRequest {
        permitted_context: "只回答两个字：收到".into(),
        max_output_tokens: 32,
        context: Some(RequestContext {
            request_id: "live-llm".into(),
            ..Default::default()
        }),
        ..Default::default()
    };
    match llm
        .generate(
            &op,
            &request,
            &mut gateway,
            entry("live-text", "live-llm", "tokens", 1_000),
        )
        .await
    {
        Ok(chunks) => {
            let terminal = chunks
                .last()
                .map(|chunk| chunk.end == StreamEnd::StreamComplete as i32)
                .unwrap_or(false);
            let endpoint = text_profile.endpoint.as_str();
            let protocol = text_profile.protocol.as_config();
            let model = text_profile.model.as_str();
            let count = chunks.len();
            let chars = text_of(&chunks).chars().count();
            println!(
                "text ok: endpoint={endpoint} protocol={protocol} model={model} chunks={count} chars={chars} terminal={terminal}"
            );
            facts.push(json!({
                "capability": "text",
                "endpoint": endpoint,
                "protocol": protocol,
                "model": model,
                "chunks": count,
                "chars": chars,
                "terminal_chunk": terminal,
            }));
        }
        Err(error) => {
            println!("text failed: {error}");
            failed = true;
        }
    }

    // ---- speech: same binding seam, one sentence, its own chunk sequence
    let speech_client = speech_profile.client(credential("NAI_TTS_API_KEY"))?;
    let speech = speech_profile.speech(&speech_client)?;
    let op =
        Operation::new(OPERATION_CEILING).expect("the smoke stays inside the security ceiling");
    let request = TtsRequest {
        context: Some(RequestContext {
            request_id: "live-tts".into(),
            ..Default::default()
        }),
        speech_text: "这是一次语音链路连通测试。".into(),
        voice: Some(VoiceRef {
            provider_ref: "live-speech".into(),
            // Sent verbatim as `voice`: the endpoint's accepted form is a
            // measured contract, so it is configured rather than assembled here.
            voice_ref: read("NAI_TTS_VOICE").expect("checked"),
            ..Default::default()
        }),
        format: Some(AudioFormat {
            codec: "mp3".into(),
            sample_rate: 24_000,
            channels: 1,
            sample_format: "s16".into(),
        }),
        speed: 1.0,
        ..Default::default()
    };
    let mut chunks: Vec<TtsChunk> = Vec::new();
    match speech
        .tts_stream(
            &op,
            &request,
            &mut gateway,
            entry("live-speech", "live-tts", "characters", 100),
            &mut |chunk| chunks.push(chunk),
        )
        .await
    {
        Ok(bytes) => {
            let endpoint = speech_profile.endpoint.as_str();
            let protocol = speech_profile.protocol.as_config();
            let model = speech_profile.model.as_str();
            let count = chunks.len();
            println!(
                "speech ok: endpoint={endpoint} protocol={protocol} model={model} bytes={bytes} chunks={count}"
            );
            facts.push(json!({
                "capability": "speech",
                "endpoint": endpoint,
                "protocol": protocol,
                "model": model,
                "audio_bytes": bytes,
                "chunks": count,
            }));
        }
        Err(error) => {
            println!("speech failed: {error}");
            failed = true;
        }
    }

    let evidence = json!({
        "run": "live-provider-smoke-bound-stack",
        "generated_by": "crates/nai-provider-core/examples/live_provider_smoke.rs",
        "boundary": [
            "driven through ProviderProfile -> ProviderClient -> adapter -> budget gateway",
            "the endpoint is configuration: Protocol::OpenAiCompatible is unchanged",
            "credentials come from the environment and are absent from this record",
            "connectivity facts only; a successful run does not accept B01 or O01",
        ],
        "links": facts,
    });
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/reports/evidence");
    std::fs::create_dir_all(&directory)?;
    std::fs::write(
        directory.join("live-provider-bound-stack.json"),
        format!("{}\n", serde_json::to_string_pretty(&evidence)?),
    )?;
    println!("evidence: docs/reports/evidence/live-provider-bound-stack.json");

    if failed {
        println!("live provider smoke: at least one link produced no result");
        std::process::exit(2);
    }
    println!("live provider smoke passed: both links answered through the bound adapter");
    Ok(())
}
