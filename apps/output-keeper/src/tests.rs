use super::*;
use nai_contracts::v1::{FrameSlotEvent, PlaybackLease, frame_slot_event::Event};

fn command(seq: u64, epoch: u64, expires: u64) -> OutputWatchdogCommand {
    OutputWatchdogCommand {
        schema_version: 1,
        session_id: "session".into(),
        seq,
        remaining_budget_ms: 250,
        action: Some(Action::Arm(PlaybackLease {
            lease_id: "test-lease".into(),
            session_id: "session".into(),
            generation_epoch: epoch,
            allowed_utterance_ids: vec!["test-utterance".into()],
            mode: SessionMode::Rehearsal as i32,
            policy_revision: "test-only".into(),
            expires_at_utc_ms: expires,
            ..Default::default()
        })),
    }
}

#[test]
fn invalid_scope_budget_mode_and_sequence_never_grant_output() {
    let now = Instant::now();
    for variant in 0..7 {
        let mut watchdog = Watchdog::new("session", 0).unwrap();
        let mut input = command(1, 1, 1500);
        match variant {
            0 => input.session_id = "foreign".into(),
            1 => input.seq = 0,
            2 => input.remaining_budget_ms = 0,
            3 => input.remaining_budget_ms = 251,
            4 => input.action = None,
            5 => input.schema_version = 2,
            _ => {
                if let Some(Action::Arm(ref mut lease)) = input.action {
                    lease.mode = SessionMode::Assisted as i32;
                }
            }
        }
        assert!(watchdog.apply(input, now, now, 1000).is_err());
        let status = watchdog.status(0, 0);
        assert!(!status.lease_valid);
        assert!(!status.output_available);
        assert_eq!(
            status.backend_error.unwrap().code,
            ErrorCode::CapabilityNotReady as i32
        );
    }
    let mut watchdog = Watchdog::new("session", 0).unwrap();
    assert!(
        watchdog
            .apply(command(1, 1, 1500), now, now + FRAME_BUDGET, 1000)
            .is_err()
    );
    assert!(!watchdog.status(0, 0).lease_valid);
}

#[test]
fn expiry_and_recovered_epoch_floor_reject_old_authority() {
    let now = Instant::now();
    let mut watchdog = Watchdog::new("session", 4).unwrap();
    assert!(watchdog.apply(command(1, 4, 1500), now, now, 1000).is_err());
    watchdog.apply(command(1, 5, 1500), now, now, 1000).unwrap();
    assert!(watchdog.status(0, 0).lease_valid);
    assert!(!watchdog.status(0, 0).output_available);
    let later = now + Duration::from_millis(500);
    assert!(watchdog.poll(later));
    assert!(!watchdog.status(0, 0).lease_valid);
    let mut renewal = command(2, 5, 2000);
    if let Some(Action::Arm(lease)) = renewal.action.take() {
        renewal.action = Some(Action::Renew(lease));
    }
    assert!(watchdog.apply(renewal, later, later, 1500).is_err());
    assert!(
        watchdog
            .apply(command(2, 5, 2000), later, later, 1500)
            .is_err()
    );
    watchdog
        .apply(command(2, 6, 2000), later, later, 1500)
        .unwrap();
    watchdog.close(ErrorCode::Cancelled, later);
    assert!(
        watchdog
            .apply(command(3, 7, 2000), later, later, 1500)
            .is_err()
    );
}

#[test]
fn renewal_preserves_identity_and_duplicate_sequences_do_not_extend_lease() {
    let now = Instant::now();
    let mut watchdog = Watchdog::new("session", 0).unwrap();
    watchdog.apply(command(1, 1, 1400), now, now, 1000).unwrap();
    let later = now + Duration::from_millis(100);
    let mut renewal = command(2, 1, 1500);
    if let Some(Action::Arm(lease)) = renewal.action.take() {
        renewal.action = Some(Action::Renew(lease));
    }
    watchdog.apply(renewal.clone(), later, later, 1100).unwrap();
    assert!(watchdog.apply(renewal, later, later, 1100).is_err());
    assert_eq!(watchdog.status(0, 0).last_command_seq, 2);
    let mut wrong = command(3, 1, 1600);
    if let Some(Action::Arm(mut lease)) = wrong.action.take() {
        lease.policy_revision = "changed".into();
        wrong.action = Some(Action::Renew(lease));
    }
    assert!(watchdog.apply(wrong, later, later, 1100).is_err());
    watchdog.poll(now + Duration::from_millis(500));
    assert!(!watchdog.status(0, 0).lease_valid);
}

