//! W103: the H15 runtime wiring. The judge already existed (W66); these tests are about the
//! step after it — that a judged-loggable observation really reaches the append-only hash chain
//! carrying the **judged** material, and that a refused one really does not reach it at all.
//!
//! The non-void guard is in `each_of_the_five_layers_reaches_the_log_as_a_judged_entry`: it
//! iterates `OBSERVATION_LAYERS` itself rather than a hand-written list of layer names, so
//! adding a kind to the enum without covering it fails here instead of passing invisibly.
mod common;
use common::*;
use nai_kernel::evaluate::harness::{
    CREDENTIAL_MARKERS, OBSERVATION_LAYERS, ObservationLayer, ObservationPolicy, ObservationRecord,
    ViewerText,
};
use nai_runtime::{Composition, ObservationOutcome};
use nai_storage::{DataPaths, DecisionEntryView};
use std::collections::BTreeMap;
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

/// A record that satisfies every field `layer` requires, with values that carry no credential
/// marker and no viewer text. The judge must find it loggable.
fn loggable_record(layer: ObservationLayer) -> ObservationRecord {
    let mut fields = BTreeMap::new();
    for field in layer.required_fields() {
        fields.insert((*field).to_string(), format!("{field}-value"));
    }
    ObservationRecord {
        layer,
        subject: format!("subject-of-{}", layer.label()),
        fields,
        viewer_text: None,
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

fn observation_rows(composition: &Composition) -> Vec<DecisionEntryView> {
    all_rows(composition)
        .into_iter()
        .filter(|row| row.kind == "observation")
        .collect()
}

/// A loggable offer MUST come back as `Logged`; any other arm is the failure under test.
fn expect_logged(outcome: ObservationOutcome) -> nai_storage::DecisionAck {
    match outcome {
        ObservationOutcome::Logged(ack) => ack,
        ObservationOutcome::Refused(refusal) => panic!(
            "a loggable record was refused: layer={} rejections={:?}",
            refusal.layer, refusal.rejections
        ),
    }
}

/// A refused offer MUST come back as `Refused`; a `Logged` here is exactly the silent-degradation
/// failure the requirement forbids.
fn expect_refusal(outcome: ObservationOutcome) -> nai_runtime::ObservationRefusal {
    match outcome {
        ObservationOutcome::Refused(refusal) => refusal,
        ObservationOutcome::Logged(ack) => panic!(
            "a record that must not be logged came back as Logged (persisted={}, hash={:?})",
            ack.persisted, ack.entry_hash
        ),
    }
}

/// One entry per observation kind, each carrying that kind's label, and the chain still verifies.
/// The loop is over `OBSERVATION_LAYERS`, so a new kind cannot be left uncovered.
#[test]
fn each_of_the_five_layers_reaches_the_log_as_a_judged_entry() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());
    let policy = ObservationPolicy::default();

    for layer in OBSERVATION_LAYERS {
        let ack = expect_logged(
            composition
                .record_observation(&loggable_record(layer), &policy)
                .unwrap(),
        );
        assert!(
            ack.persisted,
            "a judged-loggable observation of {} must persist, not sit in a buffer",
            layer.label()
        );
        assert!(ack.entry_hash.is_some(), "a persisted row carries its hash");
    }

    let rows = observation_rows(&composition);
    assert_eq!(
        rows.len(),
        OBSERVATION_LAYERS.len(),
        "exactly one entry per observation kind"
    );
    for layer in OBSERVATION_LAYERS {
        let hits = rows
            .iter()
            .filter(|row| row.payload_json.contains(layer.label()))
            .count();
        assert_eq!(
            hits,
            1,
            "kind {} is not carried exactly once",
            layer.label()
        );
    }
    verify_chain(&composition);
}

/// The payload is what the judge emitted, not what the caller passed in. An over-long field is
/// not a rejection — the judge records it as a truncation marker — so this record is loggable
/// and the difference between the two implementations is visible in the stored bytes.
#[test]
fn the_appended_payload_is_the_judged_material_not_the_raw_record() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());
    let policy = ObservationPolicy::default();

    let mut record = loggable_record(ObservationLayer::Memory);
    let oversized = "x".repeat(policy.max_field_chars + 1);
    record.fields.insert("scope".to_string(), oversized.clone());

    let ack = expect_logged(composition.record_observation(&record, &policy).unwrap());
    assert!(
        ack.persisted,
        "an over-long field is truncated, not refused"
    );

    let rows = observation_rows(&composition);
    assert_eq!(rows.len(), 1);
    assert!(
        rows[0].payload_json.contains("[truncated]"),
        "the payload carries the judge's truncation marker"
    );
    assert!(
        !rows[0].payload_json.contains(&oversized),
        "the raw over-long value must never be appended; taking `record.fields` is not correct"
    );
    verify_chain(&composition);
}

