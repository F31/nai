//! W66: harness observation (H15) and named degradation (H17), V3.9 §47.1 / §47.4 / §12.2.
//!
//! The guards here are chosen so that a plausible-but-wrong implementation fails: a field-name
//! blacklist, a single-level degradation map, and "emit a code then generate anyway" are each
//! exercised explicitly rather than assumed away.

use nai_kernel::evaluate::harness::{
    CREDENTIAL_MARKERS, DEGRADE_ORDER, DegradeContext, DegradeLevel, DegradeVerdict,
    DegradedCallContext, FAILURE_LAYERS, FailureLayer, FreeFormViolation, OBSERVATION_LAYERS,
    ObservationLayer, ObservationPolicy, ObservationRecord, ObservationRejection, ViewerText,
    assert_no_free_form, evaluate_degradation, evaluate_observation,
};
use std::collections::BTreeMap;

fn complete_fields(layer: ObservationLayer) -> BTreeMap<String, String> {
    layer
        .required_fields()
        .iter()
        .map(|field| (field.to_string(), "recorded".to_string()))
        .collect()
}

fn record(layer: ObservationLayer) -> ObservationRecord {
    ObservationRecord {
        layer,
        subject: "subject".to_string(),
        fields: complete_fields(layer),
        viewer_text: None,
    }
}

// ---------------------------------------------------------------------------
// H15: observation
// ---------------------------------------------------------------------------

#[test]
fn observation_kinds_are_exactly_five_with_distinct_labels() {
    assert_eq!(OBSERVATION_LAYERS.len(), 5);
    let labels: Vec<&str> = OBSERVATION_LAYERS.iter().map(|l| l.label()).collect();
    let unique: std::collections::BTreeSet<&str> = labels.iter().copied().collect();
    assert_eq!(unique.len(), 5, "labels must be distinct: {labels:?}");
}

#[test]
fn required_fields_are_non_empty_and_differ_between_kinds() {
    let mut seen: Vec<&[&str]> = Vec::new();
    for layer in OBSERVATION_LAYERS {
        let fields = layer.required_fields();
        assert!(!fields.is_empty(), "{:?} has no required fields", layer);
        assert!(
            !seen.contains(&fields),
            "{:?} repeats another kind's field set",
            layer
        );
        seen.push(fields);
    }
}

/// Pins each kind's field list to the §47.1 table. Without this, trimming a field would go
/// unnoticed — the other tests only check that whatever is required is actually enforced.
#[test]
fn required_fields_match_the_baseline_table() {
    assert_eq!(
        ObservationLayer::Assembly.required_fields(),
        &[
            "fragment_ref",
            "fragment_source",
            "segment_budget",
            "cropped_items",
            "crop_reason",
            "model_version",
            "prompt_version",
        ]
    );
    assert_eq!(
        ObservationLayer::Intent.required_fields(),
        &[
            "enum_value",
            "confidence",
            "evidence_ref",
            "threshold_hit",
            "guard_conflict"
        ]
    );
    assert_eq!(
        ObservationLayer::Memory.required_fields(),
        &["key", "scope", "source", "hit", "delete_status"]
    );
    assert_eq!(
        ObservationLayer::Tool.required_fields(),
        &[
            "skill_id",
            "skill_version",
            "params_hash",
            "result_summary",
            "result_size",
            "termination_reason",
            "cost",
        ]
    );
    assert_eq!(
        ObservationLayer::Cache.required_fields(),
        &["hit", "invalidation_reason", "reused_entry_version"]
    );
}

#[test]
fn rejection_labels_are_distinct_and_all_are_constructible() {
    let labels: Vec<&str> = ObservationRejection::ALL
        .iter()
        .map(|r| r.label())
        .collect();
    let unique: std::collections::BTreeSet<&str> = labels.iter().copied().collect();
    assert_eq!(
        unique.len(),
        ObservationRejection::ALL.len(),
        "labels must be distinct: {labels:?}"
    );
}

