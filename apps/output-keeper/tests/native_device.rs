//! Opt-in real Windows hardware smoke. Never part of portable certification.
#![cfg(target_os = "windows")]
use nai_contracts::v1::NativeDeviceSmokeReport;
use prost::Message;
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[test]
#[ignore = "opens real Windows audio/window and emits a short quiet test tone"]
fn real_windows_audio_window_expiry_mute_and_stalled_owner() {
    let dir = PathBuf::from(
        std::env::var_os("NAI_DEVICE_SMOKE_ARTIFACTS")
            .expect("explicit NAI_DEVICE_SMOKE_ARTIFACTS is required"),
    );
    std::fs::create_dir_all(&dir).unwrap();
    for scenario in ["expire", "mute", "stall"] {
        let path = dir.join(format!("{scenario}.pb"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_nai-output-keeper"));
        command
            .args(["--device-smoke"])
            .arg(&path)
            .arg(scenario)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // Hide the console, while retaining the explicitly tested pixel window.
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
        let mut child = command.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("native {scenario} exceeded external 10s deadline");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let report =
            NativeDeviceSmokeReport::decode(std::fs::read(&path).unwrap().as_slice()).unwrap();
        println!("{scenario}: {report:?}");
        assert!(
            status.success(),
            "native {scenario} refused: {}",
            report.error_code
        );
        assert!(report.success);
        assert!(report.audio_opened && report.window_opened);
        assert!(!report.actual_local_playback_completed);
        assert!(report.callbacks > 10 && report.window_updates > 2);
        assert!(report.copied_interleaved_samples > 0);
        assert!(report.silent_callbacks_after_stop >= 3);
        assert_eq!(report.nonzero_callbacks_after_stop, 0);
        assert!(report.last_nonzero_at_us < report.stop_at_us);
        assert!(report.max_predicted_latency_us <= 100000);
        assert!(report.elapsed_ms < 3000);
        if scenario == "mute" {
            assert!(report.stop_at_us < 600000);
        } else {
            assert_eq!(report.stop_at_us, 600000);
        }
    }
}
