//! W104 landed the H02 half of §41.5's runtime wiring: a judged-rebuildable assembly reaches the
//! append-only hash chain with its fragment bodies, a non-rebuildable one does not, and the
//! assembly can be rebuilt **from the chain alone**. W106 lands the H01 half: the §41.1–§41.3
//! judgement now has a production caller, so "an uncroppable segment is missing ⇒ refuse the call"
//! is expressible, and the two §41.5 items that live on the plan (「各段预算占用」and「裁剪项与原因」)
//! actually land — and can be read back row by row.
//!
//! Two non-void guards, one per judge: `every_assembly_rejection_is_reachable_through_the_runtime`
//! iterates `AssemblyRejection::ALL` (14) and `every_rebuild_rejection_is_reachable_through_the_runtime`
//! iterates `RebuildRejection::ALL` (6). Each per-variant sample constructor matches its enum
//! exhaustively, so adding a variant to the kernel without covering it here fails to compile
//! rather than passing invisibly.
mod common;
use common::*;
use nai_kernel::evaluate::assembly::{
    AssemblyPlan, AssemblyRecord, AssemblyRejection, Crop, Fragment, LoggedFragment,
    RebuildRejection, SEGMENT_ORDER, Segment, SegmentBudget, SegmentSlice, SegmentUsage,
    SourceKind, Trust,
};
use nai_kernel::evaluate::harness::CREDENTIAL_MARKERS;
use nai_runtime::{AssemblyOutcome, Composition};
use nai_storage::{DataPaths, DecisionEntryView};
use serde_json::Value;
use tempfile::tempdir;

const MODEL: &str = "model-1";
const PROMPT: &str = "prompt-1";
const SOURCE_REF: &str = "evidence-kb0";
const SCOPE: &str = "session-1";
const BODY: &str = "the fragment body that MUST be logged, not a pointer";

fn prepared(root: &std::path::Path) -> Composition {
    provision(root, WORKSPACE, ACTOR, PROFILE, &runtime_profile(WORKSPACE));
    Composition::prepare(
        DataPaths::at(root).unwrap(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        false,
    )
    .unwrap()
}

/// The record a producer would log: the referenced ids **in entry order** plus the two versions.
fn record<'a>(ids: &'a [&'a str]) -> AssemblyRecord<'a> {
    AssemblyRecord {
        fragment_ids: ids,
        model_version: MODEL,
        prompt_version: PROMPT,
    }
}

/// The log material for `ids`, each entry carrying a real body.
fn fragments<'a>(ids: &[&'a str]) -> Vec<LoggedFragment<'a>> {
    ids.iter()
        .copied()
        .map(|id| LoggedFragment {
            id,
            source_ref: SOURCE_REF,
            content: Some(BODY),
        })
        .collect()
}

