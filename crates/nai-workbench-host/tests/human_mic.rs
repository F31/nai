#![cfg(windows)]
use nai_contracts::{
    audition_wire,
    workbench::{MicControl, MicStatus},
};
use nai_workbench_host::human_mic::{MicOwner, stop};
use std::{path::PathBuf, time::Duration};
fn binary() -> PathBuf {
    PathBuf::from(std::env::var_os("NAI_HUMAN_MIC_WORKER").expect("explicit worker"))
}
#[test]
#[ignore = "requires Windows audio enumeration; never activates capture"]
fn native_microphone_default_off_and_p0_before_start() {
    let root = tempfile::tempdir().unwrap();
    let mut m = MicOwner::open_at(&binary(), root.path()).unwrap();
    for _ in 0..3 {
        let s = m.poll().unwrap();
        assert!(!s.active);
        assert_eq!(s.captured_frames, 0);
        assert_eq!(s.submitted_frames, 0);
    }
    stop(root.path()).unwrap();
    std::thread::sleep(Duration::from_millis(80));
    assert!(m.poll().is_err());
}
#[test]
#[ignore = "requires explicitly authorized headphone route; no recording"]
fn native_microphone_lost_lease_cannot_be_revived() {
    let root = tempfile::tempdir().unwrap();
    let mut m = MicOwner::open_at(&binary(), root.path()).unwrap();
    let instance = m.status.instance_id.clone();
    let i = std::env::var("NAI_MIC_ACCEPTANCE_INPUT")
        .unwrap()
        .parse()
        .unwrap();
    let o = std::env::var("NAI_MIC_ACCEPTANCE_OUTPUT")
        .unwrap()
        .parse()
        .unwrap();
    assert!(m.start(&instance, i, o, true).unwrap().active);
    std::thread::sleep(Duration::from_millis(650));
    assert!(m.poll().is_err());
    assert!(m.start(&instance, i, o, true).is_err());
}

#[test]
#[ignore = "requires Windows device enumeration; malformed private controls never activate capture"]
fn native_microphone_private_identity_version_and_sequence_fail_closed() {
    for (version, seq, token) in [
        (0, 1, "a".repeat(64)),
        (1, 2, "a".repeat(64)),
        (1, 1, "b".repeat(64)),
    ] {
        let root = tempfile::tempdir().unwrap();
        let mut c = std::process::Command::new(binary());
        c.env_clear()
            .env("SystemRoot", std::env::var_os("SystemRoot").unwrap())
            .env("NAI_MIC_TOKEN", "a".repeat(64))
            .env("NAI_MIC_INSTANCE", "00000000-0000-4000-8000-000000000001")
            .arg("--stop-file")
            .arg(root.path().join("human-mic.stop"))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped());
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x08000000);
        let mut child = c.spawn().unwrap();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let mut output = child.stdout.take().unwrap();
        std::thread::spawn(move || {
            let _ = tx.send(audition_wire::read::<MicStatus>(&mut output));
        });
        let initial = match rx.recv_timeout(Duration::from_secs(4)) {
            Ok(Ok(s)) => s,
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("initial device status missing")
            }
        };
        assert!(!initial.active);
        assert_eq!(initial.captured_frames, 0);
        audition_wire::write(
            child.stdin.as_mut().unwrap(),
            &MicControl {
                protocol_major: version,
                seq,
                authorization: token,
                instance_id: initial.instance_id,
                challenge: initial.challenge,
                action: 0,
                ..Default::default()
            },
        )
        .unwrap();
        let until = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if std::time::Instant::now() >= until {
                let _ = child.kill();
                let _ = child.wait();
                panic!("invalid control did not terminate worker")
            };
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
