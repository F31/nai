//! Explicit approved local output. Raw PCM/RGBA never enter control frames.
use nai_contracts::v1::LocalPlaybackStatus;
use prost::Message;
use std::path::Path;

fn send_status(status: &LocalPlaybackStatus) -> Result<(), &'static str> {
    use std::io::Write;
    let mut output = std::io::stdout().lock();
    nai_plugin_sdk::frame::write(&mut output, &status.encode_to_vec())
        .map_err(|_| "DEVICE_STATUS_IO")?;
    output.flush().map_err(|_| "DEVICE_STATUS_IO")
}

pub fn run(path: &Path) -> Result<(), &'static str> {
    #[cfg(target_os = "windows")]
    let result = windows::run(path);
    #[cfg(not(target_os = "windows"))]
    let result: Result<LocalPlaybackStatus, &'static str> = {
        let _ = path;
        Err("DEVICE_PLATFORM_UNAVAILABLE")
    };
    let status = match &result {
        Ok(status) => status.clone(),
        Err(code) => LocalPlaybackStatus {
            version: 1,
            state: "failed".into(),
            error_code: (*code).into(),
            ..Default::default()
        },
    };
    send_status(&status)?;
    result.map(|_| ())
}

/// Explicit session output; EOF revokes speech and may retain admitted neutral standby.
pub fn serve() -> Result<(), &'static str> {
    #[cfg(target_os = "windows")]
    {
        windows::serve()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("DEVICE_PLATFORM_UNAVAILABLE")
    }
}

