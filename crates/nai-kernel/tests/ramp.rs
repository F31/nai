//! W67: harness ramp gating (H18), V3.9 §5.9 / §47.2 / §47.3.
//!
//! The guards here are chosen so that a plausible-but-wrong implementation fails: a gate that
//! ignores the regression, ignores the enablement record, ignores the harness revision, scans
//! field names instead of values, or reports only the first missing category is each exercised
//! explicitly rather than assumed away.

use nai_kernel::evaluate::ramp::{
    DEFAULT_ENABLED_STAGES, EnablementRecord, PromotionRequest, RAMP_ORDER, REGRESSION_CATEGORIES,
    RampRejection, RampStage, RegressionCategory, evaluate_promotion,
};
use std::collections::BTreeMap;

const REV: u64 = 7;

fn record(mechanism: &str, stage: RampStage) -> EnablementRecord {
    EnablementRecord {
        mechanism: mechanism.to_string(),
        stage,
        fields: BTreeMap::new(),
    }
}

fn req<'a>(
    mechanism: &'a str,
    current: RampStage,
    target: RampStage,
    touched: &'a [RegressionCategory],
    passed: &'a [RegressionCategory],
    passed_at_revision: Option<u64>,
    record: Option<&'a EnablementRecord>,
) -> PromotionRequest<'a> {
    PromotionRequest {
        mechanism,
        current,
        target,
        touched,
        passed,
        passed_at_revision,
        harness_revision: REV,
        record,
    }
}

fn clean_req<'a>(
    current: RampStage,
    target: RampStage,
    all: &'a [RegressionCategory],
    rec: Option<&'a EnablementRecord>,
) -> PromotionRequest<'a> {
    req("m", current, target, all, all, Some(REV), rec)
}

// ---------------------------------------------------------------------------
// Baseline tables: pinned by content, not by count
// ---------------------------------------------------------------------------

#[test]
fn ramp_order_matches_the_baseline() {
    let labels: Vec<&str> = RAMP_ORDER.iter().map(|stage| stage.label()).collect();
    assert_eq!(
        labels,
        vec!["rehearsal", "shadow", "assisted", "auto_supervised"]
    );
    for (index, stage) in RAMP_ORDER.iter().enumerate() {
        assert_eq!(stage.index(), index);
    }
}

#[test]
fn default_enabled_stages_are_exactly_the_first_three() {
    assert_eq!(
        DEFAULT_ENABLED_STAGES,
        [RampStage::Rehearsal, RampStage::Shadow, RampStage::Assisted]
    );
    for stage in RAMP_ORDER {
        assert_eq!(
            stage.enabled_by_default(),
            DEFAULT_ENABLED_STAGES.contains(&stage),
            "{:?} disagrees with the transcribed default-enablement set",
            stage
        );
    }
    assert!(!RampStage::AutoSupervised.enabled_by_default());
}

/// The finding that must stay visible: §47.3 names four stages, but `SessionMode` has three and
/// §1.4/§17.3 place 影子模式 in R2. Shadow therefore has no landing point inside R1.
#[test]
fn every_stage_but_shadow_has_a_session_mode() {
    let modes: Vec<Option<i32>> = RAMP_ORDER
        .iter()
        .map(|stage| stage.session_mode().map(|mode| mode as i32))
        .collect();
    assert_eq!(
        modes.iter().filter(|mode| mode.is_some()).count(),
        3,
        "exactly three stages land on a SessionMode"
    );
    assert!(
        modes[RampStage::Shadow.index()].is_none(),
        "shadow MUST NOT be given a fabricated landing point"
    );
    let mut distinct: Vec<i32> = modes.iter().flatten().copied().collect();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), 3, "the three landing points are distinct");
}

#[test]
fn regression_categories_match_the_baseline_table() {
    let labels: Vec<&str> = REGRESSION_CATEGORIES
        .iter()
        .map(|category| category.label())
        .collect();
    assert_eq!(
        labels,
        vec![
            "regression.assembly",
            "regression.intent_refusal",
            "regression.tool_failure_recovery",
            "regression.cache_false_hit",
            "regression.mcp_description_drift",
            "regression.a2a_excess_delegation",
        ]
    );
}

#[test]
fn rejection_labels_are_distinct_and_all_are_constructible() {
    let mut labels: Vec<&str> = RampRejection::ALL.iter().map(|r| r.label()).collect();
    let total = labels.len();
    labels.sort_unstable();
    labels.dedup();
    assert_eq!(labels.len(), total, "rejection labels MUST be distinct");
}

// ---------------------------------------------------------------------------
// The gate
// ---------------------------------------------------------------------------

