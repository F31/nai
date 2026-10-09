//! Text-generation seam (Slice-0 vertical slice, A1).
//!
//! One generation request in, one text plus usage out. The seam is
//! transport-agnostic: a real HTTP adapter and the rehearsal stub implement the
//! same trait. Every call made through [`generate_with_budget`] is wrapped by
//! [`CallBudget`], so a generation can never bypass reservation, cancellation or
//! usage accounting.
use crate::client::ProviderError;
use crate::gateway::{CallBudget, UsageGateway};
use crate::usage::usage_charge;
use nai_contracts::v1::{ErrorCode, UsageLedgerEntry};
use serde_json::{Value, json};

/// Accepted prompt bound, in characters.
pub const MAX_PROMPT_CHARS: usize = 8192;
/// Accepted output bound, in characters.
pub const MAX_OUTPUT_CHARS: usize = 4096;

/// One generation request. Identity is carried explicitly so the seam never
/// infers a session, utterance or epoch from ambient state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRequest {
    pub request_id: String,
    pub provider_ref: String,
    pub session_id: String,
    pub utterance_id: String,
    pub generation_epoch: u64,
    pub prompt: String,
    pub max_output_chars: usize,
    /// Remaining task budget in milliseconds. Zero is an expired task, never a
    /// pass.
    pub remaining_budget_ms: u64,
    /// Explicit cancellation. A cancelled request never reaches a generator.
    pub cancelled: bool,
}

