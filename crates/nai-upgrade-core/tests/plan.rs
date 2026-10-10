//! End-to-end checks for the upgrade transaction: staging, activation,
//! convergence to `rolled_back`, and idempotent recovery across a simulated
//! crash.
//!
//! A "crash" is modelled by replaying the real effects through the host while
//! leaving the journal at an intermediate step — exactly the state a power loss
//! between an effect and its record would leave behind.
use nai_contracts::v1::ErrorCode;
use nai_security::Operation;
use nai_upgrade_core::{
    BackupRef, DeterministicVerifier, EffectCounts, FaultPoint, HostFacts, Outcome, PackageFile,
    Prepared, Probe, RecoveryAction, RehearsalHost, RehearsalJournal, ResourceState,
    RevocationList, UpgradeHost, UpgradeJournal, UpgradeRecord, UpgradeRequest, UpgradeStep,
    UpgradeTransaction, VerifyLimits,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

const CONTENT: &[(&str, &[u8])] = &[("bin/app", b"payload")];

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn manifest() -> Vec<u8> {
    let files: Vec<Value> = CONTENT
        .iter()
        .map(|(path, content)| {
            json!({ "path": path, "sha256": sha256_hex(content), "bytes": content.len().to_string() })
        })
        .collect();
    serde_json::to_vec(&json!({
        "manifest_schema": 1,
        "package_id": "plugin.demo",
        "package_version": "1.2.3",
        "kind": "plugin",
        "kernel_range": ">=1.0.0, <2.0.0",
        "requires": [{ "ref": "cap.one", "range": ">=1.0.0, <2.0.0" }],
        "hosts": ["inproc"],
        "os": ["linux"],
        "architectures": ["x86_64"],
        "runtime": [{ "id": "rt-cpu", "accel": "cpu" }],
        "files": files,
        "install_bytes": "1024",
        "migrated_bytes": "2048",
        "signer_ref": "signer.demo",
        "license_ref": "license.demo",
        "migration_ref": "migration.forward",
        "rollback_range": ">=1.0.0, <1.2.0",
        "cost": { "currency": "CNY", "cloud": "10", "download": null, "storage": "5", "runtime_delta": null }
    }))
    .unwrap()
}

fn files() -> Vec<PackageFile<'static>> {
    CONTENT
        .iter()
        .map(|&(path, content)| PackageFile { path, content })
        .collect()
}

fn facts() -> HostFacts {
    HostFacts {
        host: "inproc".into(),
        os: "linux".into(),
        architecture: "x86_64".into(),
        kernel_version: "1.96.0".into(),
        capabilities: vec!["cap.one".into()],
        runtime_ids: vec!["rt-cpu".into()],
        credentials_available: ResourceState::Sufficient,
    }
}

fn op() -> Operation {
    Operation::new(Duration::from_secs(30)).unwrap()
}

struct Fixture {
    manifest: Vec<u8>,
    signature: Vec<u8>,
    revocations: RevocationList,
    verifier: DeterministicVerifier,
    cancelled: AtomicBool,
}
impl Fixture {
    fn new() -> Self {
        let manifest = manifest();
        let signature = DeterministicVerifier::sign(&manifest);
        Self {
            manifest,
            signature,
            revocations: RevocationList::new(1_000, 60_000, []).unwrap(),
            verifier: DeterministicVerifier,
            cancelled: AtomicBool::new(false),
        }
    }
    fn request(&self) -> UpgradeRequest<'_> {
        UpgradeRequest {
            transaction_id: "tx-1".into(),
            from_manifest: "manifest-old".into(),
            manifest: &self.manifest,
            files: files(),
            signature: &self.signature,
            verifier: &self.verifier,
            revocations: &self.revocations,
            now_ms: 1_000,
            limits: VerifyLimits::default(),
            deadline: Instant::now() + Duration::from_secs(30),
            cancelled: &self.cancelled,
        }
    }
}

fn rehearsal_host(space: Probe) -> RehearsalHost {
    RehearsalHost::new(facts(), space, "manifest-old")
}

fn prepare(
    fixture: &Fixture,
    host: &mut RehearsalHost,
    journal: &mut dyn UpgradeJournal,
) -> nai_upgrade_core::Result<Prepared> {
    let request = fixture.request();
    let mut transaction = UpgradeTransaction::new(host, journal);
    transaction.prepare(&op(), &request)
}

fn activate(
    host: &mut RehearsalHost,
    journal: &mut dyn UpgradeJournal,
    prepared: &Prepared,
) -> nai_upgrade_core::Result<Outcome> {
    let mut transaction = UpgradeTransaction::new(host, journal);
    transaction.activate(&op(), prepared)
}

