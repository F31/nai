//! Opt-in native fault tests. Approval/audio are explicit protocol fixtures;
//! the Windows audio stream, window and child-process transport are real.
#![cfg(target_os = "windows")]
use nai_contracts::v1::*;
use nai_plugin_sdk::frame;
use prost::Message;
use sha2::{Digest, Sha256};
use std::{
    process::{Child, Command, Stdio},
    sync::{atomic::AtomicBool, mpsc},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn hash(v: &[u8]) -> String {
    Sha256::digest(v)
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect()
}
fn utc() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[test]
#[ignore = "opens real Windows audio/window; tests EOF, lease expiry and descriptor tampering"]
fn real_local_output_rejects_tampering_and_stops_on_eof_or_no_renewal() {
    for scenario in ["tamper", "descriptor", "eof", "expiry"] {
        let dir = tempfile::tempdir().unwrap();
        let audio: Vec<u8> = (0..16000)
            .flat_map(|i| {
                let s = ((i as f32 * 440.0 * std::f32::consts::TAU / 8000.0).sin() * 800.0) as i16;
                s.to_le_bytes()
            })
            .collect();
        let rgba = vec![127u8; 50 * 4];
        let mut cue = PerformanceCue {
            session_id: "test-local".into(),
            utterance_id: "fixture".into(),
            generation_epoch: 1,
            hash_algorithm: "sha256-canonical-v1".into(),
            approval_ref: "fixture-approval".into(),
            policy_version: "guard-rule-v1".into(),
            expires_at_utc_ms: utc() + 15000,
            ..Default::default()
        };
        cue.candidate_hash = nai_contracts::candidate_hash(&cue);
        let mut spec = LocalPlaybackMedia {
            version: 1,
            session_id: cue.session_id.clone(),
            generation_epoch: 1,
            candidate_hash: cue.candidate_hash.clone(),
            sample_rate: 8000,
            sample_count: 16000,
            width: 1,
            height: 1,
            frame_count: 50,
            fps_num: 25,
            fps_den: 1,
            audio_sha256: hash(&audio),
            frames_sha256: hash(&rgba),
            ..Default::default()
        };
        let descriptor = spec.encode_to_vec();
        std::fs::write(dir.path().join("audio.pcm"), audio).unwrap();
        std::fs::write(dir.path().join("frames.rgba"), rgba).unwrap();
        std::fs::write(dir.path().join("media.pb"), &descriptor).unwrap();
        if scenario == "tamper" {
            std::fs::write(dir.path().join("frames.rgba"), vec![0u8; 200]).unwrap();
        }
        if scenario == "descriptor" {
            let replaced = vec![0u8; 200];
            std::fs::write(dir.path().join("frames.rgba"), &replaced).unwrap();
            spec.frames_sha256 = hash(&replaced);
            std::fs::write(dir.path().join("media.pb"), spec.encode_to_vec()).unwrap();
        }
        let mut command = Command::new(env!("CARGO_BIN_EXE_nai-output-keeper"));
        command
            .arg("--local-playback")
            .arg(dir.path().join("media.pb"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
        let mut child = Owned(command.spawn().unwrap());
        let mut input = child.0.stdin.take().unwrap();
        let mut output = child.0.stdout.take().unwrap();
        let (tx, rx) = mpsc::sync_channel(3);
        std::thread::spawn(move || {
            while let Ok(Some(raw)) = frame::read(
                &mut output,
                Instant::now() + Duration::from_secs(10),
                &AtomicBool::new(false),
            ) {
                let Ok(status) = LocalPlaybackStatus::decode(raw.as_slice()) else {
                    break;
                };
                if tx.try_send(status).is_err() {
                    break;
                }
            }
        });
        let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        if scenario == "tamper" {
            assert_eq!(first.error_code, "DEVICE_MEDIA_INTEGRITY");
            assert_eq!(first.copied_samples, None);
            continue;
        }
        assert_eq!(first.state, "ready");
        let approval = Approval {
            approval_id: cue.approval_ref.clone(),
            session_id: cue.session_id.clone(),
            generation_epoch: 1,
            candidate_hash: cue.candidate_hash.clone(),
            hash_algorithm: cue.hash_algorithm.clone(),
            actor_id: "explicit-test-fixture".into(),
            action_scope: "speak".into(),
            expires_at_utc_ms: cue.expires_at_utc_ms,
            ..Default::default()
        };
        let lease = PlaybackLease {
            lease_id: "test-local-lease".into(),
            session_id: cue.session_id.clone(),
            generation_epoch: 1,
            allowed_utterance_ids: vec![cue.utterance_id.clone()],
            mode: SessionMode::Assisted as i32,
            approval_refs: vec![approval.approval_id.clone()],
            expires_at_utc_ms: utc() + 600,
            candidate_hashes: [(cue.utterance_id.clone(), cue.candidate_hash.clone())].into(),
            policy_revision: cue.policy_version.clone(),
            ..Default::default()
        };
        let grant = LocalPlaybackGrant {
            version: 1,
            seq: 1,
            cue: Some(cue),
            approval: Some(approval),
            lease: Some(lease),
            media_sha256: hash(&descriptor),
        };
        frame::write(&mut input, &grant.encode_to_vec()).unwrap();
        if scenario == "descriptor" {
            let refused = rx.recv_timeout(Duration::from_secs(2)).unwrap();
            assert_eq!(refused.error_code, "DEVICE_PROOF_MISMATCH");
            assert_eq!(refused.copied_samples, None);
            continue;
        }
        let running = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(running.state, "running");
        assert!(running.copied_samples.is_some_and(|v| v > 0));
        let start = Instant::now();
        if scenario == "eof" {
            drop(input);
        }
        let stopped = rx.recv_timeout(Duration::from_millis(1200)).unwrap();
        println!("{scenario}: after {:?}, {stopped:?}", start.elapsed());
        assert_ne!(stopped.state, "submitted");
        assert!(!stopped.actual_local_playback_completed);
        assert!(stopped.copied_samples.is_none_or(|v| v < 96000));
    }
}

#[test]
#[ignore = "opens real Windows audio and resident capture window, no API"]
fn resident_output_keeps_one_stream_and_safe_standby_after_faults() {
    use nai_contracts::v1::local_output_command::Command as Wire;
    let mut command = Command::new(env!("CARGO_BIN_EXE_nai-output-keeper"));
    use std::os::windows::process::CommandExt;
    command
        .arg("--resident-output")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .creation_flags(0x08000000);
    let mut child = Owned(command.spawn().unwrap());
    let pid = child.0.id();
    let mut input = child.0.stdin.take().unwrap();
    let mut output = child.0.stdout.take().unwrap();
    let (tx, rx) = mpsc::sync_channel(4);
    std::thread::spawn(move || {
        while let Ok(Some(raw)) = frame::read(
            &mut output,
            Instant::now() + Duration::from_secs(20),
            &AtomicBool::new(false),
        ) {
            if tx
                .try_send(LocalPlaybackStatus::decode(raw.as_slice()).unwrap())
                .is_err()
            {
                break;
            }
        }
    });
    let neutral_dir = tempfile::tempdir().unwrap();
    let pixels = [16, 32, 48, 255].repeat(640 * 360);
    let spec = LocalStandbyMedia {
        version: 1,
        session_id: "resident-test".into(),
        generation_epoch: 1,
        width: 640,
        height: 360,
        standby_sha256: hash(&pixels),
    };
    let raw = spec.encode_to_vec();
    let path = neutral_dir.path().join("standby.pb");
    std::fs::write(&path, &raw).unwrap();
    std::fs::write(neutral_dir.path().join("standby.rgba"), pixels).unwrap();
    frame::write(
        &mut input,
        &LocalOutputCommand {
            command: Some(Wire::Warmup(LocalOutputWarmup {
                descriptor_path: path.to_str().unwrap().into(),
                descriptor_sha256: hash(&raw),
                session_id: "resident-test".into(),
                generation_epoch: 1,
                remaining_budget_ms: 4000,
            })),
        }
        .encode_to_vec(),
    )
    .unwrap();
    let prepared = rx.recv_timeout(Duration::from_secs(8)).unwrap();
    assert_eq!(prepared.state, "standby", "{prepared:?}");
    assert_eq!(prepared.copied_samples, Some(0));
    assert_eq!(prepared.standby_sha256, spec.standby_sha256);
    assert_eq!(prepared.audio_stream_open_count, 1);
    assert!(!prepared.actual_local_playback_completed);
    assert!(child.0.try_wait().unwrap().is_none());
    drop(neutral_dir); // keeper owns admitted pixels, not a file mapping.
    for index in 0..6 {
        let dir = tempfile::tempdir().unwrap();
        let (width, height, frames, samples) = if index == 0 {
            (640, 360, 100, 32000) // Four seconds, 87.9MiB RGBA: exceeds old 64MiB cap.
        } else {
            (1, 1, 13, 4000)
        };
        let audio: Vec<u8> = (0..samples)
            .flat_map(|i| {
                let s = ((i as f32 * 440.0 * std::f32::consts::TAU / 8000.0).sin() * 800.0) as i16;
                s.to_le_bytes()
            })
            .collect();
        let frame_bytes = (width * height * 4) as usize;
        let rgba = vec![127u8; frames as usize * frame_bytes];
        let standby = [16, 32, 48, 255].repeat((width * height) as usize);
        let mut cue = PerformanceCue {
            session_id: "resident-test".into(),
            utterance_id: format!("sentence-{index}"),
            generation_epoch: if index >= 3 { 2 } else { 1 },
            hash_algorithm: "sha256-canonical-v1".into(),
            approval_ref: format!("approval-{index}"),
            policy_version: "guard-rule-v1".into(),
            expires_at_utc_ms: utc() + 10000,
            ..Default::default()
        };
        cue.candidate_hash = nai_contracts::candidate_hash(&cue);
        let spec = LocalPlaybackMedia {
            version: 2,
            session_id: cue.session_id.clone(),
            generation_epoch: if index >= 3 { 2 } else { 1 },
            candidate_hash: cue.candidate_hash.clone(),
            sample_rate: 8000,
            sample_count: samples as u64,
            width,
            height,
            frame_count: frames,
            fps_num: 25,
            fps_den: 1,
            audio_sha256: hash(&audio),
            frames_sha256: hash(&rgba),
            frame_sha256: vec![hash(&rgba[..frame_bytes]); frames as usize],
            standby_sha256: hash(&standby),
        };
        std::fs::write(dir.path().join("audio.pcm"), audio).unwrap();
        std::fs::write(dir.path().join("frames.rgba"), rgba).unwrap();
        std::fs::write(dir.path().join("standby.rgba"), standby).unwrap();
        if index == 0 {
            assert!(
                std::fs::metadata(dir.path().join("frames.rgba"))
                    .unwrap()
                    .len()
                    > 64 * 1024 * 1024
            );
        }
        let descriptor = spec.encode_to_vec();
        let path = dir.path().join("media.pb");
        std::fs::write(&path, &descriptor).unwrap();
        frame::write(
            &mut input,
            &LocalOutputCommand {
                command: Some(Wire::DescriptorPath(path.to_str().unwrap().into())),
            }
            .encode_to_vec(),
        )
        .unwrap();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap().state,
            "ready"
        );
        let approval = Approval {
            approval_id: cue.approval_ref.clone(),
            session_id: cue.session_id.clone(),
            generation_epoch: if index >= 3 { 2 } else { 1 },
            candidate_hash: cue.candidate_hash.clone(),
            hash_algorithm: cue.hash_algorithm.clone(),
            actor_id: "explicit-test-fixture".into(),
            action_scope: "speak".into(),
            expires_at_utc_ms: cue.expires_at_utc_ms,
            ..Default::default()
        };
        let mut grant = LocalPlaybackGrant {
            version: 1,
            seq: 0,
            cue: Some(cue.clone()),
            approval: Some(approval.clone()),
            lease: Some(PlaybackLease {
                lease_id: format!("lease-{index}"),
                session_id: cue.session_id.clone(),
                generation_epoch: if index >= 3 { 2 } else { 1 },
                allowed_utterance_ids: vec![cue.utterance_id.clone()],
                mode: SessionMode::Assisted as i32,
                approval_refs: vec![approval.approval_id],
                candidate_hashes: [(cue.utterance_id.clone(), cue.candidate_hash.clone())].into(),
                policy_revision: cue.policy_version.clone(),
                ..Default::default()
            }),
            media_sha256: hash(&descriptor),
        };
        let started = Instant::now();
        let mut tampered = false;
        loop {
            assert!(started.elapsed() < Duration::from_secs(12));
            grant.seq += 1;
            grant.lease.as_mut().unwrap().expires_at_utc_ms = utc() + 600;
            frame::write(
                &mut input,
                &LocalOutputCommand {
                    command: Some(Wire::Grant(Box::new(grant.clone()))),
                }
                .encode_to_vec(),
            )
            .unwrap();
            if let Ok(status) = rx.recv_timeout(Duration::from_millis(100)) {
                println!("cue {index}: {status:?}");
                if index != 4 {
                    assert_ne!(status.state, "failed", "{status:?}");
                }
                if index == 4 && status.state == "running" && !tampered {
                    use std::io::{Seek, SeekFrom, Write};
                    let mut file = std::fs::OpenOptions::new()
                        .write(true)
                        .open(dir.path().join("frames.rgba"))
                        .unwrap();
                    file.seek(SeekFrom::Start(5 * 4)).unwrap();
                    file.write_all(&[0; 4]).unwrap();
                    file.flush().unwrap();
                    tampered = true;
                }
                if index == 4 && status.state == "failed" {
                    assert!(tampered);
                    assert_eq!(status.error_code, "DEVICE_FRAME_INTEGRITY");
                    assert_eq!(status.standby_sha256, spec.standby_sha256);
                    assert_eq!(status.audio_stream_open_count, 1);
                    break;
                }
                if index == 2 && status.state == "running" {
                    frame::write(
                        &mut input,
                        &LocalOutputCommand {
                            command: Some(Wire::CancelCue(true)),
                        }
                        .encode_to_vec(),
                    )
                    .unwrap();
                    let cancelled = rx.recv_timeout(Duration::from_secs(2)).unwrap();
                    assert_eq!(cancelled.state, "cancelled");
                    assert_eq!(cancelled.audio_stream_open_count, 1);
                    assert_eq!(cancelled.standby_sha256, spec.standby_sha256);
                    assert!(!cancelled.actual_local_playback_completed);
                    assert!(!cancelled.device_position_observed);
                    assert!(!cancelled.video_tail_submitted);
                    assert!(cancelled.video_frame_index.is_none());
                    assert!(cancelled.video_device_samples.is_none());
                    break;
                }
                if status.state == "submitted" {
                    assert_eq!(status.copied_samples, Some(samples as u64 * 6));
                    assert_eq!(status.device_played_samples, status.copied_samples);
                    assert!(status.device_position_observed);
                    assert_eq!(status.audio_backend, "wasapi-iaudioclock");
                    assert!(status.video_tail_submitted);
                    assert_eq!(status.video_device_samples, status.device_played_samples);
                    assert_eq!(status.video_frame_index, Some(spec.frame_count - 1));
                    assert_eq!(status.media_sha256, hash(&descriptor));
                    assert_eq!(status.audio_stream_open_count, 1);
                    assert_eq!(status.standby_sha256, spec.standby_sha256);
                    assert!(!status.actual_local_playback_completed);
                    break;
                }
            }
        }
        assert_eq!(child.0.id(), pid);
        assert!(child.0.try_wait().unwrap().is_none());
        // Standby must not replay or emit another completion.
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
        assert!(child.0.try_wait().unwrap().is_none());
    }
    drop(input);
    // EOF leaves the admitted neutral image and a silent stream; it cannot
    // produce another completion or replay. Owned explicitly reaps this fixture.
    assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
    assert!(child.0.try_wait().unwrap().is_none());
    println!(
        "resident pid={pid}: two cues, cancel, new epoch, frame tamper recovery, one audio stream and EOF standby verified"
    );
}