/// The kernel fragments a legal plan offers: one per id, each carrying a source ref, a scope and
/// the data tag, so §41.3 has nothing to refuse.
fn plan_fragments<'a>(ids: &'a [&'a str]) -> Vec<Fragment<'a>> {
    ids.iter()
        .copied()
        .map(|id| Fragment {
            id,
            source: SourceKind::KnowledgeEvidence,
            source_ref: SOURCE_REF,
            scope: Some(SCOPE),
            age_ms: None,
            trust: Trust::Data,
        })
        .collect()
}

/// Nine slices in §41.1's fixed order: the three uncroppable ones carry a floor, every one carries
/// its own quota in both dimensions, and the offered ids sit in the evidence segment. Nothing is
/// cropped and nothing exceeds a limit, so this plan is exactly what §41.1–§41.3 let through.
///
/// Usage counts are indexed per segment (and therefore deliberately unrelated to the body length)
/// so that a payload which recomputes them instead of copying the judged input is visible.
fn nine_slices<'a>(offered: &'a [Fragment<'a>]) -> Vec<SegmentSlice<'a>> {
    SEGMENT_ORDER
        .iter()
        .copied()
        .enumerate()
        .map(|(index, segment)| {
            let evidence = segment == Segment::Evidence;
            SegmentSlice {
                segment: segment.label(),
                fragments: if evidence { offered } else { &[] },
                usage: SegmentUsage {
                    tokens: 100 + index as u64,
                    chars: 400 + index as u64,
                    items: if evidence { offered.len() } else { 0 },
                },
                budget: Some(SegmentBudget {
                    tokens: Some(1_000),
                    chars: Some(4_000),
                    items: Some(8),
                    floor: segment.uncroppable().then_some(1u64),
                }),
                crop: None,
            }
        })
        .collect()
}

/// Offer one assembly through the entry point with a **legal** nine-segment plan carrying `ids`.
/// The plan is built here so the borrow chain stays inside one scope.
fn offer(
    composition: &mut Composition,
    ids: &[&str],
    material: &[LoggedFragment<'_>],
) -> nai_runtime::Result<AssemblyOutcome> {
    let offered = plan_fragments(ids);
    let slices = nine_slices(&offered);
    let plan = AssemblyPlan {
        slices: &slices,
        model_version: MODEL,
        prompt_version: PROMPT,
    };
    composition.record_assembly(&plan, &record(ids), material)
}

/// Every persisted decision row, oldest first.
fn all_rows(composition: &Composition) -> Vec<DecisionEntryView> {
    let operation = store_operation();
    let store = composition.store().lock().unwrap();
    store
        .decision_entries(&operation, composition.workspace(), 500)
        .unwrap()
}

fn assembly_rows(composition: &Composition) -> Vec<DecisionEntryView> {
    all_rows(composition)
        .into_iter()
        .filter(|row| row.kind == "assembly")
        .collect()
}

/// The payload of the single `assembly` row present, parsed.
fn logged_payload(composition: &Composition) -> Value {
    let rows = assembly_rows(composition);
    assert_eq!(rows.len(), 1, "expected exactly one assembly row");
    serde_json::from_str(&rows[0].payload_json).expect("the payload is JSON")
}

/// An accepted offer MUST come back as `Logged`; any refusal arm is the failure under test.
fn expect_logged(outcome: AssemblyOutcome) -> nai_storage::DecisionAck {
    match outcome {
        AssemblyOutcome::Logged(ack) => ack,
        other => panic!("a callable, rebuildable assembly was refused: {other:?}"),
    }
}

/// A §41.5 refusal MUST come back as `NotRebuildable` **specifically** — a `NotCallable` here would
/// mean the legal plan was judged uncallable, which is a different failure.
fn expect_not_rebuildable(outcome: AssemblyOutcome) -> nai_runtime::AssemblyRefusal {
    match outcome {
        AssemblyOutcome::NotRebuildable(refusal) => refusal,
        other => panic!("expected a rebuild refusal, got: {other:?}"),
    }
}

/// A refusal of **either** kind, for the assertions that do not care which half refused.
fn expect_any_refusal(outcome: AssemblyOutcome) -> nai_runtime::AssemblyRefusal {
    match outcome {
        AssemblyOutcome::NotCallable(refusal)
        | AssemblyOutcome::NotRebuildable(refusal)
        | AssemblyOutcome::Neither(refusal) => refusal,
        AssemblyOutcome::Logged(ack) => panic!(
            "an assembly that must not be logged came back as Logged (persisted={}, hash={:?})",
            ack.persisted, ack.entry_hash
        ),
    }
}

fn object_keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    keys
}

// ---------------------------------------------------------------------------
// §41.5's four items land (W106)
// ---------------------------------------------------------------------------

/// A callable, rebuildable assembly is appended as one `assembly` entry whose payload is **exactly**
/// §41.5's four items — five top-level keys and no sixth. The key-set assertion is what keeps the
/// runtime a transport: it has the fragment bodies and the plan in hand, and could quietly add
/// fields the judge never looked at.
#[test]
fn a_legal_plan_lands_all_four_items_of_the_payload() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let ids = ["frag-a", "frag-b"];
    let ack = expect_logged(offer(&mut composition, &ids, &fragments(&ids)).unwrap());
    assert!(
        ack.persisted,
        "a callable, rebuildable assembly must persist"
    );
    assert!(ack.entry_hash.is_some(), "a persisted row carries its hash");

    let payload = logged_payload(&composition);
    assert_eq!(
        object_keys(&payload),
        vec![
            "crops",
            "fragments",
            "model_version",
            "prompt_version",
            "segment_usage"
        ],
        "the payload is §41.5's four items and nothing else"
    );

    // Item one: the entry order, with bodies — not pointers.
    let entries = payload["fragments"]
        .as_array()
        .expect("fragments is an array");
    assert_eq!(entries.len(), 2);
    for entry in entries {
        assert_eq!(
            object_keys(entry),
            vec!["content", "id", "source_ref"],
            "a fragment entry carries the id, the source ref and the literal body"
        );
        assert_eq!(entry["content"].as_str(), Some(BODY));
        assert_eq!(entry["source_ref"].as_str(), Some(SOURCE_REF));
    }
    assert_eq!(entries[0]["id"].as_str(), Some("frag-a"));
    assert_eq!(entries[1]["id"].as_str(), Some("frag-b"));

    // Item two: per-segment usage, one entry per offered segment, copied from the judged plan.
    let usage = payload["segment_usage"]
        .as_array()
        .expect("segment_usage is an array");
    assert_eq!(
        usage.len(),
        SEGMENT_ORDER.len(),
        "every segment reports its usage"
    );
    for (index, entry) in usage.iter().enumerate() {
        assert_eq!(
            object_keys(entry),
            vec!["budget", "chars", "items", "segment", "tokens"],
        );
        assert_eq!(
            entry["segment"].as_str(),
            Some(SEGMENT_ORDER[index].label())
        );
        assert_eq!(
            entry["tokens"].as_u64(),
            Some(100 + index as u64),
            "usage is the judged input, not a recomputation from the fragment bodies"
        );
        assert_eq!(entry["chars"].as_u64(), Some(400 + index as u64));
        let budget = entry["budget"]
            .as_object()
            .expect("every segment carries a budget");
        assert_eq!(budget["tokens"].as_u64(), Some(1_000));
        assert_eq!(budget["chars"].as_u64(), Some(4_000));
        assert_eq!(
            budget["floor"].as_u64().is_some(),
            SEGMENT_ORDER[index].uncroppable(),
            "only the uncroppable segments carry a floor (§41.2)"
        );
    }

    // Item three: no crop happened, so the crops list is present and empty.
    assert_eq!(
        payload["crops"].as_array().map(Vec::len),
        Some(0),
        "an uncropped assembly logs an empty crops list, not a missing key"
    );

    // Item four.
    assert_eq!(payload["model_version"].as_str(), Some(MODEL));
    assert_eq!(payload["prompt_version"].as_str(), Some(PROMPT));

    verify_chain(&composition);
}

/// The payload must not carry fields the judge never looked at. §41.5 names four items; a fragment's
/// age, its data/instruction tag and the judge's diagnostic lists are none of them.
#[test]
fn the_payload_carries_no_field_the_judge_did_not_audit() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let ids = ["frag-a"];
    expect_logged(offer(&mut composition, &ids, &fragments(&ids)).unwrap());
    let payload = logged_payload(&composition);

    for absent in [
        "age_ms",
        "trust",
        "scope",
        "source",
        "missing_segments",
        "over_budget",
        "budget_missing",
        "unprovenanced_fragments",
        "unscoped_fragments",
        "untrusted_as_instruction",
        "rebuild_rejections",
    ] {
        assert!(
            payload.get(absent).is_none(),
            "the payload MUST NOT carry `{absent}`: §41.5 does not list it"
        );
    }
    assert!(payload["fragments"][0].get("trust").is_none());
    verify_chain(&composition);
}

