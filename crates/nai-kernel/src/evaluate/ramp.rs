//! Harness ramp gating (V3.9 §5.9, §47.2, §47.3; W67, acceptance H18).
//!
//! §47.3 is two sentences with two different subjects, and they have to stay separate:
//!
//! * **Default enablement** — a new harness mechanism is enabled in rehearsal, shadow and
//!   assisted; `AutoSupervised` is the only gated stage. Judged by
//!   [`RampStage::enabled_by_default`].
//! * **Promotion** — reaching `AutoSupervised` additionally requires the §47.2 regression to
//!   have passed *and* an enablement record to exist. Both are named refusals.
//!
//! Folding the two into one predicate makes "was this refused because it is not enabled by
//! default, or because a condition was missed" undecidable again — the same reason W66 did not
//! reuse [`crate::bridge::Degradation`].
//!
//! **Not to be confused with [`crate::session::Session::change_mode`]**, which gates the
//! *session* dimension (duty lease, second confirmation, `actor_allowed`) and already
//! implements the `AutoSupervised → Assisted` fallback. This module gates the *mechanism*
//! dimension: within one session, different harness mechanisms may sit at different stages.
//!
//! **A finding that must stay visible.** §5.9 and §47.3 ramp through **four** stages, but the
//! contract `SessionMode` has only **three** values — there is no `Shadow`; §1.4 and §17.3
//! place 影子模式 in **R2 (Standard)**. So one stage named by an R1 acceptance criterion has no
//! landing point inside R1. [`RampStage::session_mode`] returns `None` for it instead of
//! inventing a mapping, and the tests pin that fact.
//!
//! **Scope boundary.** This module decides *whether a promotion is allowed*. Enabling a
//! mechanism per stage at runtime, and appending the record to the hash chain in `nai-storage`,
//! is wiring and is **not** claimed here. Producing the §47.2 regression itself is H16, which
//! V3.9 §51.3 places in `capability_pack`; the gate only consumes a declared `passed` set and
//! never runs a case, so H18 stays decidable with no capability pack installed (INV-33).

use std::collections::BTreeMap;

use nai_contracts::v1::SessionMode;

use super::harness::CREDENTIAL_MARKERS;

// ---------------------------------------------------------------------------
// §5.9 / §47.3: the ramp
// ---------------------------------------------------------------------------

/// §5.9: 演练 → 影子（候选只记录，不合成）→ 人工辅助 → 受监督自动. The variant order **is**
/// the ramp order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RampStage {
    /// Rehearsal.
    Rehearsal,
    /// Shadow: candidates are recorded, never synthesised.
    Shadow,
    /// Human-assisted.
    Assisted,
    /// Supervised automatic — the only gated stage.
    AutoSupervised,
}

/// The four stages in §5.9 order. Tests assert this order is not merely a count.
pub const RAMP_ORDER: [RampStage; 4] = [
    RampStage::Rehearsal,
    RampStage::Shadow,
    RampStage::Assisted,
    RampStage::AutoSupervised,
];

/// §47.3: the stages a new mechanism is enabled in **by default**. Transcribed from the
/// sentence, neither added to nor trimmed.
pub const DEFAULT_ENABLED_STAGES: [RampStage; 3] =
    [RampStage::Rehearsal, RampStage::Shadow, RampStage::Assisted];

impl RampStage {
    /// Position in [`RAMP_ORDER`].
    pub fn index(self) -> usize {
        match self {
            RampStage::Rehearsal => 0,
            RampStage::Shadow => 1,
            RampStage::Assisted => 2,
            RampStage::AutoSupervised => 3,
        }
    }

    /// Stable decision-log label.
    pub fn label(self) -> &'static str {
        match self {
            RampStage::Rehearsal => "rehearsal",
            RampStage::Shadow => "shadow",
            RampStage::Assisted => "assisted",
            RampStage::AutoSupervised => "auto_supervised",
        }
    }

    /// §47.3: enabled without promotion. Only supervised automatic is gated — a ramp that gates
    /// nothing is not a ramp.
    pub fn enabled_by_default(self) -> bool {
        !matches!(self, RampStage::AutoSupervised)
    }

    /// The contract session mode this stage lands on, if any.
    ///
    /// `Shadow` returns `None`: §1.4 and §17.3 place 影子模式 in R2, and `SessionMode` has no
    /// variant for it. Returning a fabricated mapping would hide the gap.
    pub fn session_mode(self) -> Option<SessionMode> {
        match self {
            RampStage::Rehearsal => Some(SessionMode::Rehearsal),
            RampStage::Shadow => None,
            RampStage::Assisted => Some(SessionMode::Assisted),
            RampStage::AutoSupervised => Some(SessionMode::AutoSupervised),
        }
    }
}

