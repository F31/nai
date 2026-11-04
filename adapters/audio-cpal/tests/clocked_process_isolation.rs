#![cfg(all(windows, feature = "clocked-experiment"))]
//! Silent device process isolation drill; never public output or playback authorization.
use std::{
    fs,
    os::windows::process::CommandExt,
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const CREATE_NO_WINDOW: u32 = 0x08000000;

struct Reap(Child);
impl Drop for Reap {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn_ignored(name: &str) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--ignored", "--nocapture"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .unwrap()
}

fn wait_exit_within(child: &mut Child, budget: Duration) -> Result<ExitStatus, &'static str> {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if start.elapsed() < budget => thread::sleep(Duration::from_millis(5)),
            Ok(None) => return Err("CLOCK_PROBE_PROCESS_STOP_TIMEOUT"),
            Err(_) => return Err("CLOCK_PROBE_PROCESS_WAIT"),
        }
    }
}

fn terminate_within(child: &mut Child, budget: Duration) -> Result<ExitStatus, &'static str> {
    child.kill().map_err(|_| "CLOCK_PROBE_PROCESS_KILL")?;
    wait_exit_within(child, budget)
}

#[test]
#[ignore = "invoked only by isolation parent"]
fn child_park_without_device() {
    loop {
        thread::park();
    }
}

#[test]
#[ignore = "invoked only by isolation parent"]
fn child_simulated_stuck_worker() {
    let marker = std::env::var_os("NAI_CLOCK_DRILL_MARKER").expect("isolated drill only");
    use nai_audio_cpal::clocked_experiment::{ExperimentalFence, open_silent};
    use std::sync::{Arc, atomic::Ordering};
    let gate = Arc::new(ExperimentalFence::default());
    let probe = open_silent(gate.clone()).unwrap();
    assert!(gate.activate());
    let deadline = Instant::now() + Duration::from_secs(3);
    while probe.played.load(Ordering::Acquire) < 240 {
        assert!(Instant::now() < deadline, "no device clock progress");
        assert!(!probe.failed.load(Ordering::Acquire));
        thread::sleep(Duration::from_millis(5));
    }
    fs::write(marker, b"ready").unwrap();
    loop {
        thread::park();
    }
}

#[test]
fn wait_timeout_keeps_child_ownership() {
    let mut child = Reap(spawn_ignored("child_park_without_device"));
    assert_eq!(
        wait_exit_within(&mut child.0, Duration::from_millis(80)),
        Err("CLOCK_PROBE_PROCESS_STOP_TIMEOUT")
    );
    assert!(child.0.try_wait().unwrap().is_none());
    assert!(terminate_within(&mut child.0, Duration::from_secs(2)).is_ok());
}

#[test]
fn parent_can_terminate_stuck_worker_with_deadline() {
    let temp = std::env::temp_dir();
    let marker = temp.join(format!(
        "nai-clock-process-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "child_simulated_stuck_worker",
            "--ignored",
            "--nocapture",
        ])
        .env("NAI_CLOCK_DRILL_MARKER", &marker)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .unwrap();
    let mut child = Reap(child);
    let start = Instant::now();
    while !marker.is_file() {
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "child exited before ready"
        );
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "child ready timeout"
        );
        thread::sleep(Duration::from_millis(5));
    }
    terminate_within(&mut child.0, Duration::from_secs(2)).expect("worker kill timeout");
    fs::remove_file(marker).unwrap();
    // A fresh client can open after termination; this is not acoustic-silence evidence.
    let gate =
        std::sync::Arc::new(nai_audio_cpal::clocked_experiment::ExperimentalFence::default());
    let mut fresh = nai_audio_cpal::clocked_experiment::open_silent(gate).unwrap();
    assert!(fresh.stop_within(Duration::from_millis(500)).is_ok());
}
