//! Interaction preferences only. Applying them never grants a platform or media permission.
use serde::Deserialize;
use std::sync::OnceLock;

const SCHEMA: &str = include_str!("../../../schemas/config/personal-live-policy.schema.json");
const MAX_POLICY_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionMode {
    TopicOnly,
    TopicAndPlatform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnswerTrigger {
    UserTriggered,
    Automatic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeScope {
    GroundedOnly,
    AllowGeneral,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalLivePolicy {
    pub schema_version: u8,
    pub interaction_mode: InteractionMode,
    pub answer_trigger: AnswerTrigger,
    pub knowledge_scope: KnowledgeScope,
    pub sampling_percent: u8,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PolicyError {
    TooLarge,
    Invalid,
}

/// Work the trusted caller must cancel before using the new preference.
/// This value does not perform cancellation or revoke any media permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyChange {
    pub cancel_pending_platform_answers: bool,
    pub cancel_pending_generated_answers: bool,
}

impl PersonalLivePolicy {
    /// Effective preferences when no explicit workspace record exists.
    /// These defaults do not start a session or authorize automatic output.
    pub fn initial() -> Self {
        Self {
            schema_version: 1,
            interaction_mode: InteractionMode::TopicOnly,
            answer_trigger: AnswerTrigger::UserTriggered,
            knowledge_scope: KnowledgeScope::GroundedOnly,
            sampling_percent: 100,
        }
    }

    /// Parse the shared contract; preferences are not evidence of authorization.
    pub fn parse(json: &str) -> Result<Self, PolicyError> {
        if json.len() > MAX_POLICY_BYTES {
            return Err(PolicyError::TooLarge);
        }
        // Deserialize the original object too: Value would silently collapse
        // duplicate keys before the schema validator could see them.
        let parsed: Self = serde_json::from_str(json).map_err(|_| PolicyError::Invalid)?;
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|_| PolicyError::Invalid)?;
        static VALIDATOR: OnceLock<jsonschema::Validator> = OnceLock::new();
        let validator = VALIDATOR.get_or_init(|| {
            let schema =
                serde_json::from_str(SCHEMA).expect("embedded policy schema must be valid JSON");
            jsonschema::validator_for(&schema).expect("embedded policy schema must compile")
        });
        if !validator.is_valid(&value) {
            return Err(PolicyError::Invalid);
        }
        Ok(parsed)
    }

    /// A caller supplies an independent uniform integer in 0..100 for each eligible event.
    /// A positive result is a preference decision, not permission to consume or broadcast.
    pub fn selects_platform_event(&self, roll: u8) -> bool {
        self.interaction_mode == InteractionMode::TopicAndPlatform
            && roll < 100
            && roll < self.sampling_percent
    }

    /// Evaluate a preference change without starting a session or granting authority.
    pub fn change_to(&self, next: &Self) -> PolicyChange {
        PolicyChange {
            cancel_pending_platform_answers: (self.interaction_mode
                == InteractionMode::TopicAndPlatform
                && next.interaction_mode == InteractionMode::TopicOnly)
                || (self.answer_trigger == AnswerTrigger::Automatic
                    && next.answer_trigger == AnswerTrigger::UserTriggered),
            cancel_pending_generated_answers: self.knowledge_scope != next.knowledge_scope,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn policy(mode: &str, percent: u8) -> String {
        format!(
            r#"{{"schema_version":1,"interaction_mode":"{mode}","answer_trigger":"automatic","knowledge_scope":"grounded_only","sampling_percent":{percent}}}"#
        )
    }
    #[test]
    fn sampling_boundaries_and_topic_only() {
        let zero = PersonalLivePolicy::parse(&policy("topic_and_platform", 0)).unwrap();
        assert!(!zero.selects_platform_event(0));
        let half = PersonalLivePolicy::parse(&policy("topic_and_platform", 50)).unwrap();
        assert!(half.selects_platform_event(49));
        assert!(!half.selects_platform_event(50));
        let all = PersonalLivePolicy::parse(&policy("topic_and_platform", 100)).unwrap();
        assert!(all.selects_platform_event(99));
        assert!(!all.selects_platform_event(100));
        assert!(
            !PersonalLivePolicy::parse(&policy("topic_only", 100))
                .unwrap()
                .selects_platform_event(0)
        );
    }
    #[test]
    fn rejects_invalid_and_forged_authority() {
        assert_eq!(
            PersonalLivePolicy::parse(&policy("topic_and_platform", 101)),
            Err(PolicyError::Invalid)
        );
        let forged = policy("topic_only", 10).replace('}', ",\"platform_authorized\":true}");
        assert_eq!(
            PersonalLivePolicy::parse(&forged),
            Err(PolicyError::Invalid)
        );
        assert_eq!(
            PersonalLivePolicy::parse(&" ".repeat(MAX_POLICY_BYTES + 1)),
            Err(PolicyError::TooLarge)
        );
        assert_eq!(PersonalLivePolicy::parse("{}"), Err(PolicyError::Invalid));
        let duplicate =
            policy("topic_only", 10).replace('}', ",\"interaction_mode\":\"topic_and_platform\"}");
        assert_eq!(
            PersonalLivePolicy::parse(&duplicate),
            Err(PolicyError::Invalid)
        );
    }

    #[test]
    fn initial_preferences_do_not_enable_platform_or_automatic_answers() {
        let initial = PersonalLivePolicy::initial();
        assert_eq!(initial.interaction_mode, InteractionMode::TopicOnly);
        assert_eq!(initial.answer_trigger, AnswerTrigger::UserTriggered);
        assert_eq!(initial.knowledge_scope, KnowledgeScope::GroundedOnly);
        assert_eq!(initial.sampling_percent, 100);
        assert!(!initial.selects_platform_event(0));
        let contract = r#"{"schema_version":1,"interaction_mode":"topic_only","answer_trigger":"user_triggered","knowledge_scope":"grounded_only","sampling_percent":100}"#;
        assert_eq!(PersonalLivePolicy::parse(contract).unwrap(), initial);
    }

    #[test]
    fn changes_cancel_stale_work_without_retroactive_resampling() {
        let old = PersonalLivePolicy::parse(&policy("topic_and_platform", 50)).unwrap();
        let ratio = PersonalLivePolicy::parse(&policy("topic_and_platform", 10)).unwrap();
        assert_eq!(
            old.change_to(&ratio),
            PolicyChange {
                cancel_pending_platform_answers: false,
                cancel_pending_generated_answers: false,
            }
        );
        let topic = PersonalLivePolicy::parse(&policy("topic_only", 50)).unwrap();
        assert!(old.change_to(&topic).cancel_pending_platform_answers);
        let mut manual = old.clone();
        manual.answer_trigger = AnswerTrigger::UserTriggered;
        assert!(old.change_to(&manual).cancel_pending_platform_answers);
        assert!(!manual.change_to(&old).cancel_pending_platform_answers);
        let mut scope = old.clone();
        scope.knowledge_scope = KnowledgeScope::AllowGeneral;
        assert!(old.change_to(&scope).cancel_pending_generated_answers);
        assert!(scope.change_to(&old).cancel_pending_generated_answers);
    }
}