#[test]
fn constructed_rejection_set_is_exactly_all() {
    let all = REGRESSION_CATEGORIES;
    let auto = RampStage::AutoSupervised;
    let good = record("m", auto);
    let other_mechanism = record("other", auto);
    let other_stage = record("m", RampStage::Assisted);
    let mut leaked = record("m", auto);
    leaked
        .fields
        .insert("note".to_string(), "sk-live-abcdef".to_string());

    let only_assembly = [RegressionCategory::AssemblyRegression];
    let two_missing = [
        RegressionCategory::AssemblyRegression,
        RegressionCategory::ToolFailureRecovery,
    ];

    let cases: Vec<(&str, PromotionRequest<'_>, RampRejection)> = vec![
        (
            "not an advance",
            req(
                "m",
                RampStage::Assisted,
                RampStage::Rehearsal,
                &all,
                &all,
                Some(REV),
                None,
            ),
            RampRejection::NotAnAdvance,
        ),
        (
            "no declared scope",
            req(
                "m",
                RampStage::Assisted,
                auto,
                &[],
                &all,
                Some(REV),
                Some(&good),
            ),
            RampRejection::RegressionScopeUndeclared,
        ),
        (
            "a declared category not passed",
            req(
                "m",
                RampStage::Assisted,
                auto,
                &two_missing,
                &only_assembly,
                Some(REV),
                Some(&good),
            ),
            RampRejection::RegressionMissing,
        ),
        (
            "regression bound to an older revision",
            req(
                "m",
                RampStage::Assisted,
                auto,
                &only_assembly,
                &only_assembly,
                Some(REV - 1),
                Some(&good),
            ),
            RampRejection::RegressionStale,
        ),
        (
            "no enablement record",
            req("m", RampStage::Assisted, auto, &all, &all, Some(REV), None),
            RampRejection::EnablementRecordMissing,
        ),
        (
            "record names another mechanism",
            req(
                "m",
                RampStage::Assisted,
                auto,
                &all,
                &all,
                Some(REV),
                Some(&other_mechanism),
            ),
            RampRejection::RecordMechanismMismatch,
        ),
        (
            "record names another stage",
            req(
                "m",
                RampStage::Assisted,
                auto,
                &all,
                &all,
                Some(REV),
                Some(&other_stage),
            ),
            RampRejection::RecordStageMismatch,
        ),
        (
            "record carries a credential value",
            req(
                "m",
                RampStage::Assisted,
                auto,
                &all,
                &all,
                Some(REV),
                Some(&leaked),
            ),
            RampRejection::RecordContainsCredential,
        ),
    ];

    let mut constructed = Vec::new();
    for (name, request, expected) in &cases {
        let verdict = evaluate_promotion(request);
        assert!(
            !verdict.allowed,
            "{}: a refused promotion MUST NOT be allowed",
            name
        );
        assert_eq!(
            verdict.rejections,
            vec![*expected],
            "{}: expected exactly one named rejection",
            name
        );
        constructed.push(*expected);
    }

    constructed.sort_by_key(|rejection| rejection.label());
    let mut expected: Vec<RampRejection> = RampRejection::ALL.to_vec();
    expected.sort_by_key(|rejection| rejection.label());
    assert_eq!(
        constructed, expected,
        "the constructed set MUST be exactly RampRejection::ALL"
    );
}

#[test]
fn a_promotion_into_auto_with_everything_in_order_is_allowed() {
    let all = REGRESSION_CATEGORIES;
    let rec = record("m", RampStage::AutoSupervised);
    let verdict = evaluate_promotion(&clean_req(
        RampStage::Assisted,
        RampStage::AutoSupervised,
        &all,
        Some(&rec),
    ));
    assert!(verdict.allowed, "every condition met ⇒ allowed");
    assert!(verdict.rejections.is_empty());
    assert_eq!(verdict.from, RampStage::Assisted);
    assert_eq!(verdict.to, RampStage::AutoSupervised);
    let appended = verdict
        .record_to_append
        .expect("an allowed promotion into auto MUST produce a record to append");
    assert_eq!(appended.mechanism, "m");
    assert_eq!(appended.stage, RampStage::AutoSupervised);
}

