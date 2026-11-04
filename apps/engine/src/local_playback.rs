//! Bounded supervision of the private local-output child. No media on control.
use nai_contracts::v1::{
    ErrorCode, LocalOutputCommand, LocalPlaybackGrant, LocalPlaybackStatus,
    local_output_command::Command as OutputCommand,
};
use nai_plugin_sdk::frame;
use prost::Message;
use std::{
    path::Path,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, atomic::AtomicBool, mpsc},
    time::{Duration, Instant},
};

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let deadline = Instant::now() + Duration::from_millis(250);
        while Instant::now() < deadline {
            if self.0.try_wait().is_ok_and(|status| status.is_some()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

fn validate_status(status: &LocalPlaybackStatus) -> Result<(), ErrorCode> {
    if status.version != 1
        || status.actual_local_playback_completed
        || !matches!(
            status.state.as_str(),
            "ready" | "running" | "submitted" | "cancelled" | "failed" | "standby"
        )
        || status.audio_stream_open_count > 1
        || ((status.video_tail_submitted
            || status.video_frame_index.is_some()
            || status.video_device_samples.is_some())
            && (!status.video_tail_submitted
                || status.state != "submitted"
                || !status.device_position_observed
                || status.video_frame_index.is_none()
                || status.video_device_samples != status.device_played_samples))
        || (!status.audio_backend.is_empty() && status.audio_backend != "wasapi-iaudioclock")
        || status
            .device_played_samples
            .is_some_and(|n| status.copied_samples.is_none_or(|copied| n > copied))
        || (status.device_position_observed
            && (status.state != "submitted"
                || status.media_sha256.is_empty()
                || status.audio_backend != "wasapi-iaudioclock"
                || status.device_played_samples.is_none_or(|n| n == 0)))
        || (!status.media_sha256.is_empty()
            && (status.media_sha256.len() != 64
                || !status
                    .media_sha256
                    .bytes()
                    .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))))
        || status
            .callback_drained_samples
            .is_some_and(|n| status.copied_samples.is_none_or(|copied| n > copied))
        || (status.callback_drain_observed
            && (status.state != "submitted"
                || status.media_sha256.is_empty()
                || status.callback_drained_samples.is_none_or(|n| n == 0)))
        || (!status.standby_sha256.is_empty()
            && (status.standby_sha256.len() != 64
                || !status
                    .standby_sha256
                    .bytes()
                    .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))))
        || status.error_code.len() > 64
        || !status
            .error_code
            .bytes()
            .all(|v| v.is_ascii_uppercase() || v.is_ascii_digit() || v == b'_')
    {
        return Err(ErrorCode::InvalidArgument);
    }
    Ok(())
}

pub(crate) struct CueTarget<'a> {
    pub descriptor: &'a Path,
    pub media_sha256: &'a str,
    pub expected_samples: u64,
    pub expected_frames: u32,
}
pub(crate) fn validate_submission(
    status: &LocalPlaybackStatus,
    target: &CueTarget<'_>,
) -> Result<(), ErrorCode> {
    validate_status(status)?;
    if status.state != "submitted"
        || !status.error_code.is_empty()
        || status.media_sha256 != target.media_sha256
        || target.expected_samples == 0
        || target.expected_frames == 0
        || status.audio_stream_open_count != 1
        || status.copied_samples != Some(target.expected_samples)
        || status.device_played_samples != Some(target.expected_samples)
        || !status.device_position_observed
        || status.audio_backend != "wasapi-iaudioclock"
        || status.window_updates.is_none_or(|v| v == 0)
        || !status.video_tail_submitted
        || status.video_frame_index != target.expected_frames.checked_sub(1)
        || status.video_device_samples != Some(target.expected_samples)
    {
        return Err(ErrorCode::InvalidArgument);
    }
    Ok(())
}

