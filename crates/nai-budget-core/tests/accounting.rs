use nai_budget_core::*;
use nai_contracts::v1::*;

fn ceiling(cost: u64) -> Ceiling {
    Ceiling {
        cost_minor: cost,
        units: [("requests".into(), 100), ("tokens".into(), 1000)].into(),
        concurrent: [("provider_slots".into(), 2)].into(),
    }
}
fn limits(cost: u64) -> Limits {
    Limits {
        currency: "CNY".into(),
        global: ceiling(cost),
        sessions: [("a".into(), ceiling(100)), ("b".into(), ceiling(100))].into(),
        providers: [("p".into(), ceiling(100))].into(),
        max_entries: 10,
    }
}
fn charge(cost: u64) -> BudgetCharge {
    BudgetCharge {
        currency: "CNY".into(),
        cost_minor: cost,
        units: [("requests".into(), 1), ("tokens".into(), 10)].into(),
        price_revision: "price-1".into(),
    }
}
fn entry(session: &str, request: &str, cost: u64) -> UsageLedgerEntry {
    UsageLedgerEntry {
        workspace_id: "w".into(),
        session_id: session.into(),
        provider_ref: "p".into(),
        request_id: request.into(),
        generation_epoch: 1,
        estimate: Some(charge(cost)),
        concurrent: [("provider_slots".into(), 1)].into(),
        state: BillingState::Reserved as i32,
        ..Default::default()
    }
}

#[test]
fn two_sessions_cannot_spend_the_same_global_balance() {
    let mut b = Book::new("w", limits(10)).unwrap();
    let first = b.reserve(entry("a", "1", 7)).unwrap();
    assert_eq!(b.reserve(entry("b", "2", 7)), Err(BudgetError::Limit));
    assert_eq!(b.total().unwrap().cost_minor, 7);
    assert_eq!(b.entries().count(), 1);
    assert_eq!(b.reserve(entry("a", "1", 7)), Ok(first));
    assert_eq!(b.reserve(entry("b", "1", 7)), Err(BudgetError::Conflict));
}
#[test]
fn provider_session_units_and_concurrency_all_gate_atomically() {
    for (scope, dimension) in [
        ("global", "tokens"),
        ("session", "cost"),
        ("provider", "requests"),
        ("global", "slots"),
    ] {
        let mut l = limits(100);
        let c = match scope {
            "session" => l.sessions.get_mut("a").unwrap(),
            "provider" => l.providers.get_mut("p").unwrap(),
            _ => &mut l.global,
        };
        match dimension {
            "tokens" => {
                c.units.insert("tokens".into(), 9);
            }
            "requests" => {
                c.units.insert("requests".into(), 0);
            }
            "slots" => {
                c.concurrent.insert("provider_slots".into(), 0);
            }
            _ => c.cost_minor = 1,
        }
        let mut b = Book::new("w", l).unwrap();
        assert_eq!(b.reserve(entry("a", "1", 2)), Err(BudgetError::Limit));
        assert_eq!(b.total().unwrap(), Total::default());
    }
}
#[test]
fn dispatched_cancellation_keeps_estimate_and_busy_resources_until_end() {
    let mut b = Book::new("w", limits(100)).unwrap();
    let e = b.reserve(entry("a", "1", 7)).unwrap();
    let e = b.change("p", "1", e.revision, Change::Dispatch).unwrap();
    let e = b.change("p", "1", e.revision, Change::Cancel).unwrap();
    assert_eq!(b.total().unwrap().cost_minor, 7);
    assert_eq!(b.total().unwrap().concurrent["provider_slots"], 1);
    let e = b
        .change("p", "1", e.revision, Change::Finish(None))
        .unwrap();
    assert_eq!(e.state, BillingState::NeedsReconciliation as i32);
    assert_eq!(b.total().unwrap().cost_minor, 7);
    assert!(b.total().unwrap().concurrent.is_empty());
    let e = b
        .change(
            "p",
            "1",
            e.revision,
            Change::Reconcile {
                id: "bill-1".into(),
                actual: charge(9),
            },
        )
        .unwrap();
    assert_eq!(b.total().unwrap().cost_minor, 9);
    assert_eq!(
        b.change(
            "p",
            "1",
            0,
            Change::Reconcile {
                id: "bill-1".into(),
                actual: charge(9)
            }
        ),
        Ok(e)
    );
    assert_eq!(
        b.change(
            "p",
            "1",
            0,
            Change::Reconcile {
                id: "bill-1".into(),
                actual: charge(8)
            }
        ),
        Err(BudgetError::Conflict)
    );
}
#[test]
fn undispatched_cancel_releases_without_claiming_zero_billed_usage() {
    let mut b = Book::new("w", limits(10)).unwrap();
    let e = b.reserve(entry("a", "1", 7)).unwrap();
    let e = b.change("p", "1", e.revision, Change::Cancel).unwrap();
    assert_eq!(e.state, BillingState::Released as i32);
    assert!(e.actual.is_none());
    assert_eq!(b.total().unwrap(), Total::default());
    assert_eq!(
        b.change("p", "1", e.revision, Change::Dispatch),
        Err(BudgetError::Conflict)
    );
}
#[test]
fn actual_overestimate_is_recorded_once_and_blocks_new_work() {
    let mut b = Book::new("w", limits(10)).unwrap();
    let e = b.reserve(entry("a", "1", 7)).unwrap();
    let e = b.change("p", "1", e.revision, Change::Dispatch).unwrap();
    let e = b
        .change("p", "1", e.revision, Change::Finish(Some(charge(12))))
        .unwrap();
    assert_eq!(b.total().unwrap().cost_minor, 12);
    assert_eq!(
        b.change("p", "1", 0, Change::Finish(Some(charge(12)))),
        Ok(e)
    );
    assert_eq!(b.reserve(entry("b", "2", 1)), Err(BudgetError::Limit));
}
#[test]
fn unknown_scope_units_currency_and_stale_revision_do_not_mutate() {
    let mut b = Book::new("w", limits(100)).unwrap();
    assert_eq!(
        b.reserve(entry("unknown", "x", 1)),
        Err(BudgetError::UnknownScope)
    );
    let mut e = entry("a", "1", 1);
    e.estimate.as_mut().unwrap().currency = "USD".into();
    assert_eq!(b.reserve(e), Err(BudgetError::Invalid));
    let mut e = entry("a", "1", 1);
    e.estimate
        .as_mut()
        .unwrap()
        .units
        .insert("unmapped".into(), 1);
    assert_eq!(b.reserve(e), Err(BudgetError::UnknownUnit));
    let e = b.reserve(entry("a", "1", 1)).unwrap();
    assert_eq!(
        b.change("p", "1", 0, Change::Dispatch),
        Err(BudgetError::Conflict)
    );
    assert_eq!(b.get("p", "1"), Some(&e));
    let dispatched = b.change("p", "1", e.revision, Change::Dispatch).unwrap();
    let mut incomplete = charge(1);
    incomplete.units.remove("requests");
    assert_eq!(
        b.change(
            "p",
            "1",
            dispatched.revision,
            Change::Finish(Some(incomplete))
        ),
        Err(BudgetError::Invalid)
    );
    assert_eq!(b.get("p", "1"), Some(&dispatched));
}
#[test]
fn restoration_is_conservative_capacity_bounded_and_checked_for_overflow() {
    let mut l = limits(u64::MAX);
    l.global.cost_minor = u64::MAX;
    l.sessions.get_mut("a").unwrap().cost_minor = u64::MAX;
    l.providers.get_mut("p").unwrap().cost_minor = u64::MAX;
    let mut b = Book::new("w", l.clone()).unwrap();
    let e = b.reserve(entry("a", "1", u64::MAX)).unwrap();
    assert_eq!(b.reserve(entry("a", "2", 1)), Err(BudgetError::Overflow));
    let mut restored = Book::new("w", limits(1)).unwrap();
    restored.restore(e).unwrap();
    assert_eq!(restored.total().unwrap().cost_minor, u64::MAX);
    l.max_entries = 1;
    let mut small = Book::new("w", l).unwrap();
    small.reserve(entry("a", "1", 1)).unwrap();
    assert_eq!(
        small.reserve(entry("a", "2", 1)),
        Err(BudgetError::Capacity)
    );
}

