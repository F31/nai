#![cfg(windows)]
use nai_contracts::{audition_wire as wire, workbench::*};
use nai_workbench_host::public_output::{OutputOwner, stop};
use std::{
    io::Write,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};
fn binary() -> PathBuf {
    std::env::var_os("NAI_PUBLIC_OUTPUT_KEEPER")
        .expect("build keeper and set binary")
        .into()
}
#[test]
fn actual_window_priority_stop_and_worker_crash_are_observed() {
    let t = tempfile::tempdir().unwrap();
    let mut owner = OutputOwner::open_at(&binary(), t.path(), true).unwrap();
    let first = owner.current();
    let visual = first
        .visual
        .as_ref()
        .expect("actual admitted visual metadata");
    assert_eq!((visual.width, visual.height), (512, 288));
    assert_eq!(first.visual_license, "MIT");
    assert_eq!(visual.package_sha256.len(), 64);
    assert!(
        first.window_open
            && first.standby
            && !first.live_ready
            && !first.audio_active
            && first.standby_frames > 0
    );
    std::thread::sleep(Duration::from_millis(120));
    let later = owner.poll().unwrap();
    assert!(later.standby_frames > first.standby_frames);
    stop(t.path()).unwrap();
    assert!(!t.path().join("audition.stop").exists());
    std::thread::sleep(Duration::from_millis(80));
    let stopped = owner.poll().unwrap();
    assert!(stopped.terminal);
    assert_eq!(stopped.reason, PublicOutputReason::Stopped as i32);
    assert!(owner.poll().unwrap().terminal);
    use std::os::windows::process::CommandExt;
    let killed = Command::new("taskkill.exe")
        .args(["/F", "/PID", &owner.process_id().to_string()])
        .creation_flags(0x08000000)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(killed.success());
    // taskkill completion is not the target process's exit notification.
    let until = std::time::Instant::now() + Duration::from_millis(250);
    while owner.poll().is_ok() {
        assert!(
            std::time::Instant::now() < until,
            "worker exit was not observed"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    drop(owner);
    let fresh = OutputOwner::open_at(&binary(), t.path(), true).unwrap();
    assert_ne!(fresh.current().output_id, first.output_id);
}
#[test]
fn stalled_controller_expires_without_restarting_output() {
    let t = tempfile::tempdir().unwrap();
    let mut owner = OutputOwner::open_at(&binary(), t.path(), true).unwrap();
    std::thread::sleep(Duration::from_millis(650));
    let s = owner.poll().unwrap();
    assert!(s.terminal && s.standby && !s.live_ready);
    assert_eq!(s.reason, PublicOutputReason::LeaseExpired as i32);
}
struct Reap(Child);
impl Drop for Reap {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn receive(rx: &mpsc::Receiver<PublicOutputStatus>) -> PublicOutputStatus {
    rx.recv_timeout(Duration::from_secs(4))
        .expect("window/protocol deadline")
}
#[test]
fn partial_stdin_eof_and_invalid_frame_do_not_block_the_window_watchdog() {
    use std::os::windows::process::CommandExt;
    for scenario in [0, 1, 2] {
        let t = tempfile::tempdir().unwrap();
        let mut child = Reap(
            Command::new(binary())
                .arg("--standby")
                .arg("--stop-file")
                .arg(t.path().join("public-output.stop"))
                .env("NAI_PUBLIC_OUTPUT_TOKEN", "a".repeat(64))
                .env(
                    "NAI_PUBLIC_OUTPUT_ID",
                    "00000000-0000-4000-8000-000000000001",
                )
                .env("NAI_PUBLIC_OUTPUT_TEST_HIDDEN", "1")
                .creation_flags(0x08000000)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let mut input = child.0.stdin.take().unwrap();
        let mut output = child.0.stdout.take().unwrap();
        let (tx, rx) = mpsc::sync_channel(4);
        std::thread::spawn(move || {
            while let Ok(s) = wire::read::<PublicOutputStatus>(&mut output) {
                if tx.send(s).is_err() {
                    break;
                }
            }
        });
        let initial = receive(&rx);
        wire::write(
            &mut input,
            &PublicOutputLease {
                authorization: "a".repeat(64),
                seq: 1,
                challenge: initial.challenge,
                lease_ms: 500,
                ..Default::default()
            },
        )
        .unwrap();
        let ack = receive(&rx);
        assert!(!ack.terminal);
        if scenario == 0 {
            input.write_all(&[20, 0]).unwrap();
            input.flush().unwrap();
        } else if scenario == 1 {
            drop(input);
        } else {
            input.write_all(&4097u32.to_le_bytes()).unwrap();
            input.flush().unwrap();
        }
        let terminal = receive(&rx);
        assert!(
            terminal.terminal && terminal.window_open && terminal.standby && !terminal.live_ready
        );
        assert!(terminal.standby_frames >= ack.standby_frames);
        assert_eq!(
            terminal.reason,
            match scenario {
                0 => PublicOutputReason::LeaseExpired as i32,
                1 => PublicOutputReason::InputClosed as i32,
                _ => PublicOutputReason::ProtocolRejected as i32,
            }
        );
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "independent window stays in standby"
        );
    }
}