/// Private neutral-only admission. No PerformanceCue, Approval or PlaybackLease.
pub(crate) fn prewarm(
    output: &Mutex<Option<ResidentOutput>>,
    binary: &Path,
    command: nai_contracts::v1::LocalOutputWarmup,
    expected_standby: &str,
    deadline: Instant,
    credential_env: &str,
    mut check: impl FnMut() -> Result<(), ErrorCode>,
) -> Result<LocalPlaybackStatus, ErrorCode> {
    check()?;
    let mut slot = output.try_lock().map_err(|_| ErrorCode::Conflict)?;
    if slot.as_mut().is_some_and(|output| {
        output
            .child
            .0
            .try_wait()
            .is_ok_and(|status| status.is_some())
    }) {
        slot.take();
    }
    if slot.is_none() {
        *slot = Some(ResidentOutput::start(binary, credential_env)?);
    }
    let result = (|| {
        check()?;
        let output = slot.as_mut().ok_or(ErrorCode::Internal)?;
        output.idle_confirmed = false;
        output
            .command_tx
            .try_send(OutputCommand::Warmup(command))
            .map_err(|_| ErrorCode::ResourceExhausted)?;
        loop {
            check()?;
            if Instant::now() >= deadline {
                return Err(ErrorCode::DeadlineExceeded);
            }
            if let Ok(status) = output.status_rx.try_recv() {
                let status = status?;
                validate_status(&status)?;
                if status.state != "standby"
                    || !status.error_code.is_empty()
                    || status.audio_stream_open_count != 1
                    || status.copied_samples != Some(0)
                    || status.window_updates.is_none_or(|v| v == 0)
                    || status.standby_sha256 != expected_standby
                {
                    return Err(ErrorCode::CapabilityNotReady);
                }
                check()?;
                output.idle_confirmed = true;
                return Ok(status);
            }
            if output
                .child
                .0
                .try_wait()
                .map_err(|_| ErrorCode::Internal)?
                .is_some()
            {
                return Err(ErrorCode::CapabilityNotReady);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    })();
    if result.is_err() {
        slot.take();
    }
    result
}

pub(crate) fn keeper() -> Result<std::path::PathBuf, ErrorCode> {
    if !cfg!(target_os = "windows") {
        return Err(ErrorCode::CapabilityNotReady);
    }
    let exe = std::env::current_exe().map_err(|_| ErrorCode::CapabilityNotReady)?;
    let dir = exe.parent().ok_or(ErrorCode::CapabilityNotReady)?;
    let plain = dir.join("nai-output-keeper.exe");
    if plain.is_file() {
        return Ok(plain);
    }
    let name = exe
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or(ErrorCode::CapabilityNotReady)?;
    let sibling = dir.join(name.replacen("nai-engine", "nai-output-keeper", 1));
    if sibling != exe && sibling.is_file() {
        Ok(sibling)
    } else {
        Err(ErrorCode::CapabilityNotReady)
    }
}

pub(crate) struct ResidentOutput {
    child: OwnedChild,
    idle_confirmed: bool,
    status_rx: mpsc::Receiver<Result<LocalPlaybackStatus, ErrorCode>>,
    command_tx: mpsc::SyncSender<OutputCommand>,
}
impl ResidentOutput {
    fn start(binary: &Path, credential_env: &str) -> Result<Self, ErrorCode> {
        let mut command = Command::new(binary);
        command.env_clear();
        for name in [
            "SystemRoot",
            "WINDIR",
            "TEMP",
            "TMP",
            "USERPROFILE",
            "LOCALAPPDATA",
            "APPDATA",
        ] {
            if name.eq_ignore_ascii_case(credential_env) {
                continue;
            }
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        command
            .arg("--resident-output")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = OwnedChild(command.spawn().map_err(|e| {
            eprintln!(
                "local playback spawn failed: {:?}, os={:?}",
                e.kind(),
                e.raw_os_error()
            );
            ErrorCode::CapabilityNotReady
        })?);
        eprintln!("resident output spawned pid={}", child.0.id());
        let mut input = child.0.stdin.take().ok_or(ErrorCode::Internal)?;
        let mut output = child.0.stdout.take().ok_or(ErrorCode::Internal)?;
        let stopped = Arc::new(AtomicBool::new(false));
        let (status_tx, status_rx) = mpsc::sync_channel(2);
        let stop = stopped.clone();
        let deadline = Instant::now() + Duration::from_secs(1800);
        std::thread::spawn(move || {
            while let Ok(Some(bytes)) = frame::read(&mut output, deadline, &stop) {
                let status = LocalPlaybackStatus::decode(bytes.as_slice())
                    .map_err(|_| ErrorCode::InvalidArgument);
                if status_tx.try_send(status).is_err() {
                    break;
                }
            }
        });
        let (grant_tx, grant_rx) = mpsc::sync_channel::<OutputCommand>(2);
        std::thread::spawn(move || {
            while let Ok(grant) = grant_rx.recv() {
                if frame::write(
                    &mut input,
                    &LocalOutputCommand {
                        command: Some(grant),
                    }
                    .encode_to_vec(),
                )
                .is_err()
                {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            idle_confirmed: false,
            status_rx,
            command_tx: grant_tx,
        })
    }
}

pub(crate) fn run(
    output: &Mutex<Option<ResidentOutput>>,
    binary: &Path,
    target: CueTarget<'_>,
    deadline: Instant,
    credential_env: &str,
    mut proof: impl FnMut(u64) -> Result<LocalPlaybackGrant, ErrorCode>,
    mut observe: impl FnMut(&LocalPlaybackStatus),
) -> Result<LocalPlaybackStatus, ErrorCode> {
    let mut slot = output.try_lock().map_err(|_| ErrorCode::Conflict)?;
    if slot.is_none() {
        *slot = Some(ResidentOutput::start(binary, credential_env)?);
    }
    let result = run_cue(
        slot.as_mut().ok_or(ErrorCode::Internal)?,
        &target,
        deadline,
        &mut proof,
        &mut observe,
    );
    if result.is_err() && !slot.as_ref().is_some_and(|output| output.idle_confirmed) {
        slot.take();
    }
    result
}

fn run_cue(
    output: &mut ResidentOutput,
    target: &CueTarget<'_>,
    deadline: Instant,
    proof: &mut impl FnMut(u64) -> Result<LocalPlaybackGrant, ErrorCode>,
    observe: &mut impl FnMut(&LocalPlaybackStatus),
) -> Result<LocalPlaybackStatus, ErrorCode> {
    output.idle_confirmed = false;
    output
        .command_tx
        .try_send(OutputCommand::DescriptorPath(
            target
                .descriptor
                .to_str()
                .filter(|p| p.len() <= 4096)
                .ok_or(ErrorCode::InvalidArgument)?
                .into(),
        ))
        .map_err(|_| ErrorCode::ResourceExhausted)?;
    let status_rx = &output.status_rx;
    let child = &mut output.child;
    let started = Instant::now();
    let mut ready = false;
    let mut seq = 0;
    let mut next = Instant::now();
    loop {
        // Recheck even before ready: init is not allowed to outlive a revoke.
        if let Err(code) = proof(0) {
            // Cancel is denial-only. No DB/model lock is needed in the keeper,
            // and stale queued grants cannot start a new cue while idle.
            if output
                .command_tx
                .try_send(OutputCommand::CancelCue(true))
                .is_ok()
            {
                let until = Instant::now() + Duration::from_millis(750);
                while Instant::now() < until {
                    if let Ok(Ok(status)) = status_rx.recv_timeout(Duration::from_millis(10)) {
                        if validate_status(&status).is_err() {
                            break;
                        }
                        if matches!(status.state.as_str(), "cancelled" | "submitted")
                            && status.media_sha256 == target.media_sha256
                        {
                            output.idle_confirmed = true;
                            eprintln!("resident output revoked to standby");
                            break;
                        }
                        if status.state == "failed" {
                            break;
                        }
                    }
                }
            }
            return Err(code);
        }
        if let Ok(status) = status_rx.try_recv() {
            let status = status?;
            validate_status(&status)?;
            eprintln!(
                "local playback child: state={}, code={}",
                status.state, status.error_code
            );
            if status.version != 1 || status.actual_local_playback_completed {
                return Err(ErrorCode::InvalidArgument);
            }
            if matches!(status.state.as_str(), "ready" | "running" | "submitted")
                && status.media_sha256 != target.media_sha256
            {
                return Err(ErrorCode::InvalidArgument);
            }
            match status.state.as_str() {
                "ready" if !ready => ready = true,
                "running"
                    if ready
                        && status.copied_samples.is_some_and(|v| v > 0)
                        && status.window_updates.is_some_and(|v| v > 0) =>
                {
                    observe(&status)
                }
                "submitted"
                    if ready
                        && status.error_code.is_empty()
                        && status.copied_samples.is_some_and(|v| v > 0)
                        && status.window_updates.is_some_and(|v| v > 0) =>
                {
                    validate_submission(&status, target)?;
                    proof(0)?;
                    return Ok(status);
                }
                "failed"
                    if status.audio_stream_open_count == 1 && !status.standby_sha256.is_empty() =>
                {
                    // Keeper returned after revoking the cue and presenting its
                    // separately admitted neutral frame. The request still fails.
                    output.idle_confirmed = true;
                    return Err(ErrorCode::CapabilityNotReady);
                }
                _ => return Err(ErrorCode::CapabilityNotReady),
            }
        }
        if Instant::now() >= deadline || (!ready && started.elapsed() >= Duration::from_secs(4)) {
            return Err(ErrorCode::DeadlineExceeded);
        }
        if ready && Instant::now() >= next {
            let fresh = proof(seq + 1)?;
            output
                .command_tx
                .try_send(OutputCommand::Grant(Box::new(fresh)))
                .map_err(|_| ErrorCode::ResourceExhausted)?;
            seq += 1;
            next = Instant::now() + Duration::from_millis(100);
        }
        if child
            .0
            .try_wait()
            .map_err(|_| ErrorCode::Internal)?
            .is_some()
        {
            // Reader may be scheduling the final status; receive is bounded.
            let status = status_rx
                .recv_timeout(Duration::from_millis(100))
                .map_err(|_| ErrorCode::CapabilityNotReady)??;
            validate_status(&status)?;
            eprintln!(
                "local playback child exit: state={}, code={}",
                status.state, status.error_code
            );
            if status.state == "submitted"
                && status.error_code.is_empty()
                && status.version == 1
                && status.copied_samples.is_some_and(|v| v > 0)
                && status.window_updates.is_some_and(|v| v > 0)
                && !status.actual_local_playback_completed
            {
                validate_submission(&status, target)?;
                proof(0)?;
                return Ok(status);
            }
            return Err(ErrorCode::CapabilityNotReady);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn submission_requires_exact_media_and_full_device_receipt_without_av_ack() {
        let hash = "a".repeat(64);
        let target = CueTarget {
            descriptor: Path::new("unused.pb"),
            media_sha256: &hash,
            expected_samples: 24000,
            expected_frames: 12,
        };
        let good = LocalPlaybackStatus {
            version: 1,
            state: "submitted".into(),
            media_sha256: hash.clone(),
            copied_samples: Some(24000),
            device_played_samples: Some(24000),
            device_position_observed: true,
            audio_backend: "wasapi-iaudioclock".into(),
            window_updates: Some(1),
            audio_stream_open_count: 1,
            video_frame_index: Some(11),
            video_device_samples: Some(24000),
            video_tail_submitted: true,
            ..Default::default()
        };
        let good = LocalPlaybackStatus::decode(good.encode_to_vec().as_slice()).unwrap();
        assert!(validate_submission(&good, &target).is_ok());
        for case in [
            "foreign",
            "backend",
            "empty-backend",
            "predicted-only",
            "missing",
            "partial",
            "overrun",
            "unobserved",
            "hardware",
            "error",
            "no-video",
            "wrong-frame",
            "missing-frame",
            "early-video",
            "missing-video-position",
        ] {
            let mut bad = good.clone();
            match case {
                "foreign" => bad.media_sha256 = "b".repeat(64),
                "backend" => bad.audio_backend = "cpal-predicted".into(),
                "empty-backend" => bad.audio_backend.clear(),
                "predicted-only" => {
                    bad.device_played_samples = None;
                    bad.device_position_observed = false;
                    bad.callback_drained_samples = Some(24000);
                    bad.callback_drain_observed = true;
                }
                "missing" => bad.device_played_samples = None,
                "partial" => bad.device_played_samples = Some(23999),
                "overrun" => bad.device_played_samples = Some(24001),
                "unobserved" => bad.device_position_observed = false,
                "hardware" => bad.actual_local_playback_completed = true,
                "no-video" => bad.video_tail_submitted = false,
                "wrong-frame" => bad.video_frame_index = Some(12),
                "missing-frame" => bad.video_frame_index = None,
                "early-video" => bad.video_device_samples = Some(23999),
                "missing-video-position" => bad.video_device_samples = None,
                _ => bad.error_code = "DEVICE_AUDIO_STREAM_ERROR".into(),
            }
            assert!(validate_submission(&bad, &target).is_err(), "{case}");
        }
    }
    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "spawns native output process to verify revoke during preparation cleanup; no Provider"]
    fn output_prewarm_revoke_reclaims_unconfirmed_child() {
        let binary = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("nai-output-keeper.exe");
        assert!(binary.is_file());
        let dir = tempfile::tempdir().unwrap();
        let rgba = [16, 32, 48, 255];
        use sha2::{Digest, Sha256};
        let digest = |bytes: &[u8]| -> String {
            Sha256::digest(bytes)
                .iter()
                .map(|v| format!("{v:02x}"))
                .collect()
        };
        let hash = digest(&rgba);
        let raw = nai_contracts::v1::LocalStandbyMedia {
            version: 1,
            session_id: "prewarm-revoke".into(),
            generation_epoch: 1,
            width: 1,
            height: 1,
            standby_sha256: hash.clone(),
        }
        .encode_to_vec();
        let path = dir.path().join("standby.pb");
        std::fs::write(&path, &raw).unwrap();
        std::fs::write(dir.path().join("standby.rgba"), rgba).unwrap();
        let output = Mutex::new(None);
        let mut checks = 0;
        let result = prewarm(
            &output,
            &binary,
            nai_contracts::v1::LocalOutputWarmup {
                descriptor_path: path.to_str().unwrap().into(),
                descriptor_sha256: digest(&raw),
                session_id: "prewarm-revoke".into(),
                generation_epoch: 1,
                remaining_budget_ms: 4000,
            },
            &hash,
            Instant::now() + Duration::from_secs(8),
            "UNUSED_TEST_CREDENTIAL",
            || {
                checks += 1;
                if checks == 3 {
                    Err(ErrorCode::StaleEpoch)
                } else {
                    Ok(())
                }
            },
        );
        assert!(matches!(result, Err(ErrorCode::StaleEpoch)));
        assert!(output.lock().unwrap().is_none());
        assert_eq!(checks, 3);
    }
    #[test]
    fn child_status_cannot_invent_completion_or_inject_diagnostic_text() {
        let mut status = LocalPlaybackStatus {
            version: 1,
            state: "submitted".into(),
            ..Default::default()
        };
        assert!(validate_status(&status).is_ok());
        status.actual_local_playback_completed = true;
        assert!(validate_status(&status).is_err());
        status.actual_local_playback_completed = false;
        status.error_code = "DEVICE_ERROR\nforged-success".into();
        assert!(validate_status(&status).is_err());
        status.error_code.clear();
        status.standby_sha256 = "g".repeat(64);
        assert!(validate_status(&status).is_err());
        status.standby_sha256 = "a".repeat(64);
        status.audio_stream_open_count = 1;
        assert!(validate_status(&status).is_ok());
        status.audio_stream_open_count = 2;
        assert!(validate_status(&status).is_err());
    }
}
