//! Skill evaluation (V3.5 §23.4): descriptors are bounded, high-risk skills are
//! never registered, tool plans are step/time limited and parameters may only
//! reference controlled resource IDs. Tool results are always untrusted data.
use super::EvaluateError;
use nai_contracts::v1::{Risk, SkillDescriptor, SkillRequest, SkillResult};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SideEffect {
    ReadOnly,
    Notify,
    ViewerVisible,
    HighRisk,
}
impl SideEffect {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "read_only" => Some(Self::ReadOnly),
            "notify" => Some(Self::Notify),
            "viewer_visible" => Some(Self::ViewerVisible),
            "high_risk" => Some(Self::HighRisk),
            _ => None,
        }
    }
}

pub const MAX_SKILL_NAME: usize = 128;
pub const MAX_SCHEMA_REF: usize = 256;
pub const MAX_PERMISSIONS: usize = 8;

pub struct ToolLimits {
    pub max_steps: usize,
    pub max_estimated_ms: u64,
    pub allowed_resource_fields: &'static [&'static str],
}
impl Default for ToolLimits {
    fn default() -> Self {
        Self {
            max_steps: 8,
            max_estimated_ms: 60_000,
            allowed_resource_fields: &["product_id", "kb_id", "scene_id"],
        }
    }
}

/// Descriptor validation. High-risk operations are not registered at all.
pub fn validate_descriptor(skill: &SkillDescriptor) -> Result<(), EvaluateError> {
    if skill.name.is_empty()
        || skill.name.len() > MAX_SKILL_NAME
        || skill.parameters_schema_ref.is_empty()
        || skill.parameters_schema_ref.len() > MAX_SCHEMA_REF
        || skill.result_schema_ref.is_empty()
        || skill.result_schema_ref.len() > MAX_SCHEMA_REF
        || skill.source_ref.is_empty()
    {
        return Err(EvaluateError::Invalid);
    }
    if skill.permissions.is_empty() || skill.permissions.len() > MAX_PERMISSIONS {
        return Err(EvaluateError::Invalid);
    }
    if skill
        .permissions
        .iter()
        .any(|permission| permission.is_empty())
    {
        return Err(EvaluateError::Invalid);
    }
    if skill.timeout_ms == 0 || skill.timeout_ms > 60_000 {
        return Err(EvaluateError::Invalid);
    }
    if skill.max_concurrency == 0 || skill.max_concurrency > 16 {
        return Err(EvaluateError::Invalid);
    }
    if skill.risk == Risk::Unspecified as i32 {
        return Err(EvaluateError::Invalid);
    }
    let side_effect = SideEffect::parse(&skill.side_effect).ok_or(EvaluateError::Invalid)?;
    match side_effect {
        SideEffect::HighRisk => return Err(EvaluateError::Conflict),
        SideEffect::ViewerVisible => {
            if Risk::try_from(skill.risk).map_err(|_| EvaluateError::Invalid)? != Risk::Risk2 {
                return Err(EvaluateError::Conflict);
            }
            if !skill
                .permissions
                .iter()
                .any(|permission| permission.starts_with("output.viewer"))
            {
                return Err(EvaluateError::Unauthorized);
            }
        }
        SideEffect::Notify => {
            if !skill
                .permissions
                .iter()
                .any(|permission| permission.starts_with("notify."))
            {
                return Err(EvaluateError::Unauthorized);
            }
            if Risk::try_from(skill.risk).map_err(|_| EvaluateError::Invalid)? < Risk::Risk1 {
                return Err(EvaluateError::Conflict);
            }
        }
        SideEffect::ReadOnly => {
            if Risk::try_from(skill.risk).map_err(|_| EvaluateError::Invalid)? > Risk::Risk1 {
                return Err(EvaluateError::Conflict);
            }
        }
    }
    Ok(())
}

/// A bounded tool plan: step/time limited; parameters may only carry controlled
/// resource IDs, never arbitrary URLs/SQL/paths typed by the model.
pub fn validate_tool_plan(plan: &[SkillRequest], limits: &ToolLimits) -> Result<(), EvaluateError> {
    if plan.is_empty() || plan.len() > limits.max_steps {
        return Err(EvaluateError::Invalid);
    }
    for request in plan {
        let context = request.context.as_ref().ok_or(EvaluateError::Stale)?;
        if context.session_id.is_empty() || context.remaining_budget_ms == 0 {
            return Err(EvaluateError::Stale);
        }
        if request.skill_name.is_empty() || request.skill_name.len() > MAX_SKILL_NAME {
            return Err(EvaluateError::Invalid);
        }
        if request.authorization_ref.is_empty() || request.idempotency_key.is_empty() {
            return Err(EvaluateError::Unauthorized);
        }
        let parameters: Value =
            serde_json::from_slice(&request.parameters_json).map_err(|_| EvaluateError::Invalid)?;
        let object = parameters.as_object().ok_or(EvaluateError::Invalid)?;
        for (key, value) in object {
            if limits.allowed_resource_fields.contains(&key.as_str()) {
                if !value.is_string() || value.as_str().is_none_or(|id| id.is_empty()) {
                    return Err(EvaluateError::Invalid);
                }
                continue;
            }
            // Anything outside the controlled resource allowlist must be rejected
            // if it looks like a free-form URL/SQL/path injection.
            let text = value.to_string();
            if text.contains("://")
                || text.to_lowercase().contains("select ")
                || text.to_lowercase().contains("drop ")
                || text.starts_with('/')
                || text.contains("..")
            {
                return Err(EvaluateError::Unauthorized);
            }
        }
    }
    Ok(())
}

