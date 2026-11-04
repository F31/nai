use nai_contracts::v1::*;
use nai_media_core::*;
use std::time::{Duration, Instant};

fn fixture() -> (PlaybackLease, PerformanceCue, Approval, Vec<RevisionRef>) {
    let facts = vec![RevisionRef {
        object_id: "price".into(),
        revision: "1".into(),
    }];
    let mut cue = PerformanceCue {
        session_id: "s".into(),
        utterance_id: "u".into(),
        generation_epoch: 1,
        candidate_hash: "trusted-canonical-hash".into(),
        hash_algorithm: "sha256-canonical-v1".into(),
        approval_ref: "a".into(),
        policy_version: "p".into(),
        fact_versions: [("price".into(), "1".into())].into(),
        expires_at_utc_ms: 2000,
        ..Default::default()
    };
    cue.candidate_hash = nai_contracts::candidate_hash(&cue);
    let approval = Approval {
        approval_id: "a".into(),
        session_id: "s".into(),
        generation_epoch: 1,
        candidate_hash: cue.candidate_hash.clone(),
        hash_algorithm: cue.hash_algorithm.clone(),
        actor_id: "operator".into(),
        action_scope: "speak".into(),
        expires_at_utc_ms: 2000,
        fact_versions: facts.clone(),
        ..Default::default()
    };
    let lease = PlaybackLease {
        lease_id: "lease".into(),
        session_id: "s".into(),
        generation_epoch: 1,
        allowed_utterance_ids: vec!["u".into()],
        mode: SessionMode::Assisted as i32,
        approval_refs: vec!["a".into()],
        expires_at_utc_ms: 1600,
        fact_versions: facts.clone(),
        candidate_hashes: [("u".into(), cue.candidate_hash.clone())].into(),
        policy_revision: "p".into(),
    };
    (lease, cue, approval, facts)
}
fn proof<'a>(
    cue: &'a PerformanceCue,
    approval: &'a Approval,
    facts: &'a [RevisionRef],
) -> Preplay<'a> {
    Preplay {
        cue,
        approval,
        facts,
        mode: SessionMode::Assisted,
        guard_passed: true,
        permission_granted: true,
        supervision_until_utc_ms: None,
    }
}

#[test]
fn p0_is_independent_of_business_lock_and_cannot_be_undone_by_renewal() {
    let (lease, cue, approval, facts) = fixture();
    let now = Instant::now();
    let mut gate = MediaGate::new("s", 2);
    gate.arm(lease.clone(), 1000, now).unwrap();
    gate.enqueue(&proof(&cue, &approval, &facts), 1000, now)
        .unwrap();
    let ordinary = std::sync::Mutex::new(());
    let _blocked = ordinary.lock().unwrap();
    let emergency = gate.emergency();
    std::thread::spawn(move || emergency.mute()).join().unwrap();
    assert_eq!(
        gate.consume(&proof(&cue, &approval, &facts), 1000, now),
        Err(MediaError::Muted)
    );
    assert_eq!(gate.queued(), 0);
    assert!(gate.renew(lease.clone(), 1000, now).is_err());
    assert_eq!(gate.arm(lease, 1000, now), Err(MediaError::Stale));
}

#[test]
fn engine_stall_expires_on_monotonic_clock_even_if_wall_clock_reverses() {
    let (lease, cue, approval, facts) = fixture();
    let now = Instant::now();
    let mut gate = MediaGate::new("s", 1);
    gate.arm(lease.clone(), 1000, now).unwrap();
    gate.enqueue(&proof(&cue, &approval, &facts), 1000, now)
        .unwrap();
    assert_eq!(
        gate.consume(
            &proof(&cue, &approval, &facts),
            500,
            now + Duration::from_millis(600)
        ),
        Err(MediaError::Expired)
    );
    assert_eq!(gate.queued(), 0);
    assert!(gate.renew(lease, 500, now).is_err());
}