#[test]
fn a_complete_clean_record_is_loggable() {
    let verdict = evaluate_observation(
        &record(ObservationLayer::Assembly),
        &ObservationPolicy::default(),
    );
    assert!(verdict.loggable, "rejections: {:?}", verdict.rejections);
    assert!(verdict.rejections.is_empty());
    assert_eq!(verdict.redacted.len(), 7);
}

#[test]
fn a_missing_required_field_is_named_not_defaulted() {
    let mut rec = record(ObservationLayer::Cache);
    rec.fields.remove("invalidation_reason");
    let verdict = evaluate_observation(&rec, &ObservationPolicy::default());
    assert!(!verdict.loggable);
    assert!(
        verdict
            .rejections
            .contains(&ObservationRejection::MissingRequiredField)
    );
    assert_eq!(verdict.missing_fields, vec!["invalidation_reason"]);
}

#[test]
fn a_blank_required_value_is_not_a_recorded_value() {
    let mut rec = record(ObservationLayer::Cache);
    rec.fields.insert("hit".to_string(), "  ".to_string());
    let verdict = evaluate_observation(&rec, &ObservationPolicy::default());
    assert!(!verdict.loggable);
    assert!(
        verdict
            .rejections
            .contains(&ObservationRejection::EmptyFieldValue)
    );
    assert_eq!(verdict.empty_fields, vec!["hit"]);
}

/// Guard ②a: the credential check reads the **value**.
#[test]
fn a_credential_in_the_value_is_rejected_and_redacted() {
    let mut rec = record(ObservationLayer::Tool);
    rec.fields.insert(
        "result_summary".to_string(),
        "Authorization: Bearer sk-live-abc123".to_string(),
    );
    let verdict = evaluate_observation(&rec, &ObservationPolicy::default());
    assert!(!verdict.loggable);
    assert!(
        verdict
            .rejections
            .contains(&ObservationRejection::PlaintextCredential)
    );
    assert!(
        verdict
            .leaked_fields
            .contains(&"result_summary".to_string())
    );
    assert_eq!(
        verdict.redacted.get("result_summary").map(String::as_str),
        Some("[redacted]"),
        "the original bytes must not survive into the loggable payload"
    );
    assert!(
        !verdict
            .redacted
            .values()
            .any(|v| v.to_ascii_lowercase().contains("sk-live"))
    );
}

/// Guard ②b: the anti-vacuous half. A field **named** `key` whose value is clean must pass —
/// otherwise the rule is a field-name blacklist, not a credential scan.
#[test]
fn a_field_named_key_with_a_clean_value_passes() {
    let mut rec = record(ObservationLayer::Memory);
    rec.fields
        .insert("key".to_string(), "viewer:topic".to_string());
    let verdict = evaluate_observation(&rec, &ObservationPolicy::default());
    assert!(
        verdict.loggable,
        "a clean value must not be rejected for its field name: {:?}",
        verdict.rejections
    );
    assert!(
        !verdict
            .rejections
            .contains(&ObservationRejection::PlaintextCredential)
    );
    assert_eq!(
        verdict.redacted.get("key").map(String::as_str),
        Some("viewer:topic")
    );
}

#[test]
fn credential_markers_are_checked_case_insensitively() {
    let mut rec = record(ObservationLayer::Tool);
    rec.fields
        .insert("cost".to_string(), "TOKEN=abc".to_string());
    let verdict = evaluate_observation(&rec, &ObservationPolicy::default());
    assert!(
        verdict
            .rejections
            .contains(&ObservationRejection::PlaintextCredential)
    );
    assert!(CREDENTIAL_MARKERS.contains(&"token="));
}

