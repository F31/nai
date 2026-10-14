//! W104: the H02 runtime wiring. The judge already existed (W69); these tests are about the step
//! after it — that a judged-rebuildable assembly really reaches the append-only hash chain with
//! its **fragment bodies**, that a non-rebuildable one does not reach it at all, and that the
//! assembly can be rebuilt **from the chain alone**.
//!
//! The non-void guard is in `every_rebuild_rejection_is_reachable_through_the_runtime`: it
//! iterates `RebuildRejection::ALL` and, for each variant, builds a runtime scenario whose refusal
//! must name it. The per-variant sample constructor matches the enum exhaustively, so adding a
//! variant to the kernel without covering it here fails to compile rather than passing invisibly.
mod common;
use common::*;
use nai_kernel::evaluate::assembly::{AssemblyRecord, LoggedFragment, RebuildRejection};
use nai_kernel::evaluate::harness::CREDENTIAL_MARKERS;
use nai_runtime::{AssemblyOutcome, Composition};
use nai_storage::{DataPaths, DecisionAck, DecisionEntryView};
use tempfile::tempdir;

const MODEL: &str = "model-1";
const PROMPT: &str = "prompt-1";
const SOURCE_REF: &str = "evidence-kb0";
const BODY: &str = "the fragment body that MUST be logged, not a pointer";

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

/// The record a producer would log: the referenced ids **in entry order** plus the two versions.
fn record<'a>(ids: &'a [&'a str]) -> AssemblyRecord<'a> {
    AssemblyRecord {
        fragment_ids: ids,
        model_version: MODEL,
        prompt_version: PROMPT,
    }
}

/// The log material for `ids`, each entry carrying a real body.
fn fragments<'a>(ids: &[&'a str]) -> Vec<LoggedFragment<'a>> {
    ids.iter()
        .copied()
        .map(|id| LoggedFragment {
            id,
            source_ref: SOURCE_REF,
            content: Some(BODY),
        })
        .collect()
}

/// Every persisted decision row, oldest first.
fn all_rows(composition: &Composition) -> Vec<DecisionEntryView> {
    let operation = store_operation();
    let store = composition.store().lock().unwrap();
    store
        .decision_entries(&operation, composition.workspace(), 500)
        .unwrap()
}

fn assembly_rows(composition: &Composition) -> Vec<DecisionEntryView> {
    all_rows(composition)
        .into_iter()
        .filter(|row| row.kind == "assembly")
        .collect()
}

/// A rebuildable offer MUST come back as `Logged`; any other arm is the failure under test.
fn expect_logged(outcome: AssemblyOutcome) -> DecisionAck {
    match outcome {
        AssemblyOutcome::Logged(ack) => ack,
        AssemblyOutcome::Refused(refusal) => panic!(
            "a rebuildable assembly was refused: rejections={:?}",
            refusal.rejections
        ),
    }
}

/// A refused offer MUST come back as `Refused`; a `Logged` here is exactly the silent-degradation
/// failure the requirement forbids.
fn expect_refusal(outcome: AssemblyOutcome) -> nai_runtime::AssemblyRefusal {
    match outcome {
        AssemblyOutcome::Refused(refusal) => refusal,
        AssemblyOutcome::Logged(ack) => panic!(
            "an assembly that must not be logged came back as Logged (persisted={}, hash={:?})",
            ack.persisted, ack.entry_hash
        ),
    }
}

/// A rebuildable assembly is appended as one `assembly` entry whose payload carries every
/// referenced fragment's id and **body**, and the chain still verifies.
#[test]
fn a_rebuildable_assembly_reaches_the_log_with_its_fragment_bodies() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let ids = ["frag-a", "frag-b"];
    let ack = expect_logged(
        composition
            .record_assembly(&record(&ids), &fragments(&ids))
            .unwrap(),
    );
    assert!(ack.persisted, "a rebuildable assembly must persist");
    assert!(ack.entry_hash.is_some(), "a persisted row carries its hash");

    let rows = assembly_rows(&composition);
    assert_eq!(rows.len(), 1, "exactly one assembly entry");
    for id in ids {
        assert!(
            rows[0].payload_json.contains(id),
            "the payload must carry the fragment id {id}"
        );
    }
    assert!(
        rows[0].payload_json.contains(BODY),
        "the payload must carry the fragment body, not a pointer into live memory"
    );
    assert!(
        rows[0].payload_json.contains(MODEL),
        "the payload must carry the model version (§41.5)"
    );
    assert!(
        rows[0].payload_json.contains(PROMPT),
        "the payload must carry the prompt version (§41.5)"
    );
    verify_chain(&composition);
}

