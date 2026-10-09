//! Show plan evaluation (V3.5 §23.6 / §24.2 `scenes`/`show_plans`): segments are
//! versioned, interrupted predictably, and sensitive scenes require current
//! evidence binding. Checkpoint recovery is explicit and versioned; an empty
//! checkpoint never resumes an invented playback state.
use super::EvaluateError;
use nai_contracts::v1::{ShowPlan, ShowRequest};

pub const KNOWN_INTERRUPT_POLICIES: [&str; 3] = ["finish_segment", "stop", "cut"];
pub const MAX_REFS: usize = 256;
pub const MAX_CHECKPOINT: usize = 1024;

pub struct ShowPolicy {
    pub rule_version: &'static str,
    pub max_duration_ms: u32,
    pub sensitive_scenes: &'static [&'static str],
}
impl Default for ShowPolicy {
    fn default() -> Self {
        Self {
            rule_version: "show-rule-v1",
            max_duration_ms: 3_600_000,
            sensitive_scenes: &[
                "price",
                "promotion",
                "stock",
                "discount",
                "refund",
                "价格",
                "优惠",
                "库存",
                "退款",
            ],
        }
    }
}

fn scene_sensitive(scene_id: &str, policy: &ShowPolicy) -> bool {
    policy
        .sensitive_scenes
        .iter()
        .any(|marker| scene_id.contains(marker))
}

/// Validate a proposed show plan against the stable policy. Recovery from a
/// checkpoint only succeeds when scene/version and checkpoint match exactly.
pub fn evaluate_show(
    request: &ShowRequest,
    plan: &ShowPlan,
    policy: &ShowPolicy,
) -> Result<(), EvaluateError> {
    let context = request.context.as_ref().ok_or(EvaluateError::Stale)?;
    if context.session_id.is_empty() || context.workspace_id.is_empty() {
        return Err(EvaluateError::Stale);
    }
    if plan.scene_id.is_empty()
        || plan.scene_id != request.scene_id
        || plan.version.is_empty()
        || plan.version != request.version
        || plan.segment_id.is_empty()
    {
        return Err(EvaluateError::Invalid);
    }
    if plan.target_duration_ms == 0 || plan.target_duration_ms > policy.max_duration_ms {
        return Err(EvaluateError::Invalid);
    }
    if !KNOWN_INTERRUPT_POLICIES.contains(&plan.interrupt_policy.as_str()) {
        return Err(EvaluateError::Invalid);
    }
    if plan.topic_refs.len() > MAX_REFS
        || plan.script_refs.len() > MAX_REFS
        || plan.completed_utterance_ids.len() > MAX_REFS
    {
        return Err(EvaluateError::Capacity);
    }
    if plan.completed_utterance_ids.iter().any(|id| id.is_empty()) {
        return Err(EvaluateError::Invalid);
    }
    if scene_sensitive(&plan.scene_id, policy)
        && (plan.evidence_binding.is_empty()
            || plan
                .evidence_binding
                .iter()
                .any(|binding| binding.object_id.is_empty() || binding.revision.is_empty()))
    {
        return Err(EvaluateError::Invalid);
    }
    // Explicit, versioned checkpoint recovery; never invent playback state.
    if !request.checkpoint_ref.is_empty()
        && (plan.checkpoint.is_empty()
            || plan.checkpoint.len() > MAX_CHECKPOINT
            || plan.checkpoint != request.checkpoint_ref)
    {
        return Err(EvaluateError::Conflict);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nai_contracts::v1::RequestContext;

    fn request(checkpoint: &str) -> ShowRequest {
        ShowRequest {
            context: Some(RequestContext {
                session_id: "session".into(),
                workspace_id: "workspace".into(),
                generation_epoch: Some(7),
                remaining_budget_ms: 500,
                ..Default::default()
            }),
            scene_id: "promotion-price".into(),
            version: "v2".into(),
            checkpoint_ref: checkpoint.into(),
        }
    }
    fn plan(checkpoint: &str, bound: bool) -> ShowPlan {
        ShowPlan {
            scene_id: "promotion-price".into(),
            version: "v2".into(),
            segment_id: "seg-1".into(),
            target_duration_ms: 60_000,
            topic_refs: vec!["topic-1".into()],
            script_refs: vec!["script-1".into()],
            interrupt_policy: "finish_segment".into(),
            checkpoint: checkpoint.into(),
            evidence_binding: if bound {
                vec![nai_contracts::v1::RevisionRef {
                    object_id: "sku-price".into(),
                    revision: "rev-3".into(),
                }]
            } else {
                vec![]
            },
            ..Default::default()
        }
    }

    #[test]
    fn sensitive_scene_requires_current_evidence_binding() {
        let policy = ShowPolicy::default();
        assert_eq!(
            evaluate_show(&request(""), &plan("", false), &policy),
            Err(EvaluateError::Invalid)
        );
        assert_eq!(
            evaluate_show(&request(""), &plan("", true), &policy),
            Ok(())
        );
    }

    #[test]
    fn checkpoint_recovery_is_explicit_and_versioned() {
        let policy = ShowPolicy::default();
        assert_eq!(
            evaluate_show(&request("cp-1"), &plan("", true), &policy),
            Err(EvaluateError::Conflict)
        );
        assert_eq!(
            evaluate_show(&request("cp-1"), &plan("cp-1", true), &policy),
            Ok(())
        );
        assert!(
            evaluate_show(
                &request("please-resume"),
                &plan("please-resume", true),
                &policy
            )
            .is_ok()
        );
    }

    #[test]
    fn unknown_segment_policy_or_duration_is_invalid() {
        let policy = ShowPolicy::default();
        let mut bad = plan("", true);
        bad.interrupt_policy = "arbitrary".into();
        assert_eq!(
            evaluate_show(&request(""), &bad, &policy),
            Err(EvaluateError::Invalid)
        );
        let mut bad = plan("", true);
        bad.target_duration_ms = 0;
        assert_eq!(
            evaluate_show(&request(""), &bad, &policy),
            Err(EvaluateError::Invalid)
        );
        let mut oversized = plan("", true);
        oversized.target_duration_ms = policy.max_duration_ms + 1;
        assert_eq!(
            evaluate_show(&request(""), &oversized, &policy),
            Err(EvaluateError::Invalid)
        );
    }
}