#[test]
fn every_consumption_rechecks_hash_facts_approval_permission_and_guard() {
    let (lease, cue, approval, facts) = fixture();
    let now = Instant::now();
    let mut gate = MediaGate::new("s", 2);
    gate.arm(lease, 1000, now).unwrap();
    gate.enqueue(&proof(&cue, &approval, &facts), 1000, now)
        .unwrap();
    let mut changed = cue.clone();
    changed.speech_text = "altered text with unchanged hash".into();
    assert!(
        gate.consume(&proof(&changed, &approval, &facts), 1000, now)
            .is_err()
    );
    changed = cue.clone();
    changed.candidate_hash = "different".into();
    assert!(
        gate.consume(&proof(&changed, &approval, &facts), 1000, now)
            .is_err()
    );
    changed = cue.clone();
    changed.session_id = "other-session".into();
    assert_eq!(
        gate.consume(&proof(&changed, &approval, &facts), 1000, now),
        Err(MediaError::Stale)
    );
    changed = cue.clone();
    changed.generation_epoch = 0;
    assert!(
        gate.consume(&proof(&changed, &approval, &facts), 1000, now)
            .is_err()
    );
    let mut revoked = approval.clone();
    revoked.revoked_at_utc_ms = Some(1000);
    assert!(
        gate.consume(&proof(&cue, &revoked, &facts), 1000, now)
            .is_err()
    );
    let mut revised = facts.clone();
    revised[0].revision = "2".into();
    assert!(
        gate.consume(&proof(&cue, &approval, &revised), 1000, now)
            .is_err()
    );
    let mut denied = proof(&cue, &approval, &facts);
    denied.guard_passed = false;
    assert!(gate.consume(&denied, 1000, now).is_err());
    denied.guard_passed = true;
    denied.permission_granted = false;
    assert!(gate.consume(&denied, 1000, now).is_err());
    assert_eq!(
        gate.consume(&proof(&cue, &approval, &facts), 1000, now),
        Ok("u".into())
    );
}

#[test]
fn queue_bound_and_supervision_are_enforced() {
    let (mut lease, cue, approval, facts) = fixture();
    lease.mode = SessionMode::AutoSupervised as i32;
    let now = Instant::now();
    let mut gate = MediaGate::new("s", 1);
    gate.arm(lease, 1000, now).unwrap();
    let mut p = proof(&cue, &approval, &facts);
    p.mode = SessionMode::AutoSupervised;
    assert_eq!(gate.enqueue(&p, 1000, now), Err(MediaError::Unauthorized));
    p.supervision_until_utc_ms = Some(1100);
    gate.enqueue(&p, 1000, now).unwrap();
    assert_eq!(gate.enqueue(&p, 1000, now), Err(MediaError::Capacity));
    assert!(
        gate.consume(&p, 1100, now + Duration::from_millis(100))
            .is_err()
    );
}

#[test]
fn renewal_preserves_identity_and_new_epoch_is_required_after_mute() {
    let (mut lease, mut cue, mut approval, facts) = fixture();
    let now = Instant::now();
    let mut gate = MediaGate::new("s", 1);
    gate.arm(lease.clone(), 1000, now).unwrap();
    lease.expires_at_utc_ms = 1700;
    gate.renew(lease.clone(), 1200, now + Duration::from_millis(200))
        .unwrap();
    assert_eq!(
        gate.renew(lease.clone(), 1200, now + Duration::from_millis(200)),
        Err(MediaError::Stale)
    );
    let mut changed = lease.clone();
    changed.policy_revision = "q".into();
    assert!(gate.renew(changed, 1200, now).is_err());
    gate.emergency().mute();
    gate.poll(now).unwrap_err();
    lease.generation_epoch = 2;
    cue.generation_epoch = 2;
    approval.generation_epoch = 2;
    cue.candidate_hash = nai_contracts::candidate_hash(&cue);
    approval.candidate_hash = cue.candidate_hash.clone();
    lease
        .candidate_hashes
        .insert("u".into(), cue.candidate_hash.clone());
    gate.arm(lease, 1200, now).unwrap();
    gate.enqueue(&proof(&cue, &approval, &facts), 1200, now)
        .unwrap();
}