/// The stored sequence is the record's **entry** order, not a re-sorting of it. The fixture above
/// uses ascending ids, where a sorted implementation and a faithful one are indistinguishable;
/// this one does not, so an implementation that sorts the fragments before persisting is caught
/// here — and would otherwise surface later as a spurious `rebuild.order_differs`.
#[test]
fn the_log_preserves_the_entry_order_rather_than_re_sorting_it() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let entered = ["frag-c", "frag-a"];
    expect_logged(
        composition
            .record_assembly(&record(&entered), &fragments(&entered))
            .unwrap(),
    );

    let rows = assembly_rows(&composition);
    assert_eq!(rows.len(), 1);
    let first_c = rows[0].payload_json.find("frag-c").expect("id logged");
    let first_a = rows[0].payload_json.find("frag-a").expect("id logged");
    assert!(
        first_c < first_a,
        "the payload stores the entry order; a sorted payload is not the record's assembly"
    );

    let rebuilt = composition
        .rebuild_assembly_from_log(&record(&entered), 500)
        .unwrap();
    assert!(
        rebuilt.rebuildable,
        "the entry order round-trips: {:?}",
        rebuilt.rejections
    );
    assert_eq!(
        rebuilt.rebuilt,
        vec![
            ("frag-c".to_string(), SOURCE_REF.to_string()),
            ("frag-a".to_string(), SOURCE_REF.to_string()),
        ],
        "and it round-trips in the entry order, not an ascending one"
    );
    verify_chain(&composition);
}

/// The chain alone reproduces the logged assembly: rebuild reads back the persisted bodies and
/// returns the same `(id, source_ref)` sequence, in the log's own order.
#[test]
fn the_chain_alone_reproduces_the_logged_assembly() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let ids = ["frag-a", "frag-b", "frag-c"];
    expect_logged(
        composition
            .record_assembly(&record(&ids), &fragments(&ids))
            .unwrap(),
    );

    let rebuilt = composition
        .rebuild_assembly_from_log(&record(&ids), 500)
        .unwrap();
    assert!(
        rebuilt.rebuildable,
        "the log alone must reproduce the assembly: {:?}",
        rebuilt.rejections
    );
    assert!(
        rebuilt.rejections.is_empty(),
        "a rebuildable verdict carries no rejection"
    );
    assert_eq!(
        rebuilt.rebuilt,
        vec![
            ("frag-a".to_string(), SOURCE_REF.to_string()),
            ("frag-b".to_string(), SOURCE_REF.to_string()),
            ("frag-c".to_string(), SOURCE_REF.to_string()),
        ],
        "the rebuilt sequence is the log's own order"
    );
    verify_chain(&composition);
}

/// The log's own order decides: a record whose entry order differs from what the log carries is
/// refused by name, and the rebuilt sequence is the log's, not the record's.
#[test]
fn the_logs_own_order_decides_and_a_difference_is_named() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let entered = ["frag-a", "frag-b"];
    expect_logged(
        composition
            .record_assembly(&record(&entered), &fragments(&entered))
            .unwrap(),
    );

    let claimed = ["frag-b", "frag-a"];
    let rebuilt = composition
        .rebuild_assembly_from_log(&record(&claimed), 500)
        .unwrap();
    assert!(!rebuilt.rebuildable, "a differing order is not rebuildable");
    assert!(
        rebuilt
            .rejections
            .contains(&RebuildRejection::RebuildOrderDiffers.label()),
        "the rebuild names the reason: {:?}",
        rebuilt.rejections
    );
    assert_eq!(
        rebuilt.rebuilt,
        vec![
            ("frag-a".to_string(), SOURCE_REF.to_string()),
            ("frag-b".to_string(), SOURCE_REF.to_string()),
        ],
        "the sequence follows the log, not the record — sorting by the record would make the check true by construction"
    );

    // The write path refuses the same shape: an offered order that differs from the record's.
    let flipped = fragments(&["frag-b", "frag-a"]);
    let before = assembly_rows(&composition).len();
    let refusal = expect_refusal(
        composition
            .record_assembly(&record(&entered), &flipped)
            .unwrap(),
    );
    assert!(
        refusal
            .rejections
            .contains(&RebuildRejection::RebuildOrderDiffers.label()),
        "the refusal names the reason: {:?}",
        refusal.rejections
    );
    assert_eq!(
        assembly_rows(&composition).len(),
        before,
        "a refusal appends nothing"
    );
    verify_chain(&composition);
}

