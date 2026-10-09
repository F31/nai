//! End-to-end decision -> four-stage guard pipeline (pure evaluation).
use nai_contracts::v1::{
    Decision, DecisionAction, Evidence, GuardRequest, GuardResult, GuardStage, GuardVerdict,
    Observation, PerformanceCue, PermissionSnapshot, RequestContext, Risk, SessionMode,
};
use nai_kernel::evaluate::{
    agent::{AgentPolicy, evaluate_decision},
    guard::{GuardPolicy, PreplayChecks, guard},
};

fn context() -> RequestContext {
    RequestContext {
        session_id: "session".into(),
        workspace_id: "workspace".into(),
        generation_epoch: Some(7),
        remaining_budget_ms: 500,
        ..Default::default()
    }
}
fn cue(speech: &str) -> PerformanceCue {
    PerformanceCue {
        cue_id: "cue".into(),
        session_id: "session".into(),
        utterance_id: "utt".into(),
        generation_epoch: 7,
        display_text: speech.into(),
        speech_text: speech.into(),
        evidence_refs: vec!["ev-1".into()],
        policy_version: "policy-v1".into(),
        expires_at_utc_ms: 1_000_000_000_000,
        risk: Risk::Unspecified as i32,
        ..Default::default()
    }
}

#[test]
fn sound_cue_passes_all_four_guard_stages() {
    let observation = Observation {
        context: Some(context()),
        mode: SessionMode::AutoSupervised as i32,
        ..Default::default()
    };
    let decision = Decision {
        action: DecisionAction::Speak as i32,
        route: "plugin".into(),
        candidate: Some(cue("本商品保修期为一年")),
        ..Default::default()
    };
    let agent_policy = AgentPolicy::default();
    let decision = evaluate_decision(&observation, Some(&decision), &agent_policy, 1_000).unwrap();
    assert_ne!(
        decision.candidate.as_ref().unwrap().risk,
        Risk::Unspecified as i32
    );

    let mut guard_request = GuardRequest {
        context: observation.context,
        candidate: decision.candidate,
        evidence: vec![Evidence {
            evidence_id: "ev-1".into(),
            text: "本商品保修期为一年".into(),
            revision: "rev-1".into(),
            permission_revision: "permission-1".into(),
            original_verifiable: true,
            valid_until_utc_ms: Some(1_000_000_000_000),
            ..Default::default()
        }],
        permissions: Some(PermissionSnapshot {
            revision: "permission-1".into(),
            valid_until_utc_ms: Some(1_000_000_000_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let guard_policy = GuardPolicy::default();
    let stages = [
        GuardStage::Input,
        GuardStage::Evidence,
        GuardStage::Output,
        GuardStage::Preplay,
    ];
    for stage in stages {
        guard_request.stage = stage as i32;
        let preplay = (stage == GuardStage::Preplay).then_some(PreplayChecks {
            approval_valid: true,
            facts_match: true,
            epoch_match: true,
            permission_current: true,
        });
        let result: GuardResult = guard(&guard_request, &guard_policy, 0, 1_000, preplay);
        assert_eq!(
            result.verdict,
            GuardVerdict::Pass as i32,
            "stage {stage:?} failed"
        );
    }
}

#[test]
fn dangerous_url_is_rewritten_once_then_a_second_pass_is_clean() {
    let observation = Observation {
        context: Some(context()),
        mode: SessionMode::AutoSupervised as i32,
        ..Default::default()
    };
    let decision = Decision {
        action: DecisionAction::Speak as i32,
        route: "plugin".into(),
        candidate: Some(cue("详情见 https://example.invalid/x")),
        ..Default::default()
    };
    let policy = AgentPolicy::default();
    let decision = evaluate_decision(&observation, Some(&decision), &policy, 1_000).unwrap();

    let mut guard_request = GuardRequest {
        context: observation.context,
        candidate: decision.candidate,
        evidence: vec![Evidence {
            evidence_id: "ev-1".into(),
            text: "详情".into(),
            revision: "rev-1".into(),
            permission_revision: "permission-1".into(),
            original_verifiable: true,
            valid_until_utc_ms: Some(1_000_000_000_000),
            ..Default::default()
        }],
        permissions: Some(PermissionSnapshot {
            revision: "permission-1".into(),
            valid_until_utc_ms: Some(1_000_000_000_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let guard_policy = GuardPolicy::default();
    guard_request.stage = GuardStage::Output as i32;
    let rewritten = guard(&guard_request, &guard_policy, 0, 1_000, None);
    assert_eq!(rewritten.verdict, GuardVerdict::Rewrite as i32);
    let rewritten_text = rewritten
        .rewritten_candidate
        .as_ref()
        .map(|cue| cue.display_text.clone())
        .unwrap_or_default();
    assert!(!rewritten_text.contains("https://"));

    guard_request.candidate = rewritten.rewritten_candidate;
    let second = guard(&guard_request, &guard_policy, 1, 1_000, None);
    assert_eq!(second.verdict, GuardVerdict::Pass as i32);
}