impl TextRequest {
    /// Rejects an incomplete, oversized, expired or cancelled request. An
    /// expired task and a cancellation are hard stops, not silent defaults.
    pub fn validate(&self) -> Result<(), ProviderError> {
        if self.request_id.is_empty()
            || self.provider_ref.is_empty()
            || self.session_id.is_empty()
            || self.utterance_id.is_empty()
            || self.generation_epoch == 0
        {
            return Err(ProviderError::new(
                ErrorCode::InvalidArgument,
                false,
                "generation request identity incomplete",
            ));
        }
        if self.prompt.trim().is_empty() || self.prompt.len() > MAX_PROMPT_CHARS {
            return Err(ProviderError::new(
                ErrorCode::InvalidArgument,
                false,
                "generation prompt empty or oversized",
            ));
        }
        if self.max_output_chars == 0 || self.max_output_chars > MAX_OUTPUT_CHARS {
            return Err(ProviderError::new(
                ErrorCode::InvalidArgument,
                false,
                "generation output bound invalid",
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

/// A generated text and the usage the generator reported for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextResponse {
    pub text: String,
    /// The identity the generator reports for itself.
    pub model_ref: String,
    /// Provider usage object, mapped by [`usage_charge`] before it is recorded.
    pub usage: Value,
}

/// A text generator. Implementations MUST NOT perform their own accounting;
/// callers MUST go through [`generate_with_budget`], which applies reservation,
/// cancellation and usage accounting around the call.
pub trait TextGenerator: Send {
    fn generate(&mut self, request: &TextRequest) -> Result<TextResponse, ProviderError>;
}

/// Reserve -> dispatch -> generate -> finish around one generation.
///
/// The generator is only invoked once the reservation obtained the exclusive
/// dispatch claim; a retry that returns `Already` is refused rather than
/// re-sent, and the final usage is charged against the stored revision.
pub fn generate_with_budget(
    generator: &mut dyn TextGenerator,
    gateway: &mut dyn UsageGateway,
    entry: UsageLedgerEntry,
    request: &TextRequest,
    price_revision: &str,
) -> Result<TextResponse, ProviderError> {
    request.validate()?;
    if price_revision.is_empty() {
        return Err(ProviderError::new(
            ErrorCode::InvalidArgument,
            false,
            "generation requires an exact price revision",
        ));
    }
    let currency = entry
        .estimate
        .as_ref()
        .map(|estimate| estimate.currency.clone())
        .ok_or_else(|| {
            ProviderError::new(
                ErrorCode::InvalidArgument,
                false,
                "usage entry has no estimate to reserve against",
            )
        })?;
    let budget = CallBudget::start(gateway, entry)?;
    if !budget.dispatched() {
        // Already dispatched elsewhere (e.g. an ACK retry): never re-send and
        // never double-account.
        return Err(ProviderError::new(
            ErrorCode::Conflict,
            false,
            "generation already dispatched",
        ));
    }
    let response = match generator.generate(request) {
        Ok(response) => response,
        Err(error) => {
            // Release the claim; the attempt stays counted for reconciliation.
            let _ = budget.finish(gateway, None);
            return Err(error);
        }
    };
    let actual = usage_charge(&currency, price_revision, &response.usage, 1)
        .map_err(|reason| ProviderError::new(ErrorCode::InvalidArgument, false, reason))?;
    budget.finish(gateway, Some(actual))?;
    Ok(response)
}

/// Deterministic, offline, rehearsal-only text generator.
///
/// It performs no network I/O and holds no credential. It MUST only be
/// constructed on rehearsal or test paths, MUST NOT be used for real
/// integration, paid calls or any certification, and never substitutes for
/// provider fault handling. Its output is reproducible: the same request always
/// yields the same text and the same usage.
pub struct RehearsalTextGenerator {
    model_ref: String,
}

impl RehearsalTextGenerator {
    /// The rehearsal identity reported for every generated response.
    pub const MODEL_REF: &'static str = "rehearsal-deterministic-v1";

    pub fn new() -> Self {
        Self {
            model_ref: Self::MODEL_REF.to_string(),
        }
    }
}

impl Default for RehearsalTextGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl TextGenerator for RehearsalTextGenerator {
    fn generate(&mut self, request: &TextRequest) -> Result<TextResponse, ProviderError> {
        request.validate()?;
        let digest = fnv1a_64(request.prompt.as_bytes());
        let rendered = format!("[rehearsal:{}] {}", digest, request.prompt.trim());
        let text: String = rendered.chars().take(request.max_output_chars).collect();
        let prompt_units = request.prompt.chars().count().max(1) as u64;
        let output_units = text.chars().count().max(1) as u64;
        Ok(TextResponse {
            text,
            model_ref: self.model_ref.clone(),
            usage: json!({
                "prompt_tokens": prompt_units,
                "completion_tokens": output_units,
                "total_tokens": prompt_units + output_units,
            }),
        })
    }
}

/// Stable 64-bit FNV-1a digest, rendered as lowercase hex. Used only to make the
/// rehearsal rendering reproducible; it is not a cryptographic hash.
fn fnv1a_64(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
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
                ("prompt_tokens".to_string(), 100_000u64),
                ("completion_tokens".to_string(), 100_000u64),
                ("total_tokens".to_string(), 100_000u64),
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
                    ("total_tokens".to_string(), 4096),
                ]),
                price_revision: "price-rehearsal-1".into(),
            }),
            state: BillingState::Reserved as i32,
            ..Default::default()
        }
    }

    fn request(prompt: &str) -> TextRequest {
        TextRequest {
            request_id: "req-1".into(),
            provider_ref: "rehearsal".into(),
            session_id: "session".into(),
            utterance_id: "utt-1".into(),
            generation_epoch: 3,
            prompt: prompt.into(),
            max_output_chars: 512,
            remaining_budget_ms: 5_000,
            cancelled: false,
        }
    }

    #[test]
    fn stub_is_reproducible_and_offline() {
        let mut stub = RehearsalTextGenerator::new();
        let first = stub.generate(&request("请介绍商品")).unwrap();
        let second = stub.generate(&request("请介绍商品")).unwrap();
        assert_eq!(first, second, "the same request must reproduce exactly");
        assert_eq!(first.model_ref, RehearsalTextGenerator::MODEL_REF);
        let other = stub.generate(&request("换一个问题")).unwrap();
        assert_ne!(
            first.text, other.text,
            "a different prompt must not collide"
        );
    }

    #[test]
    fn seam_accounts_usage_and_marks_the_request_settled() {
        let mut stub = RehearsalTextGenerator::new();
        let mut gateway = gateway();
        let response = generate_with_budget(
            &mut stub,
            &mut gateway,
            entry("req-1"),
            &request("请介绍商品"),
            "price-rehearsal-1",
        )
        .unwrap();
        assert!(response.text.starts_with("[rehearsal:"));
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
                .and_then(|charge| charge.units.get("requests")),
            Some(&1),
            "the attempt count must be recorded"
        );
    }

    #[test]
    fn retry_after_dispatch_is_refused_without_resending() {
        let mut stub = RehearsalTextGenerator::new();
        let mut gateway = gateway();
        generate_with_budget(
            &mut stub,
            &mut gateway,
            entry("req-1"),
            &request("请介绍商品"),
            "price-rehearsal-1",
        )
        .unwrap();
        // A second entry for the same provider/request must not be re-sent.
        let mut replay = entry("req-1");
        replay.state = BillingState::Reserved as i32;
        let error = generate_with_budget(
            &mut stub,
            &mut gateway,
            replay,
            &request("请介绍商品"),
            "price-rehearsal-1",
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Conflict);
    }

    #[test]
    fn cancellation_and_expiry_never_reach_the_generator() {
        let mut stub = RehearsalTextGenerator::new();
        let mut gateway = gateway();
        let mut cancelled = request("请介绍商品");
        cancelled.cancelled = true;
        let error = generate_with_budget(
            &mut stub,
            &mut gateway,
            entry("req-cancelled"),
            &cancelled,
            "price-rehearsal-1",
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Cancelled);
        assert!(
            gateway.book().get("rehearsal", "req-cancelled").is_none(),
            "a cancelled request must never be reserved"
        );

        let mut expired = request("请介绍商品");
        expired.remaining_budget_ms = 0;
        let error = generate_with_budget(
            &mut stub,
            &mut gateway,
            entry("req-expired"),
            &expired,
            "price-rehearsal-1",
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::DeadlineExceeded);
    }

    #[test]
    fn budget_ceiling_stops_an_unbounded_generation() {
        let mut stub = RehearsalTextGenerator::new();
        let mut gateway = InMemoryGateway::new(
            "workspace",
            Limits {
                currency: "CNY".into(),
                global: Ceiling {
                    cost_minor: 0,
                    units: [("requests".to_string(), 0u64)].into(),
                    concurrent: [("provider_slots".to_string(), 0u64)].into(),
                },
                sessions: BTreeMap::new(),
                providers: BTreeMap::new(),
                max_entries: 4,
            },
        )
        .unwrap();
        let error = generate_with_budget(
            &mut stub,
            &mut gateway,
            entry("req-budget"),
            &request("请介绍商品"),
            "price-rehearsal-1",
        )
        .unwrap_err();
        assert!(
            matches!(
                error.code,
                ErrorCode::ResourceExhausted | ErrorCode::InvalidArgument
            ),
            "a rejected reservation must not produce text, got {error:?}"
        );
    }
}