/// H02's crux: a fragment whose body only ever lived in working memory is refused by name and is
/// **never** persisted as a pointer. This is the case `content: None` models.
#[test]
fn a_fragment_body_that_only_lives_in_memory_is_refused_not_logged_as_a_pointer() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let ids = ["frag-a", "frag-b"];
    let mut material = fragments(&ids);
    material[1].content = None;

    let before = assembly_rows(&composition).len();
    let refusal = expect_refusal(
        composition
            .record_assembly(&record(&ids), &material)
            .unwrap(),
    );
    assert!(
        refusal
            .rejections
            .contains(&RebuildRejection::FragmentContentNotInLog.label()),
        "the refusal names the reason: {:?}",
        refusal.rejections
    );
    assert_eq!(
        refusal.contentless_ids,
        vec!["frag-b".to_string()],
        "the refusal names the body-less id"
    );
    assert_eq!(
        assembly_rows(&composition).len(),
        before,
        "a refusal appends nothing"
    );
    verify_chain(&composition);
}

/// A referenced fragment with no log entry at all is refused by name, distinctly from the
/// body-less case, and appends nothing.
#[test]
fn a_referenced_fragment_absent_from_the_log_is_refused_by_name() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let ids = ["frag-a", "frag-b"];
    let before = assembly_rows(&composition).len();
    let refusal = expect_refusal(
        composition
            .record_assembly(&record(&ids), &fragments(&["frag-a"]))
            .unwrap(),
    );
    assert!(
        refusal
            .rejections
            .contains(&RebuildRejection::FragmentNotLogged.label()),
        "the refusal names the reason: {:?}",
        refusal.rejections
    );
    assert_eq!(refusal.unlogged_ids, vec!["frag-b".to_string()]);
    assert_eq!(
        assembly_rows(&composition).len(),
        before,
        "a refusal appends nothing"
    );
    verify_chain(&composition);
}

/// Rebuilding from an empty log reports every referenced id as unlogged — proof that the rebuild
/// reads the **chain**, not a slice the caller could have supplied.
#[test]
fn rebuilding_from_an_empty_log_reports_every_id_unlogged() {
    let dir = tempdir().unwrap();
    let composition = prepared(dir.path());

    let ids = ["frag-a", "frag-b"];
    let rebuilt = composition
        .rebuild_assembly_from_log(&record(&ids), 500)
        .unwrap();
    assert!(
        !rebuilt.rebuildable,
        "an empty log cannot rebuild a non-empty assembly"
    );
    assert!(
        rebuilt
            .rejections
            .contains(&RebuildRejection::FragmentNotLogged.label()),
        "the rebuild names the reason: {:?}",
        rebuilt.rejections
    );
    assert_eq!(
        rebuilt.unlogged_ids,
        vec!["frag-a".to_string(), "frag-b".to_string()],
        "every referenced id is reported unlogged"
    );
    assert!(rebuilt.rebuilt.is_empty());
    verify_chain(&composition);
}

/// A record with neither version is refused by both names, and appends nothing.
#[test]
fn a_missing_model_or_prompt_version_is_refused_by_name() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let ids = ["frag-a"];
    let before = assembly_rows(&composition).len();
    let mut unversioned = record(&ids);
    unversioned.model_version = "";
    unversioned.prompt_version = "  ";
    let refusal = expect_refusal(
        composition
            .record_assembly(&unversioned, &fragments(&ids))
            .unwrap(),
    );
    assert!(
        refusal
            .rejections
            .contains(&RebuildRejection::ModelVersionMissing.label()),
        "the refusal names the missing model version: {:?}",
        refusal.rejections
    );
    assert!(
        refusal
            .rejections
            .contains(&RebuildRejection::PromptVersionMissing.label()),
        "the refusal names the missing prompt version: {:?}",
        refusal.rejections
    );
    assert_eq!(
        assembly_rows(&composition).len(),
        before,
        "a refusal appends nothing"
    );
    verify_chain(&composition);
}

/// A credential literal in a fragment **body** is refused by name, and the literal never reaches
/// any persisted row.
#[test]
fn a_credential_in_a_fragment_body_is_refused_and_never_reaches_the_log() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let literal = format!("{}live-body-99", CREDENTIAL_MARKERS[0]);
    let ids = ["frag-a"];
    let material = vec![LoggedFragment {
        id: "frag-a",
        source_ref: SOURCE_REF,
        content: Some(literal.as_str()),
    }];

    let before = assembly_rows(&composition).len();
    let refusal = expect_refusal(
        composition
            .record_assembly(&record(&ids), &material)
            .unwrap(),
    );
    assert!(
        refusal
            .rejections
            .contains(&RebuildRejection::PlaintextCredential.label()),
        "the refusal names the reason: {:?}",
        refusal.rejections
    );
    assert_eq!(refusal.credential_ids, vec!["frag-a".to_string()]);

    let after = all_rows(&composition);
    assert_eq!(
        assembly_rows(&composition).len(),
        before,
        "a refusal appends nothing"
    );
    assert!(
        !after.iter().any(|row| row.payload_json.contains(&literal)),
        "the credential literal must not appear in any persisted row"
    );
    verify_chain(&composition);
}

