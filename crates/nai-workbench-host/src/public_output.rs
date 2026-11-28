//! Fixed local public output child. The page supplies no path, token or media payload.
use super::{Reap, background_command};
use nai_contracts::{audition_wire as wire, workbench::*};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::mpsc,
    time::Duration,
};
use zeroize::{Zeroize, Zeroizing};
pub fn binary() -> Result<PathBuf, String> {
    #[cfg(debug_assertions)]
    if let Some(p) = std::env::var_os("NAI_PUBLIC_OUTPUT_KEEPER") {
        let p = PathBuf::from(p);
        if p.is_absolute() && p.is_file() {
            return Ok(p);
        }
        return Err("WB_ENGINE_UNAVAILABLE".into());
    }
    let p = std::env::current_exe()
        .map_err(|_| "WB_ENGINE_UNAVAILABLE")?
        .with_file_name(format!(
            "nai-public-output-keeper{}",
            std::env::consts::EXE_SUFFIX
        ));
    if p.is_file() {
        Ok(p)
    } else {
        Err("WB_ENGINE_UNAVAILABLE".into())
    }
}
pub fn stop(root: &Path) -> Result<(), String> {
    super::foundation::write_stop(root, "public-output.stop")
}
pub struct OutputOwner {
    child: Reap,
    input: mpsc::SyncSender<PublicOutputLease>,
    output: mpsc::Receiver<Result<PublicOutputStatus, std::io::Error>>,
    token: Zeroizing<String>,
    status: PublicOutputStatus,
    seq: u64,
    output_id: String,
    /// Epoch of the last execution lease sent to the keeper. Zero before that.
    lease_epoch: u64,
}
impl OutputOwner {
    pub fn open_at(binary: &Path, root: &Path, test_hidden: bool) -> Result<Self, String> {
        if !root.is_dir() {
            return Err("WB_DATA_ROOT_REJECTED".into());
        }
        super::foundation::read_stop(root, "public-output.stop")?;
        let mut bytes = [0; 32];
        ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut bytes)
            .map_err(|_| "WB_RANDOM_FAILED")?;
        let token = Zeroizing::new(bytes.iter().map(|b| format!("{b:02x}")).collect::<String>());
        bytes.fill(0);
        let mut command = background_command(binary);
        let output_id = super::foundation::random_id()?;
        command.env_clear();
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }

        command
            .arg("--standby")
            .arg("--stop-file")
            .arg(root.join("public-output.stop"))
            .env("NAI_PUBLIC_OUTPUT_TOKEN", token.as_str())
            .env("NAI_PUBLIC_OUTPUT_ID", &output_id)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if test_hidden {
            command.env("NAI_PUBLIC_OUTPUT_TEST_HIDDEN", "1");
        }
        let mut child = Reap(command.spawn().map_err(|_| "WB_ENGINE_SPAWN_FAILED")?);
        let mut input = child.0.stdin.take().ok_or("WB_PIPE_FAILED")?;
        let mut output = child.0.stdout.take().ok_or("WB_PIPE_FAILED")?;
        let (tx, rx) = mpsc::sync_channel::<PublicOutputLease>(1);
        std::thread::spawn(move || {
            while let Ok(mut r) = rx.recv() {
                let result = wire::write(&mut input, &r);
                r.authorization.zeroize();
                if let Some(x) = r.prepare.as_mut() {
                    x.execution_key.zeroize();
                }
                if let Some(x) = r.execution.as_mut() {
                    x.authorization.zeroize();
                }
                if result.is_err() {
                    break;
                }
            }
        });
        let (out, received) = mpsc::sync_channel(2);
        std::thread::spawn(move || {
            loop {
                let r = wire::read::<PublicOutputStatus>(&mut output);
                let failed = r.is_err();
                if out.send(r).is_err() || failed {
                    break;
                }
            }
        });
        let first = received
            .recv_timeout(Duration::from_secs(3))
            .map_err(|_| "WB_DEADLINE_EXCEEDED")?
            .map_err(|_| "WB_PROTOCOL_ERROR")?;
        validate(&first, 0)?;
        if first.output_id != output_id {
            return Err("WB_PROTOCOL_ERROR".into());
        }
        if first.seq != 0 || first.terminal {
            return Err("WB_PROTOCOL_ERROR".into());
        }
        let mut owner = Self {
            child,
            input: tx,
            output: received,
            token,
            status: first,
            seq: 0,
            output_id,
            lease_epoch: 0,
        };
        owner.poll()?;
        Ok(owner)
    }
    pub fn poll(&mut self) -> Result<PublicOutputStatus, String> {
        self.poll_command(None, false, None, false)
    }
    pub fn clear_prepared(&mut self) -> Result<(), String> {
        self.poll_command(None, true, None, false).map(|_| ())
    }
    pub fn prepare(&mut self, x: PublicWindowPreparation) -> Result<PublicWindowPrepared, String> {
        if x.output_id != self.output_id {
            return Err("WB_PROTOCOL_ERROR".into());
        }
        let preparation_id = x.preparation_id.clone();
        let descriptor = x.prepared_descriptor.as_ref().ok_or("WB_PROTOCOL_ERROR")?;
        let hash = nai_contracts::prepared_media_hash(descriptor);
        let audio = descriptor.audio.clone().ok_or("WB_PROTOCOL_ERROR")?;
        self.poll_command(Some(Box::new(x)), false, None, false)?;
        let until = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let s = self.poll()?;
            if s.terminal {
                return Err("WB_PUBLIC_OUTPUT_UNAVAILABLE".into());
            }
            if let Some(v) = s.window_prepared {
                if v.receipt.is_some() && v.preparation_id != preparation_id {
                    continue;
                }
                if let Some(r) = &v.receipt {
                    if r.consumer_instance_id != self.output_id
                        || r.descriptor_sha256 != hash
                        || r.pcm_sha256 != audio.pcm_sha256
                        || r.sample_count != audio.sample_count
                    {
                        return Err("WB_PROTOCOL_ERROR".into());
                    }
                } else if v.device_ready {
                    return Err("WB_PROTOCOL_ERROR".into());
                }
                if v.submitted_samples != 0 {
                    return Err("WB_PROTOCOL_ERROR".into());
                }
                return Ok(*v);
            }
            if std::time::Instant::now() >= until {
                let _ = self.clear_prepared();
                return Err("WB_DEADLINE_EXCEEDED".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    pub fn fresh_execution(&mut self) -> Result<PublicOutputStatus, String> {
        let old = self
            .status
            .window_prepared
            .as_ref()
            .map_or(0, |p| p.execution_challenge);
        self.poll_command(None, false, None, true)?;
        let until = std::time::Instant::now() + Duration::from_millis(200);
        loop {
            let s = self.poll()?;
            if let Some(p) = s.window_prepared.as_ref() {
                if p.submission_complete {
                    return Ok(s);
                }
                if p.execution_challenge > 0 && p.execution_challenge != old {
                    return Ok(s);
                }
                if p.execution_stopped {
                    return Err("WB_PUBLIC_OUTPUT_UNAVAILABLE".into());
                }
            }
            if std::time::Instant::now() >= until {
                return Err("WB_DEADLINE_EXCEEDED".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    pub fn execute(&mut self, x: PublicExecutionLease) -> Result<PublicWindowPrepared, String> {
        let seq = x.seq;
        let preparation = x.preparation_id.clone();
        self.poll_command(None, false, Some(Box::new(x)), false)?;
        let until = std::time::Instant::now() + Duration::from_millis(200);
        loop {
            let s = self.poll()?;
            if let Some(p) = s.window_prepared {
                if p.preparation_id != preparation {
                    return Err("WB_PROTOCOL_ERROR".into());
                }
                if p.submission_complete {
                    return Ok(*p);
                }
                if p.execution_seq == seq && (p.playing || p.submission_complete) {
                    return Ok(*p);
                }
                if p.execution_stopped {
                    return Err("WB_PUBLIC_OUTPUT_UNAVAILABLE".into());
                }
            }
            if std::time::Instant::now() >= until {
                let _ = self.clear_prepared();
                return Err("WB_DEADLINE_EXCEEDED".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn poll_command(
        &mut self,
        prepare: Option<Box<PublicWindowPreparation>>,
        clear_prepared: bool,
        execution: Option<Box<PublicExecutionLease>>,
        refresh_execution_challenge: bool,
    ) -> Result<PublicOutputStatus, String> {
        if self
            .child
            .0
            .try_wait()
            .map_err(|_| "WB_ENGINE_LOST")?
            .is_some()
        {
            return Err("WB_ENGINE_LOST".into());
        }
        if self.status.terminal {
            return Ok(self.status.clone());
        }
        self.seq = self.seq.checked_add(1).ok_or("WB_PROTOCOL_ERROR")?;
        let sent_epoch = execution.as_ref().map(|lease| lease.epoch);
        self.input
            .try_send(PublicOutputLease {
                authorization: self.token.to_string(),
                seq: self.seq,
                challenge: self.status.challenge,
                lease_ms: 500,
                prepare,
                clear_prepared,
                execution,
                refresh_execution_challenge,
            })
            .map_err(|_| "WB_BUSY")?;
        if let Some(epoch) = sent_epoch {
            self.lease_epoch = epoch;
        }
        let status = self
            .output
            .recv_timeout(Duration::from_millis(800))
            .map_err(|_| "WB_DEADLINE_EXCEEDED")?
            .map_err(|_| "WB_ENGINE_LOST")?;
        validate(&status, self.lease_epoch)?;
        if status.output_id != self.output_id {
            return Err("WB_PROTOCOL_ERROR".into());
        }
        if status.seq > self.seq
            || (!status.terminal && status.seq != self.seq)
            || status.standby_frames < self.status.standby_frames
        {
            return Err("WB_PROTOCOL_ERROR".into());
        }
        self.status = status.clone();
        Ok(status)
    }
    /// OS child identity for diagnostics; never accepted from web content.
    pub fn process_id(&self) -> u32 {
        self.child.0.id()
    }
    pub fn current(&self) -> PublicOutputStatus {
        self.status.clone()
    }
}
fn validate(s: &PublicOutputStatus, authorized_epoch: u64) -> Result<(), String> {
    if s.output_id.len() != 36
        || !s.output_id.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
    {
        return Err("WB_PROTOCOL_ERROR".into());
    }
    let v = s.visual.as_ref().ok_or("WB_PROTOCOL_ERROR")?;
    let sha = |s: &str| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    if v.artifact_id != "d074f2a8-a618-4b43-9e24-679bf43a0001"
        || v.width != 512
        || v.height != 288
        || v.fps != 30
        || !sha(&v.package_sha256)
        || !sha(&v.license_sha256)
        || s.visual_name != "内置简洁2D形象"
        || s.visual_license != "MIT"
    {
        return Err("WB_PROTOCOL_ERROR".into());
    }
    if let Some(p) = s.window_prepared.as_ref() {
        // A clock is optional. Absence means this stream has no hardware reading.
        // Presence is only a bound observation, not playback completion.
        if s.terminal || p.device_name.len() > 128 {
            return Err("WB_PROTOCOL_ERROR".into());
        }
        let caption =
            PublicCaptionState::try_from(p.caption_state).map_err(|_| "WB_PROTOCOL_ERROR")?;
        if p.caption_ready != (caption == PublicCaptionState::Ready) {
            return Err("WB_PROTOCOL_ERROR".into());
        }
        if p.receipt.is_some() {
            let r = p.receipt.as_ref().ok_or("WB_PROTOCOL_ERROR")?;
            let tail = r.tail_frame.as_ref().ok_or("WB_PROTOCOL_ERROR")?;
            if ![
                "DEVICE_INITIALIZED_MUTED",
                "PUBLIC_EXECUTING",
                "EXECUTION_STOPPED",
                "SUBMISSION_COMPLETE",
                "DEVICE_LOST",
            ]
            .contains(&p.reason.as_str())
                || p.reason == "PUBLIC_EXECUTING" && !p.playing
                || p.reason == "EXECUTION_STOPPED" && !p.execution_stopped
                || p.reason == "DEVICE_LOST" && (!p.execution_stopped || p.device_clock.is_some())
                || p.reason == "SUBMISSION_COMPLETE" && !p.submission_complete
                || !p.caption_ready
                || !sha(&p.caption_sha256)
                || !sha(&p.caption_font_sha256)
                || ![
                    "windows-system-font:simhei.ttf",
                    "windows-system-font:msyh.ttc",
                ]
                .contains(&p.caption_font.as_str())
                || p.preparation_id.len() != 36
                || p.submitted_samples > r.sample_count
                || p.playing
                    && (!p.device_ready
                        || p.execution_stopped
                        || p.submission_complete
                        || p.execution_seq == 0)
                || p.submission_complete
                    && (!p.execution_stopped
                        || p.device_ready
                        || p.submitted_samples != r.sample_count)
                || p.execution_stopped
                    && (p.playing || p.device_ready || p.execution_challenge != 0)
                || p.reason == "DEVICE_INITIALIZED_MUTED"
                    && (p.submitted_samples != 0
                        || p.playing
                        || !p.device_ready
                        || p.device_clock.is_some())
                || p.device_name.is_empty()
                || !device_clock_matches(s, p, r, authorized_epoch)
                || r.protocol_major != 1
                || r.consumer_instance_id != s.output_id
                || !sha(&r.descriptor_sha256)
                || !sha(&r.pcm_sha256)
                || !sha(&r.first_frame_sha256)
                || r.sample_count == 0
                || r.sample_count > 480000
                || r.expected_frames != r.sample_count.div_ceil(800)
                || tail.frame_index != r.expected_frames
                || tail.sample_start != (r.expected_frames - 1) * 800
                || u64::from(tail.sample_count) != r.sample_count - tail.sample_start
                || tail.sample_rate != 24000
                || tail.width != 512
                || tail.height != 288
                || !sha(&tail.pixels_sha256)
                || tail.openness_q16 > 65535
            {
                return Err("WB_PROTOCOL_ERROR".into());
            }
        } else if p.device_clock.is_some()
            || p.receipt.is_some()
            || !p.device_name.is_empty()
            || WorkbenchError::from_str_name(&p.reason)
                .is_none_or(|e| e == WorkbenchError::Unspecified)
        {
            return Err("WB_PROTOCOL_ERROR".into());
        }
    }
    if let Some(v) = s.presentation.as_ref() {
        let p = s.window_prepared.as_ref().ok_or("WB_PROTOCOL_ERROR")?;
        let r = p.receipt.as_ref().ok_or("WB_PROTOCOL_ERROR")?;
        if !p.playing
            || v.preparation_id != p.preparation_id
            || !v.submission_clock
            || v.frame_index == 0
            || v.frame_index > r.expected_frames
            || v.sample_start != (v.frame_index - 1) * 800
            || v.caption_sha256 != p.caption_sha256
            || !sha(&v.composited_sha256)
            || v.presentation_count == 0
            || v.presentation_count > s.standby_frames
        {
            return Err("WB_PROTOCOL_ERROR".into());
        }
    }
    let reason = PublicOutputReason::try_from(s.reason).map_err(|_| "WB_PROTOCOL_ERROR")?;
    if s.protocol_major != 1
        || !s.window_open
        || (reason == PublicOutputReason::MediaActive) != s.audio_active
        || s.standby == s.audio_active
        || s.live_ready != s.audio_active
        || s.audio_active != s.window_prepared.as_ref().is_some_and(|p| p.playing)
        || s.standby_frames == 0
        || reason == PublicOutputReason::Unspecified
        || s.terminal
            != (!matches!(
                reason,
                PublicOutputReason::StandbyOnly | PublicOutputReason::MediaActive
            ))
        || s.terminal != (s.challenge == 0)
    {
        return Err("WB_PROTOCOL_ERROR".into());
    }
    Ok(())
}

/// A present observation must pass the shared structural bound against this
/// status and the lease epoch last sent to the keeper. Muted init omits the
/// clock because that epoch is still 0. This status has no cue field, so the
/// sentence ceiling passed here is the submission cursor already on the status.
/// Absence still means unavailable.
fn device_clock_matches(
    status: &PublicOutputStatus,
    prepared: &PublicWindowPrepared,
    receipt: &PreparedMediaReceipt,
    authorized_epoch: u64,
) -> bool {
    let Some(clock) = prepared.device_clock.as_ref() else {
        return true;
    };
    nai_contracts::device_clock_bound(
        clock,
        &status.output_id,
        &prepared.preparation_id,
        prepared.submitted_samples,
        receipt.sample_count,
        authorized_epoch,
        prepared.submitted_samples,
    )
}

/// Encode only validated standby status for the desktop IPC boundary.
pub fn encoded(status: PublicOutputStatus) -> Vec<u8> {
    use prost::Message;
    status.encode_to_vec()
}

#[cfg(test)]
mod device_clock_boundary_tests {
    use super::*;

    fn standby() -> PublicOutputStatus {
        PublicOutputStatus {
            protocol_major: 1,
            challenge: 1,
            window_open: true,
            standby: true,
            standby_frames: 1,
            reason: PublicOutputReason::StandbyOnly as i32,
            output_id: "00000000-0000-0000-0000-000000000001".into(),
            visual: Some(PublicMediaVisual {
                artifact_id: "d074f2a8-a618-4b43-9e24-679bf43a0001".into(),
                package_sha256: "a".repeat(64),
                license_sha256: "b".repeat(64),
                width: 512,
                height: 288,
                fps: 30,
            }),
            visual_name: "内置简洁2D形象".into(),
            visual_license: "MIT".into(),
            window_prepared: Some(Box::new(PublicWindowPrepared {
                reason: WorkbenchError::CapabilityUnavailable.as_str_name().into(),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    fn sha(seed: char) -> String {
        seed.to_string().repeat(64)
    }

    fn executing(
        submitted: u64,
        clock: Option<PublicDeviceClockObservation>,
    ) -> PublicOutputStatus {
        let output_id = "00000000-0000-4000-8000-000000000004";
        let preparation_id = "00000000-0000-4000-8000-000000000060";
        PublicOutputStatus {
            protocol_major: 1,
            challenge: 1,
            window_open: true,
            standby: false,
            live_ready: true,
            audio_active: true,
            standby_frames: 1,
            reason: PublicOutputReason::MediaActive as i32,
            output_id: output_id.into(),
            visual: Some(PublicMediaVisual {
                artifact_id: "d074f2a8-a618-4b43-9e24-679bf43a0001".into(),
                package_sha256: sha('a'),
                license_sha256: sha('b'),
                width: 512,
                height: 288,
                fps: 30,
            }),
            visual_name: "内置简洁2D形象".into(),
            visual_license: "MIT".into(),
            window_prepared: Some(Box::new(PublicWindowPrepared {
                receipt: Some(PreparedMediaReceipt {
                    protocol_major: 1,
                    descriptor_sha256: sha('c'),
                    consumer_instance_id: output_id.into(),
                    pcm_sha256: sha('d'),
                    sample_count: 96000,
                    expected_frames: 120,
                    first_frame_sha256: sha('e'),
                    tail_frame: Some(PublicMediaFrameInfo {
                        frame_index: 120,
                        sample_start: 95200,
                        sample_count: 800,
                        sample_rate: 24000,
                        width: 512,
                        height: 288,
                        pixels_sha256: sha('f'),
                        openness_q16: 1,
                    }),
                }),
                device_ready: true,
                device_name: "Speakers".into(),
                reason: "PUBLIC_EXECUTING".into(),
                submitted_samples: submitted,
                preparation_id: preparation_id.into(),
                execution_challenge: 1,
                execution_seq: 1,
                playing: true,
                caption_ready: true,
                caption_sha256: sha('a'),
                caption_font: "windows-system-font:simhei.ttf".into(),
                caption_font_sha256: sha('b'),
                caption_state: PublicCaptionState::Ready as i32,
                device_clock: clock,
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    fn bound_clock(played: u64) -> PublicDeviceClockObservation {
        PublicDeviceClockObservation {
            output_id: "00000000-0000-4000-8000-000000000004".into(),
            preparation_id: "00000000-0000-4000-8000-000000000060".into(),
            session_id: "00000000-0000-4000-8000-000000000003".into(),
            epoch: 1,
            stream_generation: 1,
            device_frequency: 384000,
            device_position: 436992,
            sample_rate: 24000,
            played_samples: played,
        }
    }

    fn muted(clock: Option<PublicDeviceClockObservation>) -> PublicOutputStatus {
        let mut status = executing(0, clock);
        status.audio_active = false;
        status.live_ready = false;
        status.standby = true;
        status.reason = PublicOutputReason::StandbyOnly as i32;
        let prepared = status.window_prepared.as_mut().unwrap();
        prepared.reason = "DEVICE_INITIALIZED_MUTED".into();
        prepared.playing = false;
        prepared.submitted_samples = 0;
        prepared.execution_seq = 0;
        prepared.execution_stopped = false;
        prepared.device_ready = true;
        prepared.execution_challenge = 1;
        status
    }

    #[test]
    fn unverified_device_clock_rejected_without_breaking_absent_clock() {
        let mut status = standby();
        assert!(validate(&status, 0).is_ok());
        status.window_prepared.as_mut().unwrap().device_clock =
            Some(PublicDeviceClockObservation::default());
        assert_eq!(validate(&status, 0), Err("WB_PROTOCOL_ERROR".into()));
    }

    #[test]
    fn muted_init_rejects_a_sentence_clock_before_the_execution_lease() {
        assert!(validate(&muted(None), 0).is_ok());
        assert!(validate(&muted(Some(bound_clock(0))), 0).is_err());
        assert!(validate(&muted(Some(bound_clock(0))), 1).is_err());
    }

    #[test]
    fn bound_device_clock_stays_within_the_submitted_cursor() {
        assert!(validate(&executing(27840, None), 1).is_ok());
        assert!(validate(&executing(27840, Some(bound_clock(27312))), 1).is_ok());
        assert!(validate(&executing(27840, Some(bound_clock(0))), 1).is_ok());
        assert!(validate(&executing(27840, Some(bound_clock(27840))), 1).is_ok());
        assert!(validate(&executing(27840, Some(bound_clock(27841))), 1).is_err());
        let mut ahead = bound_clock(27312);
        ahead.device_position = 0;
        assert!(validate(&executing(27840, Some(ahead)), 1).is_err());
        let mut other = bound_clock(27312);
        other.output_id = "00000000-0000-4000-8000-000000000099".into();
        assert!(validate(&executing(27840, Some(other)), 1).is_err());
        let mut generation = bound_clock(27312);
        generation.stream_generation = 0;
        assert!(validate(&executing(27840, Some(generation)), 1).is_err());
        let mut frequency = bound_clock(27312);
        frequency.device_frequency = 0;
        assert!(validate(&executing(27840, Some(frequency)), 1).is_err());
        let mut rate = bound_clock(27312);
        rate.sample_rate = 48000;
        assert!(validate(&executing(27840, Some(rate)), 1).is_err());
        let mut epoch = bound_clock(27312);
        epoch.epoch = 0;
        assert!(validate(&executing(27840, Some(epoch)), 1).is_err());
        let mut later_epoch = bound_clock(27312);
        later_epoch.epoch = 2;
        assert!(validate(&executing(27840, Some(later_epoch)), 1).is_err());
        assert!(validate(&executing(27840, Some(bound_clock(27312))), 2).is_err());
        let mut session = bound_clock(27312);
        session.session_id.clear();
        assert!(validate(&executing(27840, Some(session)), 1).is_err());
    }

    fn lost(clock: Option<PublicDeviceClockObservation>) -> PublicOutputStatus {
        let mut status = executing(27840, clock);
        status.audio_active = false;
        status.live_ready = false;
        status.standby = true;
        status.reason = PublicOutputReason::StandbyOnly as i32;
        let prepared = status.window_prepared.as_mut().unwrap();
        prepared.reason = "DEVICE_LOST".into();
        prepared.execution_stopped = true;
        prepared.playing = false;
        prepared.device_ready = false;
        prepared.execution_challenge = 0;
        status
    }

    #[test]
    fn device_lost_rejects_a_stale_clock() {
        assert!(validate(&lost(None), 1).is_ok());
        assert!(validate(&lost(Some(bound_clock(27312))), 1).is_err());
        let mut playing = lost(None);
        playing.audio_active = true;
        playing.live_ready = true;
        playing.standby = false;
        playing.reason = PublicOutputReason::MediaActive as i32;
        playing.window_prepared.as_mut().unwrap().execution_stopped = false;
        playing.window_prepared.as_mut().unwrap().playing = true;
        playing.window_prepared.as_mut().unwrap().device_ready = true;
        playing
            .window_prepared
            .as_mut()
            .unwrap()
            .execution_challenge = 1;
        assert!(validate(&playing, 1).is_err());
    }
}
