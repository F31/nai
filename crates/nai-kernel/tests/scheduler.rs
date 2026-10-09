use nai_contracts::v1::*;
use nai_kernel::scheduler::*;
use std::time::{Duration, Instant};
fn event(id: &str, kind: EventKind) -> PlatformEvent {
    PlatformEvent {
        event_id: id.into(),
        session_id: "s".into(),
        platform: "test".into(),
        app_scope: "app".into(),
        room_id: "r".into(),
        room_source_ref: "source".into(),
        viewer_scoped_id: "viewer".into(),
        source: EventSource::SourceReplay as i32,
        kind: kind as i32,
        expires_at_utc_ms: 10000,
        ..Default::default()
    }
}
fn scheduler() -> Scheduler {
    Scheduler::new(
        "s",
        1,
        Policy {
            capacity: 4,
            max_speech_ms: 3000,
            cooldown: Duration::from_millis(100),
            fairness_window: 2,
            max_viewer_share_percent: 50,
            starvation: Duration::from_secs(1),
        },
    )
    .unwrap()
}
fn candidate(id: &str, viewer: &str, priority: u8, now: Instant) -> Candidate {
    Candidate {
        id: id.into(),
        session: "s".into(),
        epoch: 1,
        viewer: viewer.into(),
        topic: id.into(),
        priority,
        speech_ms: 1000,
        deadline: now + Duration::from_secs(10),
    }
}

#[test]
fn ordinary_overload_does_not_block_critical_slots_or_hide_loss() {
    let mut q = EventIngress::new("s", 1, 2, 1).unwrap();
    q.push(event("1", EventKind::Comment), 1000).unwrap();
    assert_eq!(
        q.push(event("2", EventKind::Like), 1000),
        Err(ScheduleError::Capacity)
    );
    q.push(event("permission", EventKind::PermissionChanged), 1000)
        .unwrap();
    q.push(event("end", EventKind::RoomEnded), 1000).unwrap();
    assert_eq!(q.pop(1000).unwrap().kind, EventKind::RoomEnded as i32);
    assert_eq!(
        q.pop(1000).unwrap().kind,
        EventKind::PermissionChanged as i32
    );
    assert_eq!(q.metrics.overflow, 1);
    assert_eq!(
        q.push(event("1", EventKind::Comment), 1000),
        Ok(Ingest::Duplicate)
    );
}

