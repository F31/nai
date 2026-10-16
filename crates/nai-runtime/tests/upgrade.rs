//! R10: the runtime's **upgrade activation call site**.
//!
//! W123 wired the assembly root to *answer* the activation gate from its own
//! real session. This file covers the call site that *uses* that answer: the
//! root stages a package, asks its gate at the activation boundary, and defers
//! the swap while a show is on air.
//!
//! The host and the journal stay injected seams — the real platform installer
//! and the real distribution endpoint do not exist in this repository — so these
//! tests drive the orchestration against the upgrade core's rehearsal doubles.
//! What is **not** a double here is the session fact: it comes from the real
//! `Session` the composition owns.
mod common;
use common::*;
use nai_contracts::v1::{ErrorCode, SessionLifecycle};
use nai_runtime::{Composition, UpgradeActivation};
use nai_storage::{DataPaths, UpgradeStep};
use nai_upgrade_core::{
    EffectCounts, ExportedWrites, FaultPoint, Operation, Outcome, Probe, RehearsalHost,
    RehearsalJournal, RehearsalPackage,
};
use std::{path::Path, time::Duration};
use tempfile::tempdir;

/// Ample room, so the preflight refuses for nothing but a deliberate mutation.
const SPACE: u64 = 1 << 30;

/// `CEILING`, never a `30s` literal: the registry guard forbids a second copy of
/// the operation ceiling (`tests/registry.test.mjs`).
fn op() -> Operation {
    Operation::new(Operation::CEILING).unwrap()
}

