//! W105: the H18 runtime wiring. The judge already existed (W67); these tests are about the step
//! after it — that an allowed promotion into supervised automatic really reaches the append-only
//! hash chain as its **enablement record**, that a refusal does not, and that a promotion which is
//! allowed into a stage **before** the gate is reported as its own outcome rather than being
//! collapsed into "logged" or "refused".
//!
//! The non-void guard is in `every_ramp_rejection_is_reachable_through_the_runtime`: it iterates
//! `RampRejection::ALL` and, for each variant, builds a runtime scenario whose refusal must name
//! it. The per-variant sample constructor matches the enum exhaustively, so adding a variant to
//! the kernel without covering it here fails to compile rather than passing invisibly.
mod common;
use common::*;
use nai_kernel::evaluate::harness::CREDENTIAL_MARKERS;
use nai_kernel::evaluate::ramp::{
    EnablementRecord, PromotionRequest, RampRejection, RampStage, RegressionCategory,
};
use nai_runtime::{Composition, RampOutcome};
use nai_storage::{DataPaths, DecisionAck, DecisionEntryView};
use std::collections::BTreeMap;
use tempfile::tempdir;

const MECHANISM: &str = "ramp-mech-1";
const REVISION: u64 = 7;
const OTHER_MECHANISM: &str = "ramp-mech-2";

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

/// An enablement record as a producer would offer it for the log.
fn record(mechanism: &str, stage: RampStage, fields: &[(&str, &str)]) -> EnablementRecord {
    let fields: BTreeMap<String, String> = fields
        .iter()
        .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
        .collect();
    EnablementRecord {
        mechanism: mechanism.to_string(),
        stage,
        fields,
    }
}

/// One promotion attempt, owning everything it lends to [`PromotionRequest`].
///
/// The canonical attempt is an **allowed** move from `assisted` into `auto_supervised`: one
/// declared regression category, one passed, passed against the harness revision in force, and an
/// enablement record naming the same mechanism and stage. Each test perturbs exactly one thing.
struct Attempt {
    mechanism: String,
    current: RampStage,
    target: RampStage,
    touched: Vec<RegressionCategory>,
    passed: Vec<RegressionCategory>,
    passed_at_revision: Option<u64>,
    harness_revision: u64,
    record: Option<EnablementRecord>,
}

impl Attempt {
    fn new(current: RampStage, target: RampStage) -> Self {
        Self {
            mechanism: MECHANISM.to_string(),
            current,
            target,
            touched: vec![RegressionCategory::AssemblyRegression],
            passed: vec![RegressionCategory::AssemblyRegression],
            passed_at_revision: Some(REVISION),
            harness_revision: REVISION,
            record: Some(record(MECHANISM, target, &[("window", "2026-10")])),
        }
    }

    fn request(&self) -> PromotionRequest<'_> {
        PromotionRequest {
            mechanism: &self.mechanism,
            current: self.current,
            target: self.target,
            touched: &self.touched,
            passed: &self.passed,
            passed_at_revision: self.passed_at_revision,
            harness_revision: self.harness_revision,
            record: self.record.as_ref(),
        }
    }
}

/// Every persisted decision row, oldest first.
fn all_rows(composition: &Composition) -> Vec<DecisionEntryView> {
    let operation = store_operation();
    let store = composition.store().lock().unwrap();
    store
        .decision_entries(&operation, composition.workspace(), 500)
        .unwrap()
}

fn rows_of_kind(composition: &Composition, kind: &str) -> Vec<DecisionEntryView> {
    all_rows(composition)
        .into_iter()
        .filter(|row| row.kind == kind)
        .collect()
}

fn row_count(composition: &Composition) -> usize {
    all_rows(composition).len()
}

/// An allowed offer MUST come back as `Logged`; any other arm is the failure under test.
fn expect_logged(outcome: RampOutcome) -> DecisionAck {
    match outcome {
        RampOutcome::Logged(ack) => ack,
        RampOutcome::Refused(refusal) => panic!(
            "an allowed promotion was refused: rejections={:?} missing={:?}",
            refusal.rejections, refusal.missing_categories
        ),
        RampOutcome::NoRecordRequired { from, to } => panic!(
            "an allowed promotion into supervised automatic came back as NoRecordRequired \
             ({} -> {})",
            from.label(),
            to.label()
        ),
    }
}