#[test]
fn frame_submission_without_frame_drill_is_rejected_and_never_presents() {
    let now = Instant::now();
    let mut watchdog = Watchdog::new("session", 0).unwrap();
    watchdog.apply(command(1, 1, 1500), now, now, 1000).unwrap();
    let submit = OutputWatchdogCommand {
        schema_version: 1,
        session_id: "session".into(),
        seq: 2,
        remaining_budget_ms: 250,
        action: Some(Action::SubmitFrame(FrameSlotEvent {
            event: Some(Event::WriteStarted(true)),
        })),
    };
    assert_eq!(
        watchdog.apply(submit, now, now, 1000).unwrap_err().code,
        ErrorCode::PermissionDenied as i32
    );
    let status = watchdog.status(0, 0);
    assert!(!status.presenting);
    assert_eq!(status.standby_reason, "not_ready");
    assert!(!status.output_available);
}

#[test]
fn latest_state_is_bounded_and_never_waits_for_a_held_writer_lock() {
    let latest = Latest::default();
    let watchdog = Watchdog::new("session", 0).unwrap();
    latest.publish(watchdog.status(0, 0));
    latest.publish(watchdog.status(0, 0));
    assert_eq!(latest.skipped.load(Ordering::Relaxed), 1);
    let held = latest.pending.lock().unwrap();
    latest.publish(watchdog.status(0, 0));
    assert_eq!(latest.skipped.load(Ordering::Relaxed), 2);
    assert!(held.is_some());
}

#[test]
fn blocked_output_write_does_not_block_watchdog_owner_stop() {
    struct HeldInput {
        bytes: Cursor<Vec<u8>>,
        stop: Arc<AtomicBool>,
    }
    impl Read for HeldInput {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let read = self.bytes.read(buffer)?;
            if read != 0 {
                return Ok(read);
            }
            while !self.stop.load(Ordering::Acquire) {
                thread::sleep(TICK);
            }
            Ok(0)
        }
    }
    struct HeldOutput {
        started: Option<mpsc::Sender<()>>,
        release: mpsc::Receiver<()>,
    }
    impl Write for HeldOutput {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if let Some(started) = self.started.take() {
                started.send(()).unwrap();
                self.release.recv().unwrap();
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let stop = Arc::new(AtomicBool::new(false));
    let bytes = frame::encode(&command(1, 1, utc_ms() + 600).encode_to_vec()).unwrap();
    let (started, waiting) = mpsc::channel();
    let (release, unblock) = mpsc::channel();
    let owned_stop = stop.clone();
    let owner = thread::spawn(move || {
        run(
            "session",
            0,
            HeldInput {
                bytes: Cursor::new(bytes),
                stop: owned_stop.clone(),
            },
            HeldOutput {
                started: Some(started),
                release: unblock,
            },
            owned_stop,
        )
    });
    waiting.recv_timeout(Duration::from_secs(2)).unwrap();
    thread::sleep(Duration::from_millis(650));
    stop.store(true, Ordering::Release);
    let deadline = Instant::now() + Duration::from_secs(1);
    while !owner.is_finished() && Instant::now() < deadline {
        thread::sleep(TICK);
    }
    // Always release the test sink, even if a regression made the owner wait.
    let finished = owner.is_finished();
    release.send(()).unwrap();
    assert!(finished, "owner waited on blocked stdout");
    let status = owner.join().unwrap().unwrap();
    assert!(!status.lease_valid);
    assert!(!status.output_available);
}
