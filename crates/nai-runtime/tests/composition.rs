//! Composition-root tests: storage ownership, bootstrap/permission paths,
//! decision-log grading, control ordering and duty-lease logging.
mod common;
use common::*;
use nai_contracts::v1::{ErrorCode, MediaState, SessionLifecycle, SessionMode};
use nai_kernel::duty::{DUTY_REVOKE_DELAY, DutyEnforcement};
use nai_runtime::{Composition, ControlAction};
use nai_storage::{ConfigKind, DataPaths, DecisionKind};
// `Operation` comes through the upgrade crate's re-export: nai-security is not a
// direct dependency of nai-runtime, and the gate is what needs it anyway.
use nai_upgrade_core::{Activation, Operation, SessionGate};
use serde_json::json;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tempfile::tempdir;

/// Holds the shared session lock for `hold`, signalling **after** it is held so
/// the caller knows the contention is already in place before it calls into the
/// composition. A sleep would race the scheduler; a handshake cannot.
fn hold_session(
    composition: &Composition,
    hold: Duration,
) -> (std::thread::JoinHandle<()>, mpsc::Receiver<()>) {
    let session = composition.session();
    let (held_tx, held_rx) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        let _guard = session.lock().unwrap();
        held_tx.send(()).unwrap();
        std::thread::sleep(hold);
    });
    (handle, held_rx)
}

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

/// A status read holds the shared session lock for microseconds; a command holds
/// it longer. Either is a transient contention, and neither may cost the revoke:
/// the action is either applied or reported, never skipped because a reader won.
#[test]
fn a_transient_session_lock_race_does_not_steal_the_revoke() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());

    let (holder, held) = hold_session(&composition, Duration::from_millis(20));
    held.recv().unwrap();
    let epoch = composition
        .control(ControlAction::Mute)
        .expect("a transient session-lock contention must not steal the revoke");
    holder.join().unwrap();

    assert_eq!(epoch, 1);
    assert_eq!(composition.live_epoch().unwrap(), Some(1));
    verify_chain(&composition);
}

/// The periodic liveness read runs once per watch tick and must not stall lease
/// renewal, so a lock it cannot take reports "not observed" rather than a fault.
/// Reporting an error here is what ends a supervision loop over contention.
#[test]
fn a_busy_session_lock_reads_as_not_observed_rather_than_a_fault() {
    let dir = tempdir().unwrap();
    let composition = prepared(dir.path());

    assert_eq!(
        composition.live_epoch().unwrap(),
        Some(0),
        "an uncontended read reports the epoch"
    );

    let (holder, held) = hold_session(&composition, Duration::from_millis(30));
    held.recv().unwrap();
    assert_eq!(
        composition.live_epoch().unwrap(),
        None,
        "a lock held past the poll budget is 'not observed this tick', not a fault"
    );
    holder.join().unwrap();

    // The miss was transient: the next read observes the epoch again, which is
    // what lets a watch loop retry instead of ending.
    assert_eq!(composition.live_epoch().unwrap(), Some(0));
}

/// `live_epoch` must not report "not observed" for a change it can see, or the
/// two tests above would pass on an implementation that always returns `None`.
#[test]
fn a_revoked_session_is_observed_as_a_changed_epoch() {
    let dir = tempdir().unwrap();
    let mut composition = prepared(dir.path());
    composition.control(ControlAction::Mute).unwrap();
    assert_eq!(composition.live_epoch().unwrap(), Some(1));
}