#[test]
fn unmasked_viewer_text_is_rejected_and_masked_in_the_payload() {
    let mut rec = record(ObservationLayer::Memory);
    rec.viewer_text = Some(ViewerText {
        text: "原始评论与昵称".to_string(),
        masked: false,
        age_ms: 10,
    });
    let verdict = evaluate_observation(&rec, &ObservationPolicy::default());
    assert!(!verdict.loggable);
    assert!(
        verdict
            .rejections
            .contains(&ObservationRejection::ViewerTextUnmasked)
    );
    assert_eq!(verdict.redacted_viewer.as_deref(), Some("[masked]"));
}

#[test]
fn viewer_text_past_retention_is_dropped_not_kept() {
    let mut rec = record(ObservationLayer::Memory);
    rec.viewer_text = Some(ViewerText {
        text: "已脱敏".to_string(),
        masked: true,
        age_ms: ObservationPolicy::default().max_viewer_retention_ms + 1,
    });
    let verdict = evaluate_observation(&rec, &ObservationPolicy::default());
    assert!(!verdict.loggable);
    assert!(
        verdict
            .rejections
            .contains(&ObservationRejection::ViewerTextOverRetention)
    );
    assert_eq!(
        verdict.redacted_viewer, None,
        "over-retention text must be dropped, not merely marked"
    );
}

// ---------------------------------------------------------------------------
// H17: named degradation
// ---------------------------------------------------------------------------

#[test]
fn failure_codes_are_distinct_and_non_empty() {
    let codes: Vec<&str> = FAILURE_LAYERS.iter().map(|l| l.code()).collect();
    assert_eq!(codes.len(), 6);
    let unique: std::collections::BTreeSet<&str> = codes.iter().copied().collect();
    assert_eq!(unique.len(), 6, "codes must be distinct: {codes:?}");
    for code in codes {
        assert!(!code.is_empty());
    }
}

#[test]
fn the_degrade_order_is_the_documented_four() {
    assert_eq!(DEGRADE_ORDER.len(), 4);
    assert_eq!(DEGRADE_ORDER[0], DegradeLevel::Template);
    assert_eq!(DEGRADE_ORDER[1], DegradeLevel::ApprovedCache);
    assert_eq!(DEGRADE_ORDER[2], DegradeLevel::Human);
    assert_eq!(DEGRADE_ORDER[3], DegradeLevel::SilentStandby);
    for (index, level) in DEGRADE_ORDER.iter().enumerate() {
        assert_eq!(level.index(), index);
    }
}

#[test]
fn assembly_failure_refuses_the_call_and_lands_on_template() {
    let verdict = evaluate_degradation(FailureLayer::Assembly, &DegradeContext::all_available());
    assert_eq!(verdict.level, DegradeLevel::Template);
    assert!(verdict.refuses_call, "§47.4: the call is refused outright");
    assert_eq!(verdict.code, "HARNESS_ASSEMBLY_FAILED");
    assert!(verdict.visible);
}

/// Guard ③, the discriminating half: intent and cache land on **human** even though a template
/// is available. An implementation that maps every failure to the first available level fails.
#[test]
fn intent_and_cache_land_on_human_even_when_the_template_is_available() {
    for layer in [
        FailureLayer::Intent,
        FailureLayer::Cache,
        FailureLayer::Peer,
    ] {
        let verdict = evaluate_degradation(layer, &DegradeContext::all_available());
        assert_eq!(
            verdict.level,
            DegradeLevel::Human,
            "{layer:?} must not take the template level just because it is available"
        );
        assert!(!verdict.refuses_call);
    }
    for layer in [FailureLayer::MemoryRead, FailureLayer::Tool] {
        let verdict = evaluate_degradation(layer, &DegradeContext::all_available());
        assert_eq!(verdict.level, DegradeLevel::Template, "{layer:?}");
        assert!(!verdict.refuses_call);
    }
}

