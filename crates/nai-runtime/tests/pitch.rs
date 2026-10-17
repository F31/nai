//! W136: B07's **executor** — the selling loop as something that runs, not a verdict on a
//! snapshot.
//!
//! W68's judge answers 「这份编排计划合规吗」. These tests cover the layer that advances one entry
//! at a time, and the two things that layer must never do:
//!
//! * **advance after a refusal** — a refused step leaves the timeline and the cursor untouched,
//!   so a caller that ignores the error cannot end up ahead of the rules;
//! * **treat an unmeasured face as a pass** — §5.7's resume re-check is an input, and
//!   `Unavailable` is a refusal. The five faces are named individually, because "the re-check
//!   failed" would hide *which* fact is no longer true.
//!
//! The rules are the kernel's ([`evaluate_transition`]), not a second copy: the runtime-level
//! cooldown test below drives the same timeline the kernel test drives.
mod common;
use common::{ACTOR, PROFILE, WORKSPACE, provision, runtime_profile};

use nai_contracts::v1::ErrorCode;
use nai_kernel::evaluate::selling::{
    ActionCue, Claim, ClaimKind, Entrance, FactRef, PitchRejection, SegmentKind,
};
use nai_runtime::{
    Composition, PitchInput, PitchRefusal, PitchRunner, RECHECK_FACES, RecheckFace, RecheckOutcome,
    ResumeRecheck,
};
use nai_storage::DataPaths;
use std::path::Path;
use tempfile::tempdir;

const COOLDOWN_MS: u32 = 1_000;
const MIN_SWITCH_MS: u32 = 100;

/// §53's four segments, as a plan carries them.
const SEGMENTS: [&str; 4] = ["selling", "spec", "scenario", "after_sales"];
const NO_ENTRANCES: [Entrance; 0] = [];
const NO_CUES: [ActionCue; 0] = [];

fn fact(object: &str, revision: &str) -> FactRef {
    FactRef {
        object_id: object.to_string(),
        revision: revision.to_string(),
    }
}

/// A clean plan at the shared thresholds.
fn clean_input<'a>(
    entrances: &'a [Entrance],
    cues: &'a [ActionCue],
    facts: &'a [FactRef],
) -> PitchInput<'a> {
    PitchInput {
        segments: &SEGMENTS,
        checkpoint: None,
        resume: None,
        entrances,
        cues,
        rule_version: "pitch-rule-v1",
        cooldown_ms: COOLDOWN_MS,
        min_switch_ms: MIN_SWITCH_MS,
        current_facts: facts,
    }
}

