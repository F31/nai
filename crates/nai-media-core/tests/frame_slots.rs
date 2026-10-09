use nai_contracts::v1::{ErrorCode, FrameMetadata, FrameSlotCommit};
use nai_media_core::frame_slots::{MAX_BUDGET, PoolSpec, SlotOwner, SlotPhase};
use std::time::{Duration, Instant};

fn spec() -> PoolSpec {
    PoolSpec {
        pool_id: "pool-root-1".into(),
        session_id: "s".into(),
        generation_epoch: 7,
        producer_ref: "worker-generation-1".into(),
        consumer_ref: "keeper-generation-1".into(),
        slots: 1,
        slot_bytes: 8,
    }
}
fn frame() -> FrameMetadata {
    FrameMetadata {
        session_id: "s".into(),
        utterance_id: "u".into(),
        generation_epoch: 7,
        width: 2,
        height: 1,
        pixel_format: "rgba8".into(),
        pts: 0,
        time_base_num: 1,
        time_base_den: 8000,
        duration: 320,
        ..Default::default()
    }
}
fn commit(ticket: nai_contracts::v1::FrameSlotTicket) -> FrameSlotCommit {
    FrameSlotCommit {
        ticket: Some(ticket),
        byte_length: 8,
        sha256: "a".repeat(64),
    }
}

#[test]
fn reader_hold_blocks_overwrite_and_generation_fences_aba() {
    let now = Instant::now();
    let mut owner = SlotOwner::new(spec()).unwrap();
    let first = owner
        .reserve("worker-generation-1", frame(), MAX_BUDGET, now)
        .unwrap();
    assert_eq!(owner.phase(0).unwrap(), SlotPhase::Writing);
    assert_eq!(
        owner
            .acquire("keeper-generation-1", &first, now)
            .unwrap_err()
            .code,
        ErrorCode::StaleEpoch as i32
    );
    owner
        .commit("worker-generation-1", commit(first.clone()), now)
        .unwrap();
    assert!(
        owner
            .commit("worker-generation-1", commit(first.clone()), now)
            .is_err()
    );
    owner.acquire("keeper-generation-1", &first, now).unwrap();
    assert_eq!(owner.phase(0).unwrap(), SlotPhase::Reading);
    assert_eq!(
        owner
            .reserve("worker-generation-1", frame(), MAX_BUDGET, now)
            .unwrap_err()
            .code,
        ErrorCode::ResourceExhausted as i32
    );
    owner.release("keeper-generation-1", &first, now).unwrap();
    assert!(owner.release("keeper-generation-1", &first, now).is_err());
    let second = owner
        .reserve("worker-generation-1", frame(), MAX_BUDGET, now)
        .unwrap();
    assert_eq!(second.slot_generation, first.slot_generation + 1);
    assert!(
        owner
            .commit("worker-generation-1", commit(first), now)
            .is_err()
    );
    owner
        .commit("worker-generation-1", commit(second), now)
        .unwrap();
}

#[test]
fn forged_identity_metadata_length_scope_and_unknown_roles_do_not_advance() {
    let now = Instant::now();
    let mut owner = SlotOwner::new(spec()).unwrap();
    assert_eq!(
        owner
            .reserve("alien", frame(), MAX_BUDGET, now)
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied as i32
    );
    let mut foreign = frame();
    foreign.generation_epoch = 8;
    assert!(
        owner
            .reserve("worker-generation-1", foreign, MAX_BUDGET, now)
            .is_err()
    );
    let ticket = owner
        .reserve("worker-generation-1", frame(), MAX_BUDGET, now)
        .unwrap();
    for kind in 0..5 {
        let mut bad = commit(ticket.clone());
        match kind {
            0 => bad.ticket.as_mut().unwrap().pool_id = "other-pool".into(),
            1 => bad.ticket.as_mut().unwrap().frame.as_mut().unwrap().pts = 1,
            2 => bad.ticket.as_mut().unwrap().slot_index = 15,
            3 => bad.byte_length = 7,
            _ => bad.sha256 = "G".repeat(64),
        }
        assert!(owner.commit("worker-generation-1", bad, now).is_err());
        assert_eq!(owner.phase(0).unwrap(), SlotPhase::Writing);
    }
    assert!(owner.commit("alien", commit(ticket.clone()), now).is_err());
    owner
        .commit("worker-generation-1", commit(ticket.clone()), now)
        .unwrap();
    assert!(owner.acquire("alien", &ticket, now).is_err());
    assert!(owner.release("keeper-generation-1", &ticket, now).is_err());
    assert!(owner.disconnect("alien").is_err());
    assert!(!owner.is_retired());
}

