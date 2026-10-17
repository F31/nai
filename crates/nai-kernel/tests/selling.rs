//! W68: basic selling loop (B07), V3.9 §53 / §5.3 / §5.7 / §5.4 / §45.3.
//!
//! The samples here are chosen so that a plausible-but-wrong implementation fails:
//!
//! * a checker that compares the segment **set** (sorted) instead of the sequence,
//! * a checker that reports only the first missing segment,
//! * a checker that treats an undefined segment name as a known one,
//! * a checkpoint model without a `carried_text` field (the "unplayed text" violation would be
//!   inexpressible, making the guard vacuous),
//! * a fact check that only asks "is a reference present" instead of "is it the revision in
//!   force" — its sample is a **same object, different revision** claim, because a sample that
//!   changed the object id would let the wrong implementation pass,
//! * an interval rule evaluated on a decreasing series, where the gap underflows.

use nai_kernel::evaluate::selling::{
    ActionCue, CLAIM_KINDS, Checkpoint, Claim, ClaimKind, Entrance, FactRef, PitchHistory,
    PitchPlan, PitchPolicy, PitchRejection, PitchVerdict, SEGMENT_ORDER, SegmentKind,
    evaluate_pitch, evaluate_transition,
};

const COOLDOWN_MS: u32 = 1_000;
const MIN_SWITCH_MS: u32 = 100;

/// §53's four segments, as a composition plan carries them (names, not enum values — an
/// undefined name has to be expressible).
const CLEAN_SEGMENTS: [&str; 4] = ["selling", "spec", "scenario", "after_sales"];
const NO_ENTRANCES: [Entrance; 0] = [];
const NO_CUES: [ActionCue; 0] = [];

fn fact(object: &str, revision: &str) -> FactRef {
    FactRef {
        object_id: object.to_string(),
        revision: revision.to_string(),
    }
}

fn policy(current: &[FactRef]) -> PitchPolicy<'_> {
    PitchPolicy {
        rule_version: "pitch-rule-v1",
        cooldown_ms: COOLDOWN_MS,
        min_switch_ms: MIN_SWITCH_MS,
        current_facts: current,
    }
}

fn plan<'a>(
    segments: &'a [&'a str],
    entrances: &'a [Entrance],
    cues: &'a [ActionCue],
) -> PitchPlan<'a> {
    PitchPlan {
        segments,
        checkpoint: None,
        resume: None,
        entrances,
        cues,
    }
}

fn judge<'a>(plan: &PitchPlan<'a>, current: &[FactRef]) -> PitchVerdict {
    evaluate_pitch(plan, &policy(current))
}

// ---------------------------------------------------------------------------
// Baseline tables: pinned by content, not by count
// ---------------------------------------------------------------------------

#[test]
fn segment_order_matches_the_baseline_line() {
    let labels: Vec<&str> = SEGMENT_ORDER.iter().map(|s| s.label()).collect();
    assert_eq!(
        labels,
        vec!["selling", "spec", "scenario", "after_sales"],
        "§53: 卖点 / 规格 / 适用场景 / 售后说明，顺序是这条要求的一半"
    );
    for (index, segment) in SEGMENT_ORDER.iter().enumerate() {
        assert_eq!(segment.index(), index);
        assert_eq!(SegmentKind::parse(segment.label()), Some(*segment));
    }
    assert_eq!(SegmentKind::parse("bogus"), None);
}

#[test]
fn claim_kinds_match_the_disabled_domain_line() {
    let labels: Vec<&str> = CLAIM_KINDS.iter().map(|k| k.label()).collect();
    assert_eq!(
        labels,
        vec![
            "price",
            "stock",
            "promotion",
            "commitment",
            "personal_commitment"
        ],
        "§45.3 的禁用域是五类，逐项照抄"
    );
}

