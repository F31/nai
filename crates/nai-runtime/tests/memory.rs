//! W107 lands the H03 half of §42.2's runtime wiring: a memory candidate that passes the write
//! gate lands in the **real** `memory_records` table, a refused one lands nothing, and the read
//! back is a per-column fetch rather than the §42.3 read judgement.
//!
//! Before this round both halves existed and were joined by nothing: `admit_write` had no
//! production caller, and no production path touched `memory_records`. The guard below is the
//! non-void half of that claim — `refusal_sample` matches `MemoryRejection` **exhaustively**, so a
//! new write-face rejection in the kernel fails to compile here instead of passing invisibly.
mod common;
use common::*;
use nai_contracts::v1::{ErrorCode, MemoryRecord};
use nai_kernel::evaluate::memory::{
    ClaimKind, MemoryCandidate, MemoryPolicy, MemoryRejection, SEMANTIC_KIND, SESSION_KIND,
    VIEWER_KIND, WriteSource,
};
use nai_runtime::{Composition, MemoryWriteOutcome};
use nai_storage::{DataPaths, DecisionEntryView};
use tempfile::tempdir;

const SESSION: &str = "session-1";
const OTHER_SESSION: &str = "session-2";
const SOURCE_REF: &str = "evidence-kb0";
const TIMESTAMP: u64 = 1_700_000_000_000;
/// Far enough out that no test races the clock.
const EXPIRES: u64 = 4_000_000_000_000;
const BODY: &str = "the record body that MUST be persisted verbatim, not a pointer";

/// The eight rules that can refuse a **write**. Listed in `MemoryRejection::ALL` order so the
/// guard's collected sequence can be compared against it directly.
const WRITE_FACE: [MemoryRejection; 8] = [
    MemoryRejection::UntrustedSourceAsFact,
    MemoryRejection::UnplayedCandidateAsFact,
    MemoryRejection::WriteWithoutScope,
    MemoryRejection::WriteWithoutSourceRef,
    MemoryRejection::WriteWithoutTimestamp,
    MemoryRejection::FactWithoutPublishRef,
    MemoryRejection::FactOutsideSemanticMemory,
    MemoryRejection::ViewerMemoryDisabled,
];