#[test]
fn deadline_or_disconnect_retires_the_whole_pool_without_recovery() {
    let now = Instant::now();
    let mut config = spec();
    config.slots = 2;
    let mut owner = SlotOwner::new(config).unwrap();
    let first = owner
        .reserve(
            "worker-generation-1",
            frame(),
            Duration::from_millis(10),
            now,
        )
        .unwrap();
    let second = owner
        .reserve("worker-generation-1", frame(), MAX_BUDGET, now)
        .unwrap();
    owner
        .commit("worker-generation-1", commit(second.clone()), now)
        .unwrap();
    owner.acquire("keeper-generation-1", &second, now).unwrap();
    assert_eq!(
        owner
            .poll(now + Duration::from_millis(10))
            .unwrap_err()
            .code,
        ErrorCode::DeadlineExceeded as i32
    );
    assert!(owner.is_retired());
    assert_eq!(owner.phase(1).unwrap(), SlotPhase::Retired);
    assert!(owner.release("keeper-generation-1", &second, now).is_err());
    assert!(
        owner
            .commit("worker-generation-1", commit(first), now)
            .is_err()
    );
    assert!(
        owner
            .reserve("worker-generation-1", frame(), MAX_BUDGET, now)
            .is_err()
    );
    let mut owner = SlotOwner::new(spec()).unwrap();
    owner.disconnect("keeper-generation-1").unwrap();
    assert!(owner.poll(now).is_err());
}

#[test]
fn invalid_bounds_and_frame_geometry_are_explicit() {
    for (slots, bytes) in [
        (0, 8),
        (17, 8),
        (16, 8 * 1024 * 1024),
        (1, 0),
        (1, 8 * 1024 * 1024 + 1),
    ] {
        let mut s = spec();
        s.slots = slots;
        s.slot_bytes = bytes;
        assert!(SlotOwner::new(s).is_err());
    }
    let mut owner = SlotOwner::new(spec()).unwrap();
    let now = Instant::now();
    for kind in 0..5 {
        let mut f = frame();
        match kind {
            0 => f.width = 0,
            1 => f.width = 1921,
            2 => f.pixel_format = "rgb".into(),
            3 => f.duration = 0,
            _ => f.pts = -1,
        };
        assert!(
            owner
                .reserve("worker-generation-1", f, MAX_BUDGET, now)
                .is_err()
        );
    }
    assert!(
        owner
            .reserve("worker-generation-1", frame(), Duration::ZERO, now)
            .is_err()
    );
    assert!(
        owner
            .reserve(
                "worker-generation-1",
                frame(),
                MAX_BUDGET + Duration::from_millis(1),
                now
            )
            .is_err()
    );
}

#[test]
fn both_slot_quantity_and_total_budget_have_a_hard_bound() {
    let mut config = spec();
    config.slots = 16;
    config.slot_bytes = 4 * 1024 * 1024;
    let mut owner = SlotOwner::new(config).unwrap();
    let now = Instant::now();
    for index in 0..16 {
        let ticket = owner
            .reserve("worker-generation-1", frame(), MAX_BUDGET, now)
            .unwrap();
        assert_eq!(ticket.slot_index, index);
    }
    assert!(
        owner
            .reserve("worker-generation-1", frame(), MAX_BUDGET, now)
            .is_err()
    );
}

#[test]
fn held_frame_owner_does_not_block_independent_p0_revocation() {
    use nai_contracts::v1::{PlaybackLease, SessionMode};
    use nai_media_core::{MediaError, MediaGate};
    use std::sync::{Arc, Mutex, mpsc};
    let now = Instant::now();
    let mut owner = SlotOwner::new(spec()).unwrap();
    owner
        .reserve("worker-generation-1", frame(), MAX_BUDGET, now)
        .unwrap();
    let owner = Arc::new(Mutex::new(owner));
    let (held_tx, held_rx) = mpsc::sync_channel(1);
    let (stop_tx, stop_rx) = mpsc::sync_channel(1);
    let task = std::thread::spawn(move || {
        let _held = owner.lock().unwrap();
        held_tx.send(()).unwrap();
        let _ = stop_rx.recv();
    });
    held_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let mut gate = MediaGate::new("s", 1);
    gate.arm(
        PlaybackLease {
            lease_id: "lease".into(),
            session_id: "s".into(),
            generation_epoch: 7,
            allowed_utterance_ids: vec!["u".into()],
            mode: SessionMode::Rehearsal as i32,
            expires_at_utc_ms: 1600,
            policy_revision: "policy".into(),
            ..Default::default()
        },
        1000,
        now,
    )
    .unwrap();
    gate.emergency().mute();
    assert_eq!(gate.poll(now), Err(MediaError::Muted));
    stop_tx.send(()).unwrap();
    task.join().unwrap();
}

#[cfg(not(target_os = "linux"))]
#[test]
fn unsupported_backend_is_explicitly_not_ready() {
    let result = nai_media_core::frame_slots::shared_ram::SharedFramePool::create(
        std::path::Path::new("unused"),
        spec(),
    );
    assert_eq!(
        result.err().unwrap().code,
        ErrorCode::CapabilityNotReady as i32
    );
}
