//! Speech-synthesis seam (Slice-0, text -> playback segment).
//!
//! One synthesis request in, one bounded PCM buffer plus usage out. The seam is
//! transport-agnostic: a real TTS adapter and the rehearsal stub implement the
//! same trait. Every call made through [`synthesize_with_budget`] is wrapped by
//! [`CallBudget`], so a synthesis can never bypass reservation, cancellation or
//! usage accounting.
//!
//! The rehearsal stub's sample count is a pure function of the text and the
//! target sample rate ([`RehearsalSpeechSynthesizer::sample_count`]). That is
//! what makes an actual-playback acknowledgement checkable: the expected sample
//! total is known before any audio exists, so a "complete" ACK can be compared
//! against a value the caller computed independently rather than against
//! whatever the run happened to produce.
use crate::client::ProviderError;
use crate::gateway::{CallBudget, UsageGateway};
use crate::usage::usage_charge;
use nai_contracts::v1::{ErrorCode, UsageLedgerEntry};
use serde_json::{Value, json};

/// Accepted synthesis text bound, in characters.
pub const MAX_SPEECH_CHARS: usize = 4096;
/// Accepted target sample rate range, in Hz.
pub const MIN_SAMPLE_RATE: u32 = 8_000;
pub const MAX_SAMPLE_RATE: u32 = 48_000;
/// Accepted PCM length bound, in samples (5s at 48kHz). Longer audio must be
/// streamed as bounded blocks, not returned as one buffer.
pub const MAX_SPEECH_SAMPLES: usize = 240_000;

/// One synthesis request. Identity is carried explicitly so the seam never
/// infers a session, utterance or epoch from ambient state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeechRequest {
    pub request_id: String,
    pub provider_ref: String,
    pub session_id: String,
    pub utterance_id: String,
    pub generation_epoch: u64,
    pub text: String,
    /// The output rate the caller wants samples at. A synthesizer that cannot
    /// produce this rate must fail rather than resample silently.
    pub sample_rate: u32,
    /// Voice identity from the trusted binding. Never taken from a cue.
    pub voice_ref: String,
    /// Remaining task budget in milliseconds. Zero is an expired task, never a
    /// pass.
    pub remaining_budget_ms: u64,
    /// Explicit cancellation. A cancelled request never reaches a synthesizer.
    pub cancelled: bool,
}

impl SpeechRequest {
    /// Rejects an incomplete, oversized, unsupported-rate, expired or cancelled
    /// request. An expired task and a cancellation are hard stops, not silent
    /// defaults.
    pub fn validate(&self) -> Result<(), ProviderError> {
        if self.request_id.is_empty()
            || self.provider_ref.is_empty()
            || self.session_id.is_empty()
            || self.utterance_id.is_empty()
            || self.generation_epoch == 0
        {
            return Err(ProviderError::invalid_argument(
                "synthesis request identity incomplete",
            ));
        }
        if self.text.trim().is_empty() || self.text.len() > MAX_SPEECH_CHARS {
            return Err(ProviderError::invalid_argument(
                "synthesis text empty or oversized",
            ));
        }
        if self.voice_ref.is_empty() {
            return Err(ProviderError::invalid_argument(
                "synthesis request has no voice binding",
            ));
        }
        if !(MIN_SAMPLE_RATE..=MAX_SAMPLE_RATE).contains(&self.sample_rate) {
            return Err(ProviderError::invalid_argument(
                "synthesis target sample rate unsupported",
            ));
        }
        if self.cancelled {
            return Err(ProviderError::cancelled());
        }
        if self.remaining_budget_ms == 0 {
            return Err(ProviderError::deadline());
        }
        Ok(())
    }
}

/// Synthesized audio and the usage the synthesizer reported for it.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeechResponse {
    /// Mono PCM. The caller owns the blocks it cuts out of this buffer.
    pub pcm: Vec<f32>,
    pub sample_rate: u32,
    /// The identity the synthesizer reports for itself.
    pub model_ref: String,
    /// Provider usage object, mapped by [`usage_charge`] before it is recorded.
    pub usage: Value,
}