#[test]
fn selected_work_is_the_only_request_reserved_and_denial_keeps_scheduler_atomic() {
    use nai_kernel::scheduler::*;
    use std::time::{Duration, Instant};
    let now = Instant::now();
    let mut book = Book::new("w", limits(10)).unwrap();
    let mut scheduler = Scheduler::new(
        "a",
        1,
        Policy {
            capacity: 2,
            max_speech_ms: 5000,
            cooldown: Duration::ZERO,
            fairness_window: 2,
            max_viewer_share_percent: 100,
            starvation: Duration::from_secs(1),
        },
    )
    .unwrap();
    for id in ["1", "2"] {
        scheduler
            .push(
                Candidate {
                    id: id.into(),
                    session: "a".into(),
                    epoch: 1,
                    viewer: id.into(),
                    topic: id.into(),
                    priority: 3,
                    speech_ms: 1000,
                    deadline: now + Duration::from_secs(10),
                },
                now,
            )
            .unwrap();
    }
    let first = scheduler
        .select(now, |c| {
            book.reserve(entry(&c.session, &c.id, 7))
                .map_err(|_| ScheduleError::Admission)
        })
        .unwrap()
        .unwrap();
    assert_eq!(book.entries().count(), 1);
    scheduler.finish(&first.candidate.id).unwrap();
    assert!(
        scheduler
            .select(now, |c| book
                .reserve(entry(&c.session, &c.id, 7))
                .map_err(|_| ScheduleError::Admission))
            .is_err()
    );
    assert_eq!(scheduler.pending(), 1);
    assert_eq!(scheduler.debt_ms(), 0);
    assert_eq!(book.entries().count(), 1);
    book.change("p", "1", first.reservation.revision, Change::Cancel)
        .unwrap();
    assert!(
        scheduler
            .select(now, |c| book
                .reserve(entry(&c.session, &c.id, 7))
                .map_err(|_| ScheduleError::Admission))
            .unwrap()
            .is_some()
    );
}

#[test]
fn batch_recovery_is_atomic_for_duplicate_and_overflow_snapshots() {
    let l = limits(100);
    let mut source = Book::new("w", l.clone()).unwrap();
    let first = source.reserve(entry("a", "1", 7)).unwrap();
    let second = source.reserve(entry("b", "2", 7)).unwrap();
    let mut recovered = Book::new("w", l.clone()).unwrap();
    assert_eq!(
        recovered.restore_all([first.clone(), first.clone()]),
        Err(BudgetError::Conflict)
    );
    assert_eq!(recovered.entries().count(), 0);
    recovered.restore_all([first.clone(), second]).unwrap();
    assert_eq!(recovered.total().unwrap().cost_minor, 14);
    let mut huge = first.clone();
    huge.estimate.as_mut().unwrap().cost_minor = u64::MAX;
    huge.request_id = "huge".into();
    let mut overflow = Book::new("w", l).unwrap();
    assert_eq!(
        overflow.restore_all([first, huge]),
        Err(BudgetError::Overflow)
    );
    assert_eq!(overflow.entries().count(), 0);
}
