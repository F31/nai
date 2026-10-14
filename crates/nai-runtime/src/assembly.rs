//! H02 runtime wiring: judge a context assembly, append its fragments to the real append-only
//! hash chain, and rebuild the sequence from that chain alone (V3.11 §41.5; W104).
//!
//! W69 landed the judgement (`nai_kernel::evaluate::assembly::rebuild_assembly`) and stated the
//! boundary in its own module docs and in `docs/kernel.md`: appending the record to `nai-storage`
//! and the §47.2 regression classes are runtime work, **not** claimed there. Until this module
//! that sentence was true *and* unverifiable — `grep` found `rebuild_assembly` called from tests
//! and nowhere else, so "does a non-rebuildable assembly still land in the log?" was a question
//! only a code reader could answer. This module is the runtime half.
//!
//! Two invariants, both testable:
//!
//! * **Judged rebuildable ⇒ append the judged material.** The payload is exactly what the judge
//!   let through — each referenced fragment's id, source ref and **body**, in the record's entry
//!   order, plus the model and prompt versions. The runtime is a *transport*, not an enricher: a
//!   body that only ever lived in working memory is refused (`FragmentContentNotInLog`), never
//!   persisted as a pointer.
//! * **Judged not rebuildable ⇒ a named refusal, and nothing appended.** The refusal names every
//!   rejection and the three kinds of suspect ids, and the call persists none of it.
//!
//! The two are one entry point on purpose. Offering a separate `evaluate` and `append` would move
//! the "never append a non-rebuildable assembly" duty onto every caller, where one omission breaks
//! the invariant and no test can see it.
//!
//! [`Composition::rebuild_assembly_from_log`] is the read half: it reads the chain back and hands
//! the rows to the **same** judgement. The judge is *called*, not re-written, so "which rule
//! refused this" stays decidable in one place.

use crate::composition::short;
use crate::{Composition, Result, err};
use nai_contracts::v1::ErrorCode;
use nai_kernel::evaluate::assembly::{
    AssemblyRecord, LoggedFragment, RebuildVerdict, rebuild_assembly,
};
use nai_storage::{DecisionAck, DecisionKind};
use serde_json::{Value, json};

/// Why an assembly was refused, and the suspect ids that therefore did **not** reach the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblyRefusal {
    /// Rejection labels, in the order the judge produced them.
    pub rejections: Vec<&'static str>,
    /// Referenced ids that have no log entry at all.
    pub unlogged_ids: Vec<String>,
    /// Referenced ids logged without their body (only a pointer into live memory).
    pub contentless_ids: Vec<String>,
    /// Ids whose logged body carried a credential literal.
    pub credential_ids: Vec<String>,
}

/// The outcome of offering one assembly to the decision log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssemblyOutcome {
    /// Judged rebuildable; appended as one `assembly` entry.
    Logged(DecisionAck),
    /// Refused by name; nothing was appended.
    Refused(AssemblyRefusal),
}

/// The verdict of rebuilding one assembly **from the persisted chain**. Owned, because the log
/// rows it was derived from are local to the call and dropped when it returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblyRebuild {
    /// True only when the log alone reproduces the recorded assembly.
    pub rebuildable: bool,
    /// Rejection labels, in the order the judge produced them.
    pub rejections: Vec<&'static str>,
    /// The rebuilt sequence as `(id, source_ref)`, in the log's own order.
    pub rebuilt: Vec<(String, String)>,
    /// Referenced ids that have no log entry at all.
    pub unlogged_ids: Vec<String>,
    /// Referenced ids logged without their body.
    pub contentless_ids: Vec<String>,
    /// Ids whose logged body carried a credential literal.
    pub credential_ids: Vec<String>,
}