#[test]
fn completion_requires_actual_matching_ack_and_cancelled_tasks_never_complete() {
    let mut progress = PlaybackProgress::new("out", "u", 48000);
    let mut ack = OutputAck {
        output_id: "out".into(),
        utterance_id: "u".into(),
        committed: true,
        ..Default::default()
    };
    assert_eq!(progress.acknowledge(&ack), Ok(false));
    ack.actual_local_playback_completed = true;
    assert_eq!(progress.acknowledge(&ack), Err(MediaError::InvalidAck));
    ack.played_sample_count = 48000;
    ack.output_id = "other".into();
    assert!(progress.acknowledge(&ack).is_err());
    ack.output_id = "out".into();
    assert_eq!(progress.acknowledge(&ack), Ok(true));
    let mut cancelled = PlaybackProgress::new("out", "u", 48000);
    cancelled.cancel();
    assert_eq!(cancelled.acknowledge(&ack), Err(MediaError::InvalidAck));
}

#[test]
fn arm_declares_the_recoverable_row_of_every_admitted_utterance() {
    let (lease, _cue, _approval, _facts) = fixture();
    let now = Instant::now();
    let mut gate = MediaGate::new("s", 2);
    // A cancellation that ran and named nothing is reported as an empty
    // manifest, not skipped: `None` means no cancellation ran at all.
    assert!(gate.take_cancellation().is_none());
    gate.arm(lease.clone(), 1000, now).unwrap();
    let first = gate
        .take_cancellation()
        .expect("arm reports a cancellation");
    assert_eq!(first.count(), 0);
    assert!(first.is_empty());
    let row = LineageId::new("s", "u", 1);
    assert_eq!(
        gate.lineage()
            .node(&row, FacetKind::UtteranceRow)
            .unwrap()
            .state,
        NodeState::InFlight
    );
    assert_eq!(gate.lineage().in_flight(), 1);
    assert_eq!(gate.lineage().len(), lease.allowed_utterance_ids.len());
}

#[test]
fn rearming_names_the_generation_it_dropped() {
    let (mut lease, _cue, _approval, _facts) = fixture();
    let now = Instant::now();
    let mut gate = MediaGate::new("s", 2);
    gate.arm(lease.clone(), 1000, now).unwrap();
    lease.generation_epoch = 2;
    lease.expires_at_utc_ms = 20_600;
    // The clock is past the previous generation's recorded grace, so the old
    // row is both named by the cancellation and taken by the reclamation.
    gate.arm(lease, 20_000, now).unwrap();
    let dropped = gate.take_cancellation().expect("rearm reports a cascade");
    assert_eq!(dropped.count(), 1);
    assert_eq!(dropped.manifest.len(), dropped.count());
    assert_eq!(dropped.manifest[0].id, LineageId::new("s", "u", 1));
    assert_eq!(dropped.manifest[0].kind, FacetKind::UtteranceRow);
    assert_eq!(dropped.manifest[0].state, NodeState::Cancelled);
    // Only the newest generation is still held.
    assert_eq!(gate.lineage().len(), 1);
    assert_eq!(gate.lineage().scope_epoch(), 2);
}

#[test]
fn a_mute_names_what_it_dropped_rather_than_reporting_a_number() {
    let (lease, cue, approval, facts) = fixture();
    let now = Instant::now();
    let mut gate = MediaGate::new("s", 2);
    gate.arm(lease, 1000, now).unwrap();
    gate.enqueue(&proof(&cue, &approval, &facts), 1000, now)
        .unwrap();
    assert_eq!(gate.lineage().in_flight(), 2);
    gate.emergency().mute();
    assert_eq!(gate.poll(now), Err(MediaError::Muted));
    let dropped = gate.take_cancellation().expect("mute reports a cascade");
    assert_eq!(dropped.count(), 2);
    assert_eq!(dropped.manifest.len(), dropped.count());
    let row = LineageId::new("s", "u", 1);
    assert!(
        dropped
            .manifest
            .iter()
            .any(|r| r.id == row && r.kind == FacetKind::UtteranceRow)
    );
    assert!(
        dropped
            .manifest
            .iter()
            .any(|r| r.id == row && r.kind == FacetKind::PendingGate)
    );
    assert_eq!(gate.queued(), 0);
    assert_eq!(gate.lineage().in_flight(), 0);
}