fn prepared(root: &std::path::Path) -> Composition {
    provision(root, WORKSPACE, ACTOR, PROFILE, &runtime_profile(WORKSPACE));
    let composition = Composition::prepare(
        DataPaths::at(root).unwrap(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        false,
    )
    .unwrap();
    // `memory_records.session_id` has a real foreign key onto `sessions`, so the record's scope
    // must be a session that exists. That is a storage-layer fact, not a §42.2 rule.
    open_session(&composition, SESSION);
    composition
}

fn open_session(composition: &Composition, session: &str) {
    let operation = store_operation();
    composition
        .store()
        .lock()
        .unwrap()
        .create_session(&operation, WORKSPACE, ACTOR, session)
        .unwrap();
}

/// The nine stored columns, from the perspective of the record the gate judged. `viewer_scope` is
/// only filled for a viewer record, matching the table's nullable column.
fn record(record_id: &str, kind: &str) -> MemoryRecord {
    MemoryRecord {
        record_id: record_id.into(),
        workspace_id: WORKSPACE.into(),
        session_id: SESSION.into(),
        viewer_scoped_id: if kind == VIEWER_KIND {
            "viewer-scope-7"
        } else {
            ""
        }
        .into(),
        kind: kind.into(),
        purpose: "order_status".into(),
        content: BODY.into(),
        expires_at_utc_ms: EXPIRES,
        authorization_ref: "authorization-ref-1".into(),
    }
}

/// A candidate §42.2 lets through: session memory from **approved and played** content, carrying a
/// scope, a source reference and a timestamp.
fn admissible(record_id: &str, kind: &str) -> MemoryCandidate {
    MemoryCandidate::new(record(record_id, kind), WriteSource::ApprovedPlayed)
        .with_source_ref(SOURCE_REF)
        .written_at(TIMESTAMP)
}

/// A candidate that provokes `rejection` on the write face, or `None` when the variant is not a
/// write-face rule.
///
/// Matching `MemoryRejection` **exhaustively** is the point of this function: adding a variant to
/// the kernel without deciding where it is provoked here fails to compile, so the guard cannot
/// silently lose coverage.
fn refusal_sample(rejection: MemoryRejection) -> Option<MemoryCandidate> {
    Some(match rejection {
        MemoryRejection::WriteWithoutScope => {
            let mut candidate = admissible("memory-without-scope", SESSION_KIND);
            candidate.record.session_id = String::new();
            candidate
        }
        MemoryRejection::WriteWithoutSourceRef => MemoryCandidate::new(
            record("memory-without-source-ref", SESSION_KIND),
            WriteSource::ApprovedPlayed,
        )
        .written_at(TIMESTAMP),
        MemoryRejection::WriteWithoutTimestamp => MemoryCandidate::new(
            record("memory-without-timestamp", SESSION_KIND),
            WriteSource::ApprovedPlayed,
        )
        .with_source_ref(SOURCE_REF),
        MemoryRejection::ViewerMemoryDisabled => admissible("memory-viewer-disabled", VIEWER_KIND),
        MemoryRejection::UntrustedSourceAsFact => {
            let mut candidate = admissible("memory-untrusted-source", SESSION_KIND);
            candidate.source = WriteSource::ToolReturn;
            candidate
        }
        MemoryRejection::UnplayedCandidateAsFact => {
            let mut candidate = admissible("memory-unplayed-candidate", SESSION_KIND);
            candidate.source = WriteSource::ApprovedUnplayed;
            candidate
        }
        MemoryRejection::FactOutsideSemanticMemory => {
            let mut candidate = admissible("memory-fact-outside-semantic", SESSION_KIND);
            candidate.claim = ClaimKind::AuthoritativeFact;
            candidate.with_publish_ref("publish-ref-1")
        }
        MemoryRejection::FactWithoutPublishRef => {
            let mut candidate = admissible("memory-fact-without-ref", SEMANTIC_KIND);
            candidate.claim = ClaimKind::AuthoritativeFact;
            candidate
        }
        // The read face (§42.3), the forget chain (§42.4) and the operation face (§42.6) are not
        // provoked by a write: they are refused on their own faces.
        MemoryRejection::InvalidScope
        | MemoryRejection::ViewerReadWithoutAuthorization
        | MemoryRejection::ViewerReadWithoutPurpose
        | MemoryRejection::ApproximateSubstitutionRequested
        | MemoryRejection::UnknownOperation
        | MemoryRejection::OutOfScope
        | MemoryRejection::Expired
        | MemoryRejection::Revoked
        | MemoryRejection::PermissionDenied
        | MemoryRejection::ApproximateSubstitution
        | MemoryRejection::OverBudget
        | MemoryRejection::ReadSideNotBlocked
        | MemoryRejection::ErasedClaimWithoutCompletePurge
        | MemoryRejection::ErasedClaimWithThirdPartyRetention
        | MemoryRejection::ThirdPartyRetentionUndisclosed => return None,
    })
}

/// The rows actually in `memory_records` for one session, read through the storage accessor.
fn stored(composition: &Composition, session: &str) -> Vec<MemoryRecord> {
    let operation = store_operation();
    composition
        .store()
        .lock()
        .unwrap()
        .memory_records(&operation, WORKSPACE, ACTOR, session)
        .unwrap()
}

fn logged(composition: &Composition) -> Vec<DecisionEntryView> {
    let operation = store_operation();
    composition
        .store()
        .lock()
        .unwrap()
        .decision_entries(&operation, composition.workspace(), 500)
        .unwrap()
}

fn write(composition: &mut Composition, candidate: &MemoryCandidate) -> MemoryWriteOutcome {
    composition
        .write_memory(candidate, &MemoryPolicy::default())
        .unwrap()
}

// ---------------------------------------------------------------------------
// §42.2 写入面：放行则落库，拒绝则具名且零落库
// ---------------------------------------------------------------------------

#[test]
fn an_admitted_write_lands_exactly_one_row() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    let candidate = admissible("memory-admitted", SESSION_KIND);

    match write(&mut composition, &candidate) {
        MemoryWriteOutcome::Written(record) => assert_eq!(record, candidate.record),
        MemoryWriteOutcome::Refused(refusal) => panic!("admitted candidate refused: {refusal:?}"),
    }

    assert_eq!(stored(&composition, SESSION), vec![candidate.record]);
}

#[test]
fn a_refused_write_is_named_and_lands_nothing() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    let mut candidate = admissible("memory-refused", SESSION_KIND);
    candidate.source = WriteSource::ModelDraft;

    match write(&mut composition, &candidate) {
        MemoryWriteOutcome::Refused(refusal) => {
            assert_eq!(
                refusal.reason,
                MemoryRejection::UntrustedSourceAsFact.label()
            );
            assert_eq!(refusal.record_id, "memory-refused");
        }
        MemoryWriteOutcome::Written(_) => panic!("a model draft became a stored memory"),
    }

    assert!(
        stored(&composition, SESSION).is_empty(),
        "a refused write MUST leave the store empty"
    );
}

#[test]
fn every_write_rejection_is_reachable_through_the_runtime() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());

    let mut reached = Vec::new();
    for rejection in MemoryRejection::ALL {
        let Some(candidate) = refusal_sample(rejection) else {
            continue;
        };
        match write(&mut composition, &candidate) {
            MemoryWriteOutcome::Refused(refusal) => {
                assert_eq!(
                    refusal.reason,
                    rejection.label(),
                    "{rejection:?} was refused under a different name"
                );
                assert_eq!(refusal.record_id, candidate.record.record_id);
                reached.push(rejection);
            }
            MemoryWriteOutcome::Written(_) => panic!("{rejection:?} was admitted"),
        }
    }

    assert_eq!(
        reached, WRITE_FACE,
        "the write face MUST be exactly these eight rules, and each MUST be reachable"
    );
    assert!(
        stored(&composition, SESSION).is_empty(),
        "not one of these refusals may land a row"
    );
}

