//! Real native subprocess drills only. Not engine/Windows/hardware certification.
use nai_contracts::v1::{
    ErrorCode, OutputWatchdogCommand, OutputWatchdogStatus, PlaybackLease, SessionMode,
    output_watchdog_command::Action,
};
use nai_plugin_sdk::frame;
use prost::Message;
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, Weak, atomic::AtomicBool, mpsc},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn wait_exit(child: &mut Child, timeout: Duration) -> std::process::ExitStatus {
    let end = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if Instant::now() >= end {
            let _ = child.kill();
            let _ = child.wait();
            panic!("owned subprocess did not exit");
        }
        thread::sleep(Duration::from_millis(10));
    }
}
fn source() -> Arc<tempfile::TempDir> {
    // Share a live build without leaving an immortal TempDir in a static.
    static BUILD: Mutex<Weak<tempfile::TempDir>> = Mutex::new(Weak::new());
    let mut cached = BUILD.lock().unwrap();
    if let Some(dir) = cached.upgrade() {
        return dir;
    }
    let dir = Arc::new(tempfile::tempdir().unwrap());
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/lease_source.rs");
    let mut compiler = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .arg("--edition=2024")
        .arg(source)
        .arg("-o")
        .arg(
            dir.path()
                .join(format!("lease-source{}", std::env::consts::EXE_SUFFIX)),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    assert!(wait_exit(&mut compiler, Duration::from_secs(30)).success());
    *cached = Arc::downgrade(&dir);
    dir
}
fn arm(epoch: u64) -> Vec<u8> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    frame::encode(
        &OutputWatchdogCommand {
            schema_version: 1,
            session_id: "session".into(),
            seq: 1,
            remaining_budget_ms: 250,
            action: Some(Action::Arm(PlaybackLease {
                lease_id: "fixture".into(),
                session_id: "session".into(),
                generation_epoch: epoch,
                mode: SessionMode::Rehearsal as i32,
                allowed_utterance_ids: vec!["fixture-u".into()],
                policy_revision: "test-only".into(),
                expires_at_utc_ms: now + 600,
                ..Default::default()
            })),
        }
        .encode_to_vec(),
    )
    .unwrap()
}