/// A refusal MUST come back as `Refused`; `Logged` here is exactly the silent-degradation failure
/// the requirement forbids.
fn expect_refusal(outcome: RampOutcome) -> nai_runtime::RampRefusal {
    match outcome {
        RampOutcome::Refused(refusal) => refusal,
        RampOutcome::Logged(ack) => panic!(
            "a promotion that must not be logged came back as Logged (persisted={}, hash={:?})",
            ack.persisted, ack.entry_hash
        ),
        RampOutcome::NoRecordRequired { from, to } => panic!(
            "a refused promotion came back as NoRecordRequired ({} -> {})",
            from.label(),
            to.label()
        ),
    }
}

/// Every credential literal a persisted row could carry. A logged enablement record is the one
/// place where caller-supplied strings reach the chain, so this is the scan that makes "no
/// plaintext credential was persisted" a testable statement rather than a claim.
fn assert_no_credential_in_rows(composition: &Composition) {
    for row in all_rows(composition) {
        let lowered = row.payload_json.to_ascii_lowercase();
        for marker in CREDENTIAL_MARKERS {
            assert!(
                !lowered.contains(marker),
                "a persisted row carries the credential marker {marker:?}: {}",
                row.payload_json
            );
        }
    }
}

/// An allowed promotion into supervised automatic is appended as **one** `approval` entry carrying
/// the mechanism and both stages, and the chain still verifies.
///
/// `approval` is a critical kind, so the entry must be persisted (`persisted == true`) rather than
/// parked in the droppable telemetry buffer — an enablement record that can be silently dropped
/// would make §47.3's "an enablement record exists" unverifiable.
#[test]
fn an_allowed_promotion_into_supervised_automatic_reaches_the_log_as_one_approval_entry() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());
    let before = row_count(&composition);

    let ack = expect_logged(
        composition
            .record_promotion(
                &Attempt::new(RampStage::Assisted, RampStage::AutoSupervised).request(),
            )
            .unwrap(),
    );
    assert!(
        ack.persisted,
        "a critical entry must persist, not be buffered"
    );
    assert!(ack.entry_hash.is_some(), "a persisted row carries its hash");
    assert!(!ack.dropped_oldest, "nothing was dropped");

    let rows = rows_of_kind(&composition, "approval");
    assert_eq!(rows.len(), 1, "exactly one approval entry");
    assert_eq!(
        row_count(&composition),
        before + 1,
        "an allowed promotion appends exactly one row"
    );
    assert!(
        rows[0].payload_json.contains(MECHANISM),
        "the payload carries the mechanism: {}",
        rows[0].payload_json
    );
    assert!(
        rows[0].payload_json.contains("auto_supervised"),
        "the payload carries the target stage: {}",
        rows[0].payload_json
    );
    verify_chain(&composition);
    assert_no_credential_in_rows(&composition);
}

/// The payload is the material the judge let through and nothing else: exactly four keys, and
/// **not** the declared/passed regression sets the request carried to reach its decision.
#[test]
fn the_appended_payload_is_the_judged_record_and_not_the_requests_regression_classes() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    expect_logged(
        composition
            .record_promotion(
                &Attempt::new(RampStage::Assisted, RampStage::AutoSupervised).request(),
            )
            .unwrap(),
    );

    let rows = rows_of_kind(&composition, "approval");
    assert_eq!(rows.len(), 1);
    let value: serde_json::Value = serde_json::from_str(&rows[0].payload_json).unwrap();
    let payload = value.as_object().expect("the payload is a JSON object");
    let mut keys: Vec<&str> = payload.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["fields", "from", "mechanism", "to"],
        "the payload MUST be exactly the judged material; a runtime that adds the request's \
         regression classes has stopped being a transport: {}",
        rows[0].payload_json
    );
    assert_eq!(payload["mechanism"], serde_json::json!(MECHANISM));
    assert_eq!(payload["from"], serde_json::json!("assisted"));
    assert_eq!(payload["to"], serde_json::json!("auto_supervised"));
    assert_eq!(
        payload["fields"],
        serde_json::json!({ "window": "2026-10" }),
        "the record's own fields MUST be logged verbatim"
    );
    // The categories the judge consumed to decide MUST NOT reach the log.
    assert!(
        !rows[0].payload_json.contains("regression."),
        "a regression-category label leaked into the payload: {}",
        rows[0].payload_json
    );
}

