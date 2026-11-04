#![cfg(windows)]
//! Explicit real-device execution with silence, never provider data or a fake sink.
use nai_contracts::{audition_wire as wire, workbench::*};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
#[test]
#[ignore = "requires Windows default output device; writes silent PCM"]
fn real_device_silence_submission_and_fail_closed_controls() {
    for mode in ["complete", "lease", "stop", "eof", "choosing-stop"] {
        let dir = tempfile::tempdir().unwrap();
        let media = dir.path().join("silent.pcm");
        let stop = dir.path().join("stop");
        let count = if mode == "complete" { 7200u64 } else { 72000 };
        let bytes = vec![0u8; count as usize * 2];
        std::fs::write(&media, &bytes).unwrap();
        let hash: String = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let mut child = Command::new(env!("CARGO_BIN_EXE_nai-audition-worker"))
            .arg("--media")
            .arg(media)
            .arg("--stop")
            .arg(&stop)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take();
        let mut output = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::sync_channel(4);
        std::thread::spawn(move || {
            while let Ok(s) = wire::read::<AuditionStatus>(&mut output) {
                if tx.send(s).is_err() {
                    break;
                }
            }
        });
        wire::write(
            input.as_mut().unwrap(),
            &AuditionControl {
                operation: Some(audition_control::Operation::Start(AuditionStart {
                    artifact_id: "silent-fixture".into(),
                    sample_count: count,
                    sample_rate: 24000,
                    pcm_sha256: hash,
                    stop_nonce: String::new(),
                    output_session_id: "ab".repeat(32),
                })),
            },
        )
        .unwrap();
        let end = Instant::now() + Duration::from_secs(5);
        let began = Instant::now();
        let mut epoch = 0;
        let mut terminal = false;
        let mut selected = false;
        while Instant::now() < end {
            let s = match rx.recv_timeout(Duration::from_secs(2)) {
                Ok(v) => v,
                Err(_) => break,
            };
            let v = s.view.unwrap();
            assert_ne!(
                v.state,
                AuditionState::Unavailable as i32,
                "real device unavailable: {}",
                v.reason
            );
            if v.state == AuditionState::Submitted as i32
                || v.state == AuditionState::Stopped as i32
            {
                if mode == "complete" {
                    assert_eq!(v.state, AuditionState::Submitted as i32);
                    assert_eq!(v.submitted_samples, count)
                } else {
                    assert_eq!(v.state, AuditionState::Stopped as i32);
                    assert!(v.submitted_samples < count);
                    assert!(began.elapsed() < Duration::from_millis(1600));
                }
                terminal = true;
                break;
            }
            if v.state == AuditionState::Choosing as i32 && !selected && mode != "choosing-stop" {
                assert_eq!(v.submitted_samples, 0);
                let d = v
                    .devices
                    .iter()
                    .find(|d| d.is_default)
                    .unwrap_or(&v.devices[0]);
                wire::write(
                    input.as_mut().unwrap(),
                    &AuditionControl {
                        operation: Some(audition_control::Operation::SelectDevice(
                            OutputDeviceSelection {
                                output_session_id: v.output_session_id.clone(),
                                device_id: d.device_id,
                            },
                        )),
                    },
                )
                .unwrap();
                selected = true;
                continue;
            }
            if s.challenge > 0
                && (v.state != AuditionState::Choosing as i32 || mode == "choosing-stop")
                && (mode == "complete" || epoch == 0)
            {
                epoch += 1;
                wire::write(
                    input.as_mut().unwrap(),
                    &AuditionControl {
                        operation: Some(audition_control::Operation::Lease(AuditionLease {
                            challenge: s.challenge,
                            epoch,
                            ttl_ms: 500,
                        })),
                    },
                )
                .unwrap();
                if mode == "stop" || mode == "choosing-stop" {
                    std::fs::File::create(&stop)
                        .unwrap()
                        .write_all(b"changed")
                        .unwrap();
                }
                if mode == "eof" {
                    input.take();
                }
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        assert!(terminal, "{mode}: no bounded terminal state");
    }
}
