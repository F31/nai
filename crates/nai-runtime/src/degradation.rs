//! H17 runtime wiring: judge a harness layer failure, render it for the interface, and put it
//! where the interface can read it back (V3.11 §47.4, §12.2, INV-32; W132).
//!
//! W66 landed the judgement (`evaluate_degradation`) and said so in its own module docs. Until
//! this module that sentence was true *and* unverifiable — `grep` found the judgement called from
//! tests and nowhere else, so "does a degradation actually reach the interface?" was a question
//! only a code reader could answer.
//!
//! `DegradeVerdict::visible` is `true` by construction: the judge has no way to express an
//! invisible degradation, so "no silent degradation" is not testable there. What it cannot cover
//! is the *transport*, and this module is that transport.
//!
//! Three invariants, each tested:
//!
//! * **Judged ⇒ rendered ⇒ persisted, once.** The payload is exactly what the judge emitted and
//!   the renderer copied — the runtime adds no field of its own and never recomputes a code, a
//!   level or a next action.
//! * **The read-back is decoded from the chain.** `read_degradation_view` never accepts the
//!   caller's argument as an answer, so a view cannot be handed its own content.
//! * **INV-32's second half runs here too.** A degraded call that lost its policy or persona
//!   constraint is refused by name and persists **nothing** — naming the code and generating
//!   anyway is the failure this closes.
//!
//! One entry point on purpose: offering a separate `judge` and `persist` would move the "never
//! show an unjudged degradation" duty onto every caller, where one omission breaks it invisibly.

use crate::composition::short;
use crate::{Composition, Result, err};
use nai_contracts::v1::ErrorCode;
use nai_kernel::evaluate::harness::{
    DegradationNotice, DegradeContext, DegradeVerdict, DegradedCallContext, FailureLayer,
    assert_no_free_form, evaluate_degradation, render_degradation_notices,
};
use nai_storage::DecisionKind;
use serde_json::{Value, json};

/// What the assembled context still carries once the failures have been absorbed.
///
/// This is the caller's **observed fact**, not a verdict: the caller states what survived, the
/// runtime decides what that means. It maps onto `DegradedCallContext`'s two constraint fields and
/// nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retained {
    /// The assembled context still carries the system-policy segment.
    pub policy_constraint: bool,
    /// The assembled context still carries the persona segment.
    pub persona: bool,
}

/// §47.4: one degradation as it reads back from the chain.
///
/// Owned, unlike the kernel's `DegradationNotice`, because it was decoded from stored bytes — the
/// same write/read asymmetry `LoggedAssembly` has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggedDegradation {
    pub layer: String,
    pub code: String,
    pub level: String,
    pub actionable: String,
    pub refuses_call: bool,
    pub drops: Option<String>,
    pub keeps_existing_session: bool,
}

/// What the interface shows: every degradation that happened, and whether the call is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DegradationView {
    pub notices: Vec<LoggedDegradation>,
    /// §47.4: assembly refuses the call outright, and that is part of what MUST be visible.
    pub refuses_call: bool,
}

/// INV-32 second half: why a degraded call was not allowed to proceed, and the material that
/// therefore did **not** reach the chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreeFormRefusal {
    /// `FreeFormViolation::label()`, in the order the judge produced them.
    pub violations: Vec<&'static str>,
    /// What *would* have been shown. Carried so the caller can inspect it; this call persisted
    /// none of it.
    pub notices: Vec<DegradationNotice>,
}

/// The outcome of offering one set of layer failures to the interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DegradationOutcome {
    /// Judged and rendered; shown to the interface and persisted as one `degradation` entry.
    Visible(DegradationView),
    /// Refused by name; nothing was persisted.
    Refused(FreeFormRefusal),
}

