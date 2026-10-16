//! W69: context assembly (H01/H02), V3.9 §41.1–§41.5 / §47.1.
//!
//! The samples are chosen so that a plausible-but-wrong implementation fails:
//!
//! * a checker that compares the segment **set** (sorted) instead of the sequence,
//! * a checker that reports only the first missing uncroppable segment,
//! * a checker that swallows an undefined segment name,
//! * a crop checker that only asks "is there a crop and does it carry a reason" — its sample
//!   crops the *front* croppable segment while leaving later ones intact, which no
//!   back-to-front walk can produce,
//! * a budget checker that accepts one total cap instead of per-segment budgets, or accepts a
//!   single dimension (tokens without characters),
//! * a rebuild that sorts by the **record's** order before comparing — with that implementation
//!   the order assertion is true by construction, so its sample puts the log in a different
//!   order from the record,
//! * a rebuild that keeps pulling fragment bodies from the live working memory — its sample
//!   logs an entry whose body is `None`.

use nai_kernel::evaluate::assembly::{
    AssemblyPlan, AssemblyRecord, AssemblyRejection, AssemblyVerdict, BudgetDimension, Crop,
    Fragment, LoggedFragment, RebuildRejection, RebuildVerdict, SEGMENT_ORDER, SOURCE_KINDS,
    Segment, SegmentBudget, SegmentSlice, SegmentUsage, SourceKind, Trust, entry_carries_assembly,
    evaluate_assembly, rebuild_assembly, rebuild_assembly_from_entries,
};
use nai_kernel::evaluate::harness::ObservationLayer;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const LIGHT: SegmentUsage = SegmentUsage {
    tokens: 10,
    chars: 40,
    items: 1,
};

/// §41.2's two mandatory dimensions plus a floor; `items` is left open because §41.1 caps only
/// two segments.
fn full_budget() -> SegmentBudget {
    SegmentBudget {
        tokens: Some(1_000),
        chars: Some(4_000),
        items: None,
        floor: Some(100),
    }
}

fn good_fragment() -> Fragment<'static> {
    Fragment {
        id: "f1",
        source: SourceKind::KnowledgeEvidence,
        source_ref: "kb0#p1",
        scope: Some("session-1"),
        age_ms: None,
        trust: Trust::Data,
    }
}

/// All nine segments, in order, no crops — the shape §41.1 asks for.
fn clean_slices<'a>() -> Vec<SegmentSlice<'a>> {
    SEGMENT_ORDER
        .iter()
        .map(|segment| SegmentSlice {
            segment: segment.label(),
            fragments: &[],
            usage: LIGHT,
            budget: Some(full_budget()),
            crop: None,
        })
        .collect()
}

fn at(segment: &str) -> usize {
    SEGMENT_ORDER
        .iter()
        .position(|candidate| candidate.label() == segment)
        .expect("segment must be one of the nine")
}

