//! Explicit native-device rehearsal. Never called by production output.
use nai_contracts::v1::NativeDeviceSmokeReport;
use prost::Message;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    Expire,
    Mute,
    Stall,
}
impl Scenario {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "expire" => Some(Self::Expire),
            "mute" => Some(Self::Mute),
            "stall" => Some(Self::Stall),
            _ => None,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Expire => "expire",
            Self::Mute => "mute",
            Self::Stall => "stall",
        }
    }
}

/// Replace an earlier report before opening native resources. A failed attempt
/// must never leave a previous success readable as its result.
pub fn run(path: &Path, scenario: Scenario) -> Result<bool, std::io::Error> {
    let mut report = NativeDeviceSmokeReport {
        schema_version: 1,
        scope: "native-device-smoke".into(),
        scenario: scenario.name().into(),
        backend: if cfg!(target_os = "windows") {
            "wasapi/minifb"
        } else {
            "unavailable"
        }
        .into(),
        ..Default::default()
    };
    std::fs::write(path, report.encode_to_vec())?;
    #[cfg(target_os = "windows")]
    if let Err(code) = windows::run(&mut report, scenario) {
        report.error_code = code.into();
    }
    #[cfg(not(target_os = "windows"))]
    {
        report.error_code = "DEVICE_PLATFORM_UNAVAILABLE".into();
    }
    report.success = report.error_code.is_empty()
        && report.audio_opened
        && report.window_opened
        && report.copied_interleaved_samples > 0
        && report.window_updates > 0
        && report.silent_callbacks_after_stop >= 3
        && report.nonzero_callbacks_after_stop == 0;
    if !report.success && report.error_code.is_empty() {
        report.error_code = "DEVICE_SMOKE_INCOMPLETE".into();
    }
    std::fs::write(path, report.encode_to_vec())?;
    Ok(report.success)
}

/// Native callbacks may be larger than the requested period. Keep each gate
/// invocation bounded and advance its predicted playback time by frame offset.
#[cfg(any(test, target_os = "windows"))]
pub(crate) fn fill_native_buffer(
    chunk: &mut nai_media_core::device_callback::DeviceChunk,
    data: &mut [f32],
    rate: u32,
    channels: usize,
    now: std::time::Instant,
    latency: Option<std::time::Duration>,
) -> Result<usize, &'static str> {
    use std::time::Duration;
    data.fill(0.0);
    let latency = latency
        .filter(|v| *v <= Duration::from_millis(100))
        .ok_or("DEVICE_TIMING_UNAVAILABLE")?;
    if !(8000..=192000).contains(&rate)
        || !(1..=2).contains(&channels)
        || data.is_empty()
        || !data.len().is_multiple_of(channels)
        || data.len() / channels > rate as usize / 10
    {
        return Err("DEVICE_CALLBACK_TOO_LARGE");
    }
    let part_samples = (rate as usize / 100) * channels;
    let mut copied = 0;
    for (index, part) in data.chunks_mut(part_samples).enumerate() {
        let offset = Duration::from_nanos(
            ((index * part_samples / channels) as u64 * 1_000_000_000).div_ceil(u64::from(rate)),
        );
        copied += chunk.fill(part, now + latency + offset);
    }
    // A later slice may observe P0 after an earlier one copied samples. Do not
    // let the earlier prefix escape in the same native callback buffer.
    let copied_span = Duration::from_nanos(
        ((copied / channels) as u64 * 1_000_000_000).div_ceil(u64::from(rate)),
    );
    if chunk.revoked()
        || now.max(std::time::Instant::now()) + latency + copied_span >= chunk.deadline()
    {
        data.fill(0.0);
        return Ok(0);
    }
    Ok(copied)
}