/// A promotion into a stage **before** the gate is allowed, needs no record, and — crucially — is
/// neither "logged" nor "refused". Collapsing it into either arm makes the runtime lie.
#[test]
fn a_promotion_below_the_gated_stage_needs_no_record_and_is_not_a_refusal() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());
    let before = row_count(&composition);

    let mut attempt = Attempt::new(RampStage::Rehearsal, RampStage::Assisted);
    // Below the gate there is no enablement requirement at all, so offer none.
    attempt.record = None;

    match composition.record_promotion(&attempt.request()).unwrap() {
        RampOutcome::NoRecordRequired { from, to } => {
            assert_eq!(from, RampStage::Rehearsal);
            assert_eq!(to, RampStage::Assisted);
        }
        other => panic!("a promotion below the gated stage must be its own outcome, not {other:?}"),
    }
    assert_eq!(
        row_count(&composition),
        before,
        "nothing may be appended when no record is required"
    );
}

/// The three outcomes are mutually decidable: the same entry point yields all three, and each is
/// distinguishable from the other two.
#[test]
fn the_three_outcomes_are_distinguishable() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let mut below_the_gate = Attempt::new(RampStage::Rehearsal, RampStage::Shadow);
    below_the_gate.record = None;
    let below = composition
        .record_promotion(&below_the_gate.request())
        .unwrap();

    let mut refused_attempt = Attempt::new(RampStage::Assisted, RampStage::AutoSupervised);
    refused_attempt.passed = Vec::new();
    let refused = composition
        .record_promotion(&refused_attempt.request())
        .unwrap();

    let logged = composition
        .record_promotion(&Attempt::new(RampStage::Assisted, RampStage::AutoSupervised).request())
        .unwrap();

    assert!(matches!(below, RampOutcome::NoRecordRequired { .. }));
    assert!(matches!(refused, RampOutcome::Refused(_)));
    assert!(matches!(logged, RampOutcome::Logged(_)));
    assert_ne!(below, refused, "「放行但无需记录」与「已拒绝」MUST 可区分");
    assert_ne!(refused, logged, "「已拒绝」与「已落记录」MUST 可区分");
    assert_ne!(below, logged, "「放行但无需记录」与「已落记录」MUST 可区分");
}

/// §47.3 requires **every** missing category to be named, not just the first one.
#[test]
fn every_missing_regression_category_is_named_not_just_the_first() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let mut attempt = Attempt::new(RampStage::Assisted, RampStage::AutoSupervised);
    attempt.touched = vec![
        RegressionCategory::AssemblyRegression,
        RegressionCategory::IntentRefusal,
        RegressionCategory::ToolFailureRecovery,
    ];
    attempt.passed = Vec::new();

    let refusal = expect_refusal(composition.record_promotion(&attempt.request()).unwrap());
    assert!(
        refusal
            .rejections
            .contains(&RampRejection::RegressionMissing.label()),
        "the refusal names the missing-regression rule: {:?}",
        refusal.rejections
    );
    let mut missing = refusal.missing_categories.clone();
    missing.sort_unstable();
    assert_eq!(
        missing,
        [
            RegressionCategory::AssemblyRegression.label(),
            RegressionCategory::IntentRefusal.label(),
            RegressionCategory::ToolFailureRecovery.label(),
        ],
        "all three missing categories MUST be listed"
    );
    assert_eq!(row_count(&composition), 0, "a refusal appends nothing");
}

/// INV-11 is judged **by value, never by field name**: a field whose name merely looks like a
/// secret, with a clean value, is logged.
#[test]
fn a_field_named_like_a_secret_with_a_clean_value_is_still_logged() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let mut attempt = Attempt::new(RampStage::Assisted, RampStage::AutoSupervised);
    attempt.record = Some(record(
        MECHANISM,
        RampStage::AutoSupervised,
        &[
            ("api_key_name", "rotation-window"),
            ("token_label", "primary"),
        ],
    ));

    expect_logged(composition.record_promotion(&attempt.request()).unwrap());
    let rows = rows_of_kind(&composition, "approval");
    assert_eq!(rows.len(), 1);
    assert!(
        rows[0].payload_json.contains("rotation-window"),
        "a clean value under a secret-looking name MUST be logged: {}",
        rows[0].payload_json
    );
    assert_no_credential_in_rows(&composition);
}