/// A cropped segment lands **both** halves of 「裁剪项与原因」, per segment, and its usage is the
/// judged one.
#[test]
fn a_cropped_segment_lands_its_dropped_items_and_its_reason() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let ids = ["frag-a"];
    let offered = plan_fragments(&ids);
    let mut slices = nine_slices(&offered);
    // §41.1: cropping runs from the back, so the last two croppable segments carry the crop.
    let dropped = ["utterance-1", "utterance-2"];
    for index in [7, 8] {
        slices[index].crop = Some(Crop {
            dropped: &dropped,
            reason: "budget pressure in §41.2",
        });
    }
    let plan = AssemblyPlan {
        slices: &slices,
        model_version: MODEL,
        prompt_version: PROMPT,
    };
    expect_logged(
        composition
            .record_assembly(&plan, &record(&ids), &fragments(&ids))
            .unwrap(),
    );

    let payload = logged_payload(&composition);
    let crops = payload["crops"].as_array().expect("crops is an array");
    assert_eq!(crops.len(), 2, "one entry per cropped segment");
    for (entry, segment) in crops.iter().zip(["recent_utterances", "tool_results"]) {
        assert_eq!(object_keys(entry), vec!["dropped", "reason", "segment"]);
        assert_eq!(entry["segment"].as_str(), Some(segment));
        assert_eq!(
            entry["dropped"]
                .as_array()
                .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>()),
            Some(vec!["utterance-1", "utterance-2"]),
            "the crop names what was dropped"
        );
        assert_eq!(
            entry["reason"].as_str(),
            Some("budget pressure in §41.2"),
            "and why — both halves, not one"
        );
    }

    // The cropped segments still report their usage (§41.5 item two is per segment, cropped or not).
    let usage = payload["segment_usage"].as_array().unwrap();
    assert_eq!(usage[7]["tokens"].as_u64(), Some(107));
    assert_eq!(usage[8]["tokens"].as_u64(), Some(108));
    verify_chain(&composition);
}

// ---------------------------------------------------------------------------
// H01 reaches the runtime (W106)
// ---------------------------------------------------------------------------

/// §41.1's hard rule now has a runtime expression: an assembly missing an uncroppable segment is
/// refused **by name** and appends nothing. Before this wiring the judgement had no caller, so no
/// runtime shape could express it at all.
#[test]
fn an_assembly_missing_an_uncroppable_segment_is_refused_and_appends_nothing() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let ids = ["frag-a"];
    let offered = plan_fragments(&ids);
    let mut slices = nine_slices(&offered);
    // Drop §41.1's second row (人设、场景口吻与禁答范围) — an uncroppable segment.
    slices.remove(1);
    let plan = AssemblyPlan {
        slices: &slices,
        model_version: MODEL,
        prompt_version: PROMPT,
    };

    let before = assembly_rows(&composition).len();
    let refusal = expect_any_refusal(
        composition
            .record_assembly(&plan, &record(&ids), &fragments(&ids))
            .unwrap(),
    );
    assert!(
        refusal
            .rejections
            .contains(&AssemblyRejection::UncroppableSegmentMissing.label()),
        "the refusal names the §41.1 rule: {:?}",
        refusal.rejections
    );
    assert_eq!(
        refusal.missing_segments,
        vec!["persona"],
        "and names which segment was missing"
    );
    assert_eq!(
        assembly_rows(&composition).len(),
        before,
        "a refusal appends nothing"
    );
    verify_chain(&composition);
}

/// Neither judgement short-circuits the other: an assembly that is neither callable nor
/// rebuildable reports **both** families of reason. Reporting only the first half would hide the
/// other's reasons from the caller.
#[test]
fn an_assembly_refused_by_both_judgements_names_both_kinds_of_reason() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let ids = ["frag-a", "frag-b"];
    let offered = plan_fragments(&ids);
    let mut slices = nine_slices(&offered);
    // §41.1: cropping must not reach a front segment — the evidence segment is croppable but a crop
    // on it while the later segments stay intact is a `crop_from_front`.
    let dropped = ["evidence-1"];
    slices[4].crop = Some(Crop {
        dropped: &dropped,
        reason: "claimed pressure",
    });
    let plan = AssemblyPlan {
        slices: &slices,
        model_version: MODEL,
        prompt_version: PROMPT,
    };

    // H02 refuses too: `frag-b`'s body is not in the offered material.
    let before = assembly_rows(&composition).len();
    let outcome = composition
        .record_assembly(&plan, &record(&ids), &fragments(&["frag-a"]))
        .unwrap();
    let refusal = match outcome {
        AssemblyOutcome::Neither(refusal) => refusal,
        other => panic!("both judgements refuse here, so the arm is Neither: {other:?}"),
    };
    assert!(
        refusal
            .rejections
            .contains(&AssemblyRejection::CropFromFront.label()),
        "the §41.1 reason is named: {:?}",
        refusal.rejections
    );
    assert!(
        refusal
            .rebuild_rejections
            .contains(&RebuildRejection::FragmentNotLogged.label()),
        "and so is the §41.5 reason: {:?}",
        refusal.rebuild_rejections
    );
    assert_eq!(
        assembly_rows(&composition).len(),
        before,
        "a refusal appends nothing"
    );
    verify_chain(&composition);
}

/// The three refusals are three values: an assembly refused only by §41.1, one refused only by
/// §41.5, and one refused by both are pairwise distinguishable. Folding `Neither` into either side
/// would drop the other side's names.
#[test]
fn the_three_refusals_are_pairwise_distinguishable() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let ids = ["frag-a"];

    // Only §41.1 refuses: the plan is missing an uncroppable segment, the log material is fine.
    let offered = plan_fragments(&ids);
    let mut slices = nine_slices(&offered);
    slices.remove(2);
    let plan = AssemblyPlan {
        slices: &slices,
        model_version: MODEL,
        prompt_version: PROMPT,
    };
    let not_callable = composition
        .record_assembly(&plan, &record(&ids), &fragments(&ids))
        .unwrap();

    // Only §41.5 refuses: the plan is legal, the body only lived in memory.
    let mut material = fragments(&ids);
    material[0].content = None;
    let not_rebuildable = offer(&mut composition, &ids, &material).unwrap();

    // Both refuse.
    let both = composition
        .record_assembly(&plan, &record(&ids), &material)
        .unwrap();

    let arms = [&not_callable, &not_rebuildable, &both];
    for (left, right) in [(0, 1), (0, 2), (1, 2)] {
        assert_ne!(
            std::mem::discriminant(arms[left]),
            std::mem::discriminant(arms[right]),
            "refusal {left} and refusal {right} MUST NOT collapse into one value"
        );
    }
    assert!(matches!(not_callable, AssemblyOutcome::NotCallable(_)));
    assert!(matches!(
        not_rebuildable,
        AssemblyOutcome::NotRebuildable(_)
    ));
    assert!(matches!(both, AssemblyOutcome::Neither(_)));
    verify_chain(&composition);
}