#[cfg(target_os = "windows")]
mod windows {
    use super::*;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use nai_contracts::v1::{Approval, PerformanceCue, PlaybackLease, SessionMode};
    use nai_media_core::{MediaGate, Preplay};
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicU64, Ordering},
        },
        time::{Duration, Instant},
    };

    #[derive(Default)]
    struct Counters {
        callbacks: AtomicU64,
        copied: AtomicU64,
        silent: AtomicU64,
        late: AtomicU64,
        latency: AtomicU64,
        last_nonzero: AtomicU64,
        first_nonzero: AtomicU64,
        stop_us: AtomicU64,
        error: AtomicBool,
        invalid_timing: AtomicBool,
        oversize: AtomicBool,
        max_frames: AtomicU64,
    }

    pub fn run(
        report: &mut NativeDeviceSmokeReport,
        scenario: Scenario,
    ) -> Result<(), &'static str> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("DEVICE_AUDIO_UNAVAILABLE")?;
        let supported = device
            .supported_output_configs()
            .map_err(|_| "DEVICE_CONFIG_UNAVAILABLE")?
            .find(|c| {
                c.sample_format() == cpal::SampleFormat::F32
                    && (1..=2).contains(&c.channels())
                    && c.min_sample_rate().0 <= 48000
                    && c.max_sample_rate().0 >= 48000
            })
            .ok_or("DEVICE_FORMAT_UNSUPPORTED")?;
        let mut config = supported.with_sample_rate(cpal::SampleRate(48000)).config();
        config.buffer_size = cpal::BufferSize::Fixed(240);
        let rate = config.sample_rate.0;
        let channels = usize::from(config.channels);
        report.sample_rate = rate;
        report.channels = u32::from(config.channels);
        let mut window = minifb::Window::new(
            "nai DEVICE SMOKE - test tone; Esc = mute",
            320,
            180,
            minifb::WindowOptions::default(),
        )
        .map_err(|_| "DEVICE_WINDOW_UNAVAILABLE")?;
        report.window_opened = true;

        // This fixture is restricted to this explicitly named rehearsal entry.
        // No fake provider, approval or device result is exposed to production.
        let mut cue = PerformanceCue {
            session_id: "device-smoke".into(),
            utterance_id: "test-tone".into(),
            generation_epoch: 1,
            hash_algorithm: "sha256-canonical-v1".into(),
            approval_ref: "explicit-device-smoke".into(),
            policy_version: "device-smoke-v1".into(),
            expires_at_utc_ms: 5000,
            speech_text: "440Hz device test tone".into(),
            ..Default::default()
        };
        cue.candidate_hash = nai_contracts::candidate_hash(&cue);
        let approval = Approval {
            approval_id: cue.approval_ref.clone(),
            session_id: cue.session_id.clone(),
            generation_epoch: 1,
            candidate_hash: cue.candidate_hash.clone(),
            hash_algorithm: cue.hash_algorithm.clone(),
            actor_id: "explicit-smoke-invocation".into(),
            action_scope: "speak".into(),
            expires_at_utc_ms: 5000,
            ..Default::default()
        };
        let lease = PlaybackLease {
            lease_id: "one-shot-smoke".into(),
            session_id: cue.session_id.clone(),
            generation_epoch: 1,
            allowed_utterance_ids: vec![cue.utterance_id.clone()],
            mode: SessionMode::Rehearsal as i32,
            approval_refs: vec![approval.approval_id.clone()],
            expires_at_utc_ms: 1600,
            candidate_hashes: [(cue.utterance_id.clone(), cue.candidate_hash.clone())].into(),
            policy_revision: cue.policy_version.clone(),
            ..Default::default()
        };
        let proof = Preplay {
            cue: &cue,
            approval: &approval,
            facts: &[],
            mode: SessionMode::Rehearsal,
            guard_passed: true,
            permission_granted: true,
            supervision_until_utc_ms: None,
        };
        let pcm: Vec<f32> = (0..rate * 600 / 1000)
            .flat_map(|i| {
                let sample = (i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 0.025;
                std::iter::repeat_n(sample, channels)
            })
            .collect();
        let mut gate = MediaGate::new(&cue.session_id, 1);
        let emergency = gate.emergency();
        let counters = Arc::new(Counters::default());
        counters.stop_us.store(600000, Ordering::SeqCst);
        let c = counters.clone();
        let error = counters.clone();
        let audio_emergency = emergency.clone();
        let error_emergency = emergency.clone();
        let (sender, receiver) = std::sync::mpsc::sync_channel::<(
            Instant,
            nai_media_core::device_callback::DeviceChunk,
        )>(1);
        let mut active = None;
        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
                    data.fill(0.0);
                    if active.is_none() {
                        active = receiver.try_recv().ok();
                    }
                    let Some((start, chunk)) = active.as_mut() else {
                        return;
                    };
                    let now = Instant::now();
                    let elapsed = start.elapsed().as_micros() as u64;
                    c.callbacks.fetch_add(1, Ordering::Relaxed);
                    let stamp = info.timestamp();
                    let latency = stamp.playback.duration_since(&stamp.callback);
                    c.max_frames
                        .fetch_max((data.len() / channels) as u64, Ordering::Relaxed);
                    if let Some(latency) = latency {
                        c.latency
                            .fetch_max(latency.as_micros() as u64, Ordering::Relaxed);
                    }
                    let copied = match fill_native_buffer(chunk, data, rate, channels, now, latency)
                    {
                        Ok(copied) => copied,
                        Err("DEVICE_TIMING_UNAVAILABLE") => {
                            audio_emergency.mute();
                            c.invalid_timing.store(true, Ordering::Relaxed);
                            0
                        }
                        Err(_) => {
                            audio_emergency.mute();
                            c.oversize.store(true, Ordering::Relaxed);
                            0
                        }
                    };
                    c.copied.fetch_add(copied as u64, Ordering::Relaxed);
                    let nonzero = data.iter().any(|sample| *sample != 0.0);
                    if nonzero {
                        let _ = c.first_nonzero.compare_exchange(
                            0,
                            elapsed.max(1),
                            Ordering::Relaxed,
                            Ordering::Relaxed,
                        );
                        c.last_nonzero.store(elapsed, Ordering::Relaxed);
                    }
                    if elapsed >= c.stop_us.load(Ordering::SeqCst) {
                        if nonzero {
                            c.late.fetch_add(1, Ordering::Relaxed);
                        } else {
                            c.silent.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                },
                move |_| {
                    error_emergency.mute();
                    error.error.store(true, Ordering::SeqCst);
                },
                Some(Duration::from_secs(1)),
            )
            .map_err(|_| "DEVICE_STREAM_OPEN_FAILED")?;
        report.audio_opened = true;
        if counters.error.load(Ordering::SeqCst) {
            return Err("DEVICE_STREAM_ERROR");
        }
        // Opening a native stream is not authorization and may take longer
        // than the short lease. Grant only after it exists, before play().
        let start = Instant::now();
        gate.arm(lease, 1000, start)
            .map_err(|_| "DEVICE_SMOKE_AUTHORIZATION")?;
        gate.enqueue(&proof, 1000, start)
            .map_err(|_| "DEVICE_SMOKE_AUTHORIZATION")?;
        let chunk = gate
            .device_chunk(&proof, pcm, rate, channels, 1000, start)
            .map_err(|_| "DEVICE_SMOKE_AUTHORIZATION")?;
        sender
            .try_send((start, chunk))
            .map_err(|_| "DEVICE_SMOKE_TRANSFER_FAILED")?;
        stream.play().map_err(|_| "DEVICE_STREAM_START_FAILED")?;
        let mut pixels = vec![0u32; 320 * 180];
        let mut muted = false;
        let mut stalled = false;
        let mut cancelled = false;
        while start.elapsed() < Duration::from_millis(1400) {
            if !window.is_open() || window.is_key_down(minifb::Key::Escape) {
                emergency.mute();
                counters
                    .stop_us
                    .store(start.elapsed().as_micros() as u64, Ordering::SeqCst);
                cancelled = true;
                break;
            }
            let first_audio = counters.first_nonzero.load(Ordering::Relaxed);
            if scenario == Scenario::Mute
                && !muted
                && first_audio > 0
                && start.elapsed().as_micros() >= u128::from(first_audio + 100000)
            {
                emergency.mute();
                counters
                    .stop_us
                    .store(start.elapsed().as_micros() as u64, Ordering::SeqCst);
                muted = true;
            }
            if scenario == Scenario::Stall
                && !stalled
                && start.elapsed() >= Duration::from_millis(100)
            {
                std::thread::sleep(Duration::from_millis(900));
                stalled = true;
            }
            let live =
                start.elapsed().as_micros() < u128::from(counters.stop_us.load(Ordering::SeqCst));
            for (i, pixel) in pixels.iter_mut().enumerate() {
                *pixel = if live {
                    if (i / 320 / 20 + i % 320 / 20) % 2 == 0 {
                        0x237acb
                    } else {
                        0x42cf8b
                    }
                } else {
                    0
                };
            }
            window
                .update_with_buffer(&pixels, 320, 180)
                .map_err(|_| "DEVICE_WINDOW_UPDATE_FAILED")?;
            report.window_updates += 1;
            std::thread::sleep(Duration::from_millis(5));
        }
        emergency.mute();
        // Observe several real callbacks after cancellation, never a fake ACK.
        std::thread::sleep(Duration::from_millis(60));
        drop(stream);
        report.callbacks = counters.callbacks.load(Ordering::Relaxed);
        report.copied_interleaved_samples = counters.copied.load(Ordering::Relaxed);
        report.silent_callbacks_after_stop = counters.silent.load(Ordering::Relaxed);
        report.nonzero_callbacks_after_stop = counters.late.load(Ordering::Relaxed);
        report.max_predicted_latency_us = counters.latency.load(Ordering::Relaxed);
        report.last_nonzero_at_us = counters.last_nonzero.load(Ordering::Relaxed);
        report.stop_at_us = counters.stop_us.load(Ordering::SeqCst);
        report.elapsed_ms = start.elapsed().as_millis() as u64;
        report.max_callback_frames = counters.max_frames.load(Ordering::Relaxed);
        if cancelled {
            return Err("DEVICE_SMOKE_CANCELLED");
        }
        if counters.error.load(Ordering::SeqCst) {
            return Err("DEVICE_STREAM_ERROR");
        }
        if counters.invalid_timing.load(Ordering::Relaxed) {
            return Err("DEVICE_TIMING_UNAVAILABLE");
        }
        if counters.oversize.load(Ordering::Relaxed) {
            return Err("DEVICE_CALLBACK_TOO_LARGE");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nai_contracts::v1::{Approval, PerformanceCue, PlaybackLease, SessionMode};
    use nai_media_core::{MediaGate, Preplay};
    use std::time::{Duration, Instant};

    fn audio(now: Instant) -> (MediaGate, nai_media_core::device_callback::DeviceChunk) {
        let mut cue = PerformanceCue {
            session_id: "s".into(),
            utterance_id: "u".into(),
            generation_epoch: 1,
            hash_algorithm: "sha256-canonical-v1".into(),
            approval_ref: "a".into(),
            policy_version: "p".into(),
            expires_at_utc_ms: 2000,
            ..Default::default()
        };
        cue.candidate_hash = nai_contracts::candidate_hash(&cue);
        let approval = Approval {
            approval_id: "a".into(),
            session_id: "s".into(),
            generation_epoch: 1,
            candidate_hash: cue.candidate_hash.clone(),
            hash_algorithm: cue.hash_algorithm.clone(),
            actor_id: "test".into(),
            action_scope: "speak".into(),
            expires_at_utc_ms: 2000,
            ..Default::default()
        };
        let lease = PlaybackLease {
            lease_id: "l".into(),
            session_id: "s".into(),
            generation_epoch: 1,
            allowed_utterance_ids: vec!["u".into()],
            mode: SessionMode::Rehearsal as i32,
            approval_refs: vec!["a".into()],
            expires_at_utc_ms: 1600,
            candidate_hashes: [("u".into(), cue.candidate_hash.clone())].into(),
            policy_revision: "p".into(),
            ..Default::default()
        };
        let mut gate = MediaGate::new("s", 1);
        gate.arm(lease, 1000, now).unwrap();
        let proof = Preplay {
            cue: &cue,
            approval: &approval,
            facts: &[],
            mode: SessionMode::Rehearsal,
            guard_passed: true,
            permission_granted: true,
            supervision_until_utc_ms: None,
        };
        gate.enqueue(&proof, 1000, now).unwrap();
        let chunk = gate
            .device_chunk(&proof, vec![0.25; 28800], 48000, 1, 1000, now)
            .unwrap();
        (gate, chunk)
    }

    #[test]
    fn native_buffer_is_split_and_playback_offset_respects_deadline() {
        let now = Instant::now();
        let (_gate, mut chunk) = audio(now);
        let mut out = [1.0; 1440]; // 30ms native callback, three gate slices
        assert_eq!(
            fill_native_buffer(&mut chunk, &mut out, 48000, 1, now, Some(Duration::ZERO)),
            Ok(1440)
        );
        assert!(out.iter().all(|v| *v == 0.25));
        let (_gate2, mut chunk) = audio(now);
        assert_eq!(
            fill_native_buffer(
                &mut chunk,
                &mut out,
                48000,
                1,
                now + Duration::from_millis(560),
                Some(Duration::from_millis(20))
            ),
            Ok(480)
        );
        assert!(out[..480].iter().all(|v| *v == 0.25));
        assert!(out[480..].iter().all(|v| *v == 0.0));
    }

    #[test]
    fn unknown_timing_or_excessive_buffer_cannot_copy_audio() {
        let now = Instant::now();
        let (_gate, mut chunk) = audio(now);
        let mut out = [1.0; 4801];
        assert_eq!(
            fill_native_buffer(&mut chunk, &mut out, 48000, 1, now, None),
            Err("DEVICE_TIMING_UNAVAILABLE")
        );
        assert!(out.iter().all(|v| *v == 0.0));
        assert_eq!(
            fill_native_buffer(&mut chunk, &mut out, 48000, 1, now, Some(Duration::ZERO)),
            Err("DEVICE_CALLBACK_TOO_LARGE")
        );
        assert!(out.iter().all(|v| *v == 0.0));
        assert_eq!(
            fill_native_buffer(
                &mut chunk,
                &mut out[..48],
                48000,
                1,
                now,
                Some(Duration::from_millis(101))
            ),
            Err("DEVICE_TIMING_UNAVAILABLE")
        );
    }

    #[test]
    fn p0_clears_all_slices_of_a_native_callback() {
        let now = Instant::now();
        let (gate, mut chunk) = audio(now);
        let mut out = [1.0; 1440];
        gate.emergency().mute();
        assert_eq!(
            fill_native_buffer(&mut chunk, &mut out, 48000, 1, now, Some(Duration::ZERO)),
            Ok(0)
        );
        assert!(out.iter().all(|v| *v == 0.0));
        assert!(chunk.revoked());
    }
    #[test]
    fn a_stale_callback_timestamp_cannot_outlive_the_real_deadline() {
        let old = Instant::now() - Duration::from_millis(650);
        let (_gate, mut chunk) = audio(old);
        let mut out = [1.0; 480];
        assert_eq!(
            fill_native_buffer(&mut chunk, &mut out, 48000, 1, old, Some(Duration::ZERO)),
            Ok(0)
        );
        assert!(out.iter().all(|v| *v == 0.0));
    }
    #[test]
    fn scenario_is_explicit_and_closed() {
        assert_eq!(Scenario::parse("mute"), Some(Scenario::Mute));
        assert_eq!(Scenario::parse("play"), None);
        assert_eq!(Scenario::parse(""), None);
    }
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn unsupported_target_replaces_old_success_with_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.pb");
        std::fs::write(
            &path,
            NativeDeviceSmokeReport {
                success: true,
                ..Default::default()
            }
            .encode_to_vec(),
        )
        .unwrap();
        assert!(!run(&path, Scenario::Expire).unwrap());
        let report =
            NativeDeviceSmokeReport::decode(std::fs::read(path).unwrap().as_slice()).unwrap();
        assert_eq!(report.error_code, "DEVICE_PLATFORM_UNAVAILABLE");
        assert!(!report.success);
        assert!(!report.actual_local_playback_completed);
        assert_eq!(report.callbacks, 0);
    }
}