struct Drill {
    source: Child,
    keeper: Child,
    statuses: Option<mpsc::Receiver<OutputWatchdogStatus>>,
    reader: Option<thread::JoinHandle<()>>,
    _dir: tempfile::TempDir,
    _source_dir: Arc<tempfile::TempDir>,
}
impl Drill {
    fn new(floor: u64, mode: &str, bytes: impl FnOnce() -> Vec<u8>) -> Self {
        let source_dir = source(); // Compile BEFORE short-lived lease bytes.
        let binary = source_dir
            .path()
            .join(format!("lease-source{}", std::env::consts::EXE_SUFFIX));
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("frames.bin");
        std::fs::write(&input, bytes()).unwrap();
        let mut source = Command::new(binary)
            .arg(input)
            .arg(mode)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut keeper = Command::new(env!("CARGO_BIN_EXE_nai-output-keeper"))
            .args(["--watchdog-drill", "session", &floor.to_string()])
            .stdin(Stdio::from(source.stdout.take().unwrap()))
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut output = keeper.stdout.take().unwrap();
        let (sender, statuses) = mpsc::sync_channel(16);
        let reader = thread::spawn(move || {
            while let Ok(Some(bytes)) = frame::read(
                &mut output,
                Instant::now() + Duration::from_secs(10),
                &AtomicBool::new(false),
            ) {
                let Ok(status) = OutputWatchdogStatus::decode(bytes.as_slice()) else {
                    break;
                };
                if sender.send(status).is_err() {
                    break;
                }
            }
        });
        Self {
            source,
            keeper,
            statuses: Some(statuses),
            reader: Some(reader),
            _dir: dir,
            _source_dir: source_dir,
        }
    }
    fn until(&self, predicate: impl Fn(&OutputWatchdogStatus) -> bool) -> OutputWatchdogStatus {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let status = self
                .statuses
                .as_ref()
                .unwrap()
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("bounded watchdog observation missing");
            assert!(
                !status.output_available,
                "drill must never report hardware output"
            );
            assert_eq!(
                status.backend_error.as_ref().unwrap().code,
                ErrorCode::CapabilityNotReady as i32
            );
            if predicate(&status) {
                return status;
            }
        }
    }
}
impl Drop for Drill {
    fn drop(&mut self) {
        self.statuses.take();
        let _ = self.source.kill();
        let _ = self.keeper.kill();
        wait_exit(&mut self.source, Duration::from_secs(2));
        wait_exit(&mut self.keeper, Duration::from_secs(2));
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

#[test]
fn killed_source_revokes_lease_without_killing_keeper() {
    let mut drill = Drill::new(0, "stall", || arm(1));
    drill.until(|status| status.lease_valid);
    let killed_at = Instant::now();
    drill.source.kill().unwrap();
    let exit = wait_exit(&mut drill.source, Duration::from_secs(1));
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(exit.signal(), Some(9));
    }
    #[cfg(not(unix))]
    assert!(!exit.success());
    let status = drill.until(|status| status.input_closed && !status.lease_valid);
    let elapsed = killed_at.elapsed();
    println!(
        "source kill -> logical revoke observed: {}ms",
        elapsed.as_millis()
    );
    assert!(elapsed < Duration::from_millis(900));
    assert_eq!(status.epoch_floor, 1);
    assert!(drill.keeper.try_wait().unwrap().is_none());
}

#[test]
fn live_but_stalled_source_expires_on_keeper_clock() {
    let mut drill = Drill::new(0, "stall", || arm(1));
    drill.until(|status| status.lease_valid);
    let start = Instant::now();
    let status = drill.until(|status| !status.lease_valid && status.last_command_seq == 1);
    println!(
        "stop renewal -> expiry observed: {}ms",
        start.elapsed().as_millis()
    );
    assert!(start.elapsed() < Duration::from_millis(900));
    assert!(!status.input_closed);
    assert!(drill.source.try_wait().unwrap().is_none());
}

#[test]
fn partial_control_frame_cannot_block_independent_revocation() {
    let mut drill = Drill::new(0, "partial", || arm(1));
    drill.until(|status| status.lease_valid);
    let status = drill.until(|status| status.input_closed && !status.lease_valid);
    assert_eq!(
        status.last_control_error.unwrap().code,
        ErrorCode::DeadlineExceeded as i32
    );
    assert!(drill.source.try_wait().unwrap().is_none());
}

#[test]
fn killed_keeper_restart_requires_fresh_epoch_and_new_authority() {
    let mut old = Drill::new(0, "stall", || arm(7));
    let observed = old.until(|status| status.lease_valid);
    old.keeper.kill().unwrap();
    let exit = wait_exit(&mut old.keeper, Duration::from_secs(1));
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(exit.signal(), Some(9));
    }
    #[cfg(not(unix))]
    assert!(!exit.success());
    let recovered = Drill::new(observed.epoch_floor, "stall", || arm(7));
    let status = recovered.until(|status| status.last_control_error.is_some());
    assert_eq!(status.epoch_floor, 7);
    assert!(!status.lease_valid);
    let fresh = Drill::new(7, "stall", || arm(8));
    assert_eq!(fresh.until(|status| status.lease_valid).epoch_floor, 8);
}

#[test]
fn oversized_wire_frame_and_missing_explicit_drill_mode_are_rejected() {
    let drill = Drill::new(0, "stall", || {
        (frame::MAX_FRAME_BYTES as u32 + 1).to_le_bytes().to_vec()
    });
    let status = drill.until(|status| status.input_closed);
    assert_eq!(
        status.last_control_error.unwrap().code,
        ErrorCode::ResourceExhausted as i32
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_nai-output-keeper"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    assert_eq!(
        wait_exit(&mut child, Duration::from_secs(1)).code(),
        Some(2)
    );
}

#[test]
fn control_input_storm_remains_bounded_and_closes_without_output_grant() {
    let drill = Drill::new(0, "stall", || {
        let mut bytes = Vec::new();
        for seq in 1..=1000 {
            bytes.extend(
                frame::encode(
                    &OutputWatchdogCommand {
                        schema_version: 1,
                        session_id: "session".into(),
                        seq,
                        remaining_budget_ms: 250,
                        action: Some(Action::Inspect(true)),
                    }
                    .encode_to_vec(),
                )
                .unwrap(),
            );
        }
        bytes
    });
    let status = drill.until(|status| status.input_closed);
    assert_eq!(
        status.last_control_error.unwrap().code,
        ErrorCode::ResourceExhausted as i32
    );
    assert!(status.dropped_commands > 0);
    assert!(!status.lease_valid);
}