/// §41.5's four items are four views of **one** call, so a plan and a record naming different
/// fragments are a caller error — not a verdict. No arm of [`AssemblyOutcome`] could carry it
/// without lying about which judgement refused.
#[test]
fn inputs_that_name_different_fragments_are_a_caller_error() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let declared = ["frag-a", "frag-b"];
    let offered = ["frag-a", "frag-c"];

    // The plan is built from `offered`, the record declares `declared`.
    let offered_fragments = plan_fragments(&offered);
    let slices = nine_slices(&offered_fragments);
    let plan = AssemblyPlan {
        slices: &slices,
        model_version: MODEL,
        prompt_version: PROMPT,
    };

    let before = assembly_rows(&composition).len();
    let error = composition
        .record_assembly(&plan, &record(&declared), &fragments(&offered))
        .expect_err("disagreeing inputs are a caller error");
    assert!(
        error.message.contains("assembly.input_ids_differ"),
        "the error is named: {error}"
    );
    assert_eq!(
        assembly_rows(&composition).len(),
        before,
        "a caller error appends nothing"
    );

    // The same fragments in a different order is still a disagreement: entry order is part of
    // §41.5's first item, so equal sets are not enough.
    let reordered_plan_fragments = plan_fragments(&["frag-b", "frag-a"]);
    let reordered = nine_slices(&reordered_plan_fragments);
    let reordered_plan = AssemblyPlan {
        slices: &reordered,
        model_version: MODEL,
        prompt_version: PROMPT,
    };
    let error = composition
        .record_assembly(
            &reordered_plan,
            &record(&["frag-a", "frag-b"]),
            &fragments(&["frag-b", "frag-a"]),
        )
        .expect_err("a different entry order is a caller error too");
    assert!(
        error.message.contains("assembly.input_ids_differ"),
        "{error}"
    );
    verify_chain(&composition);
}

// ---------------------------------------------------------------------------
// Reading rows back (W106)
// ---------------------------------------------------------------------------

/// The strongest evidence that the four items really landed: read them back and compare, item by
/// item, **row by row**.
#[test]
fn the_logged_items_round_trip_row_by_row() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let ids = ["frag-a", "frag-b"];
    let offered = plan_fragments(&ids);
    let mut slices = nine_slices(&offered);
    let dropped = ["utterance-9"];
    slices[8].crop = Some(Crop {
        dropped: &dropped,
        reason: "back-of-list crop",
    });
    let plan = AssemblyPlan {
        slices: &slices,
        model_version: MODEL,
        prompt_version: PROMPT,
    };
    expect_logged(
        composition
            .record_assembly(&plan, &record(&ids), &fragments(&ids))
            .unwrap(),
    );

    let rows = composition.read_logged_assemblies(500).unwrap();
    assert_eq!(rows.len(), 1, "one assembly, one row");
    let row = &rows[0];

    assert_eq!(
        row.fragments
            .iter()
            .map(|fragment| (fragment.id.as_str(), fragment.source_ref.as_str()))
            .collect::<Vec<_>>(),
        vec![("frag-a", SOURCE_REF), ("frag-b", SOURCE_REF)]
    );
    assert_eq!(row.fragments[0].content.as_deref(), Some(BODY));

    assert_eq!(row.segment_usage.len(), SEGMENT_ORDER.len());
    for (index, usage) in row.segment_usage.iter().enumerate() {
        assert_eq!(usage.segment, SEGMENT_ORDER[index].label());
        assert_eq!(usage.tokens, 100 + index as u64);
        assert_eq!(usage.chars, 400 + index as u64);
        let budget = usage
            .budget
            .as_ref()
            .expect("every segment carries a budget");
        assert_eq!(budget.tokens, Some(1_000));
        assert_eq!(budget.chars, Some(4_000));
        assert_eq!(budget.floor.is_some(), SEGMENT_ORDER[index].uncroppable());
    }

    assert_eq!(row.crops.len(), 1);
    assert_eq!(row.crops[0].segment, "tool_results");
    assert_eq!(row.crops[0].dropped, vec!["utterance-9".to_string()]);
    assert_eq!(row.crops[0].reason, "back-of-list crop");

    assert_eq!(row.model_version, MODEL);
    assert_eq!(row.prompt_version, PROMPT);
    verify_chain(&composition);
}

/// Two assemblies are two rows, each keeping its **own** usage and crops. Pooling them would leave
/// 「各段预算占用」 with no assembly to belong to.
#[test]
fn every_row_comes_back_with_its_own_usage_and_crops() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let first = ["frag-a"];
    expect_logged(offer(&mut composition, &first, &fragments(&first)).unwrap());

    let second = ["frag-b"];
    let offered = plan_fragments(&second);
    let mut slices = nine_slices(&offered);
    let dropped = ["summary-1"];
    // §41.1: cropping runs from the back, so the crop lands on the trailing segments.
    for index in [7, 8] {
        slices[index].crop = Some(Crop {
            dropped: &dropped,
            reason: "second assembly cropped",
        });
    }
    let plan = AssemblyPlan {
        slices: &slices,
        model_version: MODEL,
        prompt_version: PROMPT,
    };
    expect_logged(
        composition
            .record_assembly(&plan, &record(&second), &fragments(&second))
            .unwrap(),
    );

    let rows = composition.read_logged_assemblies(500).unwrap();
    assert_eq!(rows.len(), 2, "two assemblies, two rows");
    assert_eq!(rows[0].fragments[0].id, "frag-a");
    assert!(rows[0].crops.is_empty(), "the first row was not cropped");
    assert_eq!(rows[1].fragments[0].id, "frag-b");
    assert_eq!(
        rows[1].crops.len(),
        2,
        "the second row carries its own crops"
    );
    assert_eq!(rows[1].crops[0].segment, "recent_utterances");
    assert_eq!(rows[1].crops[1].segment, "tool_results");
    verify_chain(&composition);
}