impl SpeechResponse {
    /// Rejects a response that does not answer the request: a different rate, an
    /// empty, oversized, non-finite or out-of-range buffer. Silence is a valid
    /// response; corrupt audio is not.
    pub fn validate(&self, request: &SpeechRequest) -> Result<(), ProviderError> {
        if self.sample_rate != request.sample_rate {
            return Err(ProviderError::invalid_argument(
                "synthesizer returned a different sample rate",
            ));
        }
        if self.pcm.is_empty() || self.pcm.len() > MAX_SPEECH_SAMPLES {
            return Err(ProviderError::invalid_argument(
                "synthesized PCM empty or oversized",
            ));
        }
        if self
            .pcm
            .iter()
            .any(|sample| !sample.is_finite() || sample.abs() > 1.0)
        {
            return Err(ProviderError::invalid_argument(
                "synthesized PCM is non-finite or out of range",
            ));
        }
        Ok(())
    }
}

/// A speech synthesizer. Implementations MUST NOT perform their own accounting;
/// callers MUST go through [`synthesize_with_budget`], which applies
/// reservation, cancellation and usage accounting around the call.
pub trait SpeechSynthesizer: Send {
    fn synthesize(&mut self, request: &SpeechRequest) -> Result<SpeechResponse, ProviderError>;
}

/// Reserve -> dispatch -> synthesize -> finish around one synthesis.
///
/// The synthesizer is only invoked once the reservation obtained the exclusive
/// dispatch claim; a retry that returns `Already` is refused rather than
/// re-sent, and the final usage is charged against the stored revision.
pub fn synthesize_with_budget(
    synthesizer: &mut dyn SpeechSynthesizer,
    gateway: &mut dyn UsageGateway,
    entry: UsageLedgerEntry,
    request: &SpeechRequest,
    price_revision: &str,
) -> Result<SpeechResponse, ProviderError> {
    request.validate()?;
    if price_revision.is_empty() {
        return Err(ProviderError::invalid_argument(
            "synthesis requires an exact price revision",
        ));
    }
    let currency = entry
        .estimate
        .as_ref()
        .map(|estimate| estimate.currency.clone())
        .ok_or_else(|| {
            ProviderError::invalid_argument("usage entry has no estimate to reserve against")
        })?;
    let budget = CallBudget::start(gateway, entry)?;
    if !budget.dispatched() {
        // Already dispatched elsewhere (e.g. an ACK retry): never re-send and
        // never double-account.
        return Err(ProviderError::new(
            ErrorCode::Conflict,
            false,
            "synthesis already dispatched",
        ));
    }
    let response = match synthesizer.synthesize(request) {
        Ok(response) => response,
        Err(error) => {
            // Release the claim; the attempt stays counted for reconciliation.
            let _ = budget.finish(gateway, None);
            return Err(error);
        }
    };
    response.validate(request)?;
    let actual = usage_charge(&currency, price_revision, &response.usage, 1)
        .map_err(ProviderError::invalid_argument)?;
    budget.finish(gateway, Some(actual))?;
    Ok(response)
}

/// Deterministic, offline, rehearsal-only speech synthesizer.
///
/// It performs no network I/O and holds no credential. It MUST only be
/// constructed on rehearsal or test paths, MUST NOT be used for real
/// integration, paid calls or any certification, and never substitutes for TTS
/// fault handling. Its output is reproducible: the same request always yields
/// the same number of samples with the same values, and the sample count
/// depends only on the text and the target sample rate.
pub struct RehearsalSpeechSynthesizer {
    model_ref: String,
}

impl RehearsalSpeechSynthesizer {
    /// The rehearsal identity reported for every synthesized response.
    pub const MODEL_REF: &'static str = "rehearsal-speech-deterministic-v1";
    /// Synthesized duration per text character, in milliseconds.
    pub const MS_PER_CHAR: u64 = 60;
    /// Bounds on the synthesized duration, in milliseconds.
    pub const MIN_SPEECH_MS: u64 = 400;
    pub const MAX_SPEECH_MS: u64 = 5_000;
    /// Peak amplitude of the rehearsal waveform. It is a fixed, loud,
    /// deterministic pulse train: its only job is to drive a checkable RMS
    /// above the renderer's mouth threshold, not to sound like speech.
    pub const AMPLITUDE: f32 = 0.8;

    pub fn new() -> Self {
        Self {
            model_ref: Self::MODEL_REF.to_string(),
        }
    }

    /// The exact sample count [`Self::synthesize`] will produce for this text and
    /// rate. Exposed so a caller can compute the expected playback total — and
    /// therefore check an "actually played" acknowledgement — independently of
    /// running the synthesizer.
    pub fn sample_count(text: &str, sample_rate: u32) -> usize {
        let characters = text.chars().count().max(1) as u64;
        let milliseconds =
            (characters * Self::MS_PER_CHAR).clamp(Self::MIN_SPEECH_MS, Self::MAX_SPEECH_MS);
        ((milliseconds * u64::from(sample_rate)) / 1_000) as usize
    }
}