#[test]
fn second_storage_owner_is_rejected() {
    let dir = tempdir().unwrap();
    provision(
        dir.path(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        &runtime_profile(WORKSPACE),
    );
    let first = Composition::prepare(
        DataPaths::at(dir.path()).unwrap(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        false,
    )
    .unwrap();
    let Err(error) = Composition::prepare(
        DataPaths::at(dir.path()).unwrap(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        false,
    ) else {
        panic!("a second storage owner must be rejected");
    };
    assert_eq!(error.code, ErrorCode::Conflict);
    drop(first);
}

#[test]
fn prepare_without_profile_is_rejected() {
    let dir = tempdir().unwrap();
    let operation = store_operation();
    let mut store = open_store(dir.path());
    store
        .create_workspace(&operation, WORKSPACE, WORKSPACE)
        .unwrap();
    grant(&mut store, &operation, WORKSPACE, ACTOR, PROFILE);
    drop(store);
    assert!(
        Composition::prepare(
            DataPaths::at(dir.path()).unwrap(),
            WORKSPACE,
            ACTOR,
            PROFILE,
            false
        )
        .is_err()
    );
}

#[test]
fn prepare_without_config_permission_is_denied() {
    let dir = tempdir().unwrap();
    provision(
        dir.path(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        &runtime_profile(WORKSPACE),
    );
    let Err(error) = Composition::prepare(
        DataPaths::at(dir.path()).unwrap(),
        WORKSPACE,
        "intruder",
        PROFILE,
        false,
    ) else {
        panic!("an actor without config.read must be denied");
    };
    assert_eq!(error.code, ErrorCode::PermissionDenied);
}

#[test]
fn invalid_runtime_config_is_rejected_at_provision() {
    let dir = tempdir().unwrap();
    let operation = store_operation();
    let mut store = open_store(dir.path());
    store
        .create_workspace(&operation, WORKSPACE, WORKSPACE)
        .unwrap();
    grant(&mut store, &operation, WORKSPACE, ACTOR, PROFILE);
    let mut config = runtime_profile(WORKSPACE);
    config["media"]["lease_expire_ms"] = json!(50); // <= lease_renew_ms
    let saved = store.save_config(
        &operation,
        (WORKSPACE, ACTOR),
        ConfigKind::Runtime,
        PROFILE,
        &config,
        0,
    );
    assert_eq!(saved.unwrap_err().code, ErrorCode::InvalidArgument);
}

#[test]
fn bootstrap_creates_workspace_and_permissions_before_config_load() {
    let dir = tempdir().unwrap();
    // Bootstrap with no provisioned profile creates the workspace and grants the
    // permissions, then fails because the runtime profile does not exist yet.
    assert!(
        Composition::prepare(
            DataPaths::at(dir.path()).unwrap(),
            WORKSPACE,
            ACTOR,
            PROFILE,
            true
        )
        .is_err()
    );
    // The bootstrap-granted config.write permission lets the owner store it.
    let operation = store_operation();
    let mut store = open_store(dir.path());
    store
        .save_config(
            &operation,
            (WORKSPACE, ACTOR),
            ConfigKind::Runtime,
            PROFILE,
            &runtime_profile(WORKSPACE),
            0,
        )
        .unwrap();
    drop(store);

    let composition = Composition::prepare(
        DataPaths::at(dir.path()).unwrap(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        false,
    )
    .unwrap();
    assert_eq!(composition.session_id(), WORKSPACE);
    assert_eq!(composition.lease_renew_ms(), 100);
    assert_eq!(composition.lease_expire_ms(), 600);
    assert_eq!(
        composition
            .session_lock()
            .unwrap()
            .snapshot()
            .generation_epoch,
        0
    );
    assert_eq!(composition.gate().epoch_floor(), 0);
    assert!(!composition.boot_id().is_empty());
    let lease = composition
        .rehearsal_lease("drill", &["u".to_string()], 1, utc_ms())
        .unwrap();
    assert_eq!(lease.session_id, WORKSPACE);
    assert_eq!(lease.mode, SessionMode::Rehearsal as i32);
    assert_eq!(lease.generation_epoch, 1);
    // A lease may not reference authority the live session already revoked.
    assert!(matches!(
        composition
            .rehearsal_lease("drill", &["u".to_string()], 0, utc_ms())
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    ));
}

#[test]
fn control_mutes_revokes_and_logs_before_returning() {
    let dir = tempdir().unwrap();
    provision(
        dir.path(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        &runtime_profile(WORKSPACE),
    );
    let mut composition = Composition::prepare(
        DataPaths::at(dir.path()).unwrap(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        false,
    )
    .unwrap();
    assert_eq!(
        composition
            .session_lock()
            .unwrap()
            .snapshot()
            .generation_epoch,
        0
    );
    assert_eq!(composition.control(ControlAction::Mute).unwrap(), 1);
    let snapshot = composition.session_lock().unwrap().snapshot().clone();
    assert_eq!(snapshot.generation_epoch, 1);
    assert_eq!(snapshot.media_state, MediaState::Muted as i32);
    assert_eq!(composition.control(ControlAction::Takeover).unwrap(), 2);
    // The cancellation entries are critical: nothing is left in the telemetry buffer.
    assert_eq!(composition.decision_buffer_stats().unwrap(), (0, 0));
    verify_chain(&composition);
}

#[test]
fn duty_grant_expiry_and_revocation_are_logged() {
    let dir = tempdir().unwrap();
    provision(
        dir.path(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        &runtime_profile(WORKSPACE),
    );
    let mut composition = Composition::prepare(
        DataPaths::at(dir.path()).unwrap(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        false,
    )
    .unwrap();
    let now = Instant::now();
    composition
        .confirm_duty("operator", Duration::from_millis(100), now)
        .unwrap();
    assert_eq!(
        composition.enforce_duty(now).unwrap(),
        DutyEnforcement::Valid
    );

    let halted = now + Duration::from_millis(150);
    assert_eq!(
        composition.enforce_duty(halted).unwrap(),
        DutyEnforcement::HaltedPendingRevoke
    );
    // Repeated ticks inside the revoke delay must not duplicate the expiry entry.
    assert_eq!(
        composition.enforce_duty(halted).unwrap(),
        DutyEnforcement::HaltedPendingRevoke
    );

    let revoked = now + Duration::from_millis(100) + DUTY_REVOKE_DELAY + Duration::from_millis(1);
    assert_eq!(
        composition.enforce_duty(revoked).unwrap(),
        DutyEnforcement::Revoked
    );
    assert_eq!(
        composition
            .session_lock()
            .unwrap()
            .snapshot()
            .generation_epoch,
        1
    );
    // No buffered telemetry: the duty transitions were recorded as critical.
    assert_eq!(composition.decision_buffer_stats().unwrap(), (0, 0));
    verify_chain(&composition);
}

#[test]
fn decision_grading_reports_critical_failures_and_persists_telemetry() {
    let dir = tempdir().unwrap();
    provision(
        dir.path(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        &runtime_profile(WORKSPACE),
    );
    let mut composition = Composition::prepare(
        DataPaths::at(dir.path()).unwrap(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        false,
    )
    .unwrap();
    let session = composition.session_id().to_string();

    // A critical entry over the payload budget is reported, never silently dropped.
    let oversized = json!({"blob": "x".repeat(70_000)});
    let Err(error) = composition.record(
        DecisionKind::Cancellation,
        Some(&session),
        &oversized,
        "grading-1",
    ) else {
        panic!("an over-budget critical entry must fail");
    };
    assert_eq!(error.code, ErrorCode::ResourceExhausted);

    let ack = composition
        .record(
            DecisionKind::Cancellation,
            Some(&session),
            &json!({"event": "probe"}),
            "grading-2",
        )
        .unwrap();
    assert!(ack.persisted && ack.entry_hash.is_some());

    // Telemetry persists normally here; its loss-buffering path is verified below.
    let ack = composition
        .record(
            DecisionKind::Usage,
            Some(&session),
            &json!({"count": 1}),
            "grading-3",
        )
        .unwrap();
    assert!(ack.persisted);
    verify_chain(&composition);
}

#[test]
fn telemetry_is_buffered_when_persistence_fails_while_critical_is_reported() {
    let dir = tempdir().unwrap();
    let operation = store_operation();
    let mut store = open_store(dir.path());
    // The workspace row is absent, so the decision_log foreign key fails.
    let telemetry = store
        .append_decision(
            &operation,
            "absent-workspace",
            None,
            DecisionKind::Usage,
            &json!({"count": 1}),
            "buffer-1",
        )
        .unwrap();
    assert!(!telemetry.persisted && telemetry.entry_hash.is_none());
    assert_eq!(store.decision_buffer_stats().0, 1);
    let critical = store.append_decision(
        &operation,
        "absent-workspace",
        None,
        DecisionKind::Cancellation,
        &json!({"count": 1}),
        "buffer-2",
    );
    assert_eq!(critical.unwrap_err().code, ErrorCode::Internal);
    // The failed critical write did not consume the buffered telemetry entry.
    assert_eq!(store.decision_buffer_stats().0, 1);
}

/// R10: the activation gate reads the **real** session the assembly root owns,
/// not a rehearsal counter. Walking the real lifecycle is the whole point — the
/// gap R10 recorded was "真实会话注册表（装配根）未接线".
#[test]
fn the_upgrade_activation_gate_reads_the_real_session_lifecycle() {
    let dir = tempdir().unwrap();
    let composition = prepared(dir.path());
    // CEILING, not a 30s literal: the registry guard forbids a second copy of the
    // operation ceiling (tests/registry.test.mjs, "no Rust source hides...").
    let op = Operation::new(Operation::CEILING).unwrap();
    let gate = SessionGate::new(&composition);

    // Not on air yet (the assembly leaves the session `Created`) -> allowed.
    assert_eq!(gate.activation(&op).unwrap(), Activation::Allowed);

    // Drive the REAL session onto air.
    {
        let mut session = composition.session_lock().unwrap();
        session
            .transition(true, SessionLifecycle::Preparing, true)
            .unwrap();
        session
            .transition(true, SessionLifecycle::Ready, true)
            .unwrap();
        session
            .transition(true, SessionLifecycle::Running, true)
            .unwrap();
    }
    assert_eq!(
        gate.activation(&op).unwrap(),
        Activation::Deferred { active: 1 },
        "an on-air session must defer the version swap"
    );
    assert_eq!(
        gate.require_idle(&op).unwrap_err().code,
        ErrorCode::CapabilityNotReady
    );

    // Draining is still a live show winding down -> still deferred.
    {
        let mut session = composition.session_lock().unwrap();
        session
            .transition(true, SessionLifecycle::Draining, true)
            .unwrap();
    }
    assert_eq!(
        gate.activation(&op).unwrap(),
        Activation::Deferred { active: 1 }
    );

    // Off air -> activation may proceed on the already-staged package.
    {
        let mut session = composition.session_lock().unwrap();
        session
            .transition(true, SessionLifecycle::Stopped, true)
            .unwrap();
    }
    assert_eq!(gate.activation(&op).unwrap(), Activation::Allowed);
}

/// The unobservable case MUST fail closed: a busy session lock reported as
/// "no live session" would let a swap happen during a live show.
#[test]
fn the_upgrade_activation_gate_fails_closed_when_the_session_is_unobservable() {
    let dir = tempdir().unwrap();
    let composition = prepared(dir.path());
    let op = Operation::new(Operation::CEILING).unwrap();
    let gate = SessionGate::new(&composition);

    // A handshake, not a sleep: the contention is provably in place before the
    // gate is called, so this cannot race the scheduler.
    let (handle, held) = hold_session(&composition, Duration::from_millis(300));
    held.recv().unwrap();

    let error = gate.activation(&op).unwrap_err();
    assert_eq!(
        error.code,
        ErrorCode::Conflict,
        "an unobservable session must be reported, never read as 'no live session'"
    );

    handle.join().unwrap();
}