/// A row written before §41.5's two plan-side items existed is still readable: their absence reads
/// as **empty**, not as corruption. Without this case "the old rows still read" is an unverifiable
/// claim, and a decoder that demanded the keys would pass every other test here.
#[test]
fn a_row_logged_before_the_two_items_existed_is_still_readable() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    // The exact shape W104 wrote: three keys, no usage, no crops.
    let legacy = serde_json::json!({
        "fragments": [{ "id": "frag-a", "source_ref": SOURCE_REF, "content": BODY }],
        "model_version": MODEL,
        "prompt_version": PROMPT,
    });
    let session = composition.session_id().to_owned();
    composition
        .record(
            nai_storage::DecisionKind::Assembly,
            Some(&session),
            &legacy,
            "assembly-legacy-1",
        )
        .unwrap();

    let rows = composition.read_logged_assemblies(500).unwrap();
    assert_eq!(rows.len(), 1, "the legacy row is read, not skipped");
    assert_eq!(rows[0].fragments[0].id, "frag-a");
    assert!(
        rows[0].segment_usage.is_empty(),
        "a pre-W106 row reads its usage as empty"
    );
    assert!(
        rows[0].crops.is_empty(),
        "a pre-W106 row reads its crops as empty"
    );
    assert_eq!(rows[0].model_version, MODEL);
    verify_chain(&composition);
}

/// A row that is not decodable is a **named failure**, never a silent skip: skipping would make
/// "read back N rows" indistinguishable from "the chain had N rows".
#[test]
fn an_unparsable_assembly_row_is_a_named_failure() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let session = composition.session_id().to_owned();
    composition
        .record(
            nai_storage::DecisionKind::Assembly,
            Some(&session),
            &serde_json::json!({ "fragments": "not-an-array" }),
            "assembly-corrupt-1",
        )
        .unwrap();

    let result = composition.read_logged_assemblies(500);
    assert!(
        result.is_err(),
        "an unparsable assembly row MUST fail loudly, not read as zero rows: {result:?}"
    );
}

/// Reading the chain appends nothing and leaves its order and hashes alone.
#[test]
fn reading_the_log_appends_nothing() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let ids = ["frag-a"];
    expect_logged(offer(&mut composition, &ids, &fragments(&ids)).unwrap());

    let before = all_rows(&composition);
    let before_ids: Vec<_> = before
        .iter()
        .map(|row| (row.seq, row.payload_json.clone()))
        .collect();
    let rows = composition.read_logged_assemblies(500).unwrap();
    assert_eq!(rows.len(), 1);

    let after = all_rows(&composition);
    assert_eq!(after.len(), before.len(), "reading appends nothing");
    assert_eq!(
        after
            .iter()
            .map(|row| (row.seq, row.payload_json.clone()))
            .collect::<Vec<_>>(),
        before_ids,
        "and leaves the existing rows — order and content — untouched"
    );
    verify_chain(&composition);
}

// ---------------------------------------------------------------------------
// W104's H02 cases, re-offered through the plan-taking entry point
// ---------------------------------------------------------------------------

/// A callable, rebuildable assembly is appended with every referenced fragment's id and **body**,
/// and the chain still verifies.
#[test]
fn a_rebuildable_assembly_reaches_the_log_with_its_fragment_bodies() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let ids = ["frag-a", "frag-b"];
    let ack = expect_logged(offer(&mut composition, &ids, &fragments(&ids)).unwrap());
    assert!(ack.persisted, "a rebuildable assembly must persist");
    assert!(ack.entry_hash.is_some(), "a persisted row carries its hash");

    let rows = assembly_rows(&composition);
    assert_eq!(rows.len(), 1, "exactly one assembly entry");
    for id in ids {
        assert!(
            rows[0].payload_json.contains(id),
            "the payload must carry the fragment id {id}"
        );
    }
    assert!(
        rows[0].payload_json.contains(BODY),
        "the payload must carry the fragment body, not a pointer into live memory"
    );
    assert!(rows[0].payload_json.contains(MODEL));
    assert!(rows[0].payload_json.contains(PROMPT));
    verify_chain(&composition);
}

/// The stored sequence is the record's **entry** order, not a re-sorting of it. The fixture above
/// uses ascending ids, where a sorted implementation and a faithful one are indistinguishable;
/// this one does not, so an implementation that sorts the fragments before persisting is caught
/// here — and would otherwise surface later as a spurious `rebuild.order_differs`.
#[test]
fn the_log_preserves_the_entry_order_rather_than_re_sorting_it() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let entered = ["frag-c", "frag-a"];
    expect_logged(offer(&mut composition, &entered, &fragments(&entered)).unwrap());

    let rows = assembly_rows(&composition);
    assert_eq!(rows.len(), 1);
    let first_c = rows[0].payload_json.find("frag-c").expect("id logged");
    let first_a = rows[0].payload_json.find("frag-a").expect("id logged");
    assert!(
        first_c < first_a,
        "the payload stores the entry order; a sorted payload is not the record's assembly"
    );

    let rebuilt = composition
        .rebuild_assembly_from_log(&record(&entered), 500)
        .unwrap();
    assert!(
        rebuilt.rebuildable,
        "the entry order round-trips: {:?}",
        rebuilt.rejections
    );
    assert_eq!(
        rebuilt.rebuilt,
        vec![
            ("frag-c".to_string(), SOURCE_REF.to_string()),
            ("frag-a".to_string(), SOURCE_REF.to_string()),
        ],
        "and it round-trips in the entry order, not an ascending one"
    );
    verify_chain(&composition);
}