#[test]
fn rejection_labels_are_distinct() {
    let mut labels: Vec<&str> = PitchRejection::ALL.iter().map(|r| r.label()).collect();
    let count = labels.len();
    labels.sort();
    labels.dedup();
    assert_eq!(labels.len(), count, "两个拒绝类别共用一个日志标签");
}

// ---------------------------------------------------------------------------
// The ledger: every named refusal is reachable, and nothing is unregistered
// ---------------------------------------------------------------------------

#[test]
fn every_rejection_is_constructible_and_none_is_unregistered() {
    // One plan that breaks composition, the checkpoint and the resume at once…
    let segs: [&str; 3] = ["spec", "selling", "bogus"];
    let checkpoint = Checkpoint {
        segment: SegmentKind::Spec,
        completed_sentence_ids: vec!["s-1".to_string()],
        carried_text: Some("unplayed".to_string()),
    };
    let decreasing: [Entrance; 2] = [
        Entrance {
            segment: SegmentKind::Selling,
            at_ms: 5_000,
        },
        Entrance {
            segment: SegmentKind::Spec,
            at_ms: 4_000,
        },
    ];
    let cues: [ActionCue; 1] = [ActionCue {
        cue_id: "cue-1".to_string(),
        claims: vec![
            Claim {
                kind: ClaimKind::Price,
                fact_ref: None,
            },
            Claim {
                kind: ClaimKind::Stock,
                fact_ref: Some(fact("sku-1", "rev-old")),
            },
        ],
    }];
    let mut mixed = plan(&segs, &decreasing, &cues);
    mixed.checkpoint = Some(&checkpoint);
    mixed.resume = Some(SegmentKind::AfterSales);

    // …and one whose entrances are ordered, so the interval rules (gated on that precondition)
    // are reachable at all.
    let ordered_tight: [Entrance; 2] = [
        Entrance {
            segment: SegmentKind::Selling,
            at_ms: 0,
        },
        Entrance {
            segment: SegmentKind::Selling,
            at_ms: 50,
        },
    ];
    let tight = plan(&CLEAN_SEGMENTS, &ordered_tight, &NO_CUES);

    let current = [fact("sku-1", "rev-3")];
    let a = judge(&mixed, &current);
    let b = judge(&tight, &current);

    let mut seen: Vec<PitchRejection> = a.rejections.clone();
    seen.extend(b.rejections.iter().copied());
    seen.sort();
    seen.dedup();

    let mut all = PitchRejection::ALL.to_vec();
    all.sort();
    assert_eq!(
        seen, all,
        "构造出的拒绝集合必须恰为 ALL：既证每条可构造，也防未登记的名字"
    );
}

// ---------------------------------------------------------------------------
// §53 row 1: composition
// ---------------------------------------------------------------------------

#[test]
fn a_complete_plan_in_order_is_allowed() {
    let verdict = judge(&plan(&CLEAN_SEGMENTS, &NO_ENTRANCES, &NO_CUES), &[]);
    assert!(verdict.allowed, "{:?}", verdict.rejections);
    assert!(verdict.rejections.is_empty());
    assert!(verdict.missing_segments.is_empty());
}

#[test]
fn a_missing_segment_is_refused_and_named() {
    let segs: [&str; 2] = ["selling", "spec"];
    let verdict = judge(&plan(&segs, &NO_ENTRANCES, &NO_CUES), &[]);
    assert!(verdict.rejections.contains(&PitchRejection::SegmentMissing));
    assert_eq!(
        verdict.missing_segments,
        vec!["scenario", "after_sales"],
        "缺段必须**指名**，且不得只报第一个"
    );
    assert!(
        !verdict
            .rejections
            .contains(&PitchRejection::SegmentOutOfOrder)
    );
}