// ---------------------------------------------------------------------------
// §47.2: the six regression categories
// ---------------------------------------------------------------------------

/// §47.2: harness properties that MUST sit in the golden set. Exactly six — a category missing
/// from this list MUST NOT be read as "the other five cover it".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RegressionCategory {
    /// Assembly regression: order and budgets.
    AssemblyRegression,
    /// Intent refusal: low confidence falls to `unknown`.
    IntentRefusal,
    /// Tool failure recovery: partial and total failure.
    ToolFailureRecovery,
    /// Cache false hit: disabled domains and changed facts.
    CacheFalseHit,
    /// MCP tool description drift.
    McpDescriptionDrift,
    /// A2A delegation beyond authority.
    A2aExcessDelegation,
}

/// The six categories. Tests assert completeness and distinct labels.
pub const REGRESSION_CATEGORIES: [RegressionCategory; 6] = [
    RegressionCategory::AssemblyRegression,
    RegressionCategory::IntentRefusal,
    RegressionCategory::ToolFailureRecovery,
    RegressionCategory::CacheFalseHit,
    RegressionCategory::McpDescriptionDrift,
    RegressionCategory::A2aExcessDelegation,
];

impl RegressionCategory {
    /// Stable decision-log label.
    pub fn label(self) -> &'static str {
        match self {
            RegressionCategory::AssemblyRegression => "regression.assembly",
            RegressionCategory::IntentRefusal => "regression.intent_refusal",
            RegressionCategory::ToolFailureRecovery => "regression.tool_failure_recovery",
            RegressionCategory::CacheFalseHit => "regression.cache_false_hit",
            RegressionCategory::McpDescriptionDrift => "regression.mcp_description_drift",
            RegressionCategory::A2aExcessDelegation => "regression.a2a_excess_delegation",
        }
    }
}

// ---------------------------------------------------------------------------
// §47.3: the promotion gate
// ---------------------------------------------------------------------------

/// §47.3: the record an enablement MUST leave in the decision log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnablementRecord {
    /// The mechanism being enabled.
    pub mechanism: String,
    /// The stage it is being enabled at.
    pub stage: RampStage,
    /// What will be logged alongside. Judged by value, never by field name (INV-11).
    pub fields: BTreeMap<String, String>,
}

/// One promotion attempt.
#[derive(Debug, Clone)]
pub struct PromotionRequest<'a> {
    pub mechanism: &'a str,
    /// Where the mechanism sits now.
    pub current: RampStage,
    /// Where it is asking to sit.
    pub target: RampStage,
    /// §47.2: the categories this mechanism can affect. MUST be declared — an empty declaration
    /// makes "the relevant regression" undecidable and lets the gate be walked around.
    pub touched: &'a [RegressionCategory],
    /// The categories recorded as passed.
    pub passed: &'a [RegressionCategory],
    /// The harness revision the pass was recorded against. `None` means "passed, unknown when".
    pub passed_at_revision: Option<u64>,
    /// The harness revision now.
    pub harness_revision: u64,
    /// The enablement record offered for the log.
    pub record: Option<&'a EnablementRecord>,
}

/// §47.3 / §47.2 / INV-11: every reason a promotion must not proceed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RampRejection {
    /// The target is not ahead of the current stage; a ramp does not move sideways or back.
    NotAnAdvance,
    /// Promotion to supervised automatic declares no affected regression category.
    RegressionScopeUndeclared,
    /// A declared category has not passed.
    RegressionMissing,
    /// The regression passed, but against a different harness revision (§47.2: it takes effect
    /// only *after* the relevant regression — a later harness change voids it).
    RegressionStale,
    /// No enablement record was offered.
    EnablementRecordMissing,
    /// The record names a different mechanism.
    RecordMechanismMismatch,
    /// The record names a different stage.
    RecordStageMismatch,
    /// A **value** in the record carries a credential literal (INV-11).
    RecordContainsCredential,
}