#[test]
fn the_walk_down_skips_unavailable_levels() {
    let ctx = DegradeContext {
        template_available: false,
        approved_cache_available: false,
        human_available: true,
    };
    assert_eq!(
        evaluate_degradation(FailureLayer::Tool, &ctx).level,
        DegradeLevel::Human
    );
    // Silent standby is the floor, so the walk always terminates.
    let none = DegradeContext {
        template_available: false,
        approved_cache_available: false,
        human_available: false,
    };
    assert_eq!(
        evaluate_degradation(FailureLayer::Tool, &none).level,
        DegradeLevel::SilentStandby
    );
    assert_eq!(
        evaluate_degradation(FailureLayer::Intent, &none).level,
        DegradeLevel::SilentStandby
    );
}

#[test]
fn assembly_still_refuses_the_call_after_walking_down() {
    let none = DegradeContext {
        template_available: false,
        approved_cache_available: false,
        human_available: false,
    };
    let verdict = evaluate_degradation(FailureLayer::Assembly, &none);
    assert_eq!(verdict.level, DegradeLevel::SilentStandby);
    assert!(
        verdict.refuses_call,
        "refusing the call does not lapse when the template is unavailable"
    );
}

#[test]
fn every_degradation_is_visible_and_names_what_it_drops() {
    for layer in FAILURE_LAYERS {
        let verdict = evaluate_degradation(layer, &DegradeContext::all_available());
        assert!(
            verdict.visible,
            "{layer:?}: there is no invisible degradation"
        );
        assert!(!verdict.actionable.is_empty(), "{layer:?}");
        if layer != FailureLayer::Assembly {
            assert!(verdict.drops.is_some(), "{layer:?} must name what it drops");
        }
    }
}

#[test]
fn a_peer_going_away_leaves_running_sessions_alone() {
    let verdict = evaluate_degradation(FailureLayer::Peer, &DegradeContext::all_available());
    assert!(verdict.keeps_existing_session);
    for layer in FAILURE_LAYERS {
        if layer != FailureLayer::Peer {
            assert!(
                !evaluate_degradation(layer, &DegradeContext::all_available())
                    .keeps_existing_session,
                "{layer:?} is not a peer outage"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// INV-32: no free-form generation
// ---------------------------------------------------------------------------

fn degraded_with(degradations: Vec<DegradeVerdict>) -> DegradedCallContext {
    DegradedCallContext {
        degradations,
        has_policy_constraint: true,
        has_persona: true,
    }
}

#[test]
fn a_degraded_call_that_keeps_its_constraints_may_proceed() {
    let call = degraded_with(vec![evaluate_degradation(
        FailureLayer::Tool,
        &DegradeContext::all_available(),
    )]);
    assert!(assert_no_free_form(&call).is_empty());
}

/// Guard ④: naming the code is not enough. A degraded call that has lost its policy or persona
/// constraint is a violation even though the degradation was named.
#[test]
fn a_degraded_call_without_its_policy_constraint_is_a_violation() {
    let mut call = degraded_with(vec![evaluate_degradation(
        FailureLayer::Assembly,
        &DegradeContext::all_available(),
    )]);
    call.has_policy_constraint = false;
    let violations = assert_no_free_form(&call);
    assert_eq!(
        violations,
        vec![FreeFormViolation::DegradedWithoutPolicyConstraint]
    );

    let mut both = degraded_with(vec![evaluate_degradation(
        FailureLayer::Tool,
        &DegradeContext::all_available(),
    )]);
    both.has_policy_constraint = false;
    both.has_persona = false;
    assert_eq!(assert_no_free_form(&both), FreeFormViolation::ALL.to_vec());
}

#[test]
fn an_undegraded_call_is_not_judged_by_the_degradation_rule() {
    let mut call = degraded_with(Vec::new());
    call.has_policy_constraint = false;
    call.has_persona = false;
    assert!(
        assert_no_free_form(&call).is_empty(),
        "no degradation means the free-form rule does not apply"
    );
}

#[test]
fn free_form_violation_labels_are_distinct() {
    let labels: Vec<&str> = FreeFormViolation::ALL.iter().map(|v| v.label()).collect();
    let unique: std::collections::BTreeSet<&str> = labels.iter().copied().collect();
    assert_eq!(unique.len(), FreeFormViolation::ALL.len());
}