impl Default for RehearsalSpeechSynthesizer {
    fn default() -> Self {
        Self::new()
    }
}

impl SpeechSynthesizer for RehearsalSpeechSynthesizer {
    fn synthesize(&mut self, request: &SpeechRequest) -> Result<SpeechResponse, ProviderError> {
        request.validate()?;
        let samples = Self::sample_count(&request.text, request.sample_rate);
        if samples == 0 || samples > MAX_SPEECH_SAMPLES {
            return Err(ProviderError::invalid_argument(
                "rehearsal synthesis length out of range",
            ));
        }
        // A deterministic integer waveform: the sign stream comes from the
        // text digest, so different text produces different audio while the
        // same text reproduces exactly. No floating-point state is involved.
        let mut state = fnv1a_64(request.text.as_bytes()) | 1;
        let mut pcm = Vec::new();
        pcm.try_reserve_exact(samples).map_err(|_| {
            ProviderError::new(
                ErrorCode::ResourceExhausted,
                false,
                "rehearsal PCM allocation failed",
            )
        })?;
        for _ in 0..samples {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            pcm.push(if state & 1 == 1 {
                Self::AMPLITUDE
            } else {
                -Self::AMPLITUDE
            });
        }
        let characters = request.text.chars().count().max(1) as u64;
        let audio_seconds = (samples as u64).div_ceil(u64::from(request.sample_rate));
        Ok(SpeechResponse {
            pcm,
            sample_rate: request.sample_rate,
            model_ref: self.model_ref.clone(),
            usage: json!({
                "characters": characters,
                "audio_seconds": audio_seconds,
            }),
        })
    }
}