#[test]
fn the_right_segments_in_the_wrong_order_are_refused() {
    let segs: [&str; 4] = ["spec", "selling", "scenario", "after_sales"];
    let verdict = judge(&plan(&segs, &NO_ENTRANCES, &NO_CUES), &[]);
    assert!(
        verdict
            .rejections
            .contains(&PitchRejection::SegmentOutOfOrder),
        "四段齐备但顺序错——排序后比较集合的实现会放行，故这条必须由序列判定"
    );
    assert!(verdict.missing_segments.is_empty(), "四段都在，不应报缺段");
}

#[test]
fn an_undefined_segment_name_is_refused() {
    let segs: [&str; 5] = ["selling", "spec", "scenario", "after_sales", "bogus"];
    let verdict = judge(&plan(&segs, &NO_ENTRANCES, &NO_CUES), &[]);
    assert!(verdict.rejections.contains(&PitchRejection::UnknownSegment));
    assert_eq!(verdict.unknown_segments, vec!["bogus".to_string()]);
    assert!(verdict.missing_segments.is_empty());
}

#[test]
fn a_plan_that_is_short_and_out_of_order_reports_both() {
    let segs: [&str; 3] = ["spec", "selling", "bogus"];
    let verdict = judge(&plan(&segs, &NO_ENTRANCES, &NO_CUES), &[]);
    assert!(verdict.rejections.contains(&PitchRejection::UnknownSegment));
    assert!(verdict.rejections.contains(&PitchRejection::SegmentMissing));
    assert!(
        verdict
            .rejections
            .contains(&PitchRejection::SegmentOutOfOrder)
    );
    assert_eq!(verdict.missing_segments, vec!["scenario", "after_sales"]);
}

// ---------------------------------------------------------------------------
// §5.7: checkpoint and interjection return
// ---------------------------------------------------------------------------

#[test]
fn a_checkpoint_carrying_unplayed_text_is_refused() {
    let checkpoint = Checkpoint {
        segment: SegmentKind::Spec,
        completed_sentence_ids: vec!["s-1".to_string()],
        carried_text: Some("这段话还没播出去".to_string()),
    };
    let mut p = plan(&CLEAN_SEGMENTS, &NO_ENTRANCES, &NO_CUES);
    p.checkpoint = Some(&checkpoint);
    p.resume = Some(SegmentKind::Spec);
    let verdict = judge(&p, &[]);
    assert!(
        verdict
            .rejections
            .contains(&PitchRejection::CheckpointHoldsUnspokenText),
        "§5.7：检查点保存段落与已完成句，不保存未播出的生成文本"
    );
}

#[test]
fn a_clean_checkpoint_resumes_on_its_own_segment() {
    let checkpoint = Checkpoint {
        segment: SegmentKind::Scenario,
        completed_sentence_ids: vec!["s-1".to_string(), "s-2".to_string()],
        carried_text: None,
    };
    let mut p = plan(&CLEAN_SEGMENTS, &NO_ENTRANCES, &NO_CUES);
    p.checkpoint = Some(&checkpoint);
    p.resume = Some(SegmentKind::Scenario);
    assert!(judge(&p, &[]).allowed);
}

#[test]
fn a_resume_off_the_checkpoint_segment_is_refused() {
    let checkpoint = Checkpoint {
        segment: SegmentKind::Scenario,
        completed_sentence_ids: vec![],
        carried_text: None,
    };
    let mut p = plan(&CLEAN_SEGMENTS, &NO_ENTRANCES, &NO_CUES);
    p.checkpoint = Some(&checkpoint);
    p.resume = Some(SegmentKind::AfterSales);
    let verdict = judge(&p, &[]);
    assert!(
        verdict
            .rejections
            .contains(&PitchRejection::ResumeDivergesFromCheckpoint),
        "落在**另一个**合法段落仍然不是归位"
    );
}

// ---------------------------------------------------------------------------
// §53 row 3: cooldown and switching
// ---------------------------------------------------------------------------