impl Composition {
    /// Judge `record` against `fragments` and, only when it is rebuildable, append it to the
    /// append-only decision log as one `assembly` entry.
    ///
    /// The appended payload is the judged material and nothing else — each referenced fragment's
    /// id, source ref and body **in the record's entry order**, plus the two versions. See the
    /// module docs for why no pointer into live memory may be stored here.
    pub fn record_assembly(
        &mut self,
        record: &AssemblyRecord<'_>,
        fragments: &[LoggedFragment<'_>],
    ) -> Result<AssemblyOutcome> {
        let verdict = rebuild_assembly(record, fragments);
        if !verdict.rebuildable {
            return Ok(AssemblyOutcome::Refused(refusal_of(&verdict)));
        }
        let payload = json!({
            "fragments": ordered_fragments(record, fragments),
            "model_version": record.model_version,
            "prompt_version": record.prompt_version,
        });
        let session = self.session_id().to_owned();
        let trace = self.next_trace("assembly");
        let ack = self.record(DecisionKind::Assembly, Some(&session), &payload, &trace)?;
        Ok(AssemblyOutcome::Logged(ack))
    }

    /// Rebuild one assembly **from the persisted chain alone**: read back the `assembly` entries,
    /// decode the fragments in the **log's own order**, and hand them to the same judgement the
    /// write path used.
    ///
    /// Read-only: it never appends, repairs or reorders the chain. The judge is *called*, not
    /// re-written, so the two faces cannot drift apart.
    pub fn rebuild_assembly_from_log(
        &self,
        record: &AssemblyRecord<'_>,
        limit: u32,
    ) -> Result<AssemblyRebuild> {
        let operation = short()?;
        let owned = {
            let store = self
                .store()
                .try_lock()
                .map_err(|_| err(ErrorCode::Conflict, "decision store busy"))?;
            let rows = store.decision_entries(&operation, self.workspace(), limit)?;
            let mut owned = Vec::new();
            for row in &rows {
                if row.kind != DecisionKind::Assembly.as_str() {
                    continue;
                }
                owned.extend(decode_fragments(&row.payload_json)?);
            }
            owned
        };
        let logged: Vec<LoggedFragment<'_>> = owned
            .iter()
            .map(|fragment| LoggedFragment {
                id: &fragment.id,
                source_ref: &fragment.source_ref,
                content: fragment.content.as_deref(),
            })
            .collect();
        let verdict = rebuild_assembly(record, &logged);
        Ok(AssemblyRebuild {
            rebuildable: verdict.rebuildable,
            rejections: verdict
                .rejections
                .iter()
                .map(|reason| reason.label())
                .collect(),
            rebuilt: verdict
                .rebuilt
                .iter()
                .map(|fragment| (fragment.id.to_string(), fragment.source_ref.to_string()))
                .collect(),
            unlogged_ids: verdict.unlogged_ids,
            contentless_ids: verdict.contentless_ids,
            credential_ids: verdict.credential_ids,
        })
    }
}

/// The judge's refusal, in the runtime's own shape.
fn refusal_of(verdict: &RebuildVerdict<'_>) -> AssemblyRefusal {
    AssemblyRefusal {
        rejections: verdict
            .rejections
            .iter()
            .map(|reason| reason.label())
            .collect(),
        unlogged_ids: verdict.unlogged_ids.clone(),
        contentless_ids: verdict.contentless_ids.clone(),
        credential_ids: verdict.credential_ids.clone(),
    }
}

/// The fragments to persist, **in the record's entry order**. `rebuildable` guarantees every
/// referenced id is present with a body, so a missing lookup here is an internal invariant
/// violation, not a caller error.
fn ordered_fragments(record: &AssemblyRecord<'_>, fragments: &[LoggedFragment<'_>]) -> Vec<Value> {
    record
        .fragment_ids
        .iter()
        .map(|id| {
            let fragment = fragments
                .iter()
                .find(|fragment| fragment.id == *id)
                .expect("a rebuildable verdict guarantees every referenced id is logged");
            json!({
                "id": fragment.id,
                "source_ref": fragment.source_ref,
                "content": fragment.content,
            })
        })
        .collect()
}

/// A `LoggedFragment` decoded from a persisted `assembly` row, owning its strings.
struct OwnedFragment {
    id: String,
    source_ref: String,
    content: Option<String>,
}

/// Decode one persisted `assembly` payload into its fragments, in stored (entry) order.
///
/// A row that is not decodable is a **classified failure**, never a silent skip: the write path
/// only ever persisted this shape, so anything else is corruption and MUST be reported.
fn decode_fragments(payload_json: &str) -> Result<Vec<OwnedFragment>> {
    let value: Value = serde_json::from_str(payload_json)
        .map_err(|_| err(ErrorCode::Internal, "assembly log row is not JSON"))?;
    let entries = value
        .get("fragments")
        .and_then(Value::as_array)
        .ok_or_else(|| err(ErrorCode::Internal, "assembly log row carries no fragments"))?;
    entries
        .iter()
        .map(|entry| {
            let id = entry
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| err(ErrorCode::Internal, "assembly fragment has no id"))?;
            let source_ref = entry
                .get("source_ref")
                .and_then(Value::as_str)
                .ok_or_else(|| err(ErrorCode::Internal, "assembly fragment has no source_ref"))?;
            Ok(OwnedFragment {
                id: id.to_string(),
                source_ref: source_ref.to_string(),
                content: entry
                    .get("content")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            })
        })
        .collect()
}
