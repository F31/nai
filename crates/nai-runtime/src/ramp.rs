//! H18 runtime wiring: judge a harness ramp promotion and append the enablement record the judge
//! produced to the real append-only hash chain (V3.11 §5.9, §47.2, §47.3; W105).
//!
//! W67 landed the judgement (`nai_kernel::evaluate::ramp::evaluate_promotion`) and stated the
//! boundary in its own module docs: enabling a mechanism per stage at runtime, and appending the
//! record to `nai-storage`'s chain, "is wiring and is **not** claimed here". Until this module
//! that sentence was true *and* unverifiable — and the seam was more glaring than in W103/W104,
//! because the judge had already **computed the record to append**:
//!
//! ```text
//! RampVerdict { …, record_to_append: Option<EnablementRecord> }   // ramp.rs:257
//! ```
//!
//! `EnablementRecord` is an owned value (`mechanism: String`, `stage: RampStage`,
//! `fields: BTreeMap<String, String>`), so it can be serialised straight into the chain, and the
//! `match` that builds it (`ramp.rs:322`) exists for exactly that. Its only consumer was the
//! kernel's own test file. This module is the runtime half.
//!
//! **Three outcomes, not two.** [`RampVerdict::record_to_append`] is `None` for *two* reasons that
//! MUST stay apart:
//!
//! | case | `allowed` | `record_to_append` | reported as |
//! |---|---|---|---|
//! | refused (regression missed, record missing, credential in a value) | `false` | `None` | [`RampOutcome::Refused`] |
//! | allowed into supervised automatic | `true` | `Some(record)` | [`RampOutcome::Logged`] |
//! | allowed into a stage **before** the gate | `true` | `None` | [`RampOutcome::NoRecordRequired`] |
//!
//! Collapsing the third into either of the others makes the runtime lie: "logged" is a lie because
//! nothing was appended, and "refused" is a lie because nothing was refused — §47.3 carries no
//! enablement requirement below the gated stage ("other stages carry no enablement requirement",
//! `ramp.rs:256`). A two-armed `Logged`/`Refused` shape passes all thirteen kernel tests; this one
//! does not.
//!
//! **`allowed` and `record_to_append` are read as two fields, never folded.** Inferring "was this
//! allowed" from `record_to_append.is_some()` is precisely the folding the kernel module warns
//! about at its top ("Folding the two into one predicate makes 'was this refused because it is not
//! enabled by default, or because a condition was missed' undecidable again").
//!
//! The judge is *called*, not re-written, so "which rule refused this" stays decidable in one
//! place. Nothing here runs a regression case: the gate consumes a declared `passed` set, which is
//! why H18 stays decidable with no capability pack installed (INV-33).

use crate::{Composition, Result};
use nai_kernel::evaluate::ramp::{PromotionRequest, RampStage, RampVerdict, evaluate_promotion};
use nai_storage::{DecisionAck, DecisionKind};
use serde_json::json;

/// Why a promotion was refused, and the material that therefore did **not** reach the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RampRefusal {
    /// Rejection labels, in the order the judge produced them.
    pub rejections: Vec<&'static str>,
    /// **Every** declared-but-unpassed category, not just the first one (§47.3).
    pub missing_categories: Vec<&'static str>,
    /// Names of record fields whose **value** carried a credential literal (INV-11). The names are
    /// reportable; the values are not, and nothing here persisted either.
    pub leaked_fields: Vec<String>,
}

/// The outcome of offering one promotion to the decision log.
///
/// Three arms, because "no record was produced" has two causes that call for opposite readings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RampOutcome {
    /// Judged allowed *and* the judge produced an enablement record; appended as one `approval`
    /// entry.
    Logged(DecisionAck),
    /// Judged allowed, but the target stage sits **before** supervised automatic, which carries no
    /// enablement requirement. Nothing was appended — and this is **not** a refusal.
    NoRecordRequired {
        /// Where the mechanism sat.
        from: RampStage,
        /// Where it is allowed to sit.
        to: RampStage,
    },
    /// Refused by name; nothing was appended.
    Refused(RampRefusal),
}

impl Composition {
    /// Judge `request` and, only when the judge allowed it *and* produced an enablement record,
    /// append that record to the append-only decision log as one `approval` entry.
    ///
    /// The appended payload is the judged material and nothing else — the mechanism, the stage it
    /// came from, the stage it is moving to, and the record's own fields. The request's declared
    /// and passed regression sets are deliberately **not** logged: the judge consumed them to
    /// *decide*, it never vetted them for the log, and a runtime that adds them has stopped being a
    /// transport.
    ///
    /// The entry lands as `approval` — a **critical** kind — so a write failure is returned as an
    /// error rather than being silently buffered. An enablement record is a "lands or is reported"
    /// decision, not droppable telemetry (INV-15).
    pub fn record_promotion(&mut self, request: &PromotionRequest<'_>) -> Result<RampOutcome> {
        let verdict = evaluate_promotion(request);
        // Read the two fields separately: `allowed` answers "may this move forward at all", while
        // `record_to_append` answers "is there a record to persist". They are not the same question.
        if !verdict.allowed {
            return Ok(RampOutcome::Refused(refusal_of(&verdict)));
        }
        let record = match verdict.record_to_append {
            Some(record) => record,
            None => {
                return Ok(RampOutcome::NoRecordRequired {
                    from: verdict.from,
                    to: verdict.to,
                });
            }
        };
        let payload = json!({
            "mechanism": record.mechanism,
            "from": verdict.from.label(),
            "to": record.stage.label(),
            "fields": record.fields,
        });
        let session = self.session_id().to_owned();
        let trace = self.next_trace("ramp");
        let ack = self.record(DecisionKind::Approval, Some(&session), &payload, &trace)?;
        Ok(RampOutcome::Logged(ack))
    }
}

/// The judge's refusal, in the runtime's own shape.
fn refusal_of(verdict: &RampVerdict) -> RampRefusal {
    RampRefusal {
        rejections: verdict
            .rejections
            .iter()
            .map(|reason| reason.label())
            .collect(),
        missing_categories: verdict.missing_categories.clone(),
        leaked_fields: verdict.leaked_fields.clone(),
    }
}