#[test]
fn a_decreasing_entrance_series_is_refused_by_name() {
    let entrances: [Entrance; 2] = [
        Entrance {
            segment: SegmentKind::Selling,
            at_ms: 5_000,
        },
        Entrance {
            segment: SegmentKind::Spec,
            at_ms: 4_000,
        },
    ];
    let verdict = judge(&plan(&CLEAN_SEGMENTS, &entrances, &NO_CUES), &[]);
    assert!(
        verdict
            .rejections
            .contains(&PitchRejection::EntranceTimesDecrease),
        "趋势不得靠重排制造"
    );
    // The interval rules are statements about a gap, and a gap is undefined on a decreasing
    // series; the precondition is checked first so no bogus interval refusal (or underflow) is
    // produced. Failing the precondition is never read as a pass.
    assert!(!verdict.rejections.contains(&PitchRejection::SwitchTooSoon));
    assert!(
        !verdict
            .rejections
            .contains(&PitchRejection::CooldownNotElapsed)
    );
}

#[test]
fn re_entering_a_segment_before_its_cooldown_is_refused() {
    // Same segment, gap 500ms: above the switch interval (100) but below the cooldown (1000).
    let entrances: [Entrance; 2] = [
        Entrance {
            segment: SegmentKind::Selling,
            at_ms: 0,
        },
        Entrance {
            segment: SegmentKind::Selling,
            at_ms: 500,
        },
    ];
    let verdict = judge(&plan(&CLEAN_SEGMENTS, &entrances, &NO_CUES), &[]);
    assert!(
        verdict
            .rejections
            .contains(&PitchRejection::CooldownNotElapsed)
    );
    assert_eq!(verdict.cooldown_violations, vec!["selling"]);
    assert!(
        !verdict.rejections.contains(&PitchRejection::SwitchTooSoon),
        "切换间隔本身就是满足的——两条规则必须分开判定"
    );
}

#[test]
fn switching_too_soon_is_a_separate_refusal() {
    // Different segments, gap 50ms: no cooldown applies, but the switch interval is broken.
    let min_switch = 1_000;
    let entrances: [Entrance; 2] = [
        Entrance {
            segment: SegmentKind::Selling,
            at_ms: 0,
        },
        Entrance {
            segment: SegmentKind::Spec,
            at_ms: 50,
        },
    ];
    let policy = PitchPolicy {
        rule_version: "pitch-rule-v1",
        cooldown_ms: 5_000,
        min_switch_ms: min_switch,
        current_facts: &[],
    };
    let verdict = evaluate_pitch(&plan(&CLEAN_SEGMENTS, &entrances, &NO_CUES), &policy);
    assert!(verdict.rejections.contains(&PitchRejection::SwitchTooSoon));
    assert!(
        !verdict
            .rejections
            .contains(&PitchRejection::CooldownNotElapsed),
        "不同段落之间没有段落级冷却"
    );
    assert!(verdict.cooldown_violations.is_empty());
}

// ---------------------------------------------------------------------------
// §53 row 4: fact-bound action cues
// ---------------------------------------------------------------------------

#[test]
fn a_constrained_claim_with_no_fact_behind_it_is_refused() {
    let cues: [ActionCue; 1] = [ActionCue {
        cue_id: "cue-7".to_string(),
        claims: vec![Claim {
            kind: ClaimKind::Promotion,
            fact_ref: None,
        }],
    }];
    let verdict = judge(&plan(&CLEAN_SEGMENTS, &NO_ENTRANCES, &cues), &[]);
    assert!(
        verdict
            .rejections
            .contains(&PitchRejection::ActionCueWithoutFactBinding)
    );
    assert_eq!(
        verdict.unbound_claims,
        vec![("cue-7".to_string(), ClaimKind::Promotion)]
    );
    assert!(verdict.stale_claims.is_empty());
}