/// The chain alone reproduces the logged assembly: rebuild reads back the persisted bodies and
/// returns the same `(id, source_ref)` sequence, in the log's own order.
#[test]
fn the_chain_alone_reproduces_the_logged_assembly() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let ids = ["frag-a", "frag-b", "frag-c"];
    expect_logged(offer(&mut composition, &ids, &fragments(&ids)).unwrap());

    let rebuilt = composition
        .rebuild_assembly_from_log(&record(&ids), 500)
        .unwrap();
    assert!(
        rebuilt.rebuildable,
        "the log alone must reproduce the assembly: {:?}",
        rebuilt.rejections
    );
    assert!(
        rebuilt.rejections.is_empty(),
        "a rebuildable verdict carries no rejection"
    );
    assert_eq!(
        rebuilt.rebuilt,
        vec![
            ("frag-a".to_string(), SOURCE_REF.to_string()),
            ("frag-b".to_string(), SOURCE_REF.to_string()),
            ("frag-c".to_string(), SOURCE_REF.to_string()),
        ],
        "the rebuilt sequence is the log's own order"
    );
    verify_chain(&composition);
}

/// The log's own order decides: a record whose entry order differs from what the log carries is
/// refused by name, and the rebuilt sequence is the log's, not the record's.
#[test]
fn the_logs_own_order_decides_and_a_difference_is_named() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let entered = ["frag-a", "frag-b"];
    expect_logged(offer(&mut composition, &entered, &fragments(&entered)).unwrap());

    let claimed = ["frag-b", "frag-a"];
    let rebuilt = composition
        .rebuild_assembly_from_log(&record(&claimed), 500)
        .unwrap();
    assert!(!rebuilt.rebuildable, "a differing order is not rebuildable");
    assert!(
        rebuilt
            .rejections
            .contains(&RebuildRejection::RebuildOrderDiffers.label()),
        "the rebuild names the reason: {:?}",
        rebuilt.rejections
    );
    assert_eq!(
        rebuilt.rebuilt,
        vec![
            ("frag-a".to_string(), SOURCE_REF.to_string()),
            ("frag-b".to_string(), SOURCE_REF.to_string()),
        ],
        "the sequence follows the log, not the record — sorting by the record would make the check true by construction"
    );

    // The write path refuses the same shape: material offered in an order that differs from the
    // record's. The plan still names the record's ids, so this is H02's refusal, not the entry's.
    let flipped = fragments(&["frag-b", "frag-a"]);
    let before = assembly_rows(&composition).len();
    let refusal = expect_not_rebuildable(offer(&mut composition, &entered, &flipped).unwrap());
    assert!(
        refusal
            .rebuild_rejections
            .contains(&RebuildRejection::RebuildOrderDiffers.label()),
        "the refusal names the reason: {:?}",
        refusal.rebuild_rejections
    );
    assert!(
        refusal.rejections.is_empty(),
        "the legal plan was not refused by §41.1–§41.3"
    );
    assert_eq!(
        assembly_rows(&composition).len(),
        before,
        "a refusal appends nothing"
    );
    verify_chain(&composition);
}

/// H02's crux: a fragment whose body only ever lived in working memory is refused by name and is
/// **never** persisted as a pointer. This is the case `content: None` models.
#[test]
fn a_fragment_body_that_only_lives_in_memory_is_refused_not_logged_as_a_pointer() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let ids = ["frag-a", "frag-b"];
    let mut material = fragments(&ids);
    material[1].content = None;

    let before = assembly_rows(&composition).len();
    let refusal = expect_not_rebuildable(offer(&mut composition, &ids, &material).unwrap());
    assert!(
        refusal
            .rebuild_rejections
            .contains(&RebuildRejection::FragmentContentNotInLog.label()),
        "the refusal names the reason: {:?}",
        refusal.rebuild_rejections
    );
    assert_eq!(
        refusal.contentless_ids,
        vec!["frag-b".to_string()],
        "the refusal names the body-less id"
    );
    assert_eq!(
        assembly_rows(&composition).len(),
        before,
        "a refusal appends nothing"
    );
    verify_chain(&composition);
}

/// A referenced fragment with no log entry at all is refused by name, distinctly from the
/// body-less case, and appends nothing.
#[test]
fn a_referenced_fragment_absent_from_the_log_is_refused_by_name() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let ids = ["frag-a", "frag-b"];
    let before = assembly_rows(&composition).len();
    let refusal =
        expect_not_rebuildable(offer(&mut composition, &ids, &fragments(&["frag-a"])).unwrap());
    assert!(
        refusal
            .rebuild_rejections
            .contains(&RebuildRejection::FragmentNotLogged.label()),
        "the refusal names the reason: {:?}",
        refusal.rebuild_rejections
    );
    assert_eq!(refusal.unlogged_ids, vec!["frag-b".to_string()]);
    assert_eq!(
        assembly_rows(&composition).len(),
        before,
        "a refusal appends nothing"
    );
    verify_chain(&composition);
}

/// Rebuilding from an empty log reports every referenced id as unlogged — proof that the rebuild
/// reads the **chain**, not a slice the caller could have supplied.
#[test]
fn rebuilding_from_an_empty_log_reports_every_id_unlogged() {
    let dir = tempdir().unwrap();
    let composition = prepared(dir.path());

    let ids = ["frag-a", "frag-b"];
    let rebuilt = composition
        .rebuild_assembly_from_log(&record(&ids), 500)
        .unwrap();
    assert!(
        !rebuilt.rebuildable,
        "an empty log cannot rebuild a non-empty assembly"
    );
    assert!(
        rebuilt
            .rejections
            .contains(&RebuildRejection::FragmentNotLogged.label()),
        "the rebuild names the reason: {:?}",
        rebuilt.rejections
    );
    assert_eq!(
        rebuilt.unlogged_ids,
        vec!["frag-a".to_string(), "frag-b".to_string()],
        "every referenced id is reported unlogged"
    );
    assert!(rebuilt.rebuilt.is_empty());
    verify_chain(&composition);
}

