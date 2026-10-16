//! Assembly runtime wiring: judge a context assembly on **both** halves of §41, append its four
//! §41.5 items to the real append-only hash chain, and read the rows back (V3.11 §41.1–§41.5;
//! W104 wired H02, W106 wires H01).
//!
//! `nai_kernel::evaluate::assembly` decides two things: whether an assembly **may be called**
//! (`evaluate_assembly`, §41.1–§41.3) and whether the log alone can **rebuild** it
//! (`rebuild_assembly`, §41.5). Its module docs state the boundary — feeding the model, appending
//! the record to the chain, and the §47.2 regression classes are runtime work.
//!
//! W104 wired part of that sentence: `record_assembly` reached the chain, but only **three** of
//! §41.5's four items landed (`fragments`, `model_version`, `prompt_version`). 「各段预算占用」 and
//! 「裁剪项与原因」live on the *plan*, and the entry point did not take one — so `evaluate_assembly`
//! had no caller in production code and §41.1's "an uncroppable segment is missing ⇒ refuse the
//! call" was not expressible at all. This module now takes the plan too: both judgements run, and
//! all four items land.
//!
//! Invariants, all testable:
//!
//! * **Both judgements run; neither short-circuits.** An assembly that is neither callable nor
//!   rebuildable reports both kinds of reason — "why was this refused" is not served by the first
//!   hit alone (the same discipline `evaluate_assembly` states for its own checks).
//! * **Three refusals, not one.** `NotCallable` (§41.1 forbids *calling* it), `NotRebuildable`
//!   (§41.5 forbids *claiming* it is reconstructible) and `Neither` are different consequences;
//!   folding any two together loses the other side's names.
//! * **The payload is the judged material.** The runtime is a transport, not an enricher: usage
//!   and crops are copied from the plan the judge saw, never recomputed from the fragment bodies,
//!   and fields the judge never looked at do not land.
//! * **The two inputs must describe one assembly.** §41.5's four items are four views of the
//!   *same* call, so a plan and a record naming different fragments are a caller error, not a
//!   verdict — there is no arm of [`AssemblyOutcome`] that could carry it without lying about
//!   which judgement refused.
//!
//! Judgement and append are one entry point on purpose. Offering a separate `evaluate` and
//! `append` would move the "never append an unjudged assembly" duty onto every caller, where one
//! omission breaks the invariant and no test can see it.
//!
//! [`Composition::rebuild_assembly_from_log`] is the read half: it reads the chain back and hands
//! **each row** to the same judgement, as its own entry. The judge is *called*, not re-written, so
//! "which rule refused this" stays decidable in one place; and keeping the rows apart is what makes
//! the question "can the chain rebuild *this* assembly" rather than "can the chain supply these
//! fragments from somewhere".

use crate::composition::short;
use crate::{Composition, Result, err};
use nai_contracts::v1::ErrorCode;
use nai_kernel::evaluate::assembly::{
    AssemblyPlan, AssemblyRecord, AssemblyVerdict, LoggedFragment, RebuildVerdict,
    evaluate_assembly, rebuild_assembly, rebuild_assembly_from_entries,
};
use nai_storage::{DecisionAck, DecisionKind};
use serde_json::{Value, json};

/// Why an assembly was refused, and the suspect ids that therefore did **not** reach the log.
///
/// Carries **both** judgements' labels: an assembly can be refused by §41.1–§41.3, by §41.5, or by
/// both, and collapsing the two lists would make "which half refused this" undecidable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblyRefusal {
    /// §41.1–§41.3 rejection labels (`assembly.*`), in the order the judge produced them.
    pub rejections: Vec<&'static str>,
    /// §41.5 rebuild rejection labels (`rebuild.*`), in the order the judge produced them.
    pub rebuild_rejections: Vec<&'static str>,
    /// §41.1 uncroppable segments the plan was missing, as segment labels.
    pub missing_segments: Vec<&'static str>,
    /// Referenced ids that have no log entry at all.
    pub unlogged_ids: Vec<String>,
    /// Referenced ids logged without their body (only a pointer into live memory).
    pub contentless_ids: Vec<String>,
    /// Ids whose logged body carried a credential literal.
    pub credential_ids: Vec<String>,
}