#[test]
fn a_claim_standing_on_a_revision_no_longer_in_force_is_refused() {
    // **Same object, different revision.** A checker that only tests "is a reference present",
    // or that compares object ids and stops there, passes this sample unchanged — so the sample
    // has to move the revision, not the object.
    let cues: [ActionCue; 1] = [ActionCue {
        cue_id: "cue-9".to_string(),
        claims: vec![Claim {
            kind: ClaimKind::Price,
            fact_ref: Some(fact("sku-1", "rev-old")),
        }],
    }];
    let current = [fact("sku-1", "rev-3")];
    let verdict = judge(&plan(&CLEAN_SEGMENTS, &NO_ENTRANCES, &cues), &current);
    assert!(
        verdict
            .rejections
            .contains(&PitchRejection::ActionCueFactStale)
    );
    assert_eq!(
        verdict.stale_claims,
        vec![("cue-9".to_string(), ClaimKind::Price)]
    );
    assert!(verdict.unbound_claims.is_empty());
}

#[test]
fn a_claim_bound_to_the_revision_in_force_passes() {
    let cues: [ActionCue; 2] = [
        ActionCue {
            cue_id: "cue-9".to_string(),
            claims: vec![Claim {
                kind: ClaimKind::Price,
                fact_ref: Some(fact("sku-1", "rev-3")),
            }],
        },
        ActionCue {
            cue_id: "cue-10".to_string(),
            claims: vec![Claim {
                kind: ClaimKind::Commitment,
                fact_ref: Some(fact("policy-2", "rev-1")),
            }],
        },
    ];
    let current = [fact("sku-1", "rev-3"), fact("policy-2", "rev-1")];
    let verdict = judge(&plan(&CLEAN_SEGMENTS, &NO_ENTRANCES, &cues), &current);
    assert!(verdict.allowed, "{:?}", verdict.rejections);
}

#[test]
fn a_cue_with_no_constrained_claim_needs_no_binding() {
    // An empty claim list is not a constrained assertion; nothing to bind.
    let cues: [ActionCue; 1] = [ActionCue {
        cue_id: "cue-11".to_string(),
        claims: vec![],
    }];
    assert!(judge(&plan(&CLEAN_SEGMENTS, &NO_ENTRANCES, &cues), &[]).allowed);
}

// ---------------------------------------------------------------------------
// W136: cooldown is about the segment's own history, not about adjacent pairs
// ---------------------------------------------------------------------------

#[test]
fn a_repeat_with_another_segment_in_between_still_hits_the_cooldown() {
    // The discriminating sample: **no adjacent pair repeats a segment**, so a checker written as
    // `windows(2) + same segment` sees nothing here. §53 says 「同一段落的重复进入」, which is a
    // statement about the segment's own last entry — `selling` came back at 20 while its cooldown
    // is 1000, and the fact that `spec` ran in between does not reset it.
    let entrances: [Entrance; 3] = [
        Entrance {
            segment: SegmentKind::Selling,
            at_ms: 0,
        },
        Entrance {
            segment: SegmentKind::Spec,
            at_ms: 600,
        },
        Entrance {
            segment: SegmentKind::Selling,
            at_ms: 900,
        },
    ];
    let verdict = judge(&plan(&CLEAN_SEGMENTS, &entrances, &NO_CUES), &[]);
    assert!(
        verdict
            .rejections
            .contains(&PitchRejection::CooldownNotElapsed),
        "同段重复的冷却按该段自己的上次进入算，中间插了别的段不重置: {:?}",
        verdict.rejections
    );
    assert_eq!(verdict.cooldown_violations, vec!["selling"]);
    assert!(
        !verdict.rejections.contains(&PitchRejection::SwitchTooSoon),
        "切换间隔本身满足（600ms / 300ms 均 ≥ 100ms）——两条规则必须分开判定"
    );
}