impl Composition {
    /// Judge `failures` against `available`, render them for the interface, and persist the
    /// rendered notices as one `degradation` entry — unless INV-32's second half refuses them.
    ///
    /// The judgement runs **before** anything is written, and a refusal writes nothing: a
    /// degradation that never reached the chain cannot be shown, and showing an unjudged one is
    /// worse than showing none.
    pub fn record_degradation(
        &mut self,
        failures: &[FailureLayer],
        available: &DegradeContext,
        retained: Retained,
    ) -> Result<DegradationOutcome> {
        let verdicts: Vec<DegradeVerdict> = failures
            .iter()
            .map(|layer| evaluate_degradation(*layer, available))
            .collect();
        let notices = render_degradation_notices(&verdicts);
        let call = DegradedCallContext {
            degradations: verdicts.clone(),
            has_policy_constraint: retained.policy_constraint,
            has_persona: retained.persona,
        };

        let violations = assert_no_free_form(&call);
        if !violations.is_empty() {
            return Ok(DegradationOutcome::Refused(FreeFormRefusal {
                violations: violations
                    .iter()
                    .map(|violation| violation.label())
                    .collect(),
                notices,
            }));
        }
        // No failure means nothing to show, so nothing is written. This is a real zero, not a
        // skipped check: the judgement above has already run and found nothing to refuse.
        if verdicts.is_empty() {
            return Ok(DegradationOutcome::Visible(DegradationView {
                notices: Vec::new(),
                refuses_call: false,
            }));
        }

        let session = self.session_id().to_owned();
        let trace = self.next_trace("degradation");
        let payload = json!({
            "notices": notices.iter().map(notice_json).collect::<Vec<Value>>(),
        });
        self.record(DecisionKind::Degradation, Some(&session), &payload, &trace)?;
        Ok(DegradationOutcome::Visible(DegradationView {
            refuses_call: notices.iter().any(|notice| notice.refuses_call),
            notices: notices.iter().map(owned).collect(),
        }))
    }

    /// The degradations the interface is to show, decoded from the append-only chain.
    ///
    /// Read from storage, never from a caller's argument. A row that is not decodable is a
    /// **classified failure**, not a silent skip: this write path only ever persisted one shape,
    /// so anything else is corruption. An empty chain is a real zero and returns an empty view.
    pub fn read_degradation_view(&self, limit: u32) -> Result<DegradationView> {
        let operation = short()?;
        let store = self
            .store()
            .try_lock()
            .map_err(|_| err(ErrorCode::Conflict, "decision store busy"))?;
        let rows = store.decision_entries(&operation, self.workspace(), limit)?;
        let mut notices = Vec::new();
        for row in rows
            .iter()
            .filter(|row| row.kind == DecisionKind::Degradation.as_str())
        {
            notices.extend(decode_notices(&row.payload_json)?);
        }
        Ok(DegradationView {
            refuses_call: notices.iter().any(|notice| notice.refuses_call),
            notices,
        })
    }
}

fn notice_json(notice: &DegradationNotice) -> Value {
    json!({
        "layer": notice.layer,
        "code": notice.code,
        "level": notice.level,
        "actionable": notice.actionable,
        "refuses_call": notice.refuses_call,
        "drops": notice.drops,
        "keeps_existing_session": notice.keeps_existing_session,
    })
}

/// Borrowed at write time, owned at read time. Same fields, same order — the renderer is not a
/// second source of truth.
fn owned(notice: &DegradationNotice) -> LoggedDegradation {
    LoggedDegradation {
        layer: notice.layer.to_string(),
        code: notice.code.to_string(),
        level: notice.level.to_string(),
        actionable: notice.actionable.to_string(),
        refuses_call: notice.refuses_call,
        drops: notice.drops.map(str::to_string),
        keeps_existing_session: notice.keeps_existing_session,
    }
}

fn decode_notices(payload_json: &str) -> Result<Vec<LoggedDegradation>> {
    let value: Value = serde_json::from_str(payload_json)
        .map_err(|_| err(ErrorCode::Internal, "degradation log row is not JSON"))?;
    let entries = value
        .get("notices")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            err(
                ErrorCode::Internal,
                "degradation log row carries no notices",
            )
        })?;
    entries.iter().map(decode_notice).collect()
}

fn decode_notice(entry: &Value) -> Result<LoggedDegradation> {
    Ok(LoggedDegradation {
        layer: required_str(entry, "layer")?,
        code: required_str(entry, "code")?,
        level: required_str(entry, "level")?,
        actionable: required_str(entry, "actionable")?,
        refuses_call: required_bool(entry, "refuses_call")?,
        drops: optional_str(entry, "drops")?,
        keeps_existing_session: required_bool(entry, "keeps_existing_session")?,
    })
}

/// A field this write path always persisted. Its absence is corruption, so it is not defaulted.
fn required_str(value: &Value, key: &str) -> Result<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            err(
                ErrorCode::Internal,
                "degradation log row is missing a string field",
            )
        })
}

fn required_bool(value: &Value, key: &str) -> Result<bool> {
    value.get(key).and_then(Value::as_bool).ok_or_else(|| {
        err(
            ErrorCode::Internal,
            "degradation log row is missing a boolean field",
        )
    })
}

/// `drops` is `None` for the assembly layer, so it is legitimately absent or null. A value that is
/// present but not a string is corruption and is **not** read as `None`.
fn optional_str(value: &Value, key: &str) -> Result<Option<String>> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(found) => found
            .as_str()
            .map(|text| Some(text.to_string()))
            .ok_or_else(|| {
                err(
                    ErrorCode::Internal,
                    "degradation log row carries a non-string drop",
                )
            }),
    }
}