fn assembled(root: &Path) -> Composition {
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

// ---------------------------------------------------------------------------
// The plan is judged before the loop exists
// ---------------------------------------------------------------------------

#[test]
fn a_plan_that_could_never_run_never_starts() {
    // One segment short. The refusal names the rule **and** the loop never comes into being, so
    // there is no half-started loop to clean up.
    let broken: [&str; 3] = ["selling", "spec", "scenario"];
    let mut input = clean_input(&NO_ENTRANCES, &NO_CUES, &[]);
    input.segments = &broken;
    match PitchRunner::start(input) {
        Err(PitchRefusal::Plan(rejections)) => {
            assert!(
                rejections.contains(&PitchRejection::SegmentMissing),
                "{rejections:?}"
            );
        }
        other => panic!("expected a named plan refusal, got {:?}", other.err()),
    }
}

#[test]
fn a_plan_whose_claim_stands_on_a_stale_revision_never_starts() {
    // §53 row 4 is judged at start, not at the first cue: a cue that cannot be spoken must not
    // become reachable by walking far enough through the loop.
    let cues: [ActionCue; 1] = [ActionCue {
        cue_id: "cue-1".to_string(),
        claims: vec![Claim {
            kind: ClaimKind::Price,
            fact_ref: Some(fact("sku-1", "rev-2")),
        }],
    }];
    let current = [fact("sku-1", "rev-3")];
    match PitchRunner::start(clean_input(&NO_ENTRANCES, &cues, &current)) {
        Err(PitchRefusal::Plan(rejections)) => {
            assert!(
                rejections.contains(&PitchRejection::ActionCueFactStale),
                "同 object、不同 revision 的样本必须被判成过期: {rejections:?}"
            );
        }
        other => panic!("expected a named plan refusal, got {:?}", other.err()),
    }
}

// ---------------------------------------------------------------------------
// Advancing, and not advancing
// ---------------------------------------------------------------------------

#[test]
fn the_loop_advances_in_the_fixed_order() {
    let mut runner = PitchRunner::start(clean_input(&NO_ENTRANCES, &NO_CUES, &[])).unwrap();
    assert_eq!(runner.progress(), (0, 4));
    assert_eq!(runner.next_segment(), SegmentKind::Selling);

    let entered: Vec<SegmentKind> = [0u32, 200, 400, 600]
        .iter()
        .map(|at| runner.enter(*at).unwrap().segment)
        .collect();
    assert_eq!(
        entered,
        vec![
            SegmentKind::Selling,
            SegmentKind::Spec,
            SegmentKind::Scenario,
            SegmentKind::AfterSales
        ]
    );
    assert_eq!(runner.progress(), (4, 4));
    // A selling loop wraps: after 售后说明 the next entry is a repeat of 卖点, which is exactly
    // what the cooldown rule exists for.
    assert_eq!(runner.next_segment(), SegmentKind::Selling);
}

#[test]
fn a_refused_entry_leaves_the_timeline_untouched() {
    let mut runner = PitchRunner::start(clean_input(&NO_ENTRANCES, &NO_CUES, &[])).unwrap();
    runner.enter(0).unwrap();

    // 50ms later: above nothing, below the 100ms switch interval.
    match runner.enter(50) {
        Err(PitchRefusal::Transition(rejections)) => {
            assert!(
                rejections.contains(&PitchRejection::SwitchTooSoon),
                "{rejections:?}"
            );
        }
        other => panic!("expected a named transition refusal, got {:?}", other.err()),
    }
    // Fail closed: the refusal did not consume the step.
    assert_eq!(runner.progress(), (1, 4), "拒绝不得推进时间线");
    assert_eq!(runner.next_segment(), SegmentKind::Spec, "拒绝不得移动游标");

    // And the same step still works once the interval has elapsed.
    let entrance = runner.enter(200).unwrap();
    assert_eq!(entrance.segment, SegmentKind::Spec);
    assert_eq!(runner.progress(), (2, 4));
}

#[test]
fn a_segment_repeat_is_judged_against_its_own_last_entry() {
    // The runtime-level twin of the kernel's discriminating sample: no adjacent pair repeats a
    // segment, so a checker written as "adjacent same-segment pair" would let this through.
    let mut runner = PitchRunner::start(clean_input(&NO_ENTRANCES, &NO_CUES, &[])).unwrap();
    for at in [0u32, 200, 400, 600] {
        runner.enter(at).unwrap();
    }
    // 卖点 came back at 900 while its cooldown is 1000 — `spec`/`scenario`/`after_sales` in
    // between do not reset it.
    match runner.enter(900) {
        Err(PitchRefusal::Transition(rejections)) => {
            assert!(
                rejections.contains(&PitchRejection::CooldownNotElapsed),
                "{rejections:?}"
            );
        }
        other => panic!("expected a cooldown refusal, got {:?}", other.err()),
    }
    assert_eq!(runner.progress(), (4, 4));
    // At the boundary the same entry is allowed: 「不早于」 is `<`, not `<=`.
    let entrance = runner.enter(COOLDOWN_MS).unwrap();
    assert_eq!(entrance.segment, SegmentKind::Selling);
    assert_eq!(runner.progress(), (5, 4));
}

// ---------------------------------------------------------------------------
// §5.7: checkpoint and resume
// ---------------------------------------------------------------------------

#[test]
fn every_face_is_named_in_the_baseline_order() {
    // Transcribed from §5.7: 产品、价格、活动、素材与授权. A resume that quietly stopped
    // re-checking 授权 would still look like "all faces fresh" without this line.
    let labels: Vec<&str> = RECHECK_FACES.iter().map(|face| face.label()).collect();
    assert_eq!(
        labels,
        vec!["product", "price", "campaign", "material", "authorisation"]
    );
}

#[test]
fn a_resume_without_a_checkpoint_is_refused() {
    let mut runner = PitchRunner::start(clean_input(&NO_ENTRANCES, &NO_CUES, &[])).unwrap();
    runner.enter(0).unwrap();
    assert_eq!(
        runner.resume(&ResumeRecheck::fresh()),
        Err(PitchRefusal::NoCheckpoint)
    );
}

#[test]
fn an_unmeasured_recheck_refuses_the_resume() {
    // What a caller without a producer can honestly say: all five faces unavailable. That is a
    // refusal — assuming they were fresh would turn an unmeasured fact into an authorisation.
    let mut runner = PitchRunner::start(clean_input(&NO_ENTRANCES, &NO_CUES, &[])).unwrap();
    runner.enter(0).unwrap();
    runner.pause(vec!["s-1".to_string()]).unwrap();
    match runner.resume(&ResumeRecheck::unavailable()) {
        Err(PitchRefusal::RecheckUnavailable(faces)) => {
            assert_eq!(
                faces,
                vec!["product", "price", "campaign", "material", "authorisation"]
            );
        }
        other => panic!("expected an unavailable re-check refusal, got {other:?}"),
    }
}

#[test]
fn a_stale_face_is_named_individually() {
    let mut runner = PitchRunner::start(clean_input(&NO_ENTRANCES, &NO_CUES, &[])).unwrap();
    runner.enter(0).unwrap();
    runner.pause(vec![]).unwrap();
    let recheck = ResumeRecheck::fresh().with(RecheckFace::Material, RecheckOutcome::Stale);
    match runner.resume(&recheck) {
        Err(PitchRefusal::RecheckStale(faces)) => assert_eq!(faces, vec!["material"]),
        other => panic!("expected only 素材 to be named, got {other:?}"),
    }
}

#[test]
fn a_fresh_recheck_resumes_without_entering_again() {
    // 归位 continues the visit that was interrupted; it is not a new entrance, so the timeline
    // does not grow and the segment cooldown does not apply to it.
    let mut runner = PitchRunner::start(clean_input(&NO_ENTRANCES, &NO_CUES, &[])).unwrap();
    runner.enter(0).unwrap();
    let checkpoint = runner
        .pause(vec!["s-1".to_string(), "s-2".to_string()])
        .unwrap();
    assert_eq!(checkpoint.segment, SegmentKind::Selling);
    assert_eq!(checkpoint.carried_text, None, "检查点不保存未播出的文本");

    assert_eq!(
        runner.resume(&ResumeRecheck::fresh()),
        Ok(SegmentKind::Selling)
    );
    assert_eq!(runner.progress(), (1, 4), "归位不是一次新的进入");
    assert_eq!(runner.next_segment(), SegmentKind::Spec);
}

// ---------------------------------------------------------------------------
// The call site: a composition that runs the loop
// ---------------------------------------------------------------------------

#[test]
fn no_loop_started_is_not_the_same_as_progress_zero() {
    // `None` (never started) and "started at segment zero" are different facts; folding them
    // together would make 「没开始」 indistinguishable from 「刚开始」.
    let root = tempdir().unwrap();
    let mut composition = assembled(root.path());
    assert!(composition.pitch().is_none());
    let error = composition.enter_segment(0).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert!(
        error.message.contains("begin_pitch"),
        "拒绝必须说清缺哪一步: {}",
        error.message
    );
}

#[test]
fn the_composition_returns_named_refusals_with_distinct_codes() {
    let root = tempdir().unwrap();
    let mut composition = assembled(root.path());

    assert_eq!(
        composition
            .begin_pitch(clean_input(&NO_ENTRANCES, &NO_CUES, &[]))
            .unwrap(),
        4
    );
    assert_eq!(
        composition.enter_segment(0).unwrap().segment,
        SegmentKind::Selling
    );

    // Too soon: a transition refusal, distinct code.
    let error = composition.enter_segment(50).unwrap_err();
    assert_eq!(error.code, ErrorCode::Conflict);
    assert!(
        error.message.contains("pitch.switch_too_soon"),
        "{}",
        error.message
    );
    assert_eq!(composition.pitch().unwrap().progress(), (1, 4));

    // A resume with no checkpoint is not the same failure as an unmeasured re-check.
    let error = composition
        .resume_pitch(&ResumeRecheck::fresh())
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert!(
        error.message.contains("pitch.no_checkpoint"),
        "{}",
        error.message
    );

    composition.pause_pitch(vec!["s-1".to_string()]).unwrap();

    let error = composition
        .resume_pitch(&ResumeRecheck::unavailable())
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityNotReady);
    assert!(
        error.message.contains("pitch.recheck_unavailable"),
        "{}",
        error.message
    );

    let stale = ResumeRecheck::fresh().with(RecheckFace::Price, RecheckOutcome::Stale);
    let error = composition.resume_pitch(&stale).unwrap_err();
    assert_eq!(
        error.code,
        ErrorCode::EvidenceExpired,
        "事实过期与能力缺失必须分码"
    );
    assert!(
        error.message.contains("pitch.recheck_stale"),
        "{}",
        error.message
    );

    assert_eq!(
        composition.resume_pitch(&ResumeRecheck::fresh()).unwrap(),
        SegmentKind::Selling
    );
    assert_eq!(composition.pitch().unwrap().progress(), (1, 4));
}
