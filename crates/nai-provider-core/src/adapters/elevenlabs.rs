//! ElevenLabs speech adapter: the **second** TTS wire protocol, deliberately
//! unlike the OpenAI-compatible one so the playback path never assumes a single
//! vendor's request shape.
//!
//! Differences this adapter carries, none of which the shared transport knows
//! about: the credential header (`xi-api-key`, not `Authorization`), the voice
//! identity in the **path** rather than the body, a body with `text`/`model_id`
//! rather than `input`/`voice`/`response_format`, and a raw binary PCM response
//! whose sample rate is selected by a query parameter naming the format.
//!
//! The same seam contract holds: bounded byte blocks, one buffer reference per
//! block, and a terminal `StreamComplete` chunk. Nothing here carries a sample
//! counter across calls, so a caller that synthesizes one sentence per call
//! cannot accidentally stitch two sentences into one continuous buffer.
use crate::{
    client::{ProviderClient, ProviderError},
    gateway::{CallBudget, UsageGateway},
    usage::usage_charge,
};
use futures_util::StreamExt;
use nai_contracts::v1::{AudioFormat, ErrorCode, StreamEnd, TtsChunk, TtsRequest};
use nai_security::Operation;
use serde_json::json;

const MAX_STREAM_BYTES: u64 = 64 * 1024 * 1024;

/// Output rates this binding can ask for, in Hz. ElevenLabs names the PCM rate
/// in the format string, so a caller wanting a rate outside this set must be
/// refused rather than served a different rate: `SpeechResponse::validate`
/// forbids a synthesizer from answering at a rate it was not asked for, and
/// silently resampling here would break that contract.
const SUPPORTED_RATES: [u32; 5] = [8_000, 16_000, 22_050, 24_000, 44_100];

fn invalid(message: &str) -> ProviderError {
    ProviderError::new(ErrorCode::InvalidArgument, false, message)
}

pub struct ElevenLabsAdapter<'a> {
    client: &'a ProviderClient,
}

impl<'a> ElevenLabsAdapter<'a> {
    pub fn new(client: &'a ProviderClient) -> Self {
        Self { client }
    }

    fn model(&self) -> &str {
        &self.client.config().model
    }

    /// The `pcm_{rate}` output format this binding can request for `rate`, or
    /// `None` when the provider cannot produce that rate.
    fn output_format(rate: u32) -> Option<String> {
        SUPPORTED_RATES
            .contains(&rate)
            .then(|| format!("pcm_{rate}"))
    }

