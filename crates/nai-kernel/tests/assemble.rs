//! W133: the assembler (H01/H02), V3.11 §41.1–§41.2.
//!
//! Until this round `AssemblyPlan` had a judge and an execution body but **no producer**: no
//! production code ever built one, so `Composition::record_assembly` had no caller and the
//! chain could only ever be exercised from tests. These samples are chosen so that a
//! plausible-but-wrong assembler fails:
//!
//! * one that passes the caller's order through instead of §41.1's order,
//! * one that pads absent segments with empty slices (which would make
//!   `UncroppableSegmentMissing` unreachable on the real path),
//! * one that crops the **head** of a segment instead of its tail,
//! * one that crops an uncroppable segment because the budget says so,
//! * one that records a crop without naming what was dropped, or names it without a reason,
//! * one that reports the *offered* usage instead of the *surviving* usage,
//! * one that silently drops a segment name §41.1 never defined,
//! * one that invents a budget when the call site gave none (that would answer §41.2 twice),
//! * one that hard-codes a second copy of the nine-segment table.

use nai_kernel::assemble::{
    AssembleError, AssemblyInput, CandidateFragment, SegmentInput, plan_assembly,
};
use nai_kernel::evaluate::assembly::{
    AssemblyRejection, Fragment, SEGMENT_ORDER, Segment, SegmentBudget, SourceKind, Trust,
    evaluate_assembly,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn frag(id: &str, source: SourceKind) -> Fragment<'_> {
    Fragment {
        id,
        source,
        source_ref: "src-1",
        scope: Some("session:s1"),
        age_ms: None,
        trust: Trust::Data,
    }
}

fn cand(id: &str, source: SourceKind, tokens: u64, chars: u64) -> CandidateFragment<'_> {
    CandidateFragment {
        fragment: frag(id, source),
        tokens,
        chars,
    }
}

/// §41.2's two mandatory dimensions plus a floor. `items` is open: §41.1 caps only two
/// segments, so an item cap is something the **call site** decides, not this module.
fn budget() -> SegmentBudget {
    SegmentBudget {
        tokens: Some(1_000),
        chars: Some(4_000),
        items: None,
        floor: Some(100),
    }
}

fn budget_with_items(items: usize) -> SegmentBudget {
    SegmentBudget {
        items: Some(items),
        ..budget()
    }
}

fn input<'a>(segments: &'a [SegmentInput<'a>]) -> AssemblyInput<'a> {
    AssemblyInput {
        model_version: "model@7",
        prompt_version: "prompt@3",
        segments,
    }
}

// ---------------------------------------------------------------------------
// Order and presence
// ---------------------------------------------------------------------------

/// §41.1 fixes the order for one reason — 约束不被挤掉 — so the assembler must impose it, not
/// inherit whatever order the candidates happened to arrive in.
#[test]
fn segments_come_out_in_s41_order_regardless_of_input_order() {
    let tool = cand("t1", SourceKind::ToolResult, 10, 40);
    let policy = cand("p1", SourceKind::SystemPolicy, 10, 40);
    let evidence = cand("e1", SourceKind::KnowledgeEvidence, 10, 40);

    let segments = [
        SegmentInput {
            segment: "tool_results",
            candidates: &[tool],
            budget: Some(budget()),
        },
        SegmentInput {
            segment: "evidence",
            candidates: &[evidence],
            budget: Some(budget()),
        },
        SegmentInput {
            segment: "system_policy",
            candidates: &[policy],
            budget: Some(budget()),
        },
    ];

    let request = input(&segments);
    let planned = plan_assembly(&request).expect("every name is defined by §41.1");
    planned.with_plan(|plan| {
        let names: Vec<&str> = plan.slices.iter().map(|slice| slice.segment).collect();
        assert_eq!(names, vec!["system_policy", "evidence", "tool_results"]);
    });
}

/// §41.1 requires segments 1–3 to be present; the other six may be absent. The assembler
/// therefore must **not** pad an absent segment — the judge is what refuses a missing one, and
/// a fabricated empty slice would leave that rejection with no object on the real path.
#[test]
fn a_segment_absent_from_the_input_is_absent_from_the_plan() {
    let policy = cand("p1", SourceKind::SystemPolicy, 10, 40);
    let segments = [SegmentInput {
        segment: "system_policy",
        candidates: &[policy],
        budget: Some(budget()),
    }];

    let request = input(&segments);
    let planned = plan_assembly(&request).expect("defined name");
    planned.with_plan(|plan| {
        assert_eq!(plan.slices.len(), 1, "the assembler pads nothing");
        assert_eq!(plan.slices[0].segment, "system_policy");
    });

    assert_eq!(planned.segments().len(), 1, "one offer in, one segment out");
}

/// A closed-world guard: give every segment §41.1 defines one candidate, and the plan must come
/// back with exactly those nine, in that order. An assembler that quietly omits or reorders one
/// cannot survive this.
#[test]
fn every_segment_s41_defines_survives_in_the_order_s41_fixes() {
    let candidates: Vec<CandidateFragment<'_>> = SEGMENT_ORDER
        .iter()
        .map(|segment| {
            let id: &'static str = match segment {
                Segment::SystemPolicy => "p1",
                Segment::Persona => "r1",
                Segment::OutputContract => "c1",
                Segment::EventSummary => "e1",
                Segment::Evidence => "k1",
                Segment::Memory => "m1",
                Segment::Skills => "s1",
                Segment::RecentUtterances => "u1",
                Segment::ToolResults => "t1",
            };
            cand(id, SourceKind::SystemPolicy, 10, 40)
        })
        .collect();

    // Built in reverse so the assembler has to sort rather than echo.
    let segments: Vec<SegmentInput<'_>> = SEGMENT_ORDER
        .iter()
        .rev()
        .zip(candidates.iter())
        .map(|(segment, candidate)| SegmentInput {
            segment: segment.label(),
            candidates: std::slice::from_ref(candidate),
            budget: Some(budget()),
        })
        .collect();

    let request = input(&segments);
    let planned = plan_assembly(&request).expect("defined names");
    planned.with_plan(|plan| {
        let produced: Vec<Segment> = plan
            .slices
            .iter()
            .map(|slice| Segment::parse(slice.segment).expect("a defined name came back"))
            .collect();
        assert_eq!(produced.as_slice(), &SEGMENT_ORDER);
    });
}