#[test]
fn enqueue_registers_the_gate_child_and_consume_releases_it() {
    let (lease, cue, approval, facts) = fixture();
    let now = Instant::now();
    let mut gate = MediaGate::new("s", 2);
    gate.arm(lease, 1000, now).unwrap();
    gate.enqueue(&proof(&cue, &approval, &facts), 1000, now)
        .unwrap();
    let row = LineageId::new("s", "u", 1);
    assert_eq!(
        gate.lineage()
            .node(&row, FacetKind::PendingGate)
            .unwrap()
            .state,
        NodeState::InFlight
    );
    assert_eq!(
        gate.consume(&proof(&cue, &approval, &facts), 1000, now),
        Ok("u".into())
    );
    assert_eq!(
        gate.lineage()
            .node(&row, FacetKind::PendingGate)
            .unwrap()
            .state,
        NodeState::Released
    );
    assert_eq!(gate.queued(), 0);
}

#[test]
fn mute_precedes_epoch_cancellation_even_when_command_ledger_is_full() {
    use nai_kernel::session::{CommandLedger, Session, SessionError};
    let now = Instant::now();
    let mut session = Session::new("s", "w").unwrap();
    for state in [
        SessionLifecycle::Preparing,
        SessionLifecycle::Ready,
        SessionLifecycle::Running,
    ] {
        session.transition(true, state, true).unwrap();
    }
    let task = session.task(now + Duration::from_secs(1)).unwrap();
    let (lease, cue, approval, facts) = fixture();
    let mut gate = MediaGate::new("s", 1);
    gate.arm(lease, 1000, now).unwrap();
    gate.enqueue(&proof(&cue, &approval, &facts), 1000, now)
        .unwrap();
    let mut ledger = CommandLedger::new(0);
    // Trusted P0 route signals media before ordinary admission or persistence.
    gate.emergency().mute();
    assert_eq!(
        ledger.execute(
            "mute",
            b"mute",
            now,
            now + Duration::from_secs(1),
            || Ok(())
        ),
        Err(SessionError::Capacity)
    );
    assert!(task.valid(now)); // engine has not yet advanced its epoch
    assert_eq!(
        gate.consume(&proof(&cue, &approval, &facts), 1000, now),
        Err(MediaError::Muted)
    );
    session.revoke(true).unwrap();
    assert!(!task.valid(now));
}

fn callback_fixture(now: Instant) -> (MediaGate, device_callback::DeviceChunk) {
    let (lease, cue, approval, facts) = fixture();
    let mut gate = MediaGate::new("s", 2);
    gate.arm(lease, 1000, now).unwrap();
    let p = proof(&cue, &approval, &facts);
    gate.enqueue(&p, 1000, now).unwrap();
    let chunk = gate
        .device_chunk(&p, vec![0.25; 480], 48000, 1, 1000, now)
        .unwrap();
    (gate, chunk)
}

#[test]
fn callback_copies_bounded_pcm_and_zeroes_tail_without_device_ack() {
    let now = Instant::now();
    let (_gate, mut chunk) = callback_fixture(now);
    let mut output = [9.0; 480];
    assert_eq!(chunk.fill(&mut output, now), 480);
    assert!(output.iter().all(|v| *v == 0.25));
    assert_eq!(chunk.fill(&mut output, now), 0);
    assert_eq!(output, [0.0; 480]);
}

#[test]
fn callback_p0_does_not_need_gate_poll_or_business_lock() {
    let now = Instant::now();
    let (gate, mut chunk) = callback_fixture(now);
    let held = std::sync::Mutex::new(gate);
    let lock = held.lock().unwrap();
    let emergency = lock.emergency();
    std::thread::spawn(move || emergency.mute()).join().unwrap();
    let mut out = [1.0; 48];
    assert_eq!(chunk.fill(&mut out, now), 0);
    assert_eq!(out, [0.0; 48]);
}