impl RampRejection {
    /// All eight rejection names. Tests assert the constructed set is exactly this.
    pub const ALL: [RampRejection; 8] = [
        RampRejection::NotAnAdvance,
        RampRejection::RegressionScopeUndeclared,
        RampRejection::RegressionMissing,
        RampRejection::RegressionStale,
        RampRejection::EnablementRecordMissing,
        RampRejection::RecordMechanismMismatch,
        RampRejection::RecordStageMismatch,
        RampRejection::RecordContainsCredential,
    ];

    /// Stable decision-log label.
    pub fn label(self) -> &'static str {
        match self {
            RampRejection::NotAnAdvance => "ramp.not_an_advance",
            RampRejection::RegressionScopeUndeclared => "ramp.regression_scope_undeclared",
            RampRejection::RegressionMissing => "ramp.regression_missing",
            RampRejection::RegressionStale => "ramp.regression_stale",
            RampRejection::EnablementRecordMissing => "ramp.enablement_record_missing",
            RampRejection::RecordMechanismMismatch => "ramp.record_mechanism_mismatch",
            RampRejection::RecordStageMismatch => "ramp.record_stage_mismatch",
            RampRejection::RecordContainsCredential => "ramp.record_contains_credential",
        }
    }
}

/// Outcome of one promotion attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RampVerdict {
    pub mechanism: String,
    pub from: RampStage,
    pub to: RampStage,
    /// True only when nothing was rejected.
    pub allowed: bool,
    pub rejections: Vec<RampRejection>,
    /// Every declared-but-unpassed category, not just the first one.
    pub missing_categories: Vec<&'static str>,
    /// Record fields whose value carried a credential literal.
    pub leaked_fields: Vec<String>,
    /// The record to append to the decision log. Produced only when a promotion into
    /// supervised automatic is allowed — other stages carry no enablement requirement.
    pub record_to_append: Option<EnablementRecord>,
}

/// §47.3: judge one promotion.
///
/// Stages below supervised automatic are enabled by default, so the only requirement there is
/// that the move really is an advance. Reaching supervised automatic requires the §47.2
/// regression (declared scope, all of it passed, and passed against the harness revision that is
/// in force now) **and** an enablement record naming the same mechanism and stage with no
/// plaintext credential in any value.
pub fn evaluate_promotion(req: &PromotionRequest<'_>) -> RampVerdict {
    let mut rejections = Vec::new();
    let mut missing_categories = Vec::new();
    let mut leaked_fields = Vec::new();

    if req.target.index() <= req.current.index() {
        rejections.push(RampRejection::NotAnAdvance);
    }

    if req.target == RampStage::AutoSupervised {
        if req.touched.is_empty() {
            rejections.push(RampRejection::RegressionScopeUndeclared);
        } else {
            for category in req.touched {
                if !req.passed.contains(category) {
                    missing_categories.push(category.label());
                }
            }
            if !missing_categories.is_empty() {
                rejections.push(RampRejection::RegressionMissing);
            }
        }

        if req.passed_at_revision != Some(req.harness_revision) {
            rejections.push(RampRejection::RegressionStale);
        }

        match req.record {
            None => rejections.push(RampRejection::EnablementRecordMissing),
            Some(record) => {
                if record.mechanism != req.mechanism {
                    rejections.push(RampRejection::RecordMechanismMismatch);
                }
                if record.stage != req.target {
                    rejections.push(RampRejection::RecordStageMismatch);
                }
                let mut leaked = false;
                for (name, value) in &record.fields {
                    let lowered = value.to_ascii_lowercase();
                    if CREDENTIAL_MARKERS
                        .iter()
                        .any(|marker| lowered.contains(marker))
                    {
                        leaked_fields.push(name.clone());
                        leaked = true;
                    }
                }
                if leaked {
                    rejections.push(RampRejection::RecordContainsCredential);
                }
            }
        }
    }

    let allowed = rejections.is_empty();
    let record_to_append = match (allowed, req.record) {
        (true, Some(record)) if req.target == RampStage::AutoSupervised => Some(record.clone()),
        _ => None,
    };

    RampVerdict {
        mechanism: req.mechanism.to_string(),
        from: req.current,
        to: req.target,
        allowed,
        rejections,
        missing_categories,
        leaked_fields,
        record_to_append,
    }
}
