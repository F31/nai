//! H15 runtime wiring: judge a harness observation, then append it to the real
//! append-only hash chain (V3.11 §47.1; W103).
//!
//! W66 landed the judgement (`nai_kernel::evaluate::harness::evaluate_observation`) and said so
//! in its own module docs: appending to `nai-storage`'s chain is runtime wiring and was **not**
//! claimed there. Until this module that sentence was true *and* unverifiable — `grep` found the
//! judgement called from tests and nowhere else, so "does a refused record still land in the
//! log?" was a question only a code reader could answer. This module is the runtime half.
//!
//! Two invariants, both testable:
//!
//! * **Judged loggable ⇒ append the judged material.** The payload is exactly what the judge
//!   emitted — the layer label, the redacted fields, the redacted viewer text. The runtime is a
//!   *transport*, not an enricher: it adds no field the judge did not vet, and it never reaches
//!   for `record.fields`.
//! * **Judged not loggable ⇒ a named refusal, and nothing appended.** The refusal names the layer
//!   and every rejection, and carries the judge's redacted material so a caller can see it; that
//!   material is explicitly **not** persisted by this call.
//!
//! The two are one entry point on purpose. Offering a separate `evaluate` and `append` would move
//! the "never append an unjudged record" duty onto every caller, where one omission breaks the
//! invariant and no test can see it.

use crate::{Composition, Result};
use nai_kernel::evaluate::harness::{ObservationPolicy, ObservationRecord, evaluate_observation};
use nai_storage::{DecisionAck, DecisionKind};
use serde_json::json;
use std::collections::BTreeMap;

/// Why a record was refused, and the material that therefore did **not** reach the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationRefusal {
    /// `ObservationLayer::label()` for the refused record's layer.
    pub layer: &'static str,
    /// Rejection labels, in the order the judge produced them.
    pub rejections: Vec<&'static str>,
    /// The judge's redacted material. Carried so a caller can inspect what a loggable version
    /// would look like — the call it came from persisted none of it.
    pub redacted: BTreeMap<String, String>,
}

/// The outcome of offering one observation to the decision log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservationOutcome {
    /// Judged loggable; appended as one `observation` entry.
    Logged(DecisionAck),
    /// Refused by name; nothing was appended.
    Refused(ObservationRefusal),
}

impl Composition {
    /// Judge `record` and, only when it is loggable, append it to the append-only decision log
    /// as one `observation` entry.
    ///
    /// The appended payload is the judge's redacted material and nothing else — see the module
    /// docs for why no raw record field (not even `subject`) may be added here.
    pub fn record_observation(
        &mut self,
        record: &ObservationRecord,
        policy: &ObservationPolicy,
    ) -> Result<ObservationOutcome> {
        let verdict = evaluate_observation(record, policy);
        if !verdict.loggable {
            return Ok(ObservationOutcome::Refused(ObservationRefusal {
                layer: verdict.layer.label(),
                rejections: verdict.rejections.iter().map(|r| r.label()).collect(),
                redacted: verdict.redacted,
            }));
        }
        let session = self.session_id().to_owned();
        let trace = self.next_trace("observation");
        let payload = json!({
            "layer": verdict.layer.label(),
            "fields": verdict.redacted,
            "viewer": verdict.redacted_viewer,
        });
        let ack = self.record(DecisionKind::Observation, Some(&session), &payload, &trace)?;
        Ok(ObservationOutcome::Logged(ack))
    }
}