// ---------------------------------------------------------------------------
// Cropping
// ---------------------------------------------------------------------------

/// §41.1 says cropping runs from the back. Within a segment that means the **tail**, and a crop
/// must record both halves — what was dropped and why.
#[test]
fn items_over_the_cap_crop_the_tail_and_record_both_dropped_and_reason() {
    let first = cand("k1", SourceKind::KnowledgeEvidence, 10, 40);
    let second = cand("k2", SourceKind::KnowledgeEvidence, 10, 40);
    let third = cand("k3", SourceKind::KnowledgeEvidence, 10, 40);

    let segments = [SegmentInput {
        segment: "evidence",
        candidates: &[first, second, third],
        budget: Some(budget_with_items(2)),
    }];

    let request = input(&segments);
    let planned = plan_assembly(&request).expect("defined name");
    planned.with_plan(|plan| {
        let slice = &plan.slices[0];
        let kept: Vec<&str> = slice.fragments.iter().map(|frag| frag.id).collect();
        assert_eq!(kept, vec!["k1", "k2"], "the tail goes, not the head");

        let crop = slice
            .crop
            .as_ref()
            .expect("a crop happened, so it must be recorded");
        assert_eq!(crop.dropped, &["k3"][..], "what was dropped is named");
        assert!(!crop.reason.trim().is_empty(), "and so is the reason");
    });
}

/// No cap from the call site, no crop. An assembler that invents a cap would be answering §41.2
/// a second time.
#[test]
fn a_segment_without_an_item_cap_is_left_whole() {
    let first = cand("k1", SourceKind::KnowledgeEvidence, 10, 40);
    let second = cand("k2", SourceKind::KnowledgeEvidence, 10, 40);
    let third = cand("k3", SourceKind::KnowledgeEvidence, 10, 40);

    let segments = [SegmentInput {
        segment: "evidence",
        candidates: &[first, second, third],
        budget: Some(budget()),
    }];

    let request = input(&segments);
    let planned = plan_assembly(&request).expect("defined name");
    planned.with_plan(|plan| {
        assert_eq!(plan.slices[0].fragments.len(), 3);
        assert!(
            plan.slices[0].crop.is_none(),
            "nothing was dropped, so nothing is recorded"
        );
    });
}

/// §41.1 rows 1–3 are 不可裁. A budget that says otherwise does not win: the segment comes out
/// whole and the **judge** is what refuses the over-budget assembly.
#[test]
fn the_uncroppable_segments_are_never_cropped() {
    let first = cand("p1", SourceKind::SystemPolicy, 10, 40);
    let second = cand("p2", SourceKind::SystemPolicy, 10, 40);
    let third = cand("p3", SourceKind::SystemPolicy, 10, 40);

    let segments = [SegmentInput {
        segment: "system_policy",
        candidates: &[first, second, third],
        budget: Some(budget_with_items(1)),
    }];

    let request = input(&segments);
    let planned = plan_assembly(&request).expect("defined name");
    planned.with_plan(|plan| {
        let slice = &plan.slices[0];
        assert_eq!(
            slice.fragments.len(),
            3,
            "an uncroppable segment is not cropped, cap or no cap"
        );
        assert!(slice.crop.is_none(), "and no crop is recorded for it");
        assert!(
            !evaluate_assembly(plan).callable,
            "so the judge is what refuses it"
        );
    });
}