/// The root as assembly leaves it: the single session exists and is `Created`,
/// which is **not** on air.
fn assembled(root: &Path) -> Composition {
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

/// One legal transition of the **real** session. Illegal steps fail loudly:
/// every path below is walked step by step, never jumped.
fn to_lifecycle(composition: &Composition, target: SessionLifecycle) {
    composition
        .session_lock()
        .unwrap()
        .transition(true, target, true)
        .unwrap();
}

/// `Created → Preparing → Ready → Running`: the only legal path onto air.
fn start_show(composition: &Composition) {
    to_lifecycle(composition, SessionLifecycle::Preparing);
    to_lifecycle(composition, SessionLifecycle::Ready);
    to_lifecycle(composition, SessionLifecycle::Running);
}

/// `Running → Draining → Stopped`: the only legal path off air.
fn end_show(composition: &Composition) {
    to_lifecycle(composition, SessionLifecycle::Draining);
    to_lifecycle(composition, SessionLifecycle::Stopped);
}

fn on_air(root: &Path) -> Composition {
    let composition = assembled(root);
    start_show(&composition);
    composition
}

/// The demo package plus the two injected seams a transaction needs. Named
/// `demo` and not `package`, so the binding and the builder can share a scope.
fn demo(space: u64, current: &str) -> (RehearsalPackage, RehearsalHost, RehearsalJournal) {
    let package = RehearsalPackage::new();
    let host = package.host(Probe::Measured(space), current);
    (package, host, RehearsalJournal::new())
}

/// The journal step for a transaction, or a panic naming the missing record:
/// "no record at all" and "the wrong record" must not read the same.
fn journal_step(journal: &RehearsalJournal, transaction_id: &str) -> UpgradeStep {
    journal
        .record_for(transaction_id)
        .unwrap_or_else(|| panic!("no journal record for {transaction_id}"))
        .step
}

/// "Staged but nothing else" — a backup, a migration or a swap during a live
/// show would each show up here as a non-zero count.
const STAGED_ONLY: EffectCounts = EffectCounts {
    stage: 1,
    discard_staging: 0,
    backup: 0,
    migrate: 0,
    self_check: 0,
    commit: 0,
    rollback: 0,
};

/// The separation is the requirement: **staging is safe at any time**, only the
/// swap of a running version needs silence. So during a live show the package is
/// staged, the boundary defers, and the running version is not touched.
#[test]
fn a_live_show_stages_the_package_but_defers_the_swap() {
    let dir = tempdir().unwrap();
    let composition = on_air(dir.path());
    let (package, mut host, mut journal) = demo(SPACE, "manifest-old");

    let prepared = composition
        .stage_upgrade(
            &mut host,
            &mut journal,
            &op(),
            &package.request("tx-live", "manifest-old"),
        )
        .expect("staging MUST NOT consult the session gate");
    assert_eq!(journal_step(&journal, "tx-live"), UpgradeStep::Prepared);

    let verdict = composition
        .activate_staged(&mut host, &mut journal, &op(), &prepared)
        .unwrap();
    assert_eq!(
        verdict,
        UpgradeActivation::Deferred { active: 1 },
        "an on-air session must defer the swap, not fail it"
    );

    assert_eq!(
        host.counts(),
        STAGED_ONLY,
        "a deferred swap is a swap that did not happen"
    );
    assert!(host.swaps().is_empty(), "the manifest was not swapped");
    assert_eq!(host.current_manifest(), "manifest-old");
    assert_eq!(
        journal_step(&journal, "tx-live"),
        UpgradeStep::Prepared,
        "the journal still reads `prepared`: nothing was activated"
    );
    assert_eq!(
        composition.session_lock().unwrap().snapshot().lifecycle,
        SessionLifecycle::Running as i32,
        "the live show keeps running"
    );
}

/// The deferred package is what the **next silent boundary** uses. Re-staging it
/// would materialise the same bytes a second time, and a second swap is exactly
/// what the gate exists to prevent.
///
/// The "staged once" half is a lock on a property that already holds by
/// construction — the activation boundary is handed no package bytes, so it
/// *cannot* stage — while the deferral itself is what the injections reddened.
#[test]
fn the_deferred_package_is_swapped_at_the_next_silent_boundary_without_restaging() {
    let dir = tempdir().unwrap();
    let composition = on_air(dir.path());
    let (package, mut host, mut journal) = demo(SPACE, "manifest-old");

    let prepared = composition
        .stage_upgrade(
            &mut host,
            &mut journal,
            &op(),
            &package.request("tx", "manifest-old"),
        )
        .unwrap();
    let deferred = composition
        .activate_staged(&mut host, &mut journal, &op(), &prepared)
        .unwrap();
    assert!(
        matches!(deferred, UpgradeActivation::Deferred { .. }),
        "the show is still on air"
    );

    // The show ends.
    end_show(&composition);
    let verdict = composition
        .activate_staged(&mut host, &mut journal, &op(), &prepared)
        .unwrap();
    assert_eq!(verdict, UpgradeActivation::Ran(Outcome::Committed));

    let counts = host.counts();
    assert_eq!(
        counts.stage, 1,
        "the deferred package MUST be reused, not staged a second time"
    );
    assert_eq!(counts.commit, 1, "exactly one swap");
    assert_eq!(counts.rollback, 0);
    assert_eq!(host.swaps().to_vec(), vec![prepared.to_manifest.clone()]);
    assert_eq!(host.current_manifest(), prepared.to_manifest);
    assert_eq!(journal_step(&journal, "tx"), UpgradeStep::Committed);
}

/// The unobservable case, end to end at the call site: a busy session lock must
/// not read as "no live session" — that would swap the version out from under a
/// live show — and it must not block the staging either, because staging needs
/// no silence. A lock the root cannot take is **reported**, never degraded.
#[test]
fn an_unobservable_session_fails_the_swap_but_not_the_staging() {
    let dir = tempdir().unwrap();
    let composition = on_air(dir.path());
    let (package, mut host, mut journal) = demo(SPACE, "manifest-old");

    // A handshake, not a sleep: the contention is provably in place before the
    // call site is entered, so this cannot race the scheduler.
    let (holder, held) = hold_session(&composition, Duration::from_millis(300));
    held.recv().unwrap();

    let prepared = composition
        .stage_upgrade(
            &mut host,
            &mut journal,
            &op(),
            &package.request("tx", "manifest-old"),
        )
        .expect("staging touches no session state, so a held lock cannot stop it");
    let error = composition
        .activate_staged(&mut host, &mut journal, &op(), &prepared)
        .expect_err("an unobservable session MUST NOT be read as 'no live session'");
    assert_eq!(error.code, ErrorCode::Conflict);

    holder.join().unwrap();

    // Fail-closed means nothing happened, and the package is still usable.
    assert_eq!(host.counts(), STAGED_ONLY);
    assert!(host.swaps().is_empty());
    assert_eq!(journal_step(&journal, "tx"), UpgradeStep::Prepared);
}

/// `Ran` says the boundary **ran**; `Outcome` says how it ended. A failed swap
/// converges to an explicitly reported rollback, and calling that "waiting"
/// would drop the cause — and the exported writes — and promise a retry that can
/// never fix it.
#[test]
fn a_failed_swap_reports_a_rollback_rather_than_a_wait() {
    let dir = tempdir().unwrap();
    let composition = assembled(dir.path());
    let package = RehearsalPackage::new();
    let mut host = package
        .host(Probe::Measured(SPACE), "manifest-old")
        .fail_at(FaultPoint::Migrate)
        .with_post_upgrade_writes(3, 7);
    let mut journal = RehearsalJournal::new();

    let prepared = composition
        .stage_upgrade(
            &mut host,
            &mut journal,
            &op(),
            &package.request("tx", "manifest-old"),
        )
        .unwrap();
    let verdict = composition
        .activate_staged(&mut host, &mut journal, &op(), &prepared)
        .unwrap();

    let UpgradeActivation::Ran(Outcome::RolledBack(report)) = verdict else {
        panic!("a failed swap is a reported rollback, not a deferral: {verdict:?}");
    };
    assert_eq!(report.cause_code, ErrorCode::Conflict);
    assert_eq!(
        report.exported,
        ExportedWrites { facts: 3, audit: 7 },
        "a rollback reports what the host exported, so the writes are not lost"
    );
    assert_eq!(host.counts().rollback, 1, "the rollback really ran");
    assert_eq!(host.counts().commit, 0);
    assert!(
        host.swaps().is_empty(),
        "a rolled-back run MUST NOT have swapped the manifest"
    );
    assert_eq!(host.current_manifest(), "manifest-old");
}

/// The call site MUST inherit the gate's definition of "on air" instead of
/// re-deciding it, because a second definition is a second thing to keep true.
/// Two states pin the two ways of getting it wrong: `Draining` (a live show
/// winding down) still defers — so the call site is not comparing against
/// `Running` alone — and `Ready` (prepared but never on air) activates — so it is
/// not comparing against `Stopped` alone either.
#[test]
fn the_call_site_inherits_the_gates_definition_of_on_air() {
    let winding_down = tempdir().unwrap();
    let composition = on_air(winding_down.path());
    to_lifecycle(&composition, SessionLifecycle::Draining);
    let (winding, mut host, mut journal) = demo(SPACE, "manifest-old");
    let prepared = composition
        .stage_upgrade(
            &mut host,
            &mut journal,
            &op(),
            &winding.request("tx", "manifest-old"),
        )
        .unwrap();
    assert_eq!(
        composition
            .activate_staged(&mut host, &mut journal, &op(), &prepared)
            .unwrap(),
        UpgradeActivation::Deferred { active: 1 },
        "a show winding down is still a live show"
    );

    let not_yet_on_air = tempdir().unwrap();
    let composition = assembled(not_yet_on_air.path());
    to_lifecycle(&composition, SessionLifecycle::Preparing);
    to_lifecycle(&composition, SessionLifecycle::Ready);
    let (ready, mut host, mut journal) = demo(SPACE, "manifest-old");
    let prepared = composition
        .stage_upgrade(
            &mut host,
            &mut journal,
            &op(),
            &ready.request("tx", "manifest-old"),
        )
        .unwrap();
    assert_eq!(
        composition
            .activate_staged(&mut host, &mut journal, &op(), &prepared)
            .unwrap(),
        UpgradeActivation::Ran(Outcome::Committed),
        "a session that is prepared but not on air must not hold the swap back"
    );
}