/// A credential literal in a record value is refused, and it never reaches the chain.
///
/// The probe value is built from the kernel's own marker table so it cannot drift out of sync with
/// the rule it is meant to trip.
#[test]
fn a_credential_literal_in_the_record_is_refused_and_never_reaches_the_chain() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());
    let before = row_count(&composition);

    let leaky = format!("{}abcd1234", CREDENTIAL_MARKERS[0]);
    let mut attempt = Attempt::new(RampStage::Assisted, RampStage::AutoSupervised);
    attempt.record = Some(record(
        MECHANISM,
        RampStage::AutoSupervised,
        &[("detail", leaky.as_str())],
    ));

    let refusal = expect_refusal(composition.record_promotion(&attempt.request()).unwrap());
    assert!(
        refusal
            .rejections
            .contains(&RampRejection::RecordContainsCredential.label()),
        "the refusal names the credential rule: {:?}",
        refusal.rejections
    );
    assert_eq!(
        refusal.leaked_fields,
        vec!["detail".to_string()],
        "the offending field name is reported (and only the name)"
    );
    assert_eq!(
        row_count(&composition),
        before,
        "a refusal MUST append nothing"
    );
    assert_no_credential_in_rows(&composition);
}

/// A refusal leaves the chain untouched **even when the chain is not empty** — otherwise
/// "unchanged" would be indistinguishable from "nothing was ever there".
#[test]
fn a_refused_promotion_appends_nothing_to_a_non_empty_chain() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    // Seed one real row so the count assertion is not vacuous.
    expect_logged(
        composition
            .record_promotion(
                &Attempt::new(RampStage::Assisted, RampStage::AutoSupervised).request(),
            )
            .unwrap(),
    );
    let before = row_count(&composition);
    assert!(before > 0);

    let mut attempt = Attempt::new(RampStage::Assisted, RampStage::AutoSupervised);
    attempt.passed_at_revision = Some(REVISION + 1); // stale: a later harness revision voids it
    let refusal = expect_refusal(composition.record_promotion(&attempt.request()).unwrap());
    assert!(
        refusal
            .rejections
            .contains(&RampRejection::RegressionStale.label()),
        "the refusal names the stale rule: {:?}",
        refusal.rejections
    );
    assert_eq!(row_count(&composition), before, "the chain is untouched");
    verify_chain(&composition);
}

/// Every rejection the kernel can produce is reachable through the runtime, **and** leaves the
/// chain untouched.
///
/// The sample constructor below matches `RampRejection` exhaustively, so a new kernel variant
/// breaks this build instead of passing silently; iterating `ALL` covers the other direction
/// (a variant listed but never sampled).
#[test]
fn every_ramp_rejection_is_reachable_through_the_runtime() {
    for rejection in RampRejection::ALL {
        let attempt = attempt_refused_by(rejection);
        let dir = tempdir().unwrap();
        let mut composition = prepared(dir.path());

        let outcome = composition.record_promotion(&attempt.request()).unwrap();
        match &outcome {
            RampOutcome::Refused(refusal) => assert!(
                refusal.rejections.contains(&rejection.label()),
                "{} was not named by the refusal: {:?}",
                rejection.label(),
                refusal.rejections
            ),
            other => panic!(
                "a promotion the judge refuses must come back as Refused, not {other:?} \
                 (want {})",
                rejection.label()
            ),
        }
        assert!(
            all_rows(&composition).is_empty(),
            "a refusal MUST leave the chain untouched ({})",
            rejection.label()
        );
    }
}

/// One runtime scenario per rejection. The `match` is exhaustive on purpose: adding a variant to
/// `RampRejection` without adding a scenario here is a compile error.
fn attempt_refused_by(rejection: RampRejection) -> Attempt {
    let mut attempt = Attempt::new(RampStage::Assisted, RampStage::AutoSupervised);
    match rejection {
        RampRejection::NotAnAdvance => {
            // Sideways: the mechanism already sits where it is asking to go.
            attempt.current = RampStage::AutoSupervised;
        }
        RampRejection::RegressionScopeUndeclared => {
            attempt.touched = Vec::new();
            attempt.passed = Vec::new();
        }
        RampRejection::RegressionMissing => {
            attempt.passed = Vec::new();
        }
        RampRejection::RegressionStale => {
            attempt.passed_at_revision = Some(REVISION + 1);
        }
        RampRejection::EnablementRecordMissing => {
            attempt.record = None;
        }
        RampRejection::RecordMechanismMismatch => {
            attempt.record = Some(record(OTHER_MECHANISM, RampStage::AutoSupervised, &[]));
        }
        RampRejection::RecordStageMismatch => {
            attempt.record = Some(record(MECHANISM, RampStage::Assisted, &[]));
        }
        RampRejection::RecordContainsCredential => {
            let leaky = format!("{}sample", CREDENTIAL_MARKERS[0]);
            attempt.record = Some(record(
                MECHANISM,
                RampStage::AutoSupervised,
                &[("detail", leaky.as_str())],
            ));
        }
    }
    attempt
}
