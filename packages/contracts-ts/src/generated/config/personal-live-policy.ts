/* Generated from JSON Schema. Do not edit. */

/**
 * User preferences only; does not grant platform access or media execution.
 */
export interface PersonalLivePolicy {
  schema_version: 1;
  interaction_mode: "topic_only" | "topic_and_platform";
  answer_trigger: "user_triggered" | "automatic";
  knowledge_scope: "grounded_only" | "allow_general";
  sampling_percent: number;
}