/// Stable 64-bit FNV-1a digest. Used only to make the rehearsal waveform
/// reproducible; it is not a cryptographic hash.
fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::InMemoryGateway;
    use nai_budget_core::{Ceiling, Limits};
    use nai_contracts::v1::{BillingState, BudgetCharge};
    use std::collections::BTreeMap;

    fn ceiling() -> Ceiling {
        Ceiling {
            cost_minor: u64::MAX,
            units: [
                ("requests".to_string(), 8u64),
                ("characters".to_string(), 1_000_000u64),
                ("audio_seconds".to_string(), 1_000_000u64),
            ]
            .into(),
            concurrent: [("provider_slots".to_string(), 2u64)].into(),
        }
    }

    fn gateway() -> InMemoryGateway {
        let limit = ceiling();
        InMemoryGateway::new(
            "workspace",
            Limits {
                currency: "CNY".into(),
                global: limit.clone(),
                sessions: [("session".to_string(), limit.clone())].into(),
                providers: [("rehearsal".to_string(), limit)].into(),
                max_entries: 16,
            },
        )
        .unwrap()
    }

    fn entry(request: &str) -> UsageLedgerEntry {
        UsageLedgerEntry {
            workspace_id: "workspace".into(),
            session_id: "session".into(),
            provider_ref: "rehearsal".into(),
            request_id: request.into(),
            generation_epoch: 3,
            estimate: Some(BudgetCharge {
                currency: "CNY".into(),
                cost_minor: 0,
                units: BTreeMap::from([
                    ("requests".to_string(), 1),
                    ("audio_seconds".to_string(), 8),
                ]),
                price_revision: "price-rehearsal-1".into(),
            }),
            state: BillingState::Reserved as i32,
            ..Default::default()
        }
    }

    fn request(text: &str) -> SpeechRequest {
        SpeechRequest {
            request_id: "req-1".into(),
            provider_ref: "rehearsal".into(),
            session_id: "session".into(),
            utterance_id: "utt-1".into(),
            generation_epoch: 3,
            text: text.into(),
            sample_rate: 8_000,
            voice_ref: "voice-rehearsal".into(),
            remaining_budget_ms: 5_000,
            cancelled: false,
        }
    }

    #[test]
    fn stub_is_reproducible_and_the_sample_count_is_predictable() {
        let mut stub = RehearsalSpeechSynthesizer::new();
        let first = stub.synthesize(&request("商品价格是多少")).unwrap();
        let second = stub.synthesize(&request("商品价格是多少")).unwrap();
        assert_eq!(first, second, "the same request must reproduce exactly");
        assert_eq!(first.model_ref, RehearsalSpeechSynthesizer::MODEL_REF);
        assert_eq!(
            first.pcm.len(),
            RehearsalSpeechSynthesizer::sample_count("商品价格是多少", 8_000),
            "the produced length must equal the publicly computable length"
        );
        assert!(
            first
                .pcm
                .iter()
                .all(|sample| sample.is_finite() && sample.abs() <= 1.0)
        );
        let other = stub.synthesize(&request("换一个问题")).unwrap();
        assert_ne!(first.pcm, other.pcm, "a different text must not collide");
    }

    #[test]
    fn duration_is_bounded_at_both_ends() {
        let short = RehearsalSpeechSynthesizer::sample_count("a", 8_000);
        assert_eq!(
            short,
            (RehearsalSpeechSynthesizer::MIN_SPEECH_MS * 8_000 / 1_000) as usize
        );
        let long = RehearsalSpeechSynthesizer::sample_count(&"a".repeat(10_000), 8_000);
        assert_eq!(
            long,
            (RehearsalSpeechSynthesizer::MAX_SPEECH_MS as usize) * 8_000 / 1_000,
            "the duration must clamp instead of growing without bound"
        );
        assert!(long <= MAX_SPEECH_SAMPLES);
    }

    #[test]
    fn invalid_or_out_of_range_pcm_is_refused() {
        let mut out_of_range = request("商品价格是多少");
        let mut response = SpeechResponse {
            pcm: vec![0.0; 16],
            sample_rate: 8_000,
            model_ref: "x".into(),
            usage: json!({"characters": 1, "audio_seconds": 1}),
        };
        assert!(response.validate(&out_of_range).is_ok());
        response.pcm[3] = 1.5;
        assert!(response.validate(&out_of_range).is_err());
        response.pcm[3] = f32::NAN;
        assert!(response.validate(&out_of_range).is_err());
        response.pcm = Vec::new();
        assert!(response.validate(&out_of_range).is_err());
        out_of_range.sample_rate = 16_000;
        assert!(response.validate(&out_of_range).is_err());
    }

    #[test]
    fn cancelled_expired_or_unbound_requests_never_reach_the_synthesizer() {
        let mut stub = RehearsalSpeechSynthesizer::new();
        let mut gateway = gateway();
        let mut cancelled = request("商品价格是多少");
        cancelled.cancelled = true;
        let error = synthesize_with_budget(
            &mut stub,
            &mut gateway,
            entry("req-cancelled"),
            &cancelled,
            "price-rehearsal-1",
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Cancelled);
        assert!(gateway.book().get("rehearsal", "req-cancelled").is_none());

        let mut expired = request("商品价格是多少");
        expired.remaining_budget_ms = 0;
        let error = synthesize_with_budget(
            &mut stub,
            &mut gateway,
            entry("req-expired"),
            &expired,
            "price-rehearsal-1",
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::DeadlineExceeded);

        let mut unbound = request("商品价格是多少");
        unbound.voice_ref = String::new();
        assert_eq!(
            synthesize_with_budget(
                &mut stub,
                &mut gateway,
                entry("req-unbound"),
                &unbound,
                "price-rehearsal-1",
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidArgument
        );

        let mut bad_rate = request("商品价格是多少");
        bad_rate.sample_rate = 1;
        assert_eq!(
            synthesize_with_budget(
                &mut stub,
                &mut gateway,
                entry("req-rate"),
                &bad_rate,
                "price-rehearsal-1",
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn seam_accounts_usage_and_refuses_a_resend() {
        let mut stub = RehearsalSpeechSynthesizer::new();
        let mut gateway = gateway();
        let response = synthesize_with_budget(
            &mut stub,
            &mut gateway,
            entry("req-1"),
            &request("商品价格是多少"),
            "price-rehearsal-1",
        )
        .unwrap();
        assert_eq!(response.pcm.len(), 7 * 60 * 8);
        let stored = gateway.book().get("rehearsal", "req-1").unwrap();
        assert!(
            !matches!(
                BillingState::try_from(stored.state),
                Ok(BillingState::Reserved)
            ),
            "usage must be finished, never left reserved"
        );
        assert_eq!(
            stored
                .actual
                .as_ref()
                .and_then(|charge| charge.units.get("characters")),
            Some(&7),
            "the character count must be recorded"
        );
        let mut replay = entry("req-1");
        replay.state = BillingState::Reserved as i32;
        assert_eq!(
            synthesize_with_budget(
                &mut stub,
                &mut gateway,
                replay,
                &request("商品价格是多少"),
                "price-rehearsal-1",
            )
            .unwrap_err()
            .code,
            ErrorCode::Conflict
        );
    }
}
