//! W132: the H17 runtime wiring. W66 landed the judgement — `evaluate_degradation` maps each of
//! the six §47.4 failure layers onto one of §12.2's four levels — and said so in its own module
//! docs. Until this module that sentence was true *and* unverifiable: `grep` found the judgement
//! called from tests and nowhere else, so "does a degradation actually reach the interface?"
//! was a question only a code reader could answer.
//!
//! `DegradeVerdict::visible` is `true` by construction, so the judge is the wrong place to test
//! "no silent degradation": what it cannot express is the *transport*. This module is the
//! transport — judge, render, persist, read back — and the guards below are about the hops the
//! judge does not cover:
//!
//! * a degradation is persisted **once**, carrying exactly the rendered notices;
//! * what reads back is decoded from the chain, so a caller cannot hand the view its own answer;
//! * a row that is not decodable is a **classified failure**, never a silent skip;
//! * INV-32's second half runs on this path too: a degraded call that lost its policy or persona
//!   constraint is refused by name and appends **nothing**.
//!
//! The non-void guard is `every_failure_layer_reaches_the_chain_and_reads_back`: it iterates
//! `FAILURE_LAYERS` itself rather than a hand-written list, so adding a layer to the enum without
//! covering it fails here instead of passing invisibly.
mod common;
use common::*;
use nai_kernel::evaluate::harness::{
    DegradeContext, FAILURE_LAYERS, FailureLayer, render_degradation_notices,
};
use nai_runtime::{Composition, DegradationOutcome, Retained};
use nai_storage::{DataPaths, DecisionKind};
use serde_json::json;
use tempfile::tempdir;

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

fn all_rows(composition: &Composition) -> Vec<nai_storage::DecisionEntryView> {
    let operation = store_operation();
    let store = composition.store().lock().unwrap();
    store
        .decision_entries(&operation, composition.workspace(), 500)
        .unwrap()
}

fn degradation_rows(composition: &Composition) -> Vec<nai_storage::DecisionEntryView> {
    all_rows(composition)
        .into_iter()
        .filter(|row| row.kind == DecisionKind::Degradation.as_str())
        .collect()
}

fn expect_visible(outcome: DegradationOutcome) -> nai_runtime::DegradationView {
    match outcome {
        DegradationOutcome::Visible(view) => view,
        DegradationOutcome::Refused(refusal) => {
            panic!("expected a visible degradation, got {refusal:?}")
        }
    }
}

fn expect_refused(outcome: DegradationOutcome) -> nai_runtime::FreeFormRefusal {
    match outcome {
        DegradationOutcome::Refused(refusal) => refusal,
        DegradationOutcome::Visible(view) => panic!("expected a refusal, got {view:?}"),
    }
}

/// Every layer, judged under full availability, with both constraints retained.
fn record_one(composition: &mut Composition, layer: FailureLayer) -> nai_runtime::DegradationView {
    let outcome = composition
        .record_degradation(
            &[layer],
            &DegradeContext::all_available(),
            Retained {
                policy_constraint: true,
                persona: true,
            },
        )
        .unwrap();
    expect_visible(outcome)
}

#[test]
fn every_failure_layer_reaches_the_chain_and_reads_back() {
    for layer in FAILURE_LAYERS {
        let dir = tempdir().unwrap();
        let mut composition = prepared(dir.path());
        let view = record_one(&mut composition, layer);

        assert_eq!(
            degradation_rows(&composition).len(),
            1,
            "{layer:?} MUST land exactly once"
        );
        assert_eq!(view.notices.len(), 1, "{layer:?} MUST render one notice");

        let read_back = composition.read_degradation_view(500).unwrap();
        assert_eq!(read_back.notices.len(), 1, "{layer:?} MUST read back");
        assert_eq!(read_back.notices[0].layer, layer.label());
        assert_eq!(read_back.notices[0].code, layer.code());
        assert_eq!(read_back.notices[0].actionable, layer.actionable());
        assert_eq!(
            read_back.notices[0].drops,
            layer.drops().map(str::to_string)
        );
    }
}

/// The read-back is decoded from the chain, so it equals what the judge produced — including
/// order. A read-back that reconstructs from the caller's argument instead cannot be distinguished
/// from this side except by injecting a row the caller never supplied (next test).
#[test]
fn the_read_back_view_equals_what_was_judged_field_by_field() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());
    let failures = [FailureLayer::MemoryRead, FailureLayer::Peer];
    let verdicts: Vec<_> = failures
        .iter()
        .map(|layer| {
            nai_kernel::evaluate::harness::evaluate_degradation(
                *layer,
                &DegradeContext::all_available(),
            )
        })
        .collect();
    let rendered = render_degradation_notices(&verdicts);

    let outcome = composition
        .record_degradation(
            &failures,
            &DegradeContext::all_available(),
            Retained {
                policy_constraint: true,
                persona: true,
            },
        )
        .unwrap();
    let view = expect_visible(outcome);
    assert_eq!(view.notices.len(), 2, "both degradations MUST be visible");

    let read_back = composition.read_degradation_view(500).unwrap();
    assert_eq!(read_back.notices.len(), rendered.len());
    for (got, want) in read_back.notices.iter().zip(rendered.iter()) {
        assert_eq!(got.layer, want.layer);
        assert_eq!(got.code, want.code);
        assert_eq!(got.level, want.level);
        assert_eq!(got.actionable, want.actionable);
        assert_eq!(got.refuses_call, want.refuses_call);
        assert_eq!(got.drops, want.drops.map(str::to_string));
        assert_eq!(got.keeps_existing_session, want.keeps_existing_session);
    }
}

