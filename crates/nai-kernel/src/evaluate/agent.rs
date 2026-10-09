//! Observation -> Decision routing and candidate validation (V3.5 §23.1).
//! A Decision is a request; the runtime executes it. No action here authorizes
//! media playback or data writes, and risk comes from controlled rules, never
//! the model's self-reported low-risk claim.
use super::EvaluateError;
use nai_contracts::v1::{Decision, DecisionAction, EventKind, Observation, PerformanceCue, Risk};

/// Promise/price/keyword markers that raise the controlled risk level.
pub const RISK1_MARKERS: [&str; 9] = [
    "元", "¥", "￥", "价格", "优惠", "折扣", "库存", "退款", "红包",
];
pub const RISK2_MARKERS: [&str; 8] = [
    "保证",
    "承诺",
    "包退",
    "无条件退款",
    "假一赔十",
    "保真",
    "绝无",
    "永久",
];
pub const RISK3_MARKERS: [&str; 4] = ["http://", "https://", "<script", "select from"];

pub struct AgentPolicy {
    pub rule_version: &'static str,
    pub max_cue_chars: usize,
    pub require_evidence_for_speak: bool,
    pub max_tool_plan: usize,
}
impl Default for AgentPolicy {
    fn default() -> Self {
        Self {
            rule_version: "agent-rule-v1",
            max_cue_chars: 2048,
            require_evidence_for_speak: true,
            max_tool_plan: 8,
        }
    }
}

/// Controlled risk from text, independent of any model self-report.
pub fn risk_from_text(text: &str) -> Risk {
    if RISK3_MARKERS.iter().any(|marker| text.contains(marker)) {
        return Risk::Risk3;
    }
    if RISK2_MARKERS.iter().any(|marker| text.contains(marker)) {
        return Risk::Risk2;
    }
    if RISK1_MARKERS.iter().any(|marker| text.contains(marker)) {
        return Risk::Risk1;
    }
    Risk::Risk0
}

fn context_ok(obs: &Observation) -> Result<(), EvaluateError> {
    let context = obs.context.as_ref().ok_or(EvaluateError::Stale)?;
    if context.session_id.is_empty() || context.workspace_id.is_empty() {
        return Err(EvaluateError::Stale);
    }
    if context.remaining_budget_ms == 0 {
        return Err(EvaluateError::Expired);
    }
    Ok(())
}

fn cue_ok(
    cue: &PerformanceCue,
    obs: &Observation,
    policy: &AgentPolicy,
) -> Result<(), EvaluateError> {
    let context = obs.context.as_ref().ok_or(EvaluateError::Stale)?;
    if cue.cue_id.is_empty()
        || cue.session_id != context.session_id
        || cue.utterance_id.is_empty()
        || cue.display_text.trim().is_empty()
        || cue.speech_text.trim().is_empty()
        || cue.display_text.len() > policy.max_cue_chars
        || cue.speech_text.len() > policy.max_cue_chars
        || cue.expires_at_utc_ms == 0
        || cue.policy_version.is_empty()
    {
        return Err(EvaluateError::Invalid);
    }
    if context
        .generation_epoch
        .is_some_and(|epoch| epoch != cue.generation_epoch)
    {
        return Err(EvaluateError::Stale);
    }
    if policy.require_evidence_for_speak && cue.evidence_refs.is_empty() {
        return Err(EvaluateError::Invalid);
    }
    Ok(())
}

fn stay_silent(reason: &str) -> Decision {
    Decision {
        action: DecisionAction::StaySilent as i32,
        route: "rule-fallback".into(),
        reason_code: reason.into(),
        risk: Risk::Risk0 as i32,
        ..Default::default()
    }
}

fn ask_human(reason: &str) -> Decision {
    Decision {
        action: DecisionAction::AskHuman as i32,
        route: "rule-fallback".into(),
        reason_code: reason.into(),
        risk: Risk::Risk0 as i32,
        ..Default::default()
    }
}

/// Deterministic fallback when no agent plugin provides a decision. Never
/// fabricates content; a comment/question without a safe template becomes a
/// human request, everything else stays silent.
pub fn fallback(obs: &Observation) -> Decision {
    if obs.mode == nai_contracts::v1::SessionMode::Rehearsal as i32 {
        return stay_silent("rehearsal_no_output");
    }
    if obs.queued_speech_ms >= 30_000 {
        return stay_silent("queue_full");
    }
    let Some(trigger) = &obs.trigger else {
        return stay_silent("no_trigger");
    };
    match EventKind::try_from(trigger.kind).unwrap_or(EventKind::EventUnknown) {
        EventKind::Gift => ask_human("needs_gift_template"),
        EventKind::Comment => {
            if trigger.text.trim().is_empty() {
                stay_silent("empty_comment")
            } else {
                ask_human("needs_llm")
            }
        }
        EventKind::EventUnknown
        | EventKind::Like
        | EventKind::Follow
        | EventKind::Enter
        | EventKind::RoomStarted
        | EventKind::RoomEnded
        | EventKind::PermissionChanged
        | EventKind::ConnectionChanged
        | EventKind::Unfollow
        | EventKind::Fansclub => stay_silent("no_canned_content"),
    }
}

