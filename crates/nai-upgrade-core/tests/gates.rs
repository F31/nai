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
    Activation, Component, CostEstimate, DEMO_CONTENT, Outcome, Probe, RehearsalDrain,
    RehearsalJournal, RehearsalPackage, RehearsalSessions, SessionGate, Total, UpgradePackage,
    UpgradeTransaction, demo_manifest, downgrade, require_rollback_compatible,
};
use serde_json::{Value, json};

/// The demo package's cost table with two components left undeclared.
fn partial_cost() -> Value {
    json!({ "currency": "CNY", "cloud": "10", "download": null, "storage": "5", "runtime_delta": null })
}

/// The same table with every component declared.
fn full_cost() -> Value {
    json!({ "currency": "CNY", "cloud": "10", "download": "2", "storage": "5", "runtime_delta": "3" })
}

/// The demo manifest bytes for a cost table. The valid base comes from the
/// shared demo package, so the two tables above are the only thing this file
/// defines — a local copy of the manifest JSON would be a second one to drift.
fn cost_manifest(cost: Value) -> Vec<u8> {
    serde_json::to_vec(&demo_manifest(DEMO_CONTENT, cost)).unwrap()
}

fn op() -> Operation {
    Operation::new(Operation::CEILING).unwrap()
}

#[test]
fn a_live_session_defers_activation_but_not_staging() {
    let package = RehearsalPackage::with_cost(partial_cost());

    let mut host = package.host(Probe::Measured(1 << 30), "manifest-old");
    let mut journal = RehearsalJournal::new();
    let request = package.request("tx-live", "manifest-old");

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
    let package = UpgradePackage::parse(&cost_manifest(partial_cost())).unwrap();

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
    let package = UpgradePackage::parse(&cost_manifest(partial_cost())).unwrap();
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
    let package = UpgradePackage::parse(&cost_manifest(full_cost())).unwrap();
    let estimate = CostEstimate::from_package(&package);

    assert_eq!(estimate.download, Component::Known(2));
    assert_eq!(estimate.runtime_delta, Component::Known(3));
    assert_eq!(estimate.total(), Total::Complete(20));
}