#[test]
fn stages_below_auto_need_only_to_be_an_advance() {
    let all = REGRESSION_CATEGORIES;
    // No regression, no record: below auto this is enabled by default (§47.3).
    let verdict = evaluate_promotion(&clean_req(
        RampStage::Rehearsal,
        RampStage::Shadow,
        &all,
        None,
    ));
    assert!(verdict.allowed, "rehearsal → shadow is an advance");
    assert!(
        verdict.record_to_append.is_none(),
        "only auto carries an enablement requirement"
    );

    let sideways = evaluate_promotion(&clean_req(
        RampStage::Assisted,
        RampStage::Assisted,
        &all,
        None,
    ));
    assert!(!sideways.allowed);
    assert_eq!(sideways.rejections, vec![RampRejection::NotAnAdvance]);

    let backwards = evaluate_promotion(&clean_req(
        RampStage::AutoSupervised,
        RampStage::Assisted,
        &all,
        None,
    ));
    assert!(!backwards.allowed, "a ramp does not move backwards");
    assert_eq!(backwards.rejections, vec![RampRejection::NotAnAdvance]);
}

#[test]
fn missing_categories_are_all_named() {
    let declared = [
        RegressionCategory::AssemblyRegression,
        RegressionCategory::ToolFailureRecovery,
        RegressionCategory::CacheFalseHit,
    ];
    let passed = [RegressionCategory::AssemblyRegression];
    let rec = record("m", RampStage::AutoSupervised);
    let verdict = evaluate_promotion(&req(
        "m",
        RampStage::Assisted,
        RampStage::AutoSupervised,
        &declared,
        &passed,
        Some(REV),
        Some(&rec),
    ));
    assert_eq!(
        verdict.missing_categories,
        vec![
            "regression.tool_failure_recovery",
            "regression.cache_false_hit",
        ],
        "reporting only the first missing category does not satisfy this gate"
    );
    assert!(
        verdict
            .rejections
            .contains(&RampRejection::RegressionMissing)
    );
}

#[test]
fn an_undeclared_scope_is_refused_even_when_everything_passed() {
    let all = REGRESSION_CATEGORIES;
    let rec = record("m", RampStage::AutoSupervised);
    let verdict = evaluate_promotion(&req(
        "m",
        RampStage::Assisted,
        RampStage::AutoSupervised,
        &[],
        &all,
        Some(REV),
        Some(&rec),
    ));
    assert!(!verdict.allowed);
    assert_eq!(
        verdict.rejections,
        vec![RampRejection::RegressionScopeUndeclared],
        "declaring nothing MUST NOT be read as 'no relevant regression'"
    );
}

#[test]
fn a_regression_without_a_revision_is_stale() {
    let all = REGRESSION_CATEGORIES;

    let rec = record("m", RampStage::AutoSupervised);
    let verdict = evaluate_promotion(&req(
        "m",
        RampStage::Assisted,
        RampStage::AutoSupervised,
        &all,
        &all,
        None,
        Some(&rec),
    ));
    assert!(!verdict.allowed);
    assert!(
        verdict.rejections.contains(&RampRejection::RegressionStale),
        "'passed, unknown when' is not a pass against the revision in force"
    );
}

// ---------------------------------------------------------------------------
// INV-11: judged by value, never by field name
// ---------------------------------------------------------------------------

#[test]
fn a_field_whose_name_looks_sensitive_but_whose_value_is_clean_passes() {
    let all = REGRESSION_CATEGORIES;
    let mut rec = record("m", RampStage::AutoSupervised);
    // Every name below contains a credential marker (`sk-`, `api_key=`, `secret=`), so a
    // name-scanning implementation refuses this record. Every value is clean, so a
    // value-scanning one must let it through.
    rec.fields.insert("sk-cache".to_string(), "hot".to_string());
    rec.fields
        .insert("api_key=".to_string(), "rotated-q3".to_string());
    rec.fields
        .insert("secret=".to_string(), "not-a-credential".to_string());
    let verdict = evaluate_promotion(&clean_req(
        RampStage::Assisted,
        RampStage::AutoSupervised,
        &all,
        Some(&rec),
    ));
    assert!(
        verdict.allowed,
        "judged by value, never by field name (INV-11)"
    );
    assert!(verdict.leaked_fields.is_empty());
}

#[test]
fn a_credential_value_is_named_and_refused() {
    let all = REGRESSION_CATEGORIES;
    let mut rec = record("m", RampStage::AutoSupervised);
    rec.fields
        .insert("note".to_string(), "bearer eyJhbGciOi".to_string());
    let verdict = evaluate_promotion(&clean_req(
        RampStage::Assisted,
        RampStage::AutoSupervised,
        &all,
        Some(&rec),
    ));
    assert!(!verdict.allowed);
    assert!(
        verdict
            .rejections
            .contains(&RampRejection::RecordContainsCredential)
    );
    assert_eq!(verdict.leaked_fields, vec!["note".to_string()]);
    assert!(
        verdict.record_to_append.is_none(),
        "a refused promotion MUST NOT produce a record to append"
    );
}