/// A record with neither version is refused by both names, and appends nothing.
#[test]
fn a_missing_model_or_prompt_version_is_refused_by_name() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let ids = ["frag-a"];
    let before = assembly_rows(&composition).len();
    let offered = plan_fragments(&ids);
    let slices = nine_slices(&offered);
    let plan = AssemblyPlan {
        slices: &slices,
        model_version: MODEL,
        prompt_version: PROMPT,
    };
    let mut unversioned = record(&ids);
    unversioned.model_version = "";
    unversioned.prompt_version = "  ";
    let refusal = expect_not_rebuildable(
        composition
            .record_assembly(&plan, &unversioned, &fragments(&ids))
            .unwrap(),
    );
    assert!(
        refusal
            .rebuild_rejections
            .contains(&RebuildRejection::ModelVersionMissing.label()),
        "the refusal names the missing model version: {:?}",
        refusal.rebuild_rejections
    );
    assert!(
        refusal
            .rebuild_rejections
            .contains(&RebuildRejection::PromptVersionMissing.label()),
        "the refusal names the missing prompt version: {:?}",
        refusal.rebuild_rejections
    );
    assert_eq!(
        assembly_rows(&composition).len(),
        before,
        "a refusal appends nothing"
    );
    verify_chain(&composition);
}

/// A credential literal in a fragment **body** is refused by name, and the literal never reaches
/// any persisted row.
#[test]
fn a_credential_in_a_fragment_body_is_refused_and_never_reaches_the_log() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let literal = format!("{}live-body-99", CREDENTIAL_MARKERS[0]);
    let ids = ["frag-a"];
    let material = vec![LoggedFragment {
        id: "frag-a",
        source_ref: SOURCE_REF,
        content: Some(literal.as_str()),
    }];

    let before = assembly_rows(&composition).len();
    let refusal = expect_not_rebuildable(offer(&mut composition, &ids, &material).unwrap());
    assert!(
        refusal
            .rebuild_rejections
            .contains(&RebuildRejection::PlaintextCredential.label()),
        "the refusal names the reason: {:?}",
        refusal.rebuild_rejections
    );
    assert_eq!(refusal.credential_ids, vec!["frag-a".to_string()]);

    let after = all_rows(&composition);
    assert_eq!(
        assembly_rows(&composition).len(),
        before,
        "a refusal appends nothing"
    );
    assert!(
        !after.iter().any(|row| row.payload_json.contains(&literal)),
        "the credential literal must not appear in any persisted row"
    );
    verify_chain(&composition);
}

/// An over-large payload is a classified error, not a silent truncation.
#[test]
fn an_oversized_assembly_payload_is_an_error_not_a_silent_truncation() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let huge = "x".repeat(70_000);
    let ids = ["frag-a"];
    let material = vec![LoggedFragment {
        id: "frag-a",
        source_ref: SOURCE_REF,
        content: Some(huge.as_str()),
    }];
    let offered = plan_fragments(&ids);
    let slices = nine_slices(&offered);
    let plan = AssemblyPlan {
        slices: &slices,
        model_version: MODEL,
        prompt_version: PROMPT,
    };
    let result = composition.record_assembly(&plan, &record(&ids), &material);
    assert!(
        result.is_err(),
        "a payload past the storage bound must fail loudly"
    );
    assert!(
        assembly_rows(&composition).is_empty(),
        "a rejected write appends nothing"
    );
    verify_chain(&composition);
}

/// A refusal must be distinguishable from a successful append: the two outcomes are different
/// values of the same enum, and a caller cannot read one as the other.
#[test]
fn a_refusal_is_distinguishable_from_an_append() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let ids = ["frag-a"];
    let logged = offer(&mut composition, &ids, &fragments(&ids)).unwrap();
    let mut broken = fragments(&ids);
    broken[0].content = None;
    let refused = offer(&mut composition, &ids, &broken).unwrap();

    assert!(
        matches!(&logged, AssemblyOutcome::Logged(_)),
        "a rebuildable assembly reports an append"
    );
    assert!(
        matches!(&refused, AssemblyOutcome::NotRebuildable(_)),
        "an assembly that fails judgement reports a refusal, not an append"
    );
    assert_ne!(
        std::mem::discriminant(&logged),
        std::mem::discriminant(&refused),
        "the two outcomes MUST NOT collapse into one value"
    );
    verify_chain(&composition);
}

// ---------------------------------------------------------------------------
// Non-void guards: one per judge
// ---------------------------------------------------------------------------

/// The §41.5 guard: every reason the rebuild judge can refuse is reachable through the runtime, and
/// the runtime names it. The loop is over the kernel's `ALL`, and the per-variant sample
/// constructor matches the enum exhaustively — so a new variant cannot pass uncovered.
#[test]
fn every_rebuild_rejection_is_reachable_through_the_runtime() {
    for rejection in RebuildRejection::ALL {
        let observed = runtime_refusal_for(rejection);
        assert!(
            observed.contains(&rejection.label()),
            "rejection {} has no runtime case (observed {:?})",
            rejection.label(),
            observed
        );
    }
}