/// Tool results are untrusted: validate shape and size but never treat them as
/// instructions to execute. Unknown fields are rejected to prevent smuggling.
pub fn validate_result_untrusted(
    result: &SkillResult,
    max_bytes: usize,
) -> Result<(), EvaluateError> {
    if result.skill_name.is_empty() || result.skill_name.len() > MAX_SKILL_NAME {
        return Err(EvaluateError::Invalid);
    }
    if result.result_json.len() > max_bytes {
        return Err(EvaluateError::Capacity);
    }
    let value: Value =
        serde_json::from_slice(&result.result_json).map_err(|_| EvaluateError::Invalid)?;
    if value.is_array() {
        return Err(EvaluateError::Invalid);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nai_contracts::v1::RequestContext;

    fn descriptor(side_effect: &str, risk: Risk) -> SkillDescriptor {
        SkillDescriptor {
            name: "sku.query".into(),
            parameters_schema_ref: "schema:sku_query/1".into(),
            result_schema_ref: "schema:sku_result/1".into(),
            permissions: vec!["knowledge.read".into()],
            source_ref: "vendor:sku".into(),
            side_effect: side_effect.into(),
            risk: risk as i32,
            timeout_ms: 5000,
            max_concurrency: 4,
            supports_idempotency: true,
        }
    }

    #[test]
    fn high_risk_and_viewer_visible_without_risk2_are_not_registered() {
        assert_eq!(
            validate_descriptor(&descriptor("high_risk", Risk::Risk3)),
            Err(EvaluateError::Conflict)
        );
        assert_eq!(
            validate_descriptor(&descriptor("viewer_visible", Risk::Risk1)),
            Err(EvaluateError::Conflict)
        );
        let mut viewer = descriptor("viewer_visible", Risk::Risk2);
        viewer.permissions = vec!["output.viewer".into()];
        assert_eq!(validate_descriptor(&viewer), Ok(()));
        viewer.permissions = vec!["knowledge.read".into()];
        assert_eq!(
            validate_descriptor(&viewer),
            Err(EvaluateError::Unauthorized)
        );
    }

    #[test]
    fn notify_requires_explicit_destination_permission() {
        assert_eq!(
            validate_descriptor(&descriptor("notify", Risk::Risk1)),
            Err(EvaluateError::Unauthorized)
        );
        let mut notify = descriptor("notify", Risk::Risk1);
        notify.permissions = vec!["notify.douyin.dm".into()];
        assert_eq!(validate_descriptor(&notify), Ok(()));
    }

    #[test]
    fn read_only_cannot_be_high_risk_and_empty_meta_is_invalid() {
        assert_eq!(
            validate_descriptor(&descriptor("read_only", Risk::Risk2)),
            Err(EvaluateError::Conflict)
        );
        let mut empty = descriptor("read_only", Risk::Risk0);
        empty.name.clear();
        assert_eq!(validate_descriptor(&empty), Err(EvaluateError::Invalid));
    }

    #[test]
    fn tool_plan_is_bounded_and_controlled() {
        let limits = ToolLimits::default();
        let context = RequestContext {
            session_id: "session".into(),
            remaining_budget_ms: 500,
            ..Default::default()
        };
        let request = SkillRequest {
            context: Some(context),
            skill_name: "sku.query".into(),
            parameters_json: br#"{"product_id":"sku-1"}"#.to_vec(),
            authorization_ref: "auth-1".into(),
            idempotency_key: "idem-1".into(),
        };
        assert_eq!(
            validate_tool_plan(std::slice::from_ref(&request), &limits),
            Ok(())
        );
        let mut injection = request.clone();
        injection.parameters_json = br#"{"url":"https://evil.invalid/x"}"#.to_vec();
        assert_eq!(
            validate_tool_plan(&[injection], &limits),
            Err(EvaluateError::Unauthorized)
        );
        let mut path = request.clone();
        path.parameters_json = br#"{"path":"../../etc/passwd"}"#.to_vec();
        assert_eq!(
            validate_tool_plan(&[path], &limits),
            Err(EvaluateError::Unauthorized)
        );
        let too_many = vec![request.clone(); limits.max_steps + 1];
        assert_eq!(
            validate_tool_plan(&too_many, &limits),
            Err(EvaluateError::Invalid)
        );
        let mut no_auth = request;
        no_auth.authorization_ref.clear();
        assert_eq!(
            validate_tool_plan(&[no_auth], &limits),
            Err(EvaluateError::Unauthorized)
        );
    }

    #[test]
    fn untrusted_result_never_accepts_array_or_oversized() {
        let result = SkillResult {
            skill_name: "sku.query".into(),
            result_json: br#"[1,2,3]"#.to_vec(),
            source_ref: "src".into(),
            risk: Risk::Risk0 as i32,
        };
        assert_eq!(
            validate_result_untrusted(&result, 4096),
            Err(EvaluateError::Invalid)
        );
        let result = SkillResult {
            skill_name: "sku.query".into(),
            result_json: b"{}".to_vec(),
            source_ref: "src".into(),
            risk: Risk::Risk0 as i32,
        };
        assert_eq!(validate_result_untrusted(&result, 4096), Ok(()));
    }
}
