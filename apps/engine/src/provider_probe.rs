//! Explicit paid single-endpoint probe. No routing, rendering or playback.
use nai_contracts::v1::*;
use nai_provider_core::InMemoryGateway;
use nai_security::Operation;
use prost::Message;
use std::{
    fs,
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const REPORT: &str = "provider-probe.pb";
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn pcm_count(bytes: &[u8], format: &AudioFormat) -> Result<u64, ErrorCode> {
    if format.codec != "pcm"
        || format.sample_format != "s16"
        || format.channels != 1
        || !(8000..=48000).contains(&format.sample_rate)
        || bytes.is_empty()
        || !bytes.len().is_multiple_of(2)
    {
        return Err(ErrorCode::InvalidArgument);
    }
    let count = bytes.len() / 2;
    if count > nai_provider_core::MAX_SPEECH_SAMPLES {
        return Err(ErrorCode::ResourceExhausted);
    }
    Ok(count as u64)
}
async fn probe(scene: &Path, id: &str, report: &mut ProviderProbeReport) -> Result<(), ErrorCode> {
    let loaded = crate::operator::load(scene).map_err(|_| ErrorCode::InvalidArgument)?;
    let mut found = None;
    for cap in ["llm", "tts"] {
        if let Some(items) = loaded.config["provider_routes"][cap]["providers"].as_array() {
            for p in items {
                if p["id"].as_str() == Some(id) {
                    found = Some((cap, p));
                }
            }
        }
    }
    let (cap, p) = found.ok_or(ErrorCode::NotFound)?;
    report.capability = cap.into();
    report.model = p["model"].as_str().unwrap_or_default().into();
    if cap == "tts" {
        report.voice_ref = p["voice_ref"].as_str().unwrap_or_default().into();
    }
    let profile = crate::operator::profile_from(p).map_err(|_| ErrorCode::InvalidArgument)?;
    let client =
        crate::operator::client_from(p, &profile).map_err(|_| ErrorCode::CapabilityNotReady)?;
    let limits = crate::preflight::budget_limits(&loaded.config, &profile);
    let workspace = loaded.config["knowledge"]["scope"]
        .as_str()
        .unwrap_or("preflight");
    let mut gateway =
        InMemoryGateway::new(workspace, limits).map_err(|_| ErrorCode::InvalidArgument)?;
    let max_tokens = p["max_tokens"].as_u64().unwrap_or(128).min(128) as u32;
    let request_id = format!("probe-{}-{}", std::process::id(), now());
    let entry = UsageLedgerEntry {
        workspace_id: workspace.into(),
        session_id: "preflight".into(),
        provider_ref: id.into(),
        request_id: request_id.clone(),
        generation_epoch: 1,
        state: BillingState::Reserved as i32,
        estimate: Some(BudgetCharge {
            currency: profile.currency.clone(),
            price_revision: profile.price_revision.clone(),
            cost_minor: loaded.config["budget"]["request_estimate_minor"]
                .as_u64()
                .unwrap_or(1),
            units: [
                ("requests".into(), 1),
                (
                    if cap == "llm" { "tokens" } else { "characters" }.into(),
                    if cap == "llm" {
                        u64::from(max_tokens) + 64
                    } else {
                        4
                    },
                ),
            ]
            .into(),
        }),
        ..Default::default()
    };
    let duration = profile.timeout.min(Duration::from_secs(30));
    let op = Operation::new(duration).map_err(|e| e.code)?;
    let start = Instant::now();
    let context = RequestContext {
        workspace_id: workspace.into(),
        session_id: "preflight".into(),
        request_id: request_id.clone(),
        generation_epoch: Some(1),
        ..Default::default()
    };
    let result = tokio::time::timeout(duration, async {
        if cap == "llm" {
            let llm = profile.llm(&client).map_err(|e| e.code)?;
            let chunks = llm
                .generate(
                    &op,
                    &LlmRequest {
                        context: Some(context),
                        provider_profile: id.into(),
                        prompt_version: "provider-probe-v1".into(),
                        permitted_context: "连接测试，请只输出OK，不解释。".into(),
                        max_output_tokens: max_tokens,
                        ..Default::default()
                    },
                    &mut gateway,
                    entry,
                )
                .await
                .map_err(|e| e.code)?;
            if !chunks
                .iter()
                .any(|c| c.end == StreamEnd::StreamData as i32 && !c.text_delta.trim().is_empty())
            {
                return Err(ErrorCode::CapabilityNotReady);
            }
        } else {
            let speech = profile.speech(&client).map_err(|e| e.code)?;
            let (bytes, format) = speech
                .tts_pcm(
                    &op,
                    &TtsRequest {
                        context: Some(context),
                        utterance_id: request_id.clone(),
                        speech_text: "连接测试".into(),
                        voice: Some(VoiceRef {
                            provider_ref: id.into(),
                            voice_ref: report.voice_ref.clone(),
                            ..Default::default()
                        }),
                        format: Some(AudioFormat {
                            codec: "pcm".into(),
                            sample_rate: 24000,
                            channels: 1,
                            sample_format: "s16".into(),
                        }),
                        speed: 1.0,
                        ..Default::default()
                    },
                    &mut gateway,
                    entry,
                )
                .await
                .map_err(|e| e.code)?;
            report.sample_count = Some(pcm_count(&bytes, &format)?);
            report.sample_rate = Some(format.sample_rate);
        }
        Ok(())
    })
    .await
    .unwrap_or_else(|_| {
        op.cancel();
        Err(ErrorCode::DeadlineExceeded)
    });
    report.elapsed_ms = Some(start.elapsed().as_millis() as u64);
    report.usage = gateway.book().get(id, &request_id).cloned();
    result
}
pub async fn run(scene: &Path, id: &str, root: &Path) -> Result<(), String> {
    let mut report = ProviderProbeReport {
        version: 1,
        provider_id: id.into(),
        generated_at_utc_ms: now(),
        ..Default::default()
    };
    match probe(scene, id, &mut report).await {
        Ok(()) => report.status = "passed".into(),
        Err(code) => {
            report.status = if code == ErrorCode::CapabilityNotReady && report.elapsed_ms.is_none()
            {
                "not_ready"
            } else {
                "failed"
            }
            .into();
            report.error_code = code.as_str_name().into();
        }
    }
    let dir = root.join("operator-artifacts");
    fs::create_dir_all(&dir).map_err(|_| "PROVIDER_PROBE_REPORT_WRITE")?;
    let pending = dir.join("provider-probe.pending.pb");
    fs::write(&pending, report.encode_to_vec()).map_err(|_| "PROVIDER_PROBE_REPORT_WRITE")?;
    let target = dir.join(REPORT);
    if target.exists() {
        fs::remove_file(&target).map_err(|_| "PROVIDER_PROBE_REPORT_WRITE")?;
    }
    fs::rename(pending, target).map_err(|_| "PROVIDER_PROBE_REPORT_WRITE")?;
    println!(
        "provider probe {} {} (no playback; invoice unknown)",
        report.provider_id, report.status
    );
    if report.status == "passed" {
        Ok(())
    } else {
        Err(format!("PROVIDER_PROBE_REFUSED: {}", report.error_code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pcm_requires_actual_bounded_mono_samples() {
        let f = AudioFormat {
            codec: "pcm".into(),
            sample_rate: 24000,
            channels: 1,
            sample_format: "s16".into(),
        };
        assert_eq!(pcm_count(&[0, 0, 1, 0], &f), Ok(2));
        assert!(pcm_count(&[], &f).is_err());
        assert!(pcm_count(&[1], &f).is_err());
        assert!(pcm_count(&vec![0; 480002], &f).is_err());
        assert!(pcm_count(&[0, 0], &AudioFormat { channels: 2, ..f }).is_err());
    }
}