#[test]
fn callback_owner_drop_and_rearm_invalidate_old_audio() {
    let now = Instant::now();
    let (mut gate, mut old) = callback_fixture(now);
    let (mut lease, _, _, _) = fixture();
    lease.generation_epoch = 2;
    gate.arm(lease, 1000, now).unwrap();
    let mut out = [1.0; 48];
    assert_eq!(old.fill(&mut out, now), 0);
    let (gate, mut chunk) = callback_fixture(now);
    drop(gate);
    assert_eq!(chunk.fill(&mut out, now), 0);
    assert_eq!(out, [0.0; 48]);
}

#[test]
fn callback_stall_expires_without_poll_and_reserves_buffer_duration() {
    let now = Instant::now();
    let (_gate, mut chunk) = callback_fixture(now);
    let mut out = [1.0; 480];
    assert_eq!(chunk.fill(&mut out, now + Duration::from_millis(590)), 0);
    assert_eq!(out, [0.0; 480]);
    assert_eq!(chunk.fill(&mut out, now), 0); // permanently stopped
}

#[test]
fn callback_rechecks_approval_and_rejects_invalid_pcm_without_consuming() {
    let now = Instant::now();
    let (lease, cue, mut approval, facts) = fixture();
    let mut gate = MediaGate::new("s", 2);
    gate.arm(lease, 1000, now).unwrap();
    gate.enqueue(&proof(&cue, &approval, &facts), 1000, now)
        .unwrap();
    for pcm in [vec![f32::NAN], vec![1.1], vec![0.0; 28801], vec![]] {
        assert!(
            gate.device_chunk(&proof(&cue, &approval, &facts), pcm, 48000, 1, 1000, now)
                .is_err()
        );
        assert_eq!(gate.queued(), 1);
    }
    approval.revoked_at_utc_ms = Some(1000);
    assert!(
        gate.device_chunk(
            &proof(&cue, &approval, &facts),
            vec![0.1; 48],
            48000,
            1,
            1000,
            now
        )
        .is_err()
    );
    assert_eq!(gate.queued(), 1);
}

#[test]
fn callback_approval_expiry_precedes_lease_and_oversize_is_silent() {
    let now = Instant::now();
    let (lease, cue, mut approval, facts) = fixture();
    approval.expires_at_utc_ms = 1010;
    let mut gate = MediaGate::new("s", 2);
    gate.arm(lease, 1000, now).unwrap();
    let p = proof(&cue, &approval, &facts);
    gate.enqueue(&p, 1000, now).unwrap();
    let mut chunk = gate
        .device_chunk(&p, vec![0.25; 480], 48000, 1, 1000, now)
        .unwrap();
    assert_eq!(chunk.fill(&mut [1.0; 480], now), 0);
    let (_gate, mut chunk) = callback_fixture(now);
    let mut oversized = [1.0; 481];
    assert_eq!(chunk.fill(&mut oversized, now), 0);
    assert!(oversized.iter().all(|v| *v == 0.0));
}

#[test]
fn callback_lease_renewal_does_not_extend_previously_admitted_chunk() {
    let now = Instant::now();
    let (mut gate, mut chunk) = callback_fixture(now);
    let (mut lease, _, _, _) = fixture();
    lease.expires_at_utc_ms = 1700;
    gate.renew(lease, 1100, now + Duration::from_millis(100))
        .unwrap();
    let mut out = [1.0; 48];
    assert_eq!(chunk.fill(&mut out, now + Duration::from_millis(600)), 0);
    assert_eq!(out, [0.0; 48]);
}

#[test]
fn callback_stereo_tail_is_zero_and_foreign_gate_mute_is_isolated() {
    let now = Instant::now();
    let (lease, cue, approval, facts) = fixture();
    let mut gate = MediaGate::new("s", 2);
    gate.arm(lease, 1000, now).unwrap();
    let p = proof(&cue, &approval, &facts);
    gate.enqueue(&p, 1000, now).unwrap();
    let mut chunk = gate
        .device_chunk(&p, vec![0.1, -0.1], 48000, 2, 1000, now)
        .unwrap();
    let (other, _other_chunk) = callback_fixture(now);
    other.emergency().mute();
    let mut out = [1.0; 4];
    assert_eq!(chunk.fill(&mut out, now), 2);
    assert_eq!(out, [0.1, -0.1, 0.0, 0.0]);
}