#[test]
fn valuable_input_replaces_low_value_like_without_hiding_overflow() {
    let mut q = EventIngress::new("s", 1, 1, 3).unwrap();
    q.push(event("like", EventKind::Like), 1000).unwrap();
    q.push(event("question", EventKind::Comment), 1000).unwrap();
    assert_eq!(q.pop(1000).unwrap().kind, EventKind::Comment as i32);
    assert_eq!(q.metrics.dropped_likes, 1);
    assert_eq!(q.metrics.overflow, 1);
}
#[test]
fn gift_delta_cumulative_and_unknown_have_distinct_semantics() {
    for (semantics, expected) in [
        (QuantitySemantics::Delta, 5),
        (QuantitySemantics::Cumulative, 3),
    ] {
        let mut q = EventIngress::new("s", 1, 1, 8).unwrap();
        let mut a = event("1", EventKind::Gift);
        a.combo_id = "combo".into();
        a.quantity = Some(2);
        a.quantity_semantics = semantics as i32;
        let mut b = a.clone();
        b.event_id = "2".into();
        b.quantity = Some(3);
        q.push(a, 1000).unwrap();
        assert_eq!(q.push(b, 1000), Ok(Ingest::Merged));
        assert_eq!(q.pop(1000).unwrap().quantity, Some(expected));
    }
    let mut q = EventIngress::new("s", 2, 1, 8).unwrap();
    let mut a = event("1", EventKind::Gift);
    a.combo_id = "combo".into();
    a.quantity = Some(2);
    let mut b = a.clone();
    b.event_id = "2".into();
    q.push(a, 1000).unwrap();
    q.push(b, 1000).unwrap();
    assert_eq!(q.len(), 2);
    assert_eq!(
        q.pop(1000).unwrap().quantity_semantics,
        QuantitySemantics::QuantityUnknown as i32
    );
}
#[test]
fn expiry_scope_and_merge_never_extend_old_work_lifetime() {
    let now = Instant::now();
    let mut s = scheduler();
    let mut a = candidate("a", "v", 3, now);
    a.deadline = now + Duration::from_millis(1);
    s.push(a, now).unwrap();
    let mut b = candidate("b", "v", 3, now);
    b.topic = "a".into();
    assert_eq!(s.push(b, now), Ok(Some("a".into())));
    assert!(
        s.select::<()>(now + Duration::from_millis(1), |_| panic!(
            "expired was admitted"
        ))
        .unwrap()
        .is_none()
    );
    assert_eq!(s.metrics.expired, 1);
    let mut c = candidate("c", "v", 3, now);
    c.epoch = 0;
    assert_eq!(s.push(c, now), Err(ScheduleError::Scope));
    let mut q = EventIngress::new("s", 1, 1, 1).unwrap();
    let mut e = event("e", EventKind::Comment);
    e.expires_at_utc_ms = 1000;
    assert_eq!(q.push(e, 1000), Err(ScheduleError::Expired));
}
#[test]
fn only_the_selected_candidate_reserves_and_denial_keeps_it_queued() {
    let now = Instant::now();
    let mut s = scheduler();
    s.push(candidate("low", "v1", 5, now), now).unwrap();
    s.push(candidate("manual", "", 1, now), now).unwrap();
    let mut calls = 0;
    assert!(
        s.select::<()>(now, |c| {
            calls += 1;
            assert_eq!(c.id, "manual");
            Err(ScheduleError::Admission)
        })
        .is_err()
    );
    assert_eq!(s.pending(), 2);
    assert_eq!(s.debt_ms(), 0);
    let selected = s
        .select(now, |c| {
            calls += 1;
            assert_eq!(c.id, "manual");
            Ok("reservation")
        })
        .unwrap()
        .unwrap();
    assert_eq!(selected.reservation, "reservation");
    assert_eq!(calls, 2);
    assert_eq!(s.pending(), 1);
}
#[test]
fn cooldown_and_viewer_share_prevent_monopoly() {
    let now = Instant::now();
    let mut s = scheduler();
    for (n, time) in [("a1", now), ("a2", now + Duration::from_millis(101))] {
        s.push(candidate(n, "A", 3, time), time).unwrap();
        let picked = s.select(time, |_| Ok(())).unwrap().unwrap();
        s.finish(&picked.candidate.id).unwrap();
    }
    let later = now + Duration::from_millis(202);
    s.push(candidate("a3", "A", 3, later), later).unwrap();
    s.push(candidate("b1", "B", 5, later), later).unwrap();
    assert_eq!(
        s.select(later, |_| Ok(()))
            .unwrap()
            .unwrap()
            .candidate
            .viewer,
        "B"
    );
    s.push(candidate("b2", "B", 3, later), later).unwrap();
    let pick = s.select(later, |_| Ok(())).unwrap().unwrap();
    assert_eq!(pick.candidate.viewer, "A");
}
#[test]
fn aged_low_priority_can_overtake_new_questions() {
    let now = Instant::now();
    let mut s = scheduler();
    s.push(candidate("old", "low", 5, now), now).unwrap();
    let later = now + Duration::from_secs(1);
    s.push(candidate("new", "high", 3, later), later).unwrap();
    assert_eq!(
        s.select(later, |_| Ok(())).unwrap().unwrap().candidate.id,
        "old"
    );
}
#[test]
fn debt_and_ttl_block_more_work_until_actual_end_and_revoke_discards_epoch() {
    let now = Instant::now();
    let mut s = scheduler();
    let mut a = candidate("a", "", 1, now);
    a.speech_ms = 2500;
    s.push(a, now).unwrap();
    s.select(now, |_| Ok(())).unwrap();
    assert!(s.revise_speech("a", 4000).unwrap());
    s.push(candidate("b", "", 1, now), now).unwrap();
    assert!(
        s.select::<()>(now, |_| panic!("debt bypass"))
            .unwrap()
            .is_none()
    );
    assert_eq!(s.debt_ms(), 4000);
    s.finish("a").unwrap();
    assert!(s.select(now, |_| Ok(())).unwrap().is_some());
    assert_eq!(s.revoke(2).unwrap(), vec!["b".to_string()]);
    assert_eq!(s.debt_ms(), 0);
    assert_eq!(s.finish("b"), Err(ScheduleError::Conflict));
    assert_eq!(
        s.push(candidate("old", "", 1, now), now),
        Err(ScheduleError::Scope)
    );
}