/// An over-large payload is a classified error, not a silent truncation.
#[test]
fn an_oversized_assembly_payload_is_an_error_not_a_silent_truncation() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let huge = "x".repeat(70_000);
    let ids = ["frag-a"];
    let material = vec![LoggedFragment {
        id: "frag-a",
        source_ref: SOURCE_REF,
        content: Some(huge.as_str()),
    }];
    let result = composition.record_assembly(&record(&ids), &material);
    assert!(
        result.is_err(),
        "a payload past the storage bound must fail loudly"
    );
    assert!(
        assembly_rows(&composition).is_empty(),
        "a rejected write appends nothing"
    );
    verify_chain(&composition);
}

/// A refusal must be distinguishable from a successful append: the two outcomes are different
/// values of the same enum, and a caller cannot read one as the other.
#[test]
fn a_refusal_is_distinguishable_from_an_append() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let ids = ["frag-a"];
    let logged = composition
        .record_assembly(&record(&ids), &fragments(&ids))
        .unwrap();
    let mut broken = fragments(&ids);
    broken[0].content = None;
    let refused = composition.record_assembly(&record(&ids), &broken).unwrap();

    assert!(
        matches!(&logged, AssemblyOutcome::Logged(_)),
        "a rebuildable assembly reports an append"
    );
    assert!(
        matches!(&refused, AssemblyOutcome::Refused(_)),
        "an assembly that fails judgement reports a refusal, not an append"
    );
    assert_ne!(
        std::mem::discriminant(&logged),
        std::mem::discriminant(&refused),
        "the two outcomes MUST NOT collapse into one value"
    );
    verify_chain(&composition);
}

/// The non-void guard: every reason the judge can refuse an assembly is reachable through the
/// runtime, and the runtime names it. The loop is over the kernel's `ALL`, and the per-variant
/// sample constructor matches the enum exhaustively — so a new variant cannot pass uncovered.
#[test]
fn every_rebuild_rejection_is_reachable_through_the_runtime() {
    for rejection in RebuildRejection::ALL {
        let observed = runtime_refusal_for(rejection);
        assert!(
            observed.contains(&rejection.label()),
            "rejection {} has no runtime case (observed {:?})",
            rejection.label(),
            observed
        );
    }
}

/// Build the runtime scenario that triggers `rejection` and return the labels the runtime named.
fn runtime_refusal_for(rejection: RebuildRejection) -> Vec<&'static str> {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());
    let outcome = match rejection {
        RebuildRejection::FragmentNotLogged => {
            let ids = ["frag-a", "frag-b"];
            composition
                .record_assembly(&record(&ids), &fragments(&["frag-a"]))
                .unwrap()
        }
        RebuildRejection::FragmentContentNotInLog => {
            let ids = ["frag-a"];
            let mut material = fragments(&ids);
            material[0].content = None;
            composition
                .record_assembly(&record(&ids), &material)
                .unwrap()
        }
        RebuildRejection::RebuildOrderDiffers => {
            let ids = ["frag-a", "frag-b"];
            composition
                .record_assembly(&record(&ids), &fragments(&["frag-b", "frag-a"]))
                .unwrap()
        }
        RebuildRejection::ModelVersionMissing => {
            let ids = ["frag-a"];
            let mut unversioned = record(&ids);
            unversioned.model_version = "";
            composition
                .record_assembly(&unversioned, &fragments(&ids))
                .unwrap()
        }
        RebuildRejection::PromptVersionMissing => {
            let ids = ["frag-a"];
            let mut unversioned = record(&ids);
            unversioned.prompt_version = "";
            composition
                .record_assembly(&unversioned, &fragments(&ids))
                .unwrap()
        }
        RebuildRejection::PlaintextCredential => {
            let literal = format!("{}in-the-body", CREDENTIAL_MARKERS[0]);
            let ids = ["frag-a"];
            let material = vec![LoggedFragment {
                id: "frag-a",
                source_ref: SOURCE_REF,
                content: Some(literal.as_str()),
            }];
            composition
                .record_assembly(&record(&ids), &material)
                .unwrap()
        }
    };
    match outcome {
        AssemblyOutcome::Refused(refusal) => refusal.rejections,
        AssemblyOutcome::Logged(_) => {
            panic!(
                "{}: the runtime must refuse this scenario",
                rejection.label()
            )
        }
    }
}