    /// Binary speech stream. Blocks are surfaced one buffer reference each, and
    /// the terminal chunk closes the sequence; the returned value is the total
    /// byte count, not a sample count (media decoders own sample accounting).
    pub async fn tts_stream(
        &self,
        op: &Operation,
        req: &TtsRequest,
        gateway: &mut dyn UsageGateway,
        entry: nai_contracts::v1::UsageLedgerEntry,
        sink: &mut dyn FnMut(TtsChunk),
    ) -> Result<u64, ProviderError> {
        let budget = CallBudget::start(gateway, entry)?;
        if !budget.dispatched() {
            return Err(ProviderError::new(
                ErrorCode::Conflict,
                false,
                "tts request already dispatched",
            ));
        }
        // The voice identity is a path segment here, so it must be present and
        // safe before any URL is built.
        let voice = req
            .voice
            .as_ref()
            .map(|voice| voice.voice_ref.clone())
            .unwrap_or_default();
        if req.speech_text.is_empty() || !valid_path_segment(&voice) {
            budget.finish(gateway, None).ok();
            return Err(invalid("tts request missing text or voice"));
        }
        let sample_rate = req.format.as_ref().map(|f| f.sample_rate).unwrap_or(24_000);
        let channels = req.format.as_ref().map(|f| f.channels).unwrap_or(1);
        let Some(output_format) = Self::output_format(sample_rate) else {
            budget.finish(gateway, None).ok();
            return Err(invalid(
                "this speech provider cannot produce the requested sample rate",
            ));
        };
        if channels != 1 {
            budget.finish(gateway, None).ok();
            return Err(invalid("this speech provider produces mono PCM only"));
        }
        let body = json!({
            "text": req.speech_text,
            "model_id": self.model(),
        });
        let path = format!("/v1/text-to-speech/{voice}?output_format={output_format}");
        let stream = match self.client.stream(op, &path, &body).await {
            Ok(stream) => stream,
            Err(error) => {
                budget.finish(gateway, None).ok();
                return Err(error);
            }
        };
        // The provider bills characters and reports the count it charged on the
        // response, so reconciliation uses that number when it is usable and the
        // deterministic count of the sent text otherwise.
        let reported = character_cost(&stream.body);
        let audio_format = AudioFormat {
            codec: "pcm".into(),
            sample_rate,
            channels: 1,
            sample_format: "s16".into(),
        };
        let mut seq = 0u32;
        let mut total = 0u64;
        let mut stream = stream.body.bytes_stream();
        loop {
            op.check().map_err(|error| match error.code {
                ErrorCode::Cancelled => ProviderError::cancelled(),
                _ => ProviderError::deadline(),
            })?;
            let block = match stream.next().await {
                Some(block) => block.map_err(|error| {
                    ProviderError::unavailable(format!("provider audio stream: {error}"))
                })?,
                None => {
                    sink(TtsChunk {
                        seq,
                        end: StreamEnd::StreamComplete as i32,
                        format: Some(audio_format.clone()),
                        ..Default::default()
                    });
                    break;
                }
            };
            total += block.len() as u64;
            if total > MAX_STREAM_BYTES {
                budget.finish(gateway, None).ok();
                return Err(invalid("tts stream exceeds bound"));
            }
            if !block.is_empty() {
                sink(TtsChunk {
                    seq,
                    end: StreamEnd::StreamData as i32,
                    audio_buffer_ref: format!("provider://audio/{seq}"),
                    format: Some(audio_format.clone()),
                    sample_start: 0,
                    sample_count: 0,
                    ..Default::default()
                });
                seq += 1;
            }
        }
        let charge = reported.unwrap_or_else(|| req.speech_text.chars().count() as u64);
        let usage = json!({ "characters": charge.max(1) });
        let actual = usage_charge(
            &self.client.config().currency,
            &self.client.config().price_revision,
            &usage,
            1,
        )
        .ok();
        budget.finish(gateway, actual)?;
        Ok(total)
    }
}

/// A voice identity becomes one path segment, so it may not add a segment, a
/// query, a fragment or an escape. An unrecognised identity is refused rather
/// than percent-encoded: this binding has no allow-list to check it against, and
/// guessing would send a request the provider cannot honour.
fn valid_path_segment(voice: &str) -> bool {
    !voice.is_empty()
        && voice.len() <= 128
        && voice
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Reads the provider's reported character cost, if the response carried one.
fn character_cost(response: &reqwest::Response) -> Option<u64> {
    response
        .headers()
        .get("x-character-count")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|count| *count > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_rates_the_provider_names_are_requestable() {
        assert_eq!(
            ElevenLabsAdapter::output_format(24_000).as_deref(),
            Some("pcm_24000")
        );
        // A rate outside the provider's set must not become an approximate one.
        assert!(ElevenLabsAdapter::output_format(8_001).is_none());
        assert!(ElevenLabsAdapter::output_format(48_000).is_none());
    }

    #[test]
    fn a_voice_identity_may_not_reshapes_the_request_path() {
        assert!(valid_path_segment("voice-abc_123"));
        assert!(!valid_path_segment(""));
        assert!(!valid_path_segment("../other"));
        assert!(!valid_path_segment("voice?x=1"));
        assert!(!valid_path_segment("voice/extra"));
        assert!(!valid_path_segment(&"v".repeat(129)));
    }
}