/// Cropping changes what was spent. An assembler that reports the **offered** usage would let a
/// plan look over budget after it had already been cut to fit.
#[test]
fn usage_counts_only_what_survived() {
    let first = cand("k1", SourceKind::KnowledgeEvidence, 10, 40);
    let second = cand("k2", SourceKind::KnowledgeEvidence, 10, 40);
    let third = cand("k3", SourceKind::KnowledgeEvidence, 10, 40);

    let segments = [SegmentInput {
        segment: "evidence",
        candidates: &[first, second, third],
        budget: Some(budget_with_items(2)),
    }];

    let request = input(&segments);
    let planned = plan_assembly(&request).expect("defined name");
    planned.with_plan(|plan| {
        let usage = plan.slices[0].usage;
        assert_eq!(usage.items, 2, "two survived");
        assert_eq!(usage.tokens, 20, "and only their tokens are counted");
        assert_eq!(usage.chars, 80);
    });
}

// ---------------------------------------------------------------------------
// What the assembler must refuse, and what it must not decide
// ---------------------------------------------------------------------------

/// §41.1 defines nine names. An undefined one is not silently dropped — dropping it would make
/// `AssemblyRejection::UnknownSegment` unreachable from this side.
#[test]
fn an_undefined_segment_name_is_refused_by_name() {
    let stray = cand("x1", SourceKind::Event, 10, 40);

    let segments = [SegmentInput {
        segment: "system_prompt",
        candidates: &[stray],
        budget: Some(budget()),
    }];

    let request = input(&segments);
    assert_eq!(
        plan_assembly(&request).err(),
        Some(AssembleError::UnknownSegment),
    );
    assert_eq!(
        AssembleError::UnknownSegment.label(),
        "assemble.unknown_segment"
    );
}

/// §41.2's budgets are the call site's to give. When it gives none the assembler passes that
/// through instead of inventing one — the judge is what refuses it, and only once.
#[test]
fn a_segment_without_a_budget_keeps_no_budget() {
    let policy = cand("p1", SourceKind::SystemPolicy, 10, 40);

    let segments = [SegmentInput {
        segment: "system_policy",
        candidates: &[policy],
        budget: None,
    }];

    let request = input(&segments);
    let planned = plan_assembly(&request).expect("defined name");
    planned.with_plan(|plan| {
        assert!(
            plan.slices[0].budget.is_none(),
            "the assembler does not answer §41.2 on the call site's behalf"
        );

        let verdict = evaluate_assembly(plan);
        assert!(!verdict.callable);
        assert!(
            verdict
                .rejections
                .contains(&AssemblyRejection::SegmentBudgetMissing),
            "the judge is what says so"
        );
    });
}

/// The positive invariant: a complete, well-formed assembly built by the assembler is one the
/// judge accepts. Without this, "refuse everything" would satisfy every test above.
#[test]
fn the_plan_the_assembler_builds_passes_the_h01_judgement() {
    let policy = cand("p1", SourceKind::SystemPolicy, 10, 40);
    let persona = cand("r1", SourceKind::Scenario, 10, 40);
    let contract = cand("c1", SourceKind::Scenario, 10, 40);
    let event = cand("e1", SourceKind::Event, 10, 40);
    let evidence = cand("k1", SourceKind::KnowledgeEvidence, 10, 40);
    let memory = cand("m1", SourceKind::Memory, 10, 40);
    let skill = cand("s1", SourceKind::ToolResult, 10, 40);
    let utterance = cand("u1", SourceKind::Event, 10, 40);
    let tool = cand("t1", SourceKind::ToolResult, 10, 40);

    let segments = [
        SegmentInput {
            segment: "system_policy",
            candidates: &[policy],
            budget: Some(budget()),
        },
        SegmentInput {
            segment: "persona",
            candidates: &[persona],
            budget: Some(budget()),
        },
        SegmentInput {
            segment: "output_contract",
            candidates: &[contract],
            budget: Some(budget()),
        },
        SegmentInput {
            segment: "event_summary",
            candidates: &[event],
            budget: Some(budget()),
        },
        SegmentInput {
            segment: "evidence",
            candidates: &[evidence],
            budget: Some(budget_with_items(5)),
        },
        SegmentInput {
            segment: "memory",
            candidates: &[memory],
            budget: Some(budget()),
        },
        SegmentInput {
            segment: "skills",
            candidates: &[skill],
            budget: Some(budget_with_items(5)),
        },
        SegmentInput {
            segment: "recent_utterances",
            candidates: &[utterance],
            budget: Some(budget()),
        },
        SegmentInput {
            segment: "tool_results",
            candidates: &[tool],
            budget: Some(budget()),
        },
    ];

    let request = input(&segments);
    let planned = plan_assembly(&request).expect("defined names");
    planned.with_plan(|plan| {
        let verdict = evaluate_assembly(plan);
        assert_eq!(verdict.rejections, Vec::new(), "a legal plan is accepted");
        assert!(verdict.callable);
        assert_eq!(plan.model_version, "model@7", "§41.5 records the versions");
        assert_eq!(plan.prompt_version, "prompt@3");
    });
}