#[test]
fn the_viewer_switch_closes_the_write_face_too() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    let candidate = admissible("memory-viewer", VIEWER_KIND);

    // The default policy has viewer long-term memory **off**.
    let closed = composition
        .write_memory(&candidate, &MemoryPolicy::default())
        .unwrap();
    match closed {
        MemoryWriteOutcome::Refused(refusal) => assert_eq!(
            refusal.reason,
            MemoryRejection::ViewerMemoryDisabled.label()
        ),
        MemoryWriteOutcome::Written(_) => panic!("a closed switch let a viewer write through"),
    }
    assert!(stored(&composition, SESSION).is_empty());

    // The same candidate with the switch open *does* land — so the refusal above was the switch,
    // not an unrelated defect in the candidate.
    let open = MemoryPolicy {
        viewer_memory_enabled: true,
        ..MemoryPolicy::default()
    };
    let outcome = composition.write_memory(&candidate, &open).unwrap();
    assert!(matches!(outcome, MemoryWriteOutcome::Written(_)));
    assert_eq!(stored(&composition, SESSION), vec![candidate.record]);
}

// ---------------------------------------------------------------------------
// 落库的列 = 被判定审过的输入
// ---------------------------------------------------------------------------

#[test]
fn the_stored_row_round_trips_column_by_column() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    // A viewer record: the one case where `viewer_scoped_id` → `viewer_scope` carries a value
    // rather than the empty string, so a mapping that was dropped instead of copied is visible.
    let candidate = admissible("memory-viewer-roundtrip", VIEWER_KIND);
    let open = MemoryPolicy {
        viewer_memory_enabled: true,
        ..MemoryPolicy::default()
    };
    composition.write_memory(&candidate, &open).unwrap();

    let rows = stored(&composition, SESSION);
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.record_id, "memory-viewer-roundtrip");
    assert_eq!(row.workspace_id, WORKSPACE);
    assert_eq!(row.session_id, SESSION);
    assert_eq!(row.viewer_scoped_id, "viewer-scope-7");
    assert_eq!(row.kind, VIEWER_KIND);
    assert_eq!(row.purpose, "order_status");
    assert_eq!(row.authorization_ref, "authorization-ref-1");
    assert_eq!(row.content, BODY);
    assert_eq!(row.expires_at_utc_ms, EXPIRES);
    assert_eq!(
        row, &candidate.record,
        "every column MUST be the judged input, verbatim"
    );
}

#[test]
fn a_memory_write_reaches_the_memory_store_and_not_the_decision_log() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    assert!(logged(&composition).is_empty());

    write(
        &mut composition,
        &admissible("memory-store-only", SESSION_KIND),
    );

    assert_eq!(stored(&composition, SESSION).len(), 1);
    assert!(
        logged(&composition).is_empty(),
        "§42.2's execution body is the memory table; a memory *observation* reaching the chain is \
         the harness surface's job (W103), and a second path here would be a second definition"
    );
}

#[test]
fn a_duplicate_record_id_is_a_named_conflict_and_adds_no_second_row() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    let candidate = admissible("memory-duplicate", SESSION_KIND);
    write(&mut composition, &candidate);

    let error = composition
        .write_memory(&candidate, &MemoryPolicy::default())
        .expect_err("the same record id MUST NOT be written twice");
    assert_eq!(error.code, ErrorCode::Conflict);
    assert_eq!(stored(&composition, SESSION).len(), 1);
}

// ---------------------------------------------------------------------------
// 读回访问器是取数入口，不是 §42.3 的读取面
// ---------------------------------------------------------------------------

#[test]
fn the_read_back_does_not_cross_scopes() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    open_session(&composition, OTHER_SESSION);

    write(
        &mut composition,
        &admissible("memory-this-session", SESSION_KIND),
    );
    let mut elsewhere = admissible("memory-other-session", SESSION_KIND);
    elsewhere.record.session_id = OTHER_SESSION.into();
    write(&mut composition, &elsewhere);

    let rows = stored(&composition, SESSION);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].record_id, "memory-this-session");
}

#[test]
fn the_read_back_applies_no_read_judgement() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());

    // A record whose retention has already lapsed. §42.2 has no freshness rule, so the gate admits
    // it and it lands; the accessor must still hand it back, because expiry is §42.3's *read*-face
    // judgement and this entry point is a fetch. A fetch that filtered here would be answering a
    // question nobody asked it, under a rule the read face already owns.
    let mut candidate = admissible("memory-expired", SESSION_KIND);
    candidate.record.expires_at_utc_ms = 1;
    write(&mut composition, &candidate);

    assert_eq!(stored(&composition, SESSION), vec![candidate.record]);
}