#[cfg(any(test, target_os = "windows"))]
mod media {
    use super::*;
    use nai_contracts::v1::LocalPlaybackMedia;
    use sha2::{Digest, Sha256};
    use std::{
        fs,
        io::{Read, Seek, SeekFrom},
    };
    pub fn hash(bytes: &[u8]) -> String {
        Sha256::digest(bytes)
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect()
    }
    pub struct Neutral {
        pub spec: nai_contracts::v1::LocalStandbyMedia,
        pub rgba: Vec<u8>,
    }
    pub fn load_neutral(
        command: &nai_contracts::v1::LocalOutputWarmup,
    ) -> Result<Neutral, &'static str> {
        if command.descriptor_path.is_empty()
            || command.descriptor_path.len() > 4096
            || command.session_id.is_empty()
            || command.session_id.len() > 128
            || command.remaining_budget_ms == 0
            || command.remaining_budget_ms > 4000
        {
            return Err("DEVICE_WARMUP_INVALID");
        }
        let path = Path::new(&command.descriptor_path);
        let raw = read(path, 4096)?;
        if hash(&raw) != command.descriptor_sha256 {
            return Err("DEVICE_DESCRIPTOR_INTEGRITY");
        }
        let spec = nai_contracts::v1::LocalStandbyMedia::decode(raw.as_slice())
            .map_err(|_| "DEVICE_DESCRIPTOR_INVALID")?;
        let size = u64::from(spec.width) * u64::from(spec.height) * 4;
        if spec.version != 1
            || spec.session_id != command.session_id
            || spec.generation_epoch != command.generation_epoch
            || spec.width == 0
            || spec.height == 0
            || spec.width > 4096
            || spec.height > 4096
            || size > 16 * 1024 * 1024
        {
            return Err("DEVICE_WARMUP_INVALID");
        }
        let rgba = read(&path.with_file_name("standby.rgba"), size as usize)?;
        if rgba.len() != size as usize || hash(&rgba) != spec.standby_sha256 {
            return Err("DEVICE_MEDIA_INTEGRITY");
        }
        Ok(Neutral { spec, rgba })
    }
    fn read(path: &Path, max: usize) -> Result<Vec<u8>, &'static str> {
        let meta = fs::symlink_metadata(path).map_err(|_| "DEVICE_MEDIA_MISSING")?;
        if !meta.file_type().is_file() || meta.len() > max as u64 {
            return Err("DEVICE_MEDIA_LIMIT");
        }
        let mut out = Vec::new();
        fs::File::open(path)
            .map_err(|_| "DEVICE_MEDIA_IO")?
            .take(max as u64 + 1)
            .read_to_end(&mut out)
            .map_err(|_| "DEVICE_MEDIA_IO")?;
        if out.len() > max {
            return Err("DEVICE_MEDIA_LIMIT");
        }
        Ok(out)
    }
    pub struct Media {
        pub spec: LocalPlaybackMedia,
        pub descriptor_hash: String,
        pub pcm: Vec<i16>,
        pub frames: FrameStore,
        pub standby: Option<Vec<u8>>,
    }
    pub enum FrameStore {
        Legacy {
            rgba: Vec<u8>,
            frame_size: usize,
        },
        Indexed {
            file: fs::File,
            hashes: Vec<String>,
            frame_size: usize,
        },
    }
    impl FrameStore {
        pub fn read_frame(
            &mut self,
            index: usize,
            buffer: &mut Vec<u8>,
        ) -> Result<(), &'static str> {
            match self {
                Self::Legacy { rgba, frame_size } => {
                    let start = index.checked_mul(*frame_size).ok_or("DEVICE_MEDIA_LIMIT")?;
                    let end = start.checked_add(*frame_size).ok_or("DEVICE_MEDIA_LIMIT")?;
                    let frame = rgba.get(start..end).ok_or("DEVICE_FRAME_RANGE")?;
                    buffer.clear();
                    buffer.extend_from_slice(frame);
                }
                Self::Indexed {
                    file,
                    hashes,
                    frame_size,
                } => {
                    let expected = hashes.get(index).ok_or("DEVICE_FRAME_RANGE")?;
                    buffer.resize(*frame_size, 0);
                    file.seek(SeekFrom::Start(index as u64 * *frame_size as u64))
                        .and_then(|_| file.read_exact(buffer))
                        .map_err(|_| "DEVICE_FRAME_IO")?;
                    if hash(buffer) != *expected {
                        return Err("DEVICE_FRAME_INTEGRITY");
                    }
                }
            }
            Ok(())
        }
    }
    fn digest(value: &str) -> bool {
        value.len() == 64
            && value
                .bytes()
                .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
    }
    pub fn load(path: &Path) -> Result<Media, &'static str> {
        let raw = read(path, 256 * 1024)?;
        let spec =
            LocalPlaybackMedia::decode(raw.as_slice()).map_err(|_| "DEVICE_MEDIA_PROTOCOL")?;
        let pixels = u64::from(spec.width)
            .checked_mul(u64::from(spec.height))
            .and_then(|v| v.checked_mul(4))
            .ok_or("DEVICE_MEDIA_LIMIT")?;
        let frames_len = pixels
            .checked_mul(u64::from(spec.frame_count))
            .ok_or("DEVICE_MEDIA_LIMIT")?;
        if !matches!(spec.version, 1 | 2)
            || spec.session_id.is_empty()
            || spec.generation_epoch == 0
            || spec.candidate_hash.len() != 64
            || !(8000..=48000).contains(&spec.sample_rate)
            || spec.sample_count == 0
            || spec.sample_count > u64::from(spec.sample_rate) * 20
            || spec.width == 0
            || spec.height == 0
            || spec.width > 4096
            || spec.height > 4096
            || (spec.version == 1 && frames_len > 64 * 1024 * 1024)
            || (spec.version == 2
                && (frames_len > 512 * 1024 * 1024
                    || pixels > 16 * 1024 * 1024
                    || spec.frame_count > 2400
                    || spec.frame_sha256.len() != spec.frame_count as usize
                    || !spec.frame_sha256.iter().all(|h| digest(h))
                    || !digest(&spec.standby_sha256)
                    || !digest(&spec.frames_sha256)))
            || spec.frame_count == 0
            || spec.fps_num == 0
            || spec.fps_num > 120
            || spec.fps_den == 0
            || spec.fps_den > 1001
            || spec.frame_count
                != nai_media_core::frame_count(
                    spec.sample_count,
                    spec.sample_rate,
                    spec.fps_num,
                    spec.fps_den,
                )
        {
            return Err("DEVICE_MEDIA_LIMIT");
        }
        let dir = path.parent().ok_or("DEVICE_MEDIA_MISSING")?;
        let audio = read(&dir.join("audio.pcm"), 1920000)?;
        if audio.len() as u64 != spec.sample_count * 2 || hash(&audio) != spec.audio_sha256 {
            return Err("DEVICE_MEDIA_INTEGRITY");
        }
        let frames = if spec.version == 1 {
            let rgba = read(&dir.join("frames.rgba"), 64 * 1024 * 1024)?;
            if rgba.len() as u64 != frames_len || hash(&rgba) != spec.frames_sha256 {
                return Err("DEVICE_MEDIA_INTEGRITY");
            }
            FrameStore::Legacy {
                rgba,
                frame_size: pixels as usize,
            }
        } else {
            let path = dir.join("frames.rgba");
            if !fs::symlink_metadata(&path)
                .map_err(|_| "DEVICE_MEDIA_MISSING")?
                .file_type()
                .is_file()
            {
                return Err("DEVICE_MEDIA_LIMIT");
            }
            let file = fs::File::open(path).map_err(|_| "DEVICE_MEDIA_IO")?;
            if file.metadata().map_err(|_| "DEVICE_MEDIA_IO")?.len() != frames_len {
                return Err("DEVICE_MEDIA_INTEGRITY");
            }
            FrameStore::Indexed {
                file,
                hashes: spec.frame_sha256.clone(),
                frame_size: pixels as usize,
            }
        };
        let standby = if spec.version == 2 {
            let bytes = read(&dir.join("standby.rgba"), pixels as usize)?;
            if bytes.len() as u64 != pixels || hash(&bytes) != spec.standby_sha256 {
                return Err("DEVICE_STANDBY_INTEGRITY");
            }
            Some(bytes)
        } else {
            None
        };
        let pcm = audio
            .chunks_exact(2)
            .map(|v| i16::from_le_bytes([v[0], v[1]]))
            .collect();
        Ok(Media {
            spec,
            descriptor_hash: hash(&raw),
            pcm,
            frames,
            standby,
        })
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::*;

    use nai_contracts::v1::{
        LocalOutputCommand, LocalPlaybackGrant, SessionMode, local_output_command::Command,
    };
    use nai_media_core::{MediaGate, Preplay};
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicU64, Ordering},
            mpsc,
        },
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };
    fn utc() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    fn window() -> Result<minifb::Window, &'static str> {
        minifb::Window::new(
            "nai Live Output",
            640,
            360,
            minifb::WindowOptions::default(),
        )
        .map_err(|_| "DEVICE_WINDOW_UNAVAILABLE")
    }

    fn input(resident: bool, cancelled: Arc<AtomicBool>) -> mpsc::Receiver<Command> {
        let (tx, rx) = mpsc::sync_channel(4);
        std::thread::spawn(move || {
            let mut input = std::io::stdin();
            while let Ok(Some(raw)) = nai_plugin_sdk::frame::read(
                &mut input,
                Instant::now() + Duration::from_secs(1800),
                &cancelled,
            ) {
                let command = if resident {
                    LocalOutputCommand::decode(raw.as_slice())
                        .ok()
                        .and_then(|v| v.command)
                } else {
                    LocalPlaybackGrant::decode(raw.as_slice())
                        .ok()
                        .map(|grant| Command::Grant(Box::new(grant)))
                };
                if let Some(command) = command
                    && tx.try_send(command).is_ok()
                {
                    continue;
                }
                break;
            }
            cancelled.store(true, Ordering::SeqCst);
        });
        rx
    }

    struct Standby {
        pixels: Vec<u32>,
        width: usize,
        height: usize,
        hash: String,
    }
    impl Standby {
        fn present(&self, window: &mut minifb::Window) -> Result<(), &'static str> {
            window
                .update_with_buffer(&self.pixels, self.width, self.height)
                .map_err(|_| "DEVICE_WINDOW_UPDATE_FAILED")
        }
    }
    fn rgb(rgba: &[u8]) -> Vec<u32> {
        rgba.chunks_exact(4)
            .map(|p| {
                let a = u32::from(p[3]);
                ((u32::from(p[0]) * a / 255) << 16)
                    | ((u32::from(p[1]) * a / 255) << 8)
                    | (u32::from(p[2]) * a / 255)
            })
            .collect()
    }

    use crate::{local_audio::Packet, local_wasapi::AudioSession};

    pub fn run(path: &Path) -> Result<LocalPlaybackStatus, &'static str> {
        let audio = AudioSession::open()?;
        let mut window = window()?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let rx = input(false, cancelled.clone());
        play(path, &mut window, &rx, &cancelled, &audio, &mut None)
    }

    pub fn serve() -> Result<(), &'static str> {
        let audio = AudioSession::open()?;
        let mut window = window()?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let rx = input(true, cancelled.clone());
        let started = Instant::now();
        let mut standby = None::<Standby>;
        while window.is_open()
            && !window.is_key_down(minifb::Key::Escape)
            && started.elapsed() < Duration::from_secs(1800)
        {
            audio.health()?;
            // Parent EOF removes all speech authority; an admitted neutral
            // image may remain until local close/session cap (INV-21).
            if cancelled.load(Ordering::SeqCst) {
                if let Some(image) = &standby {
                    image.present(&mut window)?;
                } else {
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            match rx.try_recv() {
                Ok(Command::Warmup(command)) => {
                    let started = Instant::now();
                    // There is no cue, packet or speech lease on this path.
                    let prepared = (|| {
                        let neutral = media::load_neutral(&command)?;
                        let image = Standby {
                            pixels: rgb(&neutral.rgba),
                            width: neutral.spec.width as usize,
                            height: neutral.spec.height as usize,
                            hash: neutral.spec.standby_sha256,
                        };
                        if cancelled.load(Ordering::SeqCst)
                            || started.elapsed()
                                >= Duration::from_millis(u64::from(command.remaining_budget_ms))
                        {
                            return Err("DEVICE_WARMUP_EXPIRED");
                        }
                        audio.health()?;
                        image.present(&mut window)?;
                        Ok(image)
                    })();
                    let status = match prepared {
                        Ok(image) => {
                            let hash = image.hash.clone();
                            standby = Some(image);
                            LocalPlaybackStatus {
                                version: 1,
                                state: "standby".into(),
                                copied_samples: Some(0),
                                window_updates: Some(1),
                                audio_stream_open_count: audio.open_count,
                                standby_sha256: hash,
                                ..Default::default()
                            }
                        }
                        Err(code) => LocalPlaybackStatus {
                            version: 1,
                            state: "failed".into(),
                            error_code: code.into(),
                            audio_stream_open_count: audio.open_count,
                            ..Default::default()
                        },
                    };
                    send_status(&status)?;
                    if status.state == "failed" {
                        return Err("DEVICE_WARMUP_FAILED");
                    }
                }
                Ok(Command::DescriptorPath(path)) if path.len() <= 4096 => {
                    let status = match play(
                        Path::new(&path),
                        &mut window,
                        &rx,
                        &cancelled,
                        &audio,
                        &mut standby,
                    ) {
                        Ok(status) => status,
                        Err(code) => {
                            if let Some(image) = &standby {
                                image.present(&mut window)?;
                            }
                            LocalPlaybackStatus {
                                version: 1,
                                state: "failed".into(),
                                error_code: code.into(),
                                audio_stream_open_count: audio.open_count,
                                standby_sha256: if audio.health().is_ok() {
                                    standby
                                        .as_ref()
                                        .map(|image| image.hash.clone())
                                        .unwrap_or_default()
                                } else {
                                    String::new()
                                },
                                ..Default::default()
                            }
                        }
                    };
                    if send_status(&status).is_err() && !cancelled.load(Ordering::SeqCst) {
                        return Err("DEVICE_STATUS_IO");
                    }
                    if !matches!(status.state.as_str(), "submitted" | "cancelled")
                        && (standby.is_none() || audio.health().is_err())
                    {
                        return Err("DEVICE_CUE_STOPPED");
                    }
                }
                Ok(Command::Shutdown(true)) => return Ok(()),
                Ok(Command::CancelCue(true)) | Ok(Command::Grant(_)) => {}
                Ok(_) => return Err("DEVICE_COMMAND_ORDER"),
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
                Err(mpsc::TryRecvError::Empty) => {}
            }
            if let Some(image) = &standby {
                image.present(&mut window)?;
            } else {
                window.update();
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }

    struct RevokeOnDrop(nai_media_core::EmergencyHandle);
    impl Drop for RevokeOnDrop {
        fn drop(&mut self) {
            self.0.mute();
        }
    }

    fn play(
        path: &Path,
        window: &mut minifb::Window,
        grants_rx: &mpsc::Receiver<Command>,
        cancelled: &Arc<AtomicBool>,
        audio: &AudioSession,
        standby: &mut Option<Standby>,
    ) -> Result<LocalPlaybackStatus, &'static str> {
        let mut media = media::load(path)?;
        let pcm = nai_media_core::timeline::resample::resample_linear_i16(
            &media.pcm,
            media.spec.sample_rate,
            48000,
            960000,
        )
        .map_err(|_| "DEVICE_RESAMPLE_LIMIT")?;
        audio.health()?;
        let channels = audio.channels;
        let mut gate =
            MediaGate::with_epoch_floor(&media.spec.session_id, 1, media.spec.generation_epoch - 1);
        let emergency = gate.emergency();
        let copied = Arc::new(AtomicU64::new(0));
        let drained = Arc::new(AtomicU64::new(0));
        let played = Arc::new(AtomicU64::new(0));
        let cue_failure = Arc::new(AtomicU64::new(0));
        let _revoke_on_drop = RevokeOnDrop(emergency.clone());
        send_status(&LocalPlaybackStatus {
            version: 1,
            state: "ready".into(),
            media_sha256: media.descriptor_hash.clone(),
            audio_stream_open_count: audio.open_count,
            ..Default::default()
        })?;
        let start = Instant::now();
        let mut last_seq = 0;
        let mut current = None;
        let mut queued = 0;
        let mut updates = 0;
        let mut frame_buffer = Vec::new();
        media.frames.read_frame(0, &mut frame_buffer)?;
        let mut pixels = rgb(&frame_buffer);
        let mut displayed = Some(0);
        let mut state = "cancelled";
        let mut announced_running = false;
        let mut video = nai_media_core::video_clock::VideoCursor::new(
            pcm.len() as u64,
            48000,
            media.spec.fps_num,
            media.spec.fps_den,
            media.spec.frame_count,
        )
        .map_err(|_| "DEVICE_VIDEO_CLOCK_INVALID")?;
        while start.elapsed() < Duration::from_secs(24) {
            if cancelled.load(Ordering::SeqCst)
                || !window.is_open()
                || window.is_key_down(minifb::Key::Escape)
            {
                break;
            }
            if let Ok(command) = grants_rx.try_recv() {
                if matches!(command, Command::CancelCue(true)) {
                    break;
                }
                let Command::Grant(grant) = command else {
                    return Err("DEVICE_COMMAND_ORDER");
                };
                let cue = grant.cue.as_ref().ok_or("DEVICE_PROOF_MISSING")?;
                let approval = grant.approval.as_ref().ok_or("DEVICE_PROOF_MISSING")?;
                let lease = grant.lease.clone().ok_or("DEVICE_PROOF_MISSING")?;
                if grant.version != 1
                    || grant.seq <= last_seq
                    || grant.media_sha256 != media.descriptor_hash
                    || cue.candidate_hash != media.spec.candidate_hash
                    || cue.session_id != media.spec.session_id
                    || cue.generation_epoch != media.spec.generation_epoch
                    || lease.mode != SessionMode::Assisted as i32
                    || approval.candidate_hash != cue.candidate_hash
                {
                    emergency.mute();
                    return Err("DEVICE_PROOF_MISMATCH");
                }
                if current.is_none() {
                    gate.arm(lease, utc(), Instant::now())
                } else {
                    gate.renew(lease, utc(), Instant::now())
                }
                .map_err(|_| "DEVICE_LEASE_REJECTED")?;
                last_seq = grant.seq;
                current = Some(grant);
            }
            if current.is_none() {
                if start.elapsed() > Duration::from_secs(2) {
                    return Err("DEVICE_GRANT_TIMEOUT");
                }
                window.update();
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            if gate.poll(Instant::now()).is_err()
                || audio.health().is_err()
                || cue_failure.load(Ordering::SeqCst) != 0
            {
                break;
            }
            let grant = current.as_ref().unwrap();
            let cue = grant.cue.as_ref().unwrap();
            let approval = grant.approval.as_ref().unwrap();
            let facts = &grant.lease.as_ref().unwrap().fact_versions;
            let proof = Preplay {
                cue,
                approval,
                facts,
                mode: SessionMode::Assisted,
                guard_passed: true,
                permission_granted: true,
                supervision_until_utc_ms: None,
            };
            if queued < pcm.len()
                && queued.saturating_sub(copied.load(Ordering::SeqCst) as usize) < 9600
            {
                // At most one queued 200ms block plus the active block.
                let end = (queued + 9600).min(pcm.len());
                let block: Vec<f32> = pcm[queued..end]
                    .iter()
                    .flat_map(|v| std::iter::repeat_n(*v as f32 / 32768.0 * 0.15, channels))
                    .collect();
                gate.enqueue(&proof, utc(), Instant::now())
                    .map_err(|_| "DEVICE_PREPLAY_REJECTED")?;
                let chunk = gate
                    .device_chunk(&proof, block, 48000, channels, utc(), Instant::now())
                    .map_err(|_| "DEVICE_PREPLAY_REJECTED")?;
                if let Some(rgba) = media.standby.take() {
                    *standby = Some(Standby {
                        pixels: rgb(&rgba),
                        width: media.spec.width as usize,
                        height: media.spec.height as usize,
                        hash: media.spec.standby_sha256.clone(),
                    });
                }
                match audio.tx.try_send(Packet {
                    chunk,
                    copied: copied.clone(),
                    failure: cue_failure.clone(),
                    drained: drained.clone(),
                    played: played.clone(),
                }) {
                    Ok(()) => queued = end,
                    Err(mpsc::TrySendError::Full(_)) => {}
                    Err(_) => return Err("DEVICE_AUDIO_CHANNEL_CLOSED"),
                }
            }
            // Read played first: the device thread may advance both counters between loads.
            let device_position = played.load(Ordering::SeqCst);
            let position = copied.load(Ordering::SeqCst);
            let video_frame = video
                .select(device_position, position)
                .map_err(|_| "DEVICE_VIDEO_CLOCK_INVALID")?;
            let frame_index = video_frame.index() as usize;
            if displayed != Some(frame_index) {
                media.frames.read_frame(frame_index, &mut frame_buffer)?;
                pixels = rgb(&frame_buffer);
                displayed = Some(frame_index);
            }
            // Frame IO can take time: an old lease must not authorize this window update.
            if cancelled.load(Ordering::SeqCst)
                || gate.poll(Instant::now()).is_err()
                || audio.health().is_err()
                || cue_failure.load(Ordering::SeqCst) != 0
            {
                break;
            }
            window
                .update_with_buffer(
                    &pixels,
                    media.spec.width as usize,
                    media.spec.height as usize,
                )
                .map_err(|_| "DEVICE_WINDOW_UPDATE_FAILED")?;
            updates += 1;
            video
                .submitted(video_frame)
                .map_err(|_| "DEVICE_VIDEO_CLOCK_INVALID")?;
            if position > 0 && !announced_running {
                send_status(&LocalPlaybackStatus {
                    version: 1,
                    state: "running".into(),
                    media_sha256: media.descriptor_hash.clone(),
                    copied_samples: Some(position),
                    window_updates: Some(updates),
                    audio_stream_open_count: audio.open_count,
                    ..Default::default()
                })?;
                announced_running = true;
            }
            if position == pcm.len() as u64 && video.tail_submitted() {
                state = "submitted";
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if state == "submitted"
            && (cancelled.load(Ordering::SeqCst) || gate.poll(Instant::now()).is_err())
        {
            state = "cancelled";
        }
        emergency.mute();
        if let Some(image) = standby.as_ref() {
            image.present(window)?;
        } else {
            pixels.fill(0);
            if !pixels.is_empty() {
                let _ = window.update_with_buffer(
                    &pixels,
                    media.spec.width as usize,
                    media.spec.height as usize,
                );
            }
        }
        audio.health()?;
        if cancelled.load(Ordering::SeqCst) {
            state = "cancelled";
        }
        match cue_failure.load(Ordering::SeqCst) {
            0 => {}
            3 => return Err("DEVICE_AUDIO_CHUNK_REJECTED"),
            _ => return Err("DEVICE_AUDIO_FINAL_DEADLINE"),
        }
        let video_tail = if state == "submitted" {
            video.tail()
        } else {
            None
        };
        Ok(LocalPlaybackStatus {
            version: 1,
            state: state.into(),
            copied_samples: Some(copied.load(Ordering::SeqCst)),
            media_sha256: media.descriptor_hash.clone(),
            callback_drained_samples: Some(drained.load(Ordering::SeqCst)),
            callback_drain_observed: false,
            device_played_samples: Some(played.load(Ordering::SeqCst)),
            device_position_observed: state == "submitted",
            audio_backend: "wasapi-iaudioclock".into(),
            video_frame_index: video_tail.map(|v| v.0),
            video_device_samples: video_tail.map(|v| v.1),
            video_tail_submitted: video_tail.is_some(),
            window_updates: Some(updates),
            audio_stream_open_count: audio.open_count,
            standby_sha256: standby
                .as_ref()
                .map(|image| image.hash.clone())
                .unwrap_or_default(),
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nai_contracts::v1::LocalPlaybackMedia;

    #[test]
    fn neutral_preparation_checks_independent_descriptor_pixels_identity_and_caps() {
        use nai_contracts::v1::{LocalOutputWarmup, LocalStandbyMedia};
        let dir = tempfile::tempdir().unwrap();
        let rgba = vec![16, 32, 48, 255];
        let mut spec = LocalStandbyMedia {
            version: 1,
            session_id: "s".into(),
            generation_epoch: 2,
            width: 1,
            height: 1,
            standby_sha256: media::hash(&rgba),
        };
        let path = dir.path().join("standby.pb");
        let raw = spec.encode_to_vec();
        std::fs::write(&path, &raw).unwrap();
        std::fs::write(dir.path().join("standby.rgba"), &rgba).unwrap();
        let command = LocalOutputWarmup {
            descriptor_path: path.to_str().unwrap().into(),
            descriptor_sha256: media::hash(&raw),
            session_id: "s".into(),
            generation_epoch: 2,
            remaining_budget_ms: 4000,
        };
        let image = media::load_neutral(&command).unwrap();
        assert_eq!(image.rgba, rgba);
        assert_eq!(image.spec.standby_sha256, media::hash(&rgba));
        for bad in [
            LocalOutputWarmup {
                remaining_budget_ms: 0,
                ..command.clone()
            },
            LocalOutputWarmup {
                remaining_budget_ms: 4001,
                ..command.clone()
            },
            LocalOutputWarmup {
                generation_epoch: 1,
                ..command.clone()
            },
            LocalOutputWarmup {
                session_id: "other".into(),
                ..command.clone()
            },
            LocalOutputWarmup {
                descriptor_sha256: "0".repeat(64),
                ..command.clone()
            },
        ] {
            assert!(media::load_neutral(&bad).is_err());
        }
        std::fs::write(dir.path().join("standby.rgba"), [0; 4]).unwrap();
        assert!(matches!(
            media::load_neutral(&command),
            Err("DEVICE_MEDIA_INTEGRITY")
        ));
        spec.width = u32::MAX;
        let raw = spec.encode_to_vec();
        std::fs::write(&path, &raw).unwrap();
        assert!(matches!(
            media::load_neutral(&LocalOutputWarmup {
                descriptor_sha256: media::hash(&raw),
                ..command
            }),
            Err("DEVICE_WARMUP_INVALID")
        ));
    }

    fn fixture(dir: &Path) -> LocalPlaybackMedia {
        let audio = vec![0u8; 16];
        let rgba = vec![127u8; 4];
        let spec = LocalPlaybackMedia {
            version: 1,
            session_id: "s".into(),
            generation_epoch: 1,
            candidate_hash: "a".repeat(64),
            sample_rate: 8000,
            sample_count: 8,
            width: 1,
            height: 1,
            frame_count: 1,
            fps_num: 25,
            fps_den: 1,
            audio_sha256: media::hash(&audio),
            frames_sha256: media::hash(&rgba),
            ..Default::default()
        };
        std::fs::write(dir.join("audio.pcm"), audio).unwrap();
        std::fs::write(dir.join("frames.rgba"), rgba).unwrap();
        std::fs::write(dir.join("media.pb"), spec.encode_to_vec()).unwrap();
        spec
    }
    #[test]
    fn bounded_raw_media_has_independent_identity_and_hashes() {
        let dir = tempfile::tempdir().unwrap();
        let spec = fixture(dir.path());
        let mut raw = media::load(&dir.path().join("media.pb")).unwrap();
        assert_eq!(raw.spec, spec);
        assert_eq!(raw.pcm.len(), 8);
        let mut frame = Vec::new();
        raw.frames.read_frame(0, &mut frame).unwrap();
        assert_eq!(frame.len(), 4);
        assert_eq!(raw.descriptor_hash, media::hash(&spec.encode_to_vec()));
        std::fs::write(dir.path().join("frames.rgba"), [0u8; 4]).unwrap();
        assert!(matches!(
            media::load(&dir.path().join("media.pb")),
            Err("DEVICE_MEDIA_INTEGRITY")
        ));
    }
    #[test]
    fn twenty_second_media_boundary_preserves_all_samples() {
        let dir = tempfile::tempdir().unwrap();
        let mut spec = fixture(dir.path());
        let audio = vec![0u8; 8000 * 20 * 2];
        let rgba = vec![127u8; 500 * 4];
        spec.sample_count = 8000 * 20;
        spec.frame_count = 500;
        spec.audio_sha256 = media::hash(&audio);
        spec.frames_sha256 = media::hash(&rgba);
        std::fs::write(dir.path().join("audio.pcm"), audio).unwrap();
        std::fs::write(dir.path().join("frames.rgba"), rgba).unwrap();
        std::fs::write(dir.path().join("media.pb"), spec.encode_to_vec()).unwrap();
        assert_eq!(
            media::load(&dir.path().join("media.pb")).unwrap().pcm.len(),
            160000
        );
        spec.sample_count += 1;
        spec.frame_count += 1;
        std::fs::write(dir.path().join("media.pb"), spec.encode_to_vec()).unwrap();
        assert!(matches!(
            media::load(&dir.path().join("media.pb")),
            Err("DEVICE_MEDIA_LIMIT")
        ));
    }
    #[test]
    fn indexed_frames_over_64mib_are_read_individually_and_rechecked() {
        use std::io::{Seek, SeekFrom, Write};
        let dir = tempfile::tempdir().unwrap();
        let mut spec = fixture(dir.path());
        let frame = vec![0u8; 512 * 512 * 4];
        let audio = vec![0u8; 22400 * 2]; // 70 frames at 25 fps.
        spec.version = 2;
        spec.width = 512;
        spec.height = 512;
        spec.frame_count = 70;
        spec.sample_count = 22400;
        spec.frame_sha256 = vec![media::hash(&frame); 70];
        spec.standby_sha256 = media::hash(&frame);
        spec.audio_sha256 = media::hash(&audio);
        std::fs::write(dir.path().join("audio.pcm"), audio).unwrap();
        std::fs::write(dir.path().join("standby.rgba"), &frame).unwrap();
        let path = dir.path().join("frames.rgba");
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        file.set_len(70 * frame.len() as u64).unwrap();
        std::fs::write(dir.path().join("media.pb"), spec.encode_to_vec()).unwrap();
        let mut loaded = media::load(&dir.path().join("media.pb")).unwrap();
        assert!(matches!(loaded.frames, media::FrameStore::Indexed { .. }));
        let mut buffer = Vec::new();
        loaded.frames.read_frame(69, &mut buffer).unwrap();
        assert_eq!(buffer, frame);
        assert_eq!(loaded.standby.as_ref().unwrap().len(), frame.len());
        file.seek(SeekFrom::Start(0)).unwrap();
        file.write_all(&[1; 16]).unwrap();
        file.flush().unwrap();
        assert!(matches!(
            loaded.frames.read_frame(0, &mut buffer),
            Err("DEVICE_FRAME_INTEGRITY")
        ));
        file.set_len(16).unwrap();
        assert!(matches!(
            loaded.frames.read_frame(69, &mut buffer),
            Err("DEVICE_FRAME_IO")
        ));
        spec.width = 1024;
        spec.height = 1024;
        spec.frame_count = 130;
        spec.sample_count = 41600;
        spec.frame_sha256 = vec![media::hash(&frame); 130];
        std::fs::write(dir.path().join("media.pb"), spec.encode_to_vec()).unwrap();
        assert!(matches!(
            media::load(&dir.path().join("media.pb")),
            Err("DEVICE_MEDIA_LIMIT")
        ));
        // Invalid quota/standby proof is rejected before opening a device.
        spec.width = 4096;
        spec.height = 4096;
        std::fs::write(dir.path().join("media.pb"), spec.encode_to_vec()).unwrap();
        assert!(matches!(
            media::load(&dir.path().join("media.pb")),
            Err("DEVICE_MEDIA_LIMIT")
        ));
    }
    #[test]
    fn sample_duration_and_frame_size_cannot_escape_caps() {
        let dir = tempfile::tempdir().unwrap();
        let mut spec = fixture(dir.path());
        spec.sample_count = 160001;
        spec.frame_count = 501;
        std::fs::write(dir.path().join("media.pb"), spec.encode_to_vec()).unwrap();
        assert!(matches!(
            media::load(&dir.path().join("media.pb")),
            Err("DEVICE_MEDIA_LIMIT")
        ));
        spec.sample_count = 8;
        spec.width = u32::MAX;
        std::fs::write(dir.path().join("media.pb"), spec.encode_to_vec()).unwrap();
        assert!(matches!(
            media::load(&dir.path().join("media.pb")),
            Err("DEVICE_MEDIA_LIMIT")
        ));
        fixture(dir.path());
        std::fs::write(dir.path().join("audio.pcm"), [0u8; 15]).unwrap();
        assert!(matches!(
            media::load(&dir.path().join("media.pb")),
            Err("DEVICE_MEDIA_INTEGRITY")
        ));
    }
}