/// The runtime is a transport, not an enricher: it must add no field the judge did not vet.
/// `subject` is one such field — the judge never scans it — so a credential parked there must
/// not arrive in the log. An implementation that folds `record.subject` into the payload leaks
/// here while every other test still passes.
#[test]
fn the_runtime_does_not_enrich_the_payload_with_unvetted_fields() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());
    let policy = ObservationPolicy::default();

    let literal = format!("{}subject-leak-99", CREDENTIAL_MARKERS[0]);
    let mut record = loggable_record(ObservationLayer::Cache);
    record.subject = literal.clone();

    let ack = expect_logged(composition.record_observation(&record, &policy).unwrap());
    assert!(
        ack.persisted,
        "the judge only scans fields, so this record is loggable"
    );

    let rows = all_rows(&composition);
    assert!(
        !rows.iter().any(|row| row.payload_json.contains(&literal)),
        "the runtime must not append a field the judge did not vet"
    );
    verify_chain(&composition);
}

/// A credential literal in a **field value** is refused by name, and the refusal appends nothing.
#[test]
fn a_plaintext_credential_is_refused_by_name_and_never_reaches_the_log() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());
    let policy = ObservationPolicy::default();

    let literal = format!("{}live-value-1234", CREDENTIAL_MARKERS[0]);
    let mut record = loggable_record(ObservationLayer::Tool);
    record
        .fields
        .insert("result_summary".to_string(), literal.clone());

    let before = all_rows(&composition).len();
    let refusal = expect_refusal(composition.record_observation(&record, &policy).unwrap());
    assert_eq!(refusal.layer, ObservationLayer::Tool.label());
    assert!(
        refusal
            .rejections
            .contains(&"observation.plaintext_credential"),
        "the refusal names the reason: {:?}",
        refusal.rejections
    );

    let after = all_rows(&composition);
    assert_eq!(after.len(), before, "a refusal appends nothing");
    assert!(
        !after.iter().any(|row| row.payload_json.contains(&literal)),
        "the credential literal must not appear in any persisted row"
    );
    verify_chain(&composition);
}

/// A required field that is simply absent is refused by name, and append nothing.
#[test]
fn a_missing_required_field_is_refused_by_name_and_appends_nothing() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());
    let policy = ObservationPolicy::default();

    let mut record = loggable_record(ObservationLayer::Intent);
    record.fields.remove("confidence");

    let before = all_rows(&composition).len();
    let refusal = expect_refusal(composition.record_observation(&record, &policy).unwrap());
    assert!(
        refusal
            .rejections
            .contains(&"observation.missing_required_field"),
        "the refusal names the reason: {:?}",
        refusal.rejections
    );
    assert_eq!(
        all_rows(&composition).len(),
        before,
        "a refusal appends nothing"
    );
    verify_chain(&composition);
}

/// Viewer text carried unmasked is refused by name, and appends nothing.
#[test]
fn unmasked_viewer_text_is_refused_by_name_and_appends_nothing() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());
    let policy = ObservationPolicy::default();

    let mut record = loggable_record(ObservationLayer::Tool);
    record.viewer_text = Some(ViewerText {
        text: "观众原话".to_string(),
        masked: false,
        age_ms: 1_000,
    });

    let before = all_rows(&composition).len();
    let refusal = expect_refusal(composition.record_observation(&record, &policy).unwrap());
    assert!(
        refusal
            .rejections
            .contains(&"observation.viewer_text_unmasked"),
        "the refusal names the reason: {:?}",
        refusal.rejections
    );
    assert_eq!(
        all_rows(&composition).len(),
        before,
        "a refusal appends nothing"
    );
    verify_chain(&composition);
}

/// The positive half of the credential rule: sensitivity is decided by the **value**, never by
/// the field name. A field called `key` with a clean value must pass — an implementation that
/// rejects on the field-name blacklist fails here.
#[test]
fn a_field_named_key_with_a_clean_value_is_still_loggable() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());
    let policy = ObservationPolicy::default();

    let record = loggable_record(ObservationLayer::Memory);
    assert!(
        record.fields.contains_key("key"),
        "the fixture must exercise the field named `key`"
    );

    let ack = expect_logged(composition.record_observation(&record, &policy).unwrap());
    assert!(ack.persisted);

    let rows = observation_rows(&composition);
    assert_eq!(rows.len(), 1);
    assert!(
        rows[0].payload_json.contains("key-value"),
        "a clean value under a sensitive-looking name is logged, not masked or refused"
    );
    verify_chain(&composition);
}

/// A refusal must be distinguishable from a successful append: the two outcomes are different
/// values of the same enum, and a caller cannot read one as the other.
#[test]
fn a_refusal_is_distinguishable_from_an_append() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());
    let policy = ObservationPolicy::default();

    let logged = composition
        .record_observation(&loggable_record(ObservationLayer::Cache), &policy)
        .unwrap();
    let mut broken = loggable_record(ObservationLayer::Cache);
    broken.fields.insert("hit".to_string(), String::new());
    let refused = composition.record_observation(&broken, &policy).unwrap();

    assert!(
        matches!(&logged, ObservationOutcome::Logged(_)),
        "a loggable record reports an append"
    );
    assert!(
        matches!(&refused, ObservationOutcome::Refused(_)),
        "a record that fails judgement reports a refusal, not an append"
    );
    assert_ne!(
        std::mem::discriminant(&logged),
        std::mem::discriminant(&refused),
        "the two outcomes MUST NOT collapse into one value"
    );
    verify_chain(&composition);
}