fn recover(
    host: &mut RehearsalHost,
    journal: &mut dyn UpgradeJournal,
    transaction_id: &str,
) -> nai_upgrade_core::Result<RecoveryAction> {
    let mut transaction = UpgradeTransaction::new(host, journal);
    transaction.recover(&op(), transaction_id)
}

/// Replay an activation's real effects through the host, leaving the journal at
/// `step` to model a crash between an effect and its record.
fn crash_at(
    host: &mut RehearsalHost,
    journal: &mut dyn UpgradeJournal,
    prepared: &Prepared,
    step: UpgradeStep,
) -> BackupRef {
    let backup = host.backup(&op(), &prepared.staging).unwrap();
    let mut record = UpgradeRecord {
        transaction_id: prepared.transaction_id.clone(),
        from_manifest: prepared.from_manifest.clone(),
        to_manifest: prepared.to_manifest.clone(),
        step: UpgradeStep::Migrated,
        backup_ref: Some(backup.as_str().to_owned()),
        error_code: None,
    };
    journal.record(&op(), &record).unwrap();
    if step == UpgradeStep::Migrated {
        return backup;
    }
    host.migrate(&op(), &prepared.staging).unwrap();
    host.self_check(&op()).unwrap();
    record.step = UpgradeStep::Checked;
    record.backup_ref = None;
    journal.record(&op(), &record).unwrap();
    backup
}

#[test]
fn activation_commits_and_swaps_the_manifest_once() {
    let fixture = Fixture::new();
    let mut host = rehearsal_host(Probe::Measured(1 << 30));
    let mut journal = RehearsalJournal::new();

    let prepared = prepare(&fixture, &mut host, &mut journal).unwrap();
    assert_eq!(prepared.content_bytes, 7);
    assert!(host.is_staged("tx-1"));
    assert_eq!(
        journal.record_for("tx-1").unwrap().step,
        UpgradeStep::Prepared
    );

    assert_eq!(
        activate(&mut host, &mut journal, &prepared).unwrap(),
        Outcome::Committed
    );

    let record = journal.record_for("tx-1").unwrap();
    assert_eq!(record.step, UpgradeStep::Committed);
    assert_eq!(
        record.backup_ref.as_deref(),
        Some("backup-staging-tx-1"),
        "the backup reference survives to the committed record"
    );
    assert_eq!(host.current_manifest(), prepared.to_manifest.as_str());
    assert_eq!(host.swaps().len(), 1, "exactly one swap");
    assert_eq!(host.swaps()[0], prepared.to_manifest);
    assert_eq!(
        host.counts(),
        EffectCounts {
            stage: 1,
            backup: 1,
            migrate: 1,
            self_check: 1,
            commit: 1,
            ..EffectCounts::default()
        }
    );
}

#[test]
fn an_insufficient_or_unknown_preflight_leaves_no_trace() {
    let fixture = Fixture::new();
    for (space, code) in [
        (Probe::Measured(100), ErrorCode::ResourceExhausted),
        (Probe::Unknown("no probe"), ErrorCode::CapabilityNotReady),
    ] {
        let mut host = rehearsal_host(space);
        let mut journal = RehearsalJournal::new();
        let failure = prepare(&fixture, &mut host, &mut journal).unwrap_err();
        assert_eq!(failure.code, code);
        // A refusal costs no bytes, leaves no record, and cannot disturb the
        // running version.
        assert_eq!(host.counts(), EffectCounts::default());
        assert!(journal.record_for("tx-1").is_none());
        assert_eq!(host.current_manifest(), "manifest-old");
    }
}

#[test]
fn a_target_equal_to_the_current_version_is_refused() {
    let fixture = Fixture::new();
    let mut host = rehearsal_host(Probe::Measured(1 << 30));
    let mut journal = RehearsalJournal::new();
    // Derive the real target identity, then claim it is already installed.
    let prepared = prepare(&fixture, &mut host, &mut journal).unwrap();
    let mut request = fixture.request();
    request.from_manifest = prepared.to_manifest.clone();
    let mut transaction = UpgradeTransaction::new(&mut host, &mut journal);
    let failure = transaction.prepare(&op(), &request).unwrap_err();
    assert_eq!(failure.code, ErrorCode::InvalidArgument);
    assert_eq!(host.counts().stage, 1, "the first prepare still staged");
}