fn plan<'a>(slices: &'a [SegmentSlice<'a>]) -> AssemblyPlan<'a> {
    AssemblyPlan {
        slices,
        model_version: "llm-2026-09",
        prompt_version: "prompt-7",
    }
}

// ---------------------------------------------------------------------------
// §41.1 / §41.3: the baseline tables, pinned item by item
// ---------------------------------------------------------------------------

#[test]
fn segment_order_matches_the_baseline_line() {
    // §41.1's table, transcribed. Pinning the *content* (not merely the count) is what makes a
    // quiet deletion survivable or not.
    let expected = [
        "system_policy",
        "persona",
        "output_contract",
        "event_summary",
        "evidence",
        "memory",
        "skills",
        "recent_utterances",
        "tool_results",
    ];
    let actual: Vec<&str> = SEGMENT_ORDER.iter().map(|s| s.label()).collect();
    assert_eq!(actual, expected);
}

#[test]
fn the_first_three_segments_are_the_uncroppable_ones() {
    let uncroppable: Vec<&str> = SEGMENT_ORDER
        .iter()
        .filter(|s| s.uncroppable())
        .map(|s| s.label())
        .collect();
    assert_eq!(uncroppable, ["system_policy", "persona", "output_contract"]);
    // And the next segment is croppable, so the boundary is where §41.1 puts it.
    assert!(!Segment::EventSummary.uncroppable());
}

#[test]
fn source_kinds_match_the_baseline_line() {
    let expected = [
        "system_policy",
        "scenario",
        "event",
        "knowledge_evidence",
        "memory",
        "tool_result",
        "external_agent",
    ];
    let actual: Vec<&str> = SOURCE_KINDS.iter().map(|s| s.label()).collect();
    assert_eq!(actual, expected);
}

#[test]
fn the_four_untrusted_sources_are_the_ones_the_baseline_names() {
    // §41.3: 观众文本、检索片段、工具返回、外部智能体返回一律标为数据.
    let must_be_data: Vec<&str> = SOURCE_KINDS
        .iter()
        .filter(|s| s.must_be_data())
        .map(|s| s.label())
        .collect();
    assert_eq!(
        must_be_data,
        [
            "event",
            "knowledge_evidence",
            "tool_result",
            "external_agent"
        ]
    );
    assert!(!SourceKind::SystemPolicy.must_be_data());
    assert!(!SourceKind::Scenario.must_be_data());
    assert!(!SourceKind::Memory.must_be_data());
}

// ---------------------------------------------------------------------------
// H01: order and the uncroppable three
// ---------------------------------------------------------------------------

#[test]
fn a_complete_ordered_assembly_is_callable() {
    let slices = clean_slices();
    let verdict = evaluate_assembly(&plan(&slices));
    assert!(verdict.callable, "{:?}", verdict.rejections);
    assert!(verdict.rejections.is_empty());
}

#[test]
fn an_undefined_segment_name_is_refused() {
    let mut slices = clean_slices();
    slices[3].segment = "tool_loop";
    let verdict = evaluate_assembly(&plan(&slices));
    assert!(
        verdict
            .rejections
            .contains(&AssemblyRejection::UnknownSegment)
    );
    assert!(!verdict.callable);
}

#[test]
fn a_missing_uncroppable_segment_is_refused_and_named() {
    let mut slices = clean_slices();
    slices.remove(at("persona"));
    let verdict = evaluate_assembly(&plan(&slices));
    assert!(
        verdict
            .rejections
            .contains(&AssemblyRejection::UncroppableSegmentMissing)
    );
    // Named, not counted — a bare "one segment missing" would not say which discipline lapsed.
    assert_eq!(verdict.missing_segments, [Segment::Persona]);
    assert!(!verdict.callable);
}

#[test]
fn two_missing_uncroppable_segments_are_both_named() {
    // Two, not one: with a single-segment sample an implementation that stops at the first
    // missing discipline would still look correct.
    let mut slices = clean_slices();
    // Later index first: removing an earlier one shifts the later one left.
    slices.remove(at("output_contract"));
    slices.remove(at("persona"));
    let verdict = evaluate_assembly(&plan(&slices));
    assert_eq!(
        verdict.missing_segments,
        [Segment::Persona, Segment::OutputContract]
    );
}

#[test]
fn a_missing_croppable_segment_is_not_a_refusal() {
    // §41.1 requires only rows 1–3 to be present; the other six may be absent.
    let mut slices = clean_slices();
    slices.remove(at("evidence"));
    let verdict = evaluate_assembly(&plan(&slices));
    assert!(verdict.callable, "{:?}", verdict.rejections);
    assert!(verdict.missing_segments.is_empty());
}

#[test]
fn the_right_segments_in_the_wrong_order_are_refused() {
    // Same nine segments, two of them swapped. A set comparison would pass this.
    let mut slices = clean_slices();
    slices.swap(at("evidence"), at("memory"));
    let verdict = evaluate_assembly(&plan(&slices));
    assert!(
        verdict
            .rejections
            .contains(&AssemblyRejection::SegmentOutOfOrder)
    );
}

#[test]
fn a_plan_that_is_short_and_out_of_order_reports_both() {
    let mut slices = clean_slices();
    slices.remove(at("output_contract"));
    slices.swap(at("evidence"), at("skills"));
    let verdict = evaluate_assembly(&plan(&slices));
    assert!(
        verdict
            .rejections
            .contains(&AssemblyRejection::UncroppableSegmentMissing)
    );
    assert!(
        verdict
            .rejections
            .contains(&AssemblyRejection::SegmentOutOfOrder)
    );
}

// ---------------------------------------------------------------------------
// H01: cropping
// ---------------------------------------------------------------------------

#[test]
fn cropping_an_uncroppable_segment_is_refused() {
    let mut slices = clean_slices();
    slices[at("system_policy")].crop = Some(Crop {
        dropped: &["policy-1"],
        reason: "over budget",
    });
    let verdict = evaluate_assembly(&plan(&slices));
    assert!(
        verdict
            .rejections
            .contains(&AssemblyRejection::UncroppableSegmentCropped)
    );
}

#[test]
fn a_crop_without_a_reason_is_refused() {
    let mut slices = clean_slices();
    slices[at("tool_results")].crop = Some(Crop {
        dropped: &["call-1"],
        reason: "   ",
    });
    let verdict = evaluate_assembly(&plan(&slices));
    assert!(
        verdict
            .rejections
            .contains(&AssemblyRejection::CropWithoutReason)
    );
}

#[test]
fn a_crop_without_dropped_items_is_refused() {
    let mut slices = clean_slices();
    slices[at("tool_results")].crop = Some(Crop {
        dropped: &[],
        reason: "over budget",
    });
    let verdict = evaluate_assembly(&plan(&slices));
    assert!(
        verdict
            .rejections
            .contains(&AssemblyRejection::CropWithoutDroppedItems)
    );
    // The reason is fine, so only the second half is reported.
    assert!(
        !verdict
            .rejections
            .contains(&AssemblyRejection::CropWithoutReason)
    );
}

#[test]
fn cropping_that_skips_a_front_croppable_segment_is_refused() {
    // 「evidence」 cropped while 「skills」 and 「recent_utterances」 are intact: no back-to-front
    // walk produces this shape.
    let mut slices = clean_slices();
    slices[at("evidence")].crop = Some(Crop {
        dropped: &["kb0#p1"],
        reason: "over budget",
    });
    let verdict = evaluate_assembly(&plan(&slices));
    assert!(
        verdict
            .rejections
            .contains(&AssemblyRejection::CropFromFront)
    );
}

#[test]
fn a_back_to_front_crop_is_accepted() {
    // A positive guard: the rule is "cropping runs from the back", not "never crop". Without
    // this sample, an implementation that refuses every crop would also look correct.
    let mut slices = clean_slices();
    for segment in ["memory", "skills", "recent_utterances", "tool_results"] {
        slices[at(segment)].crop = Some(Crop {
            dropped: &["x"],
            reason: "over budget",
        });
    }
    let verdict = evaluate_assembly(&plan(&slices));
    assert!(
        !verdict
            .rejections
            .contains(&AssemblyRejection::CropFromFront)
    );
    assert!(verdict.callable, "{:?}", verdict.rejections);
}

// ---------------------------------------------------------------------------
// H01: per-segment budgets
// ---------------------------------------------------------------------------

#[test]
fn a_segment_without_its_own_budget_is_refused() {
    let mut slices = clean_slices();
    slices[at("evidence")].budget = None;
    let verdict = evaluate_assembly(&plan(&slices));
    assert!(
        verdict
            .rejections
            .contains(&AssemblyRejection::SegmentBudgetMissing)
    );
    assert_eq!(verdict.budget_missing, [Segment::Evidence]);
}

#[test]
fn recording_only_one_dimension_is_refused() {
    let mut slices = clean_slices();
    slices[at("memory")].budget = Some(SegmentBudget {
        tokens: Some(500),
        chars: None,
        items: None,
        floor: None,
    });
    let verdict = evaluate_assembly(&plan(&slices));
    assert!(
        verdict
            .rejections
            .contains(&AssemblyRejection::BudgetDimensionMissing)
    );
}

#[test]
fn an_uncroppable_segment_without_a_floor_is_refused() {
    let mut slices = clean_slices();
    slices[at("persona")].budget = Some(SegmentBudget {
        tokens: Some(500),
        chars: Some(2_000),
        items: None,
        floor: None,
    });
    let verdict = evaluate_assembly(&plan(&slices));
    assert!(
        verdict
            .rejections
            .contains(&AssemblyRejection::UncroppableWithoutFloor)
    );
}

#[test]
fn exceeding_a_segment_budget_names_the_dimension() {
    let mut slices = clean_slices();
    slices[at("evidence")].budget = Some(SegmentBudget {
        tokens: Some(5),
        chars: Some(4_000),
        items: Some(2),
        floor: None,
    });
    slices[at("evidence")].usage = SegmentUsage {
        tokens: 9,
        chars: 40,
        items: 3,
    };
    let verdict = evaluate_assembly(&plan(&slices));
    assert!(
        verdict
            .rejections
            .contains(&AssemblyRejection::SegmentOverBudget)
    );
    // Which segment and which dimension is carried by the findings, not by the variant.
    let dimensions: Vec<BudgetDimension> = verdict
        .over_budget
        .iter()
        .filter(|finding| finding.segment == Segment::Evidence)
        .map(|finding| finding.dimension)
        .collect();
    assert_eq!(
        dimensions,
        [BudgetDimension::Tokens, BudgetDimension::Items]
    );
    assert_eq!(verdict.over_budget[0].limit, 5);
}

#[test]
fn usage_exactly_at_the_limit_is_accepted() {
    // A boundary guard: "over budget" means strictly over.
    let mut slices = clean_slices();
    slices[at("skills")].budget = Some(SegmentBudget {
        tokens: Some(10),
        chars: Some(40),
        items: Some(1),
        floor: None,
    });
    let verdict = evaluate_assembly(&plan(&slices));
    assert!(
        !verdict
            .rejections
            .contains(&AssemblyRejection::SegmentOverBudget)
    );
}

// ---------------------------------------------------------------------------
// H01: §41.3 tags
// ---------------------------------------------------------------------------

#[test]
fn a_fragment_without_a_source_reference_is_refused() {
    let fragments = [Fragment {
        source_ref: "",
        ..good_fragment()
    }];
    let mut slices = clean_slices();
    slices[at("evidence")].fragments = &fragments;
    let verdict = evaluate_assembly(&plan(&slices));
    assert!(
        verdict
            .rejections
            .contains(&AssemblyRejection::FragmentWithoutSourceRef)
    );
    assert_eq!(verdict.unprovenanced_fragments, ["f1"]);
}

#[test]
fn a_fragment_without_a_scope_is_refused() {
    let fragments = [Fragment {
        scope: None,
        ..good_fragment()
    }];
    let mut slices = clean_slices();
    slices[at("memory")].fragments = &fragments;
    let verdict = evaluate_assembly(&plan(&slices));
    assert!(
        verdict
            .rejections
            .contains(&AssemblyRejection::FragmentWithoutScope)
    );
    assert_eq!(verdict.unscoped_fragments, ["f1"]);
}

#[test]
fn a_missing_age_is_not_a_violation() {
    // §41.3 writes 「时效（如适用）」. Judging its absence would be over-reading the baseline, so
    // this test pins the choice rather than leaving it implicit.
    let fragments = [good_fragment()];
    let mut slices = clean_slices();
    slices[at("evidence")].fragments = &fragments;
    let verdict = evaluate_assembly(&plan(&slices));
    assert!(verdict.callable, "{:?}", verdict.rejections);
}

#[test]
fn an_untrusted_fragment_marked_as_an_instruction_is_refused() {
    for source in [
        SourceKind::Event,
        SourceKind::KnowledgeEvidence,
        SourceKind::ToolResult,
        SourceKind::ExternalAgent,
    ] {
        let fragments = [Fragment {
            source,
            trust: Trust::Instruction,
            ..good_fragment()
        }];
        let mut slices = clean_slices();
        slices[at("tool_results")].fragments = &fragments;
        let verdict = evaluate_assembly(&plan(&slices));
        assert!(
            verdict
                .rejections
                .contains(&AssemblyRejection::UntrustedMarkedAsInstruction),
            "{:?} must be data",
            source
        );
        assert_eq!(verdict.untrusted_as_instruction, ["f1"]);
    }
}

#[test]
fn the_same_fragment_tagged_as_data_is_fine() {
    // The positive half, without which "refuse every tagged fragment" would also pass.
    let fragments = [Fragment {
        source: SourceKind::Event,
        trust: Trust::Data,
        ..good_fragment()
    }];
    let mut slices = clean_slices();
    slices[at("event_summary")].fragments = &fragments;
    let verdict = evaluate_assembly(&plan(&slices));
    assert!(verdict.callable, "{:?}", verdict.rejections);
}

/// Union one verdict's refusals into `seen`. A function rather than a closure: the two ledger
/// tests below feed it verdicts that borrow different locals, and a closure cannot be generic
/// over those lifetimes.
fn collect_assembly(seen: &mut Vec<AssemblyRejection>, verdict: &AssemblyVerdict<'_>) {
    for rejection in &verdict.rejections {
        if !seen.contains(rejection) {
            seen.push(*rejection);
        }
    }
}

/// The H02 twin of [`collect_assembly`].
fn collect_rebuild(seen: &mut Vec<RebuildRejection>, verdict: &RebuildVerdict<'_>) {
    for rejection in &verdict.rejections {
        if !seen.contains(rejection) {
            seen.push(*rejection);
        }
    }
}

#[test]
fn every_rejection_is_constructible_and_none_is_unregistered() {
    // Fourteen plans, one per reason; the union must be exactly `ALL`. This proves each name is
    // reachable (no dead variant) and that no name was forgotten.
    let mut seen: Vec<AssemblyRejection> = Vec::new();

    {
        let mut slices = clean_slices();
        slices[0].segment = "nope";
        collect_assembly(&mut seen, &evaluate_assembly(&plan(&slices)));
    }
    {
        let mut slices = clean_slices();
        slices.swap(0, 1);
        collect_assembly(&mut seen, &evaluate_assembly(&plan(&slices)));
    }
    {
        let mut slices = clean_slices();
        slices.remove(0);
        collect_assembly(&mut seen, &evaluate_assembly(&plan(&slices)));
    }
    {
        let mut slices = clean_slices();
        slices[0].crop = Some(Crop {
            dropped: &["a"],
            reason: "r",
        });
        collect_assembly(&mut seen, &evaluate_assembly(&plan(&slices)));
    }
    {
        let mut slices = clean_slices();
        slices[8].crop = Some(Crop {
            dropped: &["a"],
            reason: "",
        });
        collect_assembly(&mut seen, &evaluate_assembly(&plan(&slices)));
    }
    {
        let mut slices = clean_slices();
        slices[8].crop = Some(Crop {
            dropped: &[],
            reason: "r",
        });
        collect_assembly(&mut seen, &evaluate_assembly(&plan(&slices)));
    }
    {
        let mut slices = clean_slices();
        slices[4].crop = Some(Crop {
            dropped: &["a"],
            reason: "r",
        });
        collect_assembly(&mut seen, &evaluate_assembly(&plan(&slices)));
    }
    {
        let mut slices = clean_slices();
        slices[4].budget = None;
        collect_assembly(&mut seen, &evaluate_assembly(&plan(&slices)));
    }
    {
        let mut slices = clean_slices();
        slices[4].budget = Some(SegmentBudget {
            tokens: Some(1),
            chars: None,
            items: None,
            floor: None,
        });
        collect_assembly(&mut seen, &evaluate_assembly(&plan(&slices)));
    }
    {
        let mut slices = clean_slices();
        slices[1].budget = Some(SegmentBudget {
            tokens: Some(1),
            chars: Some(1),
            items: None,
            floor: None,
        });
        collect_assembly(&mut seen, &evaluate_assembly(&plan(&slices)));
    }
    {
        let mut slices = clean_slices();
        slices[4].budget = Some(SegmentBudget {
            tokens: Some(1),
            chars: Some(1),
            items: None,
            floor: None,
        });
        collect_assembly(&mut seen, &evaluate_assembly(&plan(&slices)));
    }
    {
        let fragments = [Fragment {
            source_ref: "",
            ..good_fragment()
        }];
        let mut slices = clean_slices();
        slices[4].fragments = &fragments;
        collect_assembly(&mut seen, &evaluate_assembly(&plan(&slices)));
    }
    {
        let fragments = [Fragment {
            scope: None,
            ..good_fragment()
        }];
        let mut slices = clean_slices();
        slices[4].fragments = &fragments;
        collect_assembly(&mut seen, &evaluate_assembly(&plan(&slices)));
    }
    {
        let fragments = [Fragment {
            source: SourceKind::Event,
            trust: Trust::Instruction,
            ..good_fragment()
        }];
        let mut slices = clean_slices();
        slices[4].fragments = &fragments;
        collect_assembly(&mut seen, &evaluate_assembly(&plan(&slices)));
    }

    let mut expected = AssemblyRejection::ALL.to_vec();
    seen.sort_by_key(|rejection| rejection.label());
    expected.sort_by_key(|rejection| rejection.label());
    assert_eq!(seen, expected);
    // And the labels are distinct, so the set above is a set of names and not of coincidences.
    let mut labels: Vec<&str> = AssemblyRejection::ALL.iter().map(|r| r.label()).collect();
    labels.sort_unstable();
    let before = labels.len();
    labels.dedup();
    assert_eq!(labels.len(), before);
}

// ---------------------------------------------------------------------------
// H02: rebuilding from the decision log alone
// ---------------------------------------------------------------------------

const RECORDED: [&str; 3] = ["f1", "f2", "f3"];

fn record<'a>(ids: &'a [&'a str]) -> AssemblyRecord<'a> {
    AssemblyRecord {
        fragment_ids: ids,
        model_version: "llm-2026-09",
        prompt_version: "prompt-7",
    }
}

#[test]
fn the_assembly_record_fields_match_the_harness_l1_table() {
    // §41.5 fixes what the record carries; §47.1 fixes the L1 row of the observation table.
    // They are the same seven fields seen from two sides, and this test keeps them in step —
    // editing one table without the other fails here.
    assert_eq!(
        AssemblyRecord::logged_field_names(),
        ObservationLayer::Assembly.required_fields()
    );
}

#[test]
fn a_logged_assembly_rebuilds_into_the_same_sequence() {
    let logged = [
        LoggedFragment {
            id: "f1",
            source_ref: "kb0#p1",
            content: Some("price is 19"),
        },
        LoggedFragment {
            id: "f2",
            source_ref: "kb0#p2",
            content: Some("in stock"),
        },
        LoggedFragment {
            id: "f3",
            source_ref: "kb0#p3",
            content: Some("ships today"),
        },
    ];
    let verdict = rebuild_assembly(&record(&RECORDED), &logged);
    assert!(verdict.rebuildable, "{:?}", verdict.rejections);
    let ids: Vec<&str> = verdict.rebuilt.iter().map(|f| f.id).collect();
    assert_eq!(ids, RECORDED);
    assert_eq!(verdict.rebuilt[0].source_ref, "kb0#p1");
}

#[test]
fn a_fragment_that_was_never_logged_is_refused() {
    let logged = [
        LoggedFragment {
            id: "f1",
            source_ref: "kb0#p1",
            content: Some("price is 19"),
        },
        LoggedFragment {
            id: "f3",
            source_ref: "kb0#p3",
            content: Some("ships today"),
        },
    ];
    let verdict = rebuild_assembly(&record(&RECORDED), &logged);
    assert!(!verdict.rebuildable);
    assert!(
        verdict
            .rejections
            .contains(&RebuildRejection::FragmentNotLogged)
    );
    assert_eq!(verdict.unlogged_ids, ["f2"]);
}

#[test]
fn a_fragment_whose_body_was_not_logged_is_refused() {
    // The entry exists — only a pointer into live working memory. This is the sample that makes
    // "must not depend on the memory of the moment" decidable.
    let logged = [
        LoggedFragment {
            id: "f1",
            source_ref: "kb0#p1",
            content: Some("price is 19"),
        },
        LoggedFragment {
            id: "f2",
            source_ref: "kb0#p2",
            content: None,
        },
        LoggedFragment {
            id: "f3",
            source_ref: "kb0#p3",
            content: Some("ships today"),
        },
    ];
    let verdict = rebuild_assembly(&record(&RECORDED), &logged);
    assert!(
        verdict
            .rejections
            .contains(&RebuildRejection::FragmentContentNotInLog)
    );
    assert_eq!(verdict.contentless_ids, ["f2"]);
    // And it is a *different* reason from "never logged" — the two are not merged.
    assert!(
        !verdict
            .rejections
            .contains(&RebuildRejection::FragmentNotLogged)
    );
}

#[test]
fn a_log_whose_order_differs_from_the_record_is_refused() {
    // Every fragment is present and every body is logged, so the only thing wrong is the order.
    // An implementation that sorts by the record before comparing cannot fail this test.
    let logged = [
        LoggedFragment {
            id: "f1",
            source_ref: "kb0#p1",
            content: Some("price is 19"),
        },
        LoggedFragment {
            id: "f3",
            source_ref: "kb0#p3",
            content: Some("ships today"),
        },
        LoggedFragment {
            id: "f2",
            source_ref: "kb0#p2",
            content: Some("in stock"),
        },
    ];
    let verdict = rebuild_assembly(&record(&RECORDED), &logged);
    assert!(
        verdict
            .rejections
            .contains(&RebuildRejection::RebuildOrderDiffers)
    );
    assert!(verdict.unlogged_ids.is_empty());
    assert!(verdict.contentless_ids.is_empty());
}

#[test]
fn a_record_without_versions_is_refused() {
    let logged = [LoggedFragment {
        id: "f1",
        source_ref: "kb0#p1",
        content: Some("price is 19"),
    }];
    let bare = AssemblyRecord {
        fragment_ids: &["f1"],
        model_version: " ",
        prompt_version: "",
    };
    let verdict = rebuild_assembly(&bare, &logged);
    assert!(
        verdict
            .rejections
            .contains(&RebuildRejection::ModelVersionMissing)
    );
    assert!(
        verdict
            .rejections
            .contains(&RebuildRejection::PromptVersionMissing)
    );
}

#[test]
fn a_credential_in_a_logged_body_is_refused() {
    let logged = [LoggedFragment {
        id: "f1",
        source_ref: "kb0#p1",
        content: Some("authorization: bearer sk-live-123"),
    }];
    let verdict = rebuild_assembly(&record(&["f1"]), &logged);
    assert!(
        verdict
            .rejections
            .contains(&RebuildRejection::PlaintextCredential)
    );
    assert_eq!(verdict.credential_ids, ["f1"]);
}

#[test]
fn a_field_name_that_looks_sensitive_is_not_a_credential() {
    // The marker table matches **values**, never field names — the same discipline W66 pinned
    // for H15. A body that merely mentions `api_key` without a marker must pass.
    let logged = [LoggedFragment {
        id: "f1",
        source_ref: "kb0#p1",
        content: Some("the api_key field is documented separately"),
    }];
    let verdict = rebuild_assembly(&record(&["f1"]), &logged);
    assert!(verdict.rebuildable, "{:?}", verdict.rejections);
}

/// Two fragments with real bodies — the pool the per-entry tests split and regroup.
fn two_bodies() -> Vec<LoggedFragment<'static>> {
    vec![
        LoggedFragment {
            id: "f1",
            source_ref: "kb0#p1",
            content: Some("price is 19"),
        },
        LoggedFragment {
            id: "f2",
            source_ref: "kb0#p2",
            content: Some("in stock"),
        },
    ]
}

#[test]
fn an_entry_carries_the_assembly_by_multiset_and_ignores_order() {
    let full = two_bodies();
    let both = full.as_slice();

    assert!(entry_carries_assembly(&record(&["f1", "f2"]), both));
    assert!(
        entry_carries_assembly(&record(&["f2", "f1"]), both),
        "order is not part of carrying — that is what keeps order_differs reachable"
    );
    assert!(
        !entry_carries_assembly(&record(&["f1"]), both),
        "an entry holding *more* is a different assembly, not this one"
    );
    assert!(
        !entry_carries_assembly(&record(&["f1", "f9"]), both),
        "an id the entry never held is not carried by it"
    );

    let single = [full[0].clone()];
    assert!(
        !entry_carries_assembly(&record(&["f1", "f1"]), &single),
        "the entry holds only one fragment; the record names two"
    );
    // Equal lengths, different multiplicities: this is the case the length guard above does **not**
    // catch, so it is what actually exercises the one-to-one pairing. A `set` reading (or a
    // `contains`-per-id reading that lets one entry fragment answer for two record ids) calls these
    // equal — an injection that did exactly that was caught by this line alone.
    assert!(
        !entry_carries_assembly(&record(&["f1", "f1"]), both),
        "the record names f1 twice while the entry holds f1 and f2 — the multiplicities differ"
    );
}

#[test]
fn a_split_across_two_entries_is_refused_by_name_and_nothing_is_called_unlogged() {
    let full = two_bodies();
    let half_a = [full[0].clone()];
    let half_b = [full[1].clone()];

    let verdict =
        rebuild_assembly_from_entries(&record(&["f1", "f2"]), &[&half_a[..], &half_b[..]]);
    assert!(!verdict.rebuildable, "no entry ever held this context");
    assert!(
        verdict
            .rejections
            .contains(&RebuildRejection::NotInOneEntry),
        "{:?}",
        verdict.rejections
    );
    assert!(
        verdict.unlogged_ids.is_empty(),
        "both ids *are* in the chain; 'not logged' would be false: {:?}",
        verdict.unlogged_ids
    );
    assert!(verdict.rebuilt.is_empty());
}

#[test]
fn one_carrying_entry_that_reproduces_the_record_is_enough() {
    let full = two_bodies();
    let wrong_order = [full[1].clone(), full[0].clone()];
    let right_order = [full[0].clone(), full[1].clone()];

    let verdict = rebuild_assembly_from_entries(
        &record(&["f1", "f2"]),
        &[&wrong_order[..], &right_order[..]],
    );
    assert!(
        verdict.rebuildable,
        "one entry that reproduces it is enough for the chain to reproduce it: {:?}",
        verdict.rejections
    );
    assert!(verdict.rejections.is_empty());
    assert_eq!(
        verdict
            .rebuilt
            .iter()
            .map(|fragment| fragment.id)
            .collect::<Vec<_>>(),
        vec!["f1", "f2"],
        "the sequence comes from the entry that reproduced it"
    );

    // With only the wrong-ordered entry, the specific reason is still the specific one — the
    // existential rule must not swallow `order_differs` into a vaguer answer.
    let only_wrong = rebuild_assembly_from_entries(&record(&["f1", "f2"]), &[&wrong_order[..]]);
    assert!(!only_wrong.rebuildable);
    assert!(
        only_wrong
            .rejections
            .contains(&RebuildRejection::RebuildOrderDiffers),
        "{:?}",
        only_wrong.rejections
    );
    assert!(
        !only_wrong
            .rejections
            .contains(&RebuildRejection::NotInOneEntry),
        "an entry *does* carry it; the assembly is simply not reproducible from it"
    );
}

#[test]
fn an_empty_chain_names_every_id_unlogged_and_not_in_one_entry() {
    let verdict = rebuild_assembly_from_entries(&record(&["f1", "f2"]), &[]);
    assert!(!verdict.rebuildable);
    assert_eq!(
        verdict.unlogged_ids,
        vec!["f1".to_string(), "f2".to_string()]
    );
    assert!(
        verdict
            .rejections
            .contains(&RebuildRejection::FragmentNotLogged)
    );
    assert!(
        verdict
            .rejections
            .contains(&RebuildRejection::NotInOneEntry),
        "an empty chain is also a chain with no carrying entry: {:?}",
        verdict.rejections
    );
}

#[test]
fn every_rebuild_rejection_is_constructible_and_none_is_unregistered() {
    let mut seen: Vec<RebuildRejection> = Vec::new();

    let full = [
        LoggedFragment {
            id: "f1",
            source_ref: "kb0#p1",
            content: Some("ok"),
        },
        LoggedFragment {
            id: "f2",
            source_ref: "kb0#p2",
            content: Some("ok"),
        },
        LoggedFragment {
            id: "f3",
            source_ref: "kb0#p3",
            content: Some("ok"),
        },
    ];
    // FragmentNotLogged
    collect_rebuild(&mut seen, &rebuild_assembly(&record(&["f9"]), &full));
    // FragmentContentNotInLog
    let contentless = [
        LoggedFragment {
            id: "f1",
            source_ref: "kb0#p1",
            content: None,
        },
        full[1].clone(),
        full[2].clone(),
    ];
    collect_rebuild(
        &mut seen,
        &rebuild_assembly(&record(&RECORDED), &contentless),
    );
    // RebuildOrderDiffers
    let reordered = [full[1].clone(), full[0].clone(), full[2].clone()];
    collect_rebuild(&mut seen, &rebuild_assembly(&record(&RECORDED), &reordered));
    // ModelVersionMissing / PromptVersionMissing
    collect_rebuild(
        &mut seen,
        &rebuild_assembly(
            &AssemblyRecord {
                fragment_ids: &["f1"],
                model_version: "",
                prompt_version: "",
            },
            &full,
        ),
    );
    // PlaintextCredential
    let secret = [LoggedFragment {
        id: "f1",
        source_ref: "kb0#p1",
        content: Some("token=abc"),
    }];
    collect_rebuild(&mut seen, &rebuild_assembly(&record(&["f1"]), &secret));
    // NotInOneEntry — the chain holds every id, but no single entry holds this assembly.
    let half_a = [full[0].clone()];
    let half_b = [full[1].clone(), full[2].clone()];
    collect_rebuild(
        &mut seen,
        &rebuild_assembly_from_entries(&record(&RECORDED), &[&half_a[..], &half_b[..]]),
    );

    let mut expected = RebuildRejection::ALL.to_vec();
    seen.sort_by_key(|rejection| rejection.label());
    expected.sort_by_key(|rejection| rejection.label());
    assert_eq!(seen, expected);
}