/// The outcome of offering one assembly to the decision log.
///
/// Four arms, because the two judgements have four combinations and every one has a different
/// consequence. A caller that sees `NotCallable` MUST NOT call the model; one that sees
/// `NotRebuildable` MUST NOT claim the row is reconstructible; `Neither` says both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssemblyOutcome {
    /// Both judgements let it through; appended as one `assembly` entry.
    Logged(DecisionAck),
    /// §41.1–§41.3 refused it (§41.1: an assembly missing an uncroppable segment MUST NOT be used
    /// to call the model). Nothing was appended.
    NotCallable(AssemblyRefusal),
    /// §41.5 refused it: the log alone does not reproduce this assembly. Nothing was appended.
    NotRebuildable(AssemblyRefusal),
    /// Both refused it. Both sets of reasons are in the refusal, and nothing was appended.
    Neither(AssemblyRefusal),
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

/// One fragment of a row read back from the chain, owning its strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggedAssemblyFragment {
    pub id: String,
    pub source_ref: String,
    pub content: Option<String>,
}

/// §41.5 「各段预算占用」 as it was logged: one segment's three usage dimensions and its own quota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggedSegmentUsage {
    pub segment: String,
    pub tokens: u64,
    pub chars: u64,
    pub items: u64,
    /// The segment's own quota, or `None` when it carried none.
    pub budget: Option<LoggedSegmentBudget>,
}

/// §41.2's per-segment quota as it was logged. Every dimension is optional, matching the judge's
/// input type — "this dimension was not recorded" stays expressible on the way back out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggedSegmentBudget {
    pub tokens: Option<u64>,
    pub chars: Option<u64>,
    pub items: Option<u64>,
    pub floor: Option<u64>,
}

/// §41.5 「裁剪项与原因」 as it was logged — both halves, per segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggedCrop {
    pub segment: String,
    pub dropped: Vec<String>,
    pub reason: String,
}

/// One `assembly` row read back from the chain: exactly §41.5's four items, **per row**.
///
/// Row by row, not pooled: §41.5's 「各段预算占用」 belongs to *one* assembly, so merging rows
/// would leave the usage with no assembly to belong to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggedAssembly {
    /// 进入的片段标识 (with their source refs and bodies), in the row's stored order.
    pub fragments: Vec<LoggedAssemblyFragment>,
    /// 各段预算占用. Empty for a row logged before W106 added it (not an error — see
    /// `decode_segment_usage`).
    pub segment_usage: Vec<LoggedSegmentUsage>,
    /// 裁剪项与原因. Empty for a row logged before W106 added it.
    pub crops: Vec<LoggedCrop>,
    /// 本次调用的模型版本.
    pub model_version: String,
    /// 本次调用的提示词版本.
    pub prompt_version: String,
}