/// Validate a proposed decision and recompute risk from controlled rules. A
/// broken SPEAK candidate is downgraded to ask_human/stay_silent with a reason
/// code rather than silently dropped.
pub fn evaluate_decision(
    obs: &Observation,
    proposed: Option<&Decision>,
    policy: &AgentPolicy,
    now_ms: u64,
) -> Result<Decision, EvaluateError> {
    context_ok(obs)?;
    let Some(proposed) = proposed else {
        return Ok(fallback(obs));
    };
    let action = DecisionAction::try_from(proposed.action).map_err(|_| EvaluateError::Invalid)?;
    let mut decision = proposed.clone();
    match action {
        DecisionAction::ActionUnspecified => return Err(EvaluateError::Invalid),
        DecisionAction::Speak => {
            let Some(cue) = proposed.candidate.clone() else {
                return Ok(ask_human("invalid_candidate"));
            };
            if cue_ok(&cue, obs, policy).is_err() || cue.expires_at_utc_ms <= now_ms {
                return Ok(ask_human("invalid_candidate"));
            }
            let mut cue = cue;
            cue.risk = risk_from_text(&format!("{} {}", cue.display_text, cue.speech_text)) as i32;
            decision.candidate = Some(cue);
            decision.risk = decision
                .candidate
                .as_ref()
                .map(|cue| cue.risk)
                .unwrap_or(Risk::Risk0 as i32);
        }
        DecisionAction::StaySilent | DecisionAction::AskHuman => {
            if proposed.action == DecisionAction::AskHuman as i32 && proposed.reason_code.is_empty()
            {
                return Err(EvaluateError::Invalid);
            }
        }
        DecisionAction::RequestSkill => {
            if proposed.tool_plan.is_empty() || proposed.tool_plan.len() > policy.max_tool_plan {
                return Err(EvaluateError::Invalid);
            }
        }
        DecisionAction::RequestSceneChange | DecisionAction::Defer => {
            if proposed.route.is_empty() || proposed.reason_code.is_empty() {
                return Err(EvaluateError::Invalid);
            }
        }
    }
    decision.route = if proposed.route.is_empty() {
        "rule-fallback".into()
    } else {
        proposed.route.clone()
    };
    if decision.risk == Risk::Unspecified as i32 {
        decision.risk = risk_from_text(
            proposed
                .candidate
                .as_ref()
                .map(|cue| cue.display_text.as_str())
                .unwrap_or_default(),
        ) as i32;
    }
    Ok(decision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nai_contracts::v1::{RequestContext, SessionMode};

    fn obs(mode: i32) -> Observation {
        Observation {
            context: Some(RequestContext {
                session_id: "session".into(),
                workspace_id: "workspace".into(),
                generation_epoch: Some(7),
                remaining_budget_ms: 500,
                ..Default::default()
            }),
            mode,
            ..Default::default()
        }
    }
    fn cue() -> PerformanceCue {
        PerformanceCue {
            cue_id: "cue".into(),
            session_id: "session".into(),
            utterance_id: "utt".into(),
            generation_epoch: 7,
            display_text: "这是已核实内容".into(),
            speech_text: "这是已核实内容".into(),
            evidence_refs: vec!["ev-1".into()],
            policy_version: "policy-v1".into(),
            expires_at_utc_ms: 1_000_000_000_000,
            ..Default::default()
        }
    }

    #[test]
    fn speak_with_invalid_or_empty_candidate_is_downgraded() {
        let observation = obs(SessionMode::Assisted as i32);
        let policy = AgentPolicy::default();
        let mut empty = Decision {
            action: DecisionAction::Speak as i32,
            candidate: Some(cue()),
            ..Default::default()
        };
        empty.candidate = Some(PerformanceCue::default());
        assert_eq!(
            evaluate_decision(&observation, Some(&empty), &policy, 0)
                .unwrap()
                .action,
            DecisionAction::AskHuman as i32
        );
        let mut no_evidence = cue();
        no_evidence.evidence_refs.clear();
        let decision = Decision {
            action: DecisionAction::Speak as i32,
            candidate: Some(no_evidence),
            ..Default::default()
        };
        assert_eq!(
            evaluate_decision(&observation, Some(&decision), &policy, 0)
                .unwrap()
                .action,
            DecisionAction::AskHuman as i32
        );
        let mut expired = cue();
        expired.expires_at_utc_ms = 100;
        let decision = Decision {
            action: DecisionAction::Speak as i32,
            candidate: Some(expired),
            ..Default::default()
        };
        assert_eq!(
            evaluate_decision(&observation, Some(&decision), &policy, 1_000_000_000_000)
                .unwrap()
                .action,
            DecisionAction::AskHuman as i32
        );
    }

    #[test]
    fn model_reported_low_risk_is_recomputed_from_rules() {
        let observation = obs(SessionMode::Assisted as i32);
        let mut cue = cue();
        cue.display_text = "本商品价格 199 元，包退".into();
        cue.speech_text = "本商品价格 199 元，包退".into();
        cue.risk = Risk::Risk0 as i32;
        let decision = Decision {
            action: DecisionAction::Speak as i32,
            candidate: Some(cue),
            ..Default::default()
        };
        let result =
            evaluate_decision(&observation, Some(&decision), &AgentPolicy::default(), 0).unwrap();
        let risk = Risk::try_from(result.candidate.unwrap().risk).unwrap();
        assert!(matches!(risk, Risk::Risk1 | Risk::Risk2));
    }

    #[test]
    fn fallback_never_fabricates_content() {
        let observation = obs(SessionMode::AutoSupervised as i32);
        let decision = evaluate_decision(&observation, None, &AgentPolicy::default(), 0).unwrap();
        assert_eq!(decision.action, DecisionAction::StaySilent as i32);
        assert!(decision.candidate.is_none());
        let mut observation = observation;
        observation.trigger = Some(nai_contracts::v1::PlatformEvent {
            kind: EventKind::Comment as i32,
            text: "这个多少钱？".into(),
            ..Default::default()
        });
        let decision = evaluate_decision(&observation, None, &AgentPolicy::default(), 0).unwrap();
        assert_eq!(decision.action, DecisionAction::AskHuman as i32);
        assert!(decision.candidate.is_none());
    }
}
