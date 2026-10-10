//! Session gating, downgrade drains and cost display.
//!
//! The one property under test throughout: an upgrade that cannot proceed now
//! is *deferred*, not failed, and a downgrade that cannot be done safely is
//! *refused*, not attempted. Staging survives a live session; a capability
//! survives an unfinished drain; a cost component survives as "unknown" rather
//! than as zero.
use nai_contracts::v1::ErrorCode;
use nai_security::Operation;
use nai_upgrade_core::{
    Activation, Component, CostEstimate, DeterministicVerifier, HostFacts, Outcome, PackageFile,
    Probe, RehearsalDrain, RehearsalHost, RehearsalJournal, RehearsalSessions, ResourceState,
    RevocationList, SessionGate, Total, UpgradePackage, UpgradeRequest, UpgradeTransaction,
    VerifyLimits, downgrade, require_rollback_compatible,
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

fn manifest(cost: Value) -> Vec<u8> {
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
        "cost": cost
    }))
    .unwrap()
}

fn partial_cost_manifest() -> Vec<u8> {
    manifest(
        json!({ "currency": "CNY", "cloud": "10", "download": null, "storage": "5", "runtime_delta": null }),
    )
}

fn full_cost_manifest() -> Vec<u8> {
    manifest(
        json!({ "currency": "CNY", "cloud": "10", "download": "2", "storage": "5", "runtime_delta": "3" }),
    )
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

#[test]
fn a_live_session_defers_activation_but_not_staging() {
    let manifest = partial_cost_manifest();
    let signature = DeterministicVerifier::sign(&manifest);
    let revocations = RevocationList::new(1_000, 60_000, []).unwrap();
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(30);

    let mut host = RehearsalHost::new(facts(), Probe::Measured(1 << 30), "manifest-old");
    let mut journal = RehearsalJournal::new();
    let request = UpgradeRequest {
        transaction_id: "tx-live".into(),
        from_manifest: "manifest-old".into(),
        manifest: &manifest,
        files: files(),
        signature: &signature,
        verifier: &DeterministicVerifier,
        revocations: &revocations,
        now_ms: 1_000,
        limits: VerifyLimits::default(),
        deadline,
        cancelled: &cancelled,
    };

    // Staging is safe during a live session: the running version is untouched.
    let prepared = {
        let mut transaction = UpgradeTransaction::new(&mut host, &mut journal);
        transaction.prepare(&op(), &request).unwrap()
    };
    assert!(host.is_staged("tx-live"));

    let sessions = RehearsalSessions::new(2);
    let gate = SessionGate::new(&sessions);
    assert_eq!(
        gate.activation(&op()).unwrap(),
        Activation::Deferred { active: 2 }
    );
    let blocked = gate.require_idle(&op()).unwrap_err();
    assert_eq!(blocked.code, ErrorCode::CapabilityNotReady);

    // Activation was gated, so the transaction is still merely prepared and the
    // running version is unchanged. Nothing needs to be re-staged.
    assert_eq!(
        journal.record_for("tx-live").unwrap().step,
        nai_upgrade_core::UpgradeStep::Prepared
    );
    assert_eq!(host.current_manifest(), "manifest-old");

    // The session ends: the same staged package activates, no second staging.
    sessions.set(0);
    assert_eq!(gate.activation(&op()).unwrap(), Activation::Allowed);
    gate.require_idle(&op()).unwrap();
    let outcome = {
        let mut transaction = UpgradeTransaction::new(&mut host, &mut journal);
        transaction.activate(&op(), &prepared).unwrap()
    };
    assert_eq!(outcome, Outcome::Committed);
    assert_eq!(host.counts().stage, 1, "activation reuses the staging");
    assert_eq!(
        journal.record_for("tx-live").unwrap().step,
        nai_upgrade_core::UpgradeStep::Committed
    );
}

#[test]
fn a_downgrade_drains_then_retires_and_keeps_records() {
    let capabilities = ["cap.tier-high".to_owned(), "cap.tier-mid".to_owned()];
    let mut drain = RehearsalDrain::new()
        .with_dependents("cap.tier-high", 3)
        .with_dependents("cap.tier-mid", 1)
        .with_records(42);

    let report = downgrade(&op(), &mut drain, &capabilities).unwrap();

    let expected: &[String] = &["cap.tier-high".to_owned(), "cap.tier-mid".to_owned()];
    assert_eq!(report.retired, expected);
    assert_eq!(report.sessions_ended, 4);
    // Retiring a capability takes away code, never the stored facts.
    assert_eq!(drain.records(), 42);
}

#[test]
fn a_downgrade_stops_when_sessions_outlive_the_drain() {
    let capabilities = ["cap.keep".to_owned(), "cap.hold".to_owned()];
    let mut drain = RehearsalDrain::new()
        .with_stubborn("cap.hold", 2)
        .with_records(7);

    let failure = downgrade(&op(), &mut drain, &capabilities).unwrap_err();
    assert_eq!(failure.code, ErrorCode::Conflict);

    // The first capability drained and retired; the second was left running
    // rather than stranding its sessions on something that no longer exists.
    let expected: &[String] = &["cap.keep".to_owned()];
    assert_eq!(drain.retired(), expected);
    assert_eq!(drain.records(), 7);
}

#[test]
fn a_downgrade_retiring_nothing_is_refused() {
    let mut drain = RehearsalDrain::new();
    let failure = downgrade(&op(), &mut drain, &[]).unwrap_err();
    assert_eq!(failure.code, ErrorCode::InvalidArgument);
    assert!(drain.retired().is_empty());
}

#[test]
fn an_incompatible_rollback_target_is_refused() {
    let package = UpgradePackage::parse(&partial_cost_manifest()).unwrap();

    // Inside the declared range: permitted.
    require_rollback_compatible(&package, "1.1.0").unwrap();

    // The version being upgraded away from, and anything newer, is outside it.
    for target in ["1.2.3", "1.2.0", "2.0.0"] {
        let failure = require_rollback_compatible(&package, target).unwrap_err();
        assert_eq!(failure.code, ErrorCode::VersionIncompatible, "{target}");
    }

    // An unreadable target is a bad argument, not a version verdict.
    assert_eq!(
        require_rollback_compatible(&package, "not-a-version")
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
}

#[test]
fn cost_marks_unknown_components_instead_of_zeroing_them() {
    let package = UpgradePackage::parse(&partial_cost_manifest()).unwrap();
    let estimate = CostEstimate::from_package(&package);

    assert_eq!(estimate.currency, "CNY");
    assert_eq!(estimate.cloud, Component::Known(10));
    assert_eq!(estimate.storage, Component::Known(5));
    assert!(
        matches!(estimate.download, Component::Unknown(_)),
        "an undeclared component is unknown, not free"
    );
    assert!(matches!(estimate.runtime_delta, Component::Unknown(_)));

    // The floor is honest about being a floor.
    assert_eq!(
        estimate.total(),
        Total::Partial {
            floor: 15,
            unknown: 2,
        }
    );
}

#[test]
fn a_fully_known_cost_totals_completely() {
    let package = UpgradePackage::parse(&full_cost_manifest()).unwrap();
    let estimate = CostEstimate::from_package(&package);

    assert_eq!(estimate.download, Component::Known(2));
    assert_eq!(estimate.runtime_delta, Component::Known(3));
    assert_eq!(estimate.total(), Total::Complete(20));
}