impl Composition {
    /// Judge `plan` and `record` and, only when **both** judgements let them through, append the
    /// judged material to the append-only decision log as one `assembly` entry.
    ///
    /// The appended payload is the judged material and nothing else — §41.5's four items:
    /// each referenced fragment's id, source ref and body **in the record's entry order**, the
    /// per-segment usage and quotas, the crops with their reasons, and the two versions. See the
    /// module docs for why a body that only lived in memory may not be stored here, and why usage
    /// is copied rather than recomputed.
    ///
    /// Returns `Err` (not a refusal arm) when the two inputs do not name the same fragments in the
    /// same order: that is a caller error, and §41.5's four items would contradict each other if
    /// such a payload were written.
    pub fn record_assembly(
        &mut self,
        plan: &AssemblyPlan<'_>,
        record: &AssemblyRecord<'_>,
        fragments: &[LoggedFragment<'_>],
    ) -> Result<AssemblyOutcome> {
        // §41.5's four items describe **one** assembly, so the plan's fragments and the record's
        // entry order must be the same sequence — not merely the same set (entry order is itself
        // part of item one, and H02 keys a rejection on it).
        if plan_fragment_ids(plan).as_slice() != record.fragment_ids {
            return Err(err(
                ErrorCode::InvalidArgument,
                "assembly.input_ids_differ: the plan and the record do not name the same fragments",
            ));
        }

        // Both judgements run, and neither short-circuits the other: a refusal that reported only
        // the first half would hide the second half's reasons from the caller.
        let callable = evaluate_assembly(plan);
        let rebuild = rebuild_assembly(record, fragments);

        if !callable.callable || !rebuild.rebuildable {
            let refusal = refusal_of(&callable, &rebuild);
            return Ok(match (callable.callable, rebuild.rebuildable) {
                (false, false) => AssemblyOutcome::Neither(refusal),
                (true, false) => AssemblyOutcome::NotRebuildable(refusal),
                (false, true) => AssemblyOutcome::NotCallable(refusal),
                (true, true) => unreachable!("guarded by the enclosing condition"),
            });
        }

        let payload = assembly_payload(plan, record, fragments);
        let session = self.session_id().to_owned();
        let trace = self.next_trace("assembly");
        let ack = self.record(DecisionKind::Assembly, Some(&session), &payload, &trace)?;
        Ok(AssemblyOutcome::Logged(ack))
    }

    /// Rebuild one assembly **from the persisted chain alone**: read back the `assembly` entries,
    /// decode each entry's fragments in **that entry's own order**, and hand each entry to the same
    /// judgement the write path used.
    ///
    /// Read-only: it never appends, repairs or reorders the chain. The judge is *called*, not
    /// re-written, so the two faces cannot drift apart.
    ///
    /// Each row is passed as **its own entry**, never merged. One entry is one assembly — §41.5's
    /// four items describe a single assembly and the write path appends them together — so a record
    /// whose halves sit in two different rows is **not** rebuildable: no entry ever assembled it.
    /// Pooling the rows first (the W104 shape) answered a weaker question and reported such a
    /// record as rebuildable; that is now `rebuild.not_in_one_entry`.
    pub fn rebuild_assembly_from_log(
        &self,
        record: &AssemblyRecord<'_>,
        limit: u32,
    ) -> Result<AssemblyRebuild> {
        let rows: Vec<Vec<LoggedAssemblyFragment>> = self
            .read_logged_assemblies(limit)?
            .into_iter()
            .map(|row| row.fragments)
            .collect();
        let entries: Vec<Vec<LoggedFragment<'_>>> = rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|fragment| LoggedFragment {
                        id: &fragment.id,
                        source_ref: &fragment.source_ref,
                        content: fragment.content.as_deref(),
                    })
                    .collect()
            })
            .collect();
        let entry_refs: Vec<&[LoggedFragment<'_>]> =
            entries.iter().map(|entry| entry.as_slice()).collect();
        let verdict = rebuild_assembly_from_entries(record, &entry_refs);
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

    /// Read every `assembly` row back from the chain, **one row per assembly**.
    ///
    /// Read-only: it never appends, repairs or reorders the chain. A row that is not decodable is a
    /// **classified failure**, never a silent skip — skipping would make "read back N rows"
    /// indistinguishable from "the chain had N rows".
    pub fn read_logged_assemblies(&self, limit: u32) -> Result<Vec<LoggedAssembly>> {
        let operation = short()?;
        let store = self
            .store()
            .try_lock()
            .map_err(|_| err(ErrorCode::Conflict, "decision store busy"))?;
        let rows = store.decision_entries(&operation, self.workspace(), limit)?;
        rows.iter()
            .filter(|row| row.kind == DecisionKind::Assembly.as_str())
            .map(|row| decode_assembly(&row.payload_json))
            .collect()
    }
}