/// Build the runtime scenario that triggers `rejection` and return the labels the runtime named.
fn runtime_refusal_for(rejection: RebuildRejection) -> Vec<&'static str> {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());
    let outcome = match rejection {
        RebuildRejection::FragmentNotLogged => {
            let ids = ["frag-a", "frag-b"];
            offer(&mut composition, &ids, &fragments(&["frag-a"])).unwrap()
        }
        RebuildRejection::FragmentContentNotInLog => {
            let ids = ["frag-a"];
            let mut material = fragments(&ids);
            material[0].content = None;
            offer(&mut composition, &ids, &material).unwrap()
        }
        RebuildRejection::RebuildOrderDiffers => {
            let ids = ["frag-a", "frag-b"];
            offer(&mut composition, &ids, &fragments(&["frag-b", "frag-a"])).unwrap()
        }
        RebuildRejection::ModelVersionMissing => {
            let ids = ["frag-a"];
            let offered = plan_fragments(&ids);
            let slices = nine_slices(&offered);
            let plan = AssemblyPlan {
                slices: &slices,
                model_version: MODEL,
                prompt_version: PROMPT,
            };
            let mut unversioned = record(&ids);
            unversioned.model_version = "";
            composition
                .record_assembly(&plan, &unversioned, &fragments(&ids))
                .unwrap()
        }
        RebuildRejection::PromptVersionMissing => {
            let ids = ["frag-a"];
            let offered = plan_fragments(&ids);
            let slices = nine_slices(&offered);
            let plan = AssemblyPlan {
                slices: &slices,
                model_version: MODEL,
                prompt_version: PROMPT,
            };
            let mut unversioned = record(&ids);
            unversioned.prompt_version = "";
            composition
                .record_assembly(&plan, &unversioned, &fragments(&ids))
                .unwrap()
        }
        RebuildRejection::PlaintextCredential => {
            let literal = format!("{}in-the-body", CREDENTIAL_MARKERS[0]);
            let ids = ["frag-a"];
            let material = vec![LoggedFragment {
                id: "frag-a",
                source_ref: SOURCE_REF,
                content: Some(literal.as_str()),
            }];
            offer(&mut composition, &ids, &material).unwrap()
        }
    };
    match outcome {
        AssemblyOutcome::NotCallable(refusal)
        | AssemblyOutcome::NotRebuildable(refusal)
        | AssemblyOutcome::Neither(refusal) => refusal.rebuild_rejections,
        AssemblyOutcome::Logged(_) => {
            panic!(
                "{}: the runtime must refuse this scenario",
                rejection.label()
            )
        }
    }
}

/// The §41.1–§41.3 guard: every reason the assembly judge can refuse is reachable through the
/// runtime entry point, and the runtime names it. The sample constructor matches the enum
/// exhaustively, so a variant added to the kernel fails to compile here rather than passing
/// invisibly.
#[test]
fn every_assembly_rejection_is_reachable_through_the_runtime() {
    for rejection in AssemblyRejection::ALL {
        let observed = runtime_assembly_refusal_for(rejection);
        assert!(
            observed.contains(&rejection.label()),
            "rejection {} has no runtime case (observed {:?})",
            rejection.label(),
            observed
        );
    }
}

/// Offer `ids` under a plan deformed to trigger `rejection`, and return every label the runtime
/// named.
///
/// The three fragment-level rejections are deformation of the **fragment** (done before the slices
/// are built, since the slices borrow it); every other rejection is deformation of the **segment
/// slice** that carries it.
fn runtime_assembly_refusal_for(rejection: AssemblyRejection) -> Vec<&'static str> {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());
    let ids = ["frag-a"];

    let mut frags = plan_fragments(&ids);
    match rejection {
        AssemblyRejection::FragmentWithoutSourceRef => frags[0].source_ref = "",
        AssemblyRejection::FragmentWithoutScope => frags[0].scope = None,
        AssemblyRejection::UntrustedMarkedAsInstruction => frags[0].trust = Trust::Instruction,
        _ => {}
    }
    let mut slices = nine_slices(&frags);
    let dropped = ["evidence-1"];

    match rejection {
        AssemblyRejection::UnknownSegment => slices[0].segment = "not_a_segment_in_41_1",
        AssemblyRejection::SegmentOutOfOrder => slices.swap(0, 1),
        AssemblyRejection::UncroppableSegmentMissing => {
            slices.remove(0);
        }
        AssemblyRejection::UncroppableSegmentCropped => {
            slices[0].crop = Some(Crop {
                dropped: &dropped,
                reason: "claimed pressure",
            });
        }
        AssemblyRejection::CropWithoutReason => {
            slices[8].crop = Some(Crop {
                dropped: &dropped,
                reason: "   ",
            });
        }
        AssemblyRejection::CropWithoutDroppedItems => {
            slices[8].crop = Some(Crop {
                dropped: &[],
                reason: "claimed pressure",
            });
        }
        AssemblyRejection::CropFromFront => {
            // A crop on the evidence segment while every later croppable segment stays intact.
            slices[4].crop = Some(Crop {
                dropped: &dropped,
                reason: "claimed pressure",
            });
        }
        AssemblyRejection::SegmentBudgetMissing => slices[3].budget = None,
        AssemblyRejection::BudgetDimensionMissing => {
            slices[3].budget = Some(SegmentBudget {
                tokens: None,
                chars: Some(4_000),
                items: Some(8),
                floor: None,
            });
        }
        AssemblyRejection::UncroppableWithoutFloor => {
            slices[0].budget = Some(SegmentBudget {
                tokens: Some(1_000),
                chars: Some(4_000),
                items: Some(8),
                floor: None,
            });
        }
        AssemblyRejection::SegmentOverBudget => slices[3].usage.tokens = 10_000,
        AssemblyRejection::FragmentWithoutSourceRef
        | AssemblyRejection::FragmentWithoutScope
        | AssemblyRejection::UntrustedMarkedAsInstruction => {}
    }

    let plan = AssemblyPlan {
        slices: &slices,
        model_version: MODEL,
        prompt_version: PROMPT,
    };
    let outcome = composition
        .record_assembly(&plan, &record(&ids), &fragments(&ids))
        .unwrap();
    match outcome {
        AssemblyOutcome::NotCallable(refusal)
        | AssemblyOutcome::NotRebuildable(refusal)
        | AssemblyOutcome::Neither(refusal) => refusal.rejections,
        AssemblyOutcome::Logged(_) => {
            panic!(
                "{}: the runtime must refuse this scenario",
                rejection.label()
            )
        }
    }
}