#[test]
fn a_failure_after_the_backup_converges_to_rolled_back() {
    let fixture = Fixture::new();
    for fault in [
        FaultPoint::Migrate,
        FaultPoint::SelfCheck,
        FaultPoint::Commit,
    ] {
        let mut host = rehearsal_host(Probe::Measured(1 << 30))
            .fail_at(fault)
            .with_post_upgrade_writes(4, 9);
        let mut journal = RehearsalJournal::new();
        let prepared = prepare(&fixture, &mut host, &mut journal).unwrap();

        let report = match activate(&mut host, &mut journal, &prepared).unwrap() {
            Outcome::RolledBack(report) => report,
            other => panic!("{fault:?} produced {other:?}"),
        };
        assert_eq!(report.exported.facts, 4, "{fault:?}");
        assert_eq!(report.exported.audit, 9, "{fault:?}");

        assert_eq!(
            journal.record_for("tx-1").unwrap().step,
            UpgradeStep::RolledBack,
            "{fault:?}"
        );
        assert_eq!(host.current_manifest(), "manifest-old", "{fault:?}");
        assert!(host.swaps().is_empty(), "{fault:?}");
        assert_eq!(host.counts().rollback, 1, "{fault:?}");

        // The failing step itself did not run; the steps before it did.
        match fault {
            FaultPoint::Migrate => assert_eq!(host.counts().migrate, 0),
            FaultPoint::SelfCheck => {
                assert_eq!(host.counts().migrate, 1);
                assert_eq!(host.counts().self_check, 0);
            }
            FaultPoint::Commit => {
                assert_eq!(host.counts().self_check, 1);
                assert_eq!(host.counts().commit, 0);
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn a_backup_failure_closes_the_record_without_a_restore() {
    let fixture = Fixture::new();
    let mut host = rehearsal_host(Probe::Measured(1 << 30)).fail_at(FaultPoint::Backup);
    let mut journal = RehearsalJournal::new();
    let prepared = prepare(&fixture, &mut host, &mut journal).unwrap();

    let failure = activate(&mut host, &mut journal, &prepared).unwrap_err();
    assert_eq!(failure.code, ErrorCode::ResourceExhausted);
    let record = journal.record_for("tx-1").unwrap();
    assert_eq!(record.step, UpgradeStep::RolledBack);
    assert!(
        record.backup_ref.is_none(),
        "nothing was migrated, so there is no backup to restore"
    );
    assert_eq!(host.counts().rollback, 0);
    assert_eq!(host.counts().backup, 0);
    assert_eq!(host.current_manifest(), "manifest-old");
}

#[test]
fn a_stop_signal_converges_before_it_propagates() {
    let fixture = Fixture::new();
    let mut host = rehearsal_host(Probe::Measured(1 << 30))
        .fail_with(FaultPoint::Migrate, ErrorCode::Cancelled);
    let mut journal = RehearsalJournal::new();
    let prepared = prepare(&fixture, &mut host, &mut journal).unwrap();

    let failure = activate(&mut host, &mut journal, &prepared).unwrap_err();
    assert_eq!(failure.code, ErrorCode::Cancelled);
    // The rollback happened *before* the cancellation surfaced: the system is
    // consistent even though the caller sees a stop signal.
    assert_eq!(
        journal.record_for("tx-1").unwrap().step,
        UpgradeStep::RolledBack
    );
    assert_eq!(host.counts().rollback, 1);
    assert_eq!(host.current_manifest(), "manifest-old");
}

#[test]
fn a_cancellation_before_activation_leaves_the_record_prepared() {
    let fixture = Fixture::new();
    let mut host = rehearsal_host(Probe::Measured(1 << 30));
    let mut journal = RehearsalJournal::new();
    let prepared = prepare(&fixture, &mut host, &mut journal).unwrap();

    let stopped = op();
    stopped.cancel();
    let mut transaction = UpgradeTransaction::new(&mut host, &mut journal);
    let failure = transaction.activate(&stopped, &prepared).unwrap_err();
    assert_eq!(failure.code, ErrorCode::Cancelled);
    assert_eq!(
        journal.record_for("tx-1").unwrap().step,
        UpgradeStep::Prepared,
        "a stop before any effect touches nothing"
    );
    assert_eq!(
        host.counts(),
        EffectCounts {
            stage: 1,
            ..EffectCounts::default()
        }
    );

    // Recovery then abandons it: discard the staging, close the record.
    assert_eq!(
        recover(&mut host, &mut journal, "tx-1").unwrap(),
        RecoveryAction::Abandoned
    );
    assert!(!host.is_staged("tx-1"));
    assert_eq!(host.counts().discard_staging, 1);
    assert_eq!(host.counts().rollback, 0);
    assert_eq!(
        journal.record_for("tx-1").unwrap().step,
        UpgradeStep::RolledBack
    );
}

#[test]
fn recovery_from_migrated_restores_and_is_idempotent() {
    let fixture = Fixture::new();
    let mut host = rehearsal_host(Probe::Measured(1 << 30)).with_post_upgrade_writes(2, 3);
    let mut journal = RehearsalJournal::new();
    let prepared = prepare(&fixture, &mut host, &mut journal).unwrap();
    crash_at(&mut host, &mut journal, &prepared, UpgradeStep::Migrated);

    {
        let mut transaction = UpgradeTransaction::new(&mut host, &mut journal);
        match transaction.recover(&op(), "tx-1").unwrap() {
            RecoveryAction::RolledBack(report) => {
                assert_eq!(report.exported.facts, 2);
                assert_eq!(report.exported.audit, 3);
            }
            other => panic!("expected a rollback, got {other:?}"),
        }
        // Re-entering a terminal record repeats nothing.
        assert_eq!(
            transaction.recover(&op(), "tx-1").unwrap(),
            RecoveryAction::AlreadyRolledBack
        );
    }
    assert_eq!(
        journal.record_for("tx-1").unwrap().step,
        UpgradeStep::RolledBack
    );
    assert_eq!(host.counts().rollback, 1);
    assert_eq!(host.current_manifest(), "manifest-old");
}

#[test]
fn recovery_after_a_landed_swap_confirms_commit_without_repeating_effects() {
    let fixture = Fixture::new();
    let mut host = rehearsal_host(Probe::Measured(1 << 30));
    let mut journal = RehearsalJournal::new();
    let prepared = prepare(&fixture, &mut host, &mut journal).unwrap();
    crash_at(&mut host, &mut journal, &prepared, UpgradeStep::Checked);
    // The swap landed but the journal never advanced past `checked`.
    host.commit(&op(), &prepared.staging, &prepared.to_manifest)
        .unwrap();
    assert_eq!(host.counts().commit, 1);
    assert_eq!(
        journal.record_for("tx-1").unwrap().step,
        UpgradeStep::Checked
    );

    {
        let mut transaction = UpgradeTransaction::new(&mut host, &mut journal);
        assert_eq!(
            transaction.recover(&op(), "tx-1").unwrap(),
            RecoveryAction::AlreadyCommitted
        );
        assert_eq!(
            transaction.recover(&op(), "tx-1").unwrap(),
            RecoveryAction::AlreadyCommitted
        );
    }
    assert_eq!(
        journal.record_for("tx-1").unwrap().step,
        UpgradeStep::Committed
    );
    assert_eq!(host.counts().commit, 1, "no second swap");
    assert_eq!(host.counts().rollback, 0);
}

#[test]
fn recovery_before_the_swap_rolls_back() {
    let fixture = Fixture::new();
    let mut host = rehearsal_host(Probe::Measured(1 << 30));
    let mut journal = RehearsalJournal::new();
    let prepared = prepare(&fixture, &mut host, &mut journal).unwrap();
    crash_at(&mut host, &mut journal, &prepared, UpgradeStep::Checked);

    let action = recover(&mut host, &mut journal, "tx-1").unwrap();
    assert!(matches!(action, RecoveryAction::RolledBack(_)));
    assert_eq!(host.counts().rollback, 1);
    assert_eq!(host.current_manifest(), "manifest-old");
    assert_eq!(
        journal.record_for("tx-1").unwrap().step,
        UpgradeStep::RolledBack
    );
}

#[test]
fn recovery_of_an_absent_or_committed_transaction_has_no_effects() {
    let fixture = Fixture::new();
    let mut host = rehearsal_host(Probe::Measured(1 << 30));
    let mut journal = RehearsalJournal::new();

    assert_eq!(
        recover(&mut host, &mut journal, "never-started").unwrap(),
        RecoveryAction::NotStarted
    );

    let prepared = prepare(&fixture, &mut host, &mut journal).unwrap();
    assert_eq!(
        activate(&mut host, &mut journal, &prepared).unwrap(),
        Outcome::Committed
    );
    let before = host.counts();
    assert_eq!(
        recover(&mut host, &mut journal, "tx-1").unwrap(),
        RecoveryAction::AlreadyCommitted
    );
    assert_eq!(
        host.counts(),
        before,
        "a committed re-entry changes nothing"
    );
}

#[test]
fn the_transaction_journals_into_the_storage_table() {
    let dir = tempfile::tempdir().unwrap();
    let paths = nai_storage::DataPaths::at(dir.path()).unwrap();
    let mut store = nai_storage::Store::open(paths, &op()).unwrap();

    let fixture = Fixture::new();
    let mut host = rehearsal_host(Probe::Measured(1 << 30));
    let prepared = prepare(&fixture, &mut host, &mut store).unwrap();
    assert_eq!(
        activate(&mut host, &mut store, &prepared).unwrap(),
        Outcome::Committed
    );

    let record = store.upgrade_record(&op(), "tx-1").unwrap().unwrap();
    assert_eq!(record.step, UpgradeStep::Committed);
    assert_eq!(record.from_manifest, "manifest-old");
    assert_eq!(record.to_manifest, prepared.to_manifest);
    assert!(record.backup_ref.is_some());
}