/// §41.1–§41.3 and §41.5 refusals, in the runtime's own shape. Both judges are read exactly once
/// each; nothing is inferred from the other's result.
fn refusal_of(callable: &AssemblyVerdict<'_>, rebuild: &RebuildVerdict<'_>) -> AssemblyRefusal {
    AssemblyRefusal {
        rejections: callable
            .rejections
            .iter()
            .map(|reason| reason.label())
            .collect(),
        rebuild_rejections: rebuild
            .rejections
            .iter()
            .map(|reason| reason.label())
            .collect(),
        missing_segments: callable
            .missing_segments
            .iter()
            .map(|segment| segment.label())
            .collect(),
        unlogged_ids: rebuild.unlogged_ids.clone(),
        contentless_ids: rebuild.contentless_ids.clone(),
        credential_ids: rebuild.credential_ids.clone(),
    }
}

/// The fragment ids the plan would assemble, in **assembly order** (segment order, then each
/// segment's own order). Compared against the record's declared entry order by the entry point.
fn plan_fragment_ids<'plan>(plan: &'plan AssemblyPlan<'plan>) -> Vec<&'plan str> {
    plan.slices
        .iter()
        .flat_map(|slice| slice.fragments.iter().map(|fragment| fragment.id))
        .collect()
}

/// §41.5's four items, and nothing else.
///
/// `fragments` is item one; `segment_usage` is item two; `crops` is item three; the two versions are
/// item four. No deliberate field of the plan lands beyond these: the judge looked at each fragment's
/// provenance, scope and trust tag, but §41.5 does not list them, so they stay out.
fn assembly_payload(
    plan: &AssemblyPlan<'_>,
    record: &AssemblyRecord<'_>,
    fragments: &[LoggedFragment<'_>],
) -> Value {
    json!({
        "fragments": ordered_fragments(record, fragments),
        "segment_usage": segment_usage(plan),
        "crops": crop_records(plan),
        "model_version": record.model_version,
        "prompt_version": record.prompt_version,
    })
}

/// §41.5 「各段预算占用」: one entry per offered segment, **copied** from the plan the judge saw.
///
/// Copied, not recomputed: the runtime has the fragment bodies and could re-derive character
/// counts, but §41.2's discipline is that the *caller's* numbers are what the judgement was made
/// on, and a transport that quietly corrected them would be writing a different account than the
/// one that was audited.
fn segment_usage(plan: &AssemblyPlan<'_>) -> Vec<Value> {
    plan.slices
        .iter()
        .map(|slice| {
            json!({
                "segment": slice.segment,
                "tokens": slice.usage.tokens,
                "chars": slice.usage.chars,
                "items": slice.usage.items,
                "budget": slice.budget.map(|budget| json!({
                    "tokens": budget.tokens,
                    "chars": budget.chars,
                    "items": budget.items,
                    "floor": budget.floor,
                })),
            })
        })
        .collect()
}

/// §41.5 「裁剪项与原因」: one entry per **cropped** segment, carrying both halves.
fn crop_records(plan: &AssemblyPlan<'_>) -> Vec<Value> {
    plan.slices
        .iter()
        .filter_map(|slice| {
            slice.crop.as_ref().map(|crop| {
                json!({
                    "segment": slice.segment,
                    "dropped": crop.dropped,
                    "reason": crop.reason,
                })
            })
        })
        .collect()
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

/// Decode one persisted `assembly` payload into §41.5's four items, in stored (entry) order.
///
/// A row that is not decodable is a **classified failure**, never a silent skip: the write path
/// only ever persisted this shape, so anything else is corruption and MUST be reported.
fn decode_assembly(payload_json: &str) -> Result<LoggedAssembly> {
    let value: Value = serde_json::from_str(payload_json)
        .map_err(|_| err(ErrorCode::Internal, "assembly log row is not JSON"))?;
    Ok(LoggedAssembly {
        fragments: decode_fragments(&value)?,
        segment_usage: decode_segment_usage(&value)?,
        crops: decode_crops(&value)?,
        model_version: required_str(&value, "model_version")?,
        prompt_version: required_str(&value, "prompt_version")?,
    })
}

/// The fragment entries of one row. Absent or non-array `fragments` is corruption, not an empty set
/// — every row this write path produced carries at least the `fragments` key.
fn decode_fragments(value: &Value) -> Result<Vec<LoggedAssemblyFragment>> {
    let entries = value
        .get("fragments")
        .and_then(Value::as_array)
        .ok_or_else(|| err(ErrorCode::Internal, "assembly log row carries no fragments"))?;
    entries
        .iter()
        .map(|entry| {
            Ok(LoggedAssemblyFragment {
                id: required_str(entry, "id")?,
                source_ref: required_str(entry, "source_ref")?,
                content: entry
                    .get("content")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            })
        })
        .collect()
}

/// §41.5 「各段预算占用」 read back.
///
/// A row logged **before** this item existed carries no `segment_usage` key at all, and that is
/// `Ok(vec![])`: rows written by W104 are still readable, and their two missing items read as
/// empty rather than as corruption. A key that is *present* but not an array is corruption.
fn decode_segment_usage(value: &Value) -> Result<Vec<LoggedSegmentUsage>> {
    let Some(entries) = value.get("segment_usage") else {
        return Ok(Vec::new());
    };
    let entries = entries.as_array().ok_or_else(|| {
        err(
            ErrorCode::Internal,
            "assembly log row has a non-array segment_usage",
        )
    })?;
    entries
        .iter()
        .map(|entry| {
            let budget = match entry.get("budget").filter(|budget| !budget.is_null()) {
                None => None,
                Some(budget) => Some(LoggedSegmentBudget {
                    tokens: optional_u64(budget, "tokens")?,
                    chars: optional_u64(budget, "chars")?,
                    items: optional_u64(budget, "items")?,
                    floor: optional_u64(budget, "floor")?,
                }),
            };
            Ok(LoggedSegmentUsage {
                segment: required_str(entry, "segment")?,
                tokens: required_u64(entry, "tokens")?,
                chars: required_u64(entry, "chars")?,
                items: required_u64(entry, "items")?,
                budget,
            })
        })
        .collect()
}

/// §41.5 「裁剪项与原因」 read back, with the same "absent means a pre-W106 row" rule as usage.
fn decode_crops(value: &Value) -> Result<Vec<LoggedCrop>> {
    let Some(entries) = value.get("crops") else {
        return Ok(Vec::new());
    };
    let entries = entries.as_array().ok_or_else(|| {
        err(
            ErrorCode::Internal,
            "assembly log row has a non-array crops",
        )
    })?;
    entries
        .iter()
        .map(|entry| {
            let dropped = entry
                .get("dropped")
                .and_then(Value::as_array)
                .ok_or_else(|| err(ErrorCode::Internal, "assembly crop has no dropped items"))?;
            Ok(LoggedCrop {
                segment: required_str(entry, "segment")?,
                dropped: dropped
                    .iter()
                    .map(|item| {
                        item.as_str().map(str::to_string).ok_or_else(|| {
                            err(ErrorCode::Internal, "assembly crop item is not a string")
                        })
                    })
                    .collect::<Result<Vec<String>>>()?,
                reason: required_str(entry, "reason")?,
            })
        })
        .collect()
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
                "assembly log row is missing a string field",
            )
        })
}

/// A count this write path always persisted. Absence is corruption.
fn required_u64(value: &Value, key: &str) -> Result<u64> {
    value.get(key).and_then(Value::as_u64).ok_or_else(|| {
        err(
            ErrorCode::Internal,
            "assembly log row is missing a count field",
        )
    })
}

/// A count §41.2 lets a segment leave unrecorded. Absence stays `None` — the judge's input type
/// makes "this dimension was not recorded" expressible, so the way back out must too.
fn optional_u64(value: &Value, key: &str) -> Result<Option<u64>> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(field) => field
            .as_u64()
            .map(Some)
            .ok_or_else(|| err(ErrorCode::Internal, "assembly budget field is not a count")),
    }
}