#[test]
fn a_repeat_exactly_at_the_cooldown_boundary_is_allowed() {
    // The other side of the same boundary: `<` not `<=`. A guard that also refused the boundary
    // would be answering a different question than §53's 「不早于」.
    let entrances: [Entrance; 2] = [
        Entrance {
            segment: SegmentKind::Selling,
            at_ms: 0,
        },
        Entrance {
            segment: SegmentKind::Selling,
            at_ms: COOLDOWN_MS,
        },
    ];
    let verdict = judge(&plan(&CLEAN_SEGMENTS, &entrances, &NO_CUES), &[]);
    assert!(verdict.allowed, "{:?}", verdict.rejections);
    assert!(verdict.cooldown_violations.is_empty());
}

#[test]
fn the_incremental_transition_agrees_with_the_batch_verdict() {
    // A live loop advances one entry at a time; the batch judge replays the whole timeline. Both
    // call `evaluate_transition`, and this pins that they answer the same thing for the same
    // timeline — otherwise the running loop and the audit would disagree.
    let timeline: [Entrance; 3] = [
        Entrance {
            segment: SegmentKind::Selling,
            at_ms: 0,
        },
        Entrance {
            segment: SegmentKind::Spec,
            at_ms: 600,
        },
        Entrance {
            segment: SegmentKind::Selling,
            at_ms: 900,
        },
    ];
    let policy = policy(&[]);
    let mut history = PitchHistory::default();
    let mut incremental = Vec::new();
    for entrance in timeline {
        for rejection in evaluate_transition(&history, entrance, &policy) {
            if !incremental.contains(&rejection) {
                incremental.push(rejection);
            }
        }
        history.record(entrance);
    }
    let batch = evaluate_pitch(&plan(&CLEAN_SEGMENTS, &timeline, &NO_CUES), &policy);
    assert_eq!(incremental, batch.rejections);
}

#[test]
fn the_first_entrance_has_no_interval_rule_to_break() {
    // With no history there is no previous entry and no previous entry of the segment. A
    // transition judge that reported a refusal here would be reading "no history" as a violation.
    let policy = policy(&[]);
    let history = PitchHistory::default();
    let refusals = evaluate_transition(
        &history,
        Entrance {
            segment: SegmentKind::AfterSales,
            at_ms: 0,
        },
        &policy,
    );
    assert!(refusals.is_empty(), "{refusals:?}");
    assert_eq!(history.last(), None);
    assert_eq!(history.last_entry_of(SegmentKind::AfterSales), None);
}

#[test]
fn a_decreasing_transition_reports_only_the_monotonic_refusal() {
    // The interval rules are undefined on a decreasing series. Reporting `SwitchTooSoon` here
    // would be an underflow artefact (`u32` subtraction) dressed up as a rule violation.
    let policy = policy(&[]);
    let mut history = PitchHistory::default();
    history.record(Entrance {
        segment: SegmentKind::Selling,
        at_ms: 5_000,
    });
    let refusals = evaluate_transition(
        &history,
        Entrance {
            segment: SegmentKind::Selling,
            at_ms: 4_000,
        },
        &policy,
    );
    assert_eq!(refusals, vec![PitchRejection::EntranceTimesDecrease]);
}

#[test]
fn the_history_keeps_the_latest_entry_of_each_segment() {
    // Folding "what happened" into "what was allowed" would let a violation erase its own
    // evidence; recording is unconditional and keeps the later time.
    let mut history = PitchHistory::default();
    history.record(Entrance {
        segment: SegmentKind::Spec,
        at_ms: 300,
    });
    history.record(Entrance {
        segment: SegmentKind::Selling,
        at_ms: 900,
    });
    history.record(Entrance {
        segment: SegmentKind::Spec,
        at_ms: 1_400,
    });
    assert_eq!(history.last_entry_of(SegmentKind::Spec), Some(1_400));
    assert_eq!(history.last_entry_of(SegmentKind::Selling), Some(900));
    assert_eq!(history.last_entry_of(SegmentKind::Scenario), None);
    assert_eq!(
        history.last(),
        Some(Entrance {
            segment: SegmentKind::Spec,
            at_ms: 1_400
        })
    );
}