/// INV-32, second half: degraded **and** the policy constraint is gone ⇒ refused by name, and
/// nothing at all is appended. Naming the code is not enough; the refusal is what stops the call.
#[test]
fn a_degraded_call_that_lost_its_policy_constraint_is_refused_and_appends_nothing() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());
    let before = degradation_rows(&composition).len();

    let outcome = composition
        .record_degradation(
            &[FailureLayer::Tool],
            &DegradeContext::all_available(),
            Retained {
                policy_constraint: false,
                persona: true,
            },
        )
        .unwrap();
    let refusal = expect_refused(outcome);

    assert_eq!(
        refusal.violations,
        vec!["free_form.degraded_without_policy_constraint"]
    );
    assert_eq!(
        degradation_rows(&composition).len(),
        before,
        "a refused call MUST NOT append"
    );
    assert!(
        composition
            .read_degradation_view(500)
            .unwrap()
            .notices
            .is_empty(),
        "nothing was persisted, so nothing reads back"
    );
}

/// The complementary arm: losing the persona is a *different* named violation, so the two are not
/// interchangeable and one guard does not cover the other.
#[test]
fn a_degraded_call_that_lost_its_persona_is_refused_by_its_own_name() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let outcome = composition
        .record_degradation(
            &[FailureLayer::Tool],
            &DegradeContext::all_available(),
            Retained {
                policy_constraint: false,
                persona: false,
            },
        )
        .unwrap();
    let refusal = expect_refused(outcome);

    assert_eq!(refusal.violations.len(), 2, "both losses MUST be named");
    assert!(
        refusal
            .violations
            .contains(&"free_form.degraded_without_persona")
    );
}

/// No failure ⇒ no degradation ⇒ the free-form rule does not apply, and the empty set is a real
/// zero rather than a skip.
#[test]
fn an_undegraded_call_is_not_judged_by_the_degradation_rule() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let outcome = composition
        .record_degradation(
            &[],
            &DegradeContext::all_available(),
            Retained {
                policy_constraint: false,
                persona: false,
            },
        )
        .unwrap();
    let view = expect_visible(outcome);

    assert!(view.notices.is_empty(), "no failure means no notice");
    assert_eq!(
        composition
            .read_degradation_view(500)
            .unwrap()
            .notices
            .len(),
        0
    );
}

/// A `degradation` row that is not decodable is corruption, never a silent skip: the write path
/// only ever persisted this shape.
#[test]
fn an_undecodable_degradation_row_is_a_named_failure_not_a_skip() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    composition
        .record(
            DecisionKind::Degradation,
            None,
            &json!({"unexpected": 1}),
            "trace-injected",
        )
        .unwrap();

    let error = composition
        .read_degradation_view(500)
        .expect_err("a row without `notices` MUST be reported, not skipped");
    assert!(
        error.message.contains("notices"),
        "the failure MUST name what is missing, got {:?}",
        error.message
    );
}

/// The view's domain is the `degradation` kind: another kind's row is not a degradation.
#[test]
fn only_degradation_rows_enter_the_view() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    // 这条载荷是**可解码**的：只有这样才能区分「按 kind 界定」与「解不出来恰好为零」。
    composition
        .record(
            DecisionKind::Observation,
            None,
            &json!({"notices":[{"layer":"harness.tool","code":"HARNESS_TOOL_FAILED","level":"template","actionable":"a","refuses_call":false,"keeps_existing_session":false}]}),
            "trace-other-kind",
        )
        .unwrap();

    assert_eq!(
        degradation_rows(&composition).len(),
        0,
        "the injected row is not a degradation"
    );
    assert_eq!(
        composition
            .read_degradation_view(500)
            .unwrap()
            .notices
            .len(),
        0
    );
}

/// The level the interface shows is the level the **walk-down** reached, not the layer's preferred
/// level. Every other sample here uses `all_available()`, where the two are equal by construction
/// — so without this test nothing exercises the walk-down through the runtime path, and a write
/// path that persisted `layer.preferred_level()` would pass every test above.
#[test]
fn a_failure_whose_preferred_level_is_unavailable_shows_the_level_it_walked_down_to() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());
    // Tool prefers §12.2 level 1 (template); with the first two levels gone the walk must land on
    // level 3 (human), and *that* is what the interface is told.
    let available = DegradeContext {
        template_available: false,
        approved_cache_available: false,
        human_available: true,
    };

    let outcome = composition
        .record_degradation(
            &[FailureLayer::Tool],
            &available,
            Retained {
                policy_constraint: true,
                persona: true,
            },
        )
        .unwrap();
    let view = expect_visible(outcome);
    assert_eq!(view.notices.len(), 1);
    assert_ne!(
        view.notices[0].level, "template",
        "the preferred level is unavailable, so it MUST NOT be shown"
    );

    let read_back = composition.read_degradation_view(500).unwrap();
    assert_eq!(read_back.notices[0].level, "human");
    assert_eq!(read_back.notices[0].code, FailureLayer::Tool.code());
}

/// Assembly refuses the call outright (§47.4). That refusal is part of what the interface shows,
/// so it survives the round trip.
#[test]
fn an_assembly_failure_still_refuses_the_call_after_a_round_trip() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());
    let view = record_one(&mut composition, FailureLayer::Assembly);

    assert!(view.refuses_call, "assembly MUST refuse the call");
    let read_back = composition.read_degradation_view(500).unwrap();
    assert!(
        read_back.refuses_call,
        "the refusal MUST be visible after a round trip"
    );
}
