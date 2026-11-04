//! Dev-only local bridge. Not the public-output keeper and not a MuseTalk renderer.
//!
//! Plays one PCM wav through the existing clocked WASAPI experiment. The public
//! execution gate is a fixture: its 512×288 identity is what that gate requires,
//! and it does not approve the 960×540 research frames. Those pixels stay in the
//! research process. Only the audio thread calls `ExecutionFence::allows`.
#![cfg(windows)]

use nai_audio_cpal::clocked_experiment::{ExperimentalFence, Probe, open_pcm_controlled};
use nai_contracts::{prepared_media_hash, public_media_hash, workbench::*};
use nai_runtime_core::{
    foundation::Operation,
    public_execution::{PURPOSE, PublicExecutionGate},
};
use std::{
    io::{self, BufRead, Write},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

fn elapsed_ms(base: Instant) -> u64 {
    u64::try_from(base.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn emit(line: &str) {
    let mut out = io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

fn id(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012}")
}

fn fixture(samples: u64) -> Result<(PublicExecutionGate, PublicMediaBundle, String), ()> {
    let frames = samples.div_ceil(800);
    let mut bundle = PublicMediaBundle {
        protocol_major: 1,
        bundle_id: id(1),
        workspace_id: id(2),
        session_id: id(3),
        epoch: 1,
        output_id: id(4),
        snapshot_id: id(5),
        snapshot_sha256: "a".repeat(64),
        facts_sha256: "b".repeat(64),
        approval_id: id(6),
        guard_permit_id: id(7),
        caption_sha256: "c".repeat(64),
        audio: Some(PublicMediaAudio {
            artifact_id: id(8),
            pcm_sha256: "d".repeat(64),
            sample_count: samples,
            sample_rate: 24000,
            channels: 1,
        }),
        visual: Some(PublicMediaVisual {
            artifact_id: id(9),
            package_sha256: "e".repeat(64),
            license_sha256: "f".repeat(64),
            width: 512,
            height: 288,
            fps: 30,
        }),
        lease_ms: 500,
        scope: DeliveryScope::Public as i32,
        ..Default::default()
    };
    let descriptor = PreparedMediaDescriptor {
        protocol_major: 1,
        workspace_id: bundle.workspace_id.clone(),
        task_id: bundle.audio.as_ref().unwrap().artifact_id.clone(),
        task_sha256: "a".repeat(64),
        audio: bundle.audio.clone(),
        visual: bundle.visual.clone(),
        expected_frames: frames,
        samples_per_frame: 800,
        first_frame_sha256: "e".repeat(64),
        pcm_format: "s16le-mono-24000".into(),
        pixel_format: "rgb32le".into(),
        renderer_id: "rms-sample-v1".into(),
    };
    bundle.prepared_media_sha256 = prepared_media_hash(&descriptor);
    bundle.approved_sha256 = public_media_hash(&bundle);
    let receipt = PreparedMediaReceipt {
        protocol_major: 1,
        descriptor_sha256: bundle.prepared_media_sha256.clone(),
        consumer_instance_id: bundle.output_id.clone(),
        pcm_sha256: bundle.audio.as_ref().unwrap().pcm_sha256.clone(),
        sample_count: samples,
        expected_frames: frames,
        first_frame_sha256: descriptor.first_frame_sha256.clone(),
        tail_frame: Some(PublicMediaFrameInfo {
            frame_index: frames,
            sample_start: (frames - 1) * 800,
            sample_count: u32::try_from(samples - (frames - 1) * 800).map_err(|_| ())?,
            sample_rate: 24000,
            width: 512,
            height: 288,
            pixels_sha256: "f".repeat(64),
            openness_q16: 1,
        }),
        ..Default::default()
    };
    let preparation = id(60);
    let gate = PublicExecutionGate::new(
        "f".repeat(64),
        bundle.clone(),
        &descriptor,
        &receipt,
        preparation.clone(),
    )
    .map_err(|_| ())?;
    Ok((gate, bundle, preparation))
}

fn initial_lease(
    bundle: &PublicMediaBundle,
    preparation: &str,
    challenge: u64,
) -> PublicExecutionLease {
    PublicExecutionLease {
        authorization: "f".repeat(64),
        output_id: bundle.output_id.clone(),
        bundle_sha256: bundle.approved_sha256.clone(),
        prepared_media_sha256: bundle.prepared_media_sha256.clone(),
        session_id: bundle.session_id.clone(),
        epoch: bundle.epoch,
        challenge,
        seq: 1,
        lease_ms: 350,
        purpose: PURPOSE.into(),
        preparation_id: preparation.to_string(),
    }
}

fn load_wav(path: &Path) -> Result<zeroize::Zeroizing<Vec<i16>>, &'static str> {
    let bytes = std::fs::read(path).map_err(|_| "bad_wav")?;
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("bad_wav");
    }
    let mut offset = 12usize;
    let mut audio_format = None;
    let mut channels = None;
    let mut rate = None;
    let mut bits = None;
    let mut data = None;
    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes(
            bytes[offset + 4..offset + 8]
                .try_into()
                .map_err(|_| "bad_wav")?,
        ) as usize;
        let start = offset + 8;
        let end = start.checked_add(size).ok_or("bad_wav")?;
        if end > bytes.len() {
            return Err("bad_wav");
        }
        if id == b"fmt " {
            if size < 16 {
                return Err("bad_wav");
            }
            audio_format = Some(u16::from_le_bytes(
                bytes[start..start + 2].try_into().map_err(|_| "bad_wav")?,
            ));
            channels = Some(u16::from_le_bytes(
                bytes[start + 2..start + 4]
                    .try_into()
                    .map_err(|_| "bad_wav")?,
            ));
            rate = Some(u32::from_le_bytes(
                bytes[start + 4..start + 8]
                    .try_into()
                    .map_err(|_| "bad_wav")?,
            ));
            bits = Some(u16::from_le_bytes(
                bytes[start + 14..start + 16]
                    .try_into()
                    .map_err(|_| "bad_wav")?,
            ));
        } else if id == b"data" && data.is_none() {
            data = Some(start..end);
        }
        offset = end + (size & 1);
    }
    if audio_format != Some(1) || channels != Some(1) || rate != Some(24000) || bits != Some(16) {
        return Err("bad_wav");
    }
    let range = data.ok_or("bad_wav")?;
    if !range.len().is_multiple_of(2) {
        return Err("bad_wav");
    }
    let mut pcm = Vec::with_capacity(range.len() / 2);
    for chunk in bytes[range].chunks_exact(2) {
        pcm.push(i16::from_le_bytes([chunk[0], chunk[1]]));
    }
    if pcm.is_empty() || pcm.len() > 480_000 {
        return Err("bad_wav");
    }
    Ok(zeroize::Zeroizing::new(pcm))
}

fn renew(
    mut gate: PublicExecutionGate,
    bundle: PublicMediaBundle,
    preparation: String,
    base: Instant,
    experimental: Arc<ExperimentalFence>,
    running: mpsc::Sender<()>,
    commands: mpsc::Receiver<&'static str>,
) -> &'static str {
    if gate.initialization_barrier().is_err() {
        return "barrier_failed";
    }
    let now = elapsed_ms(base);
    let Ok(challenge) = gate.challenge(now) else {
        return "challenge_failed";
    };
    // `seq` is the lease epoch. The challenge value is the gate's own sequence.
    let mut lease = initial_lease(&bundle, &preparation, challenge);
    let Ok(operation) = Operation::new(1000) else {
        return "operation_failed";
    };
    if gate.accept(&lease, elapsed_ms(base), &operation).is_err() {
        return "accept_failed";
    }
    if !experimental.activate() {
        gate.stop();
        return "activate_failed";
    }
    let _ = running.send(());
    let mut epoch = 2u64;
    loop {
        match commands.recv_timeout(Duration::from_millis(100)) {
            Ok(reason) => {
                gate.stop();
                return reason;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                gate.stop();
                return "control_lost";
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let now = elapsed_ms(base);
                let Ok(challenge) = gate.challenge(now) else {
                    gate.stop();
                    return "renewal_failed";
                };
                lease.challenge = challenge;
                lease.seq = epoch;
                let Ok(operation) = Operation::new(1000) else {
                    gate.stop();
                    return "renewal_failed";
                };
                if gate.accept(&lease, elapsed_ms(base), &operation).is_err() {
                    gate.stop();
                    return "renewal_failed";
                }
                epoch = epoch.saturating_add(1);
            }
        }
    }
}

fn acknowledge_pause(probe: &Probe) -> Option<(u64, u64)> {
    probe.set_hold(true);
    let deadline = Instant::now() + Duration::from_millis(800);
    let mut previous = None;
    while Instant::now() < deadline {
        if probe.failed.load(Ordering::Acquire) || probe.is_finished() {
            return None;
        }
        if probe.is_holding() {
            let sample = (
                probe.played.load(Ordering::Acquire),
                probe.submitted.load(Ordering::Acquire),
            );
            if previous == Some(sample) {
                return Some(sample);
            }
            previous = Some(sample);
        }
        thread::sleep(Duration::from_millis(20));
    }
    None
}

fn acknowledge_resume(probe: &Probe) -> Option<(u64, u64)> {
    probe.set_hold(false);
    let deadline = Instant::now() + Duration::from_millis(800);
    while Instant::now() < deadline {
        if probe.failed.load(Ordering::Acquire) || probe.is_finished() {
            return None;
        }
        if !probe.is_holding() {
            if probe.failed.load(Ordering::Acquire) || probe.is_finished() {
                return None;
            }
            return Some((
                probe.played.load(Ordering::Acquire),
                probe.submitted.load(Ordering::Acquire),
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
    None
}

fn finish(mut reason: &'static str, mut probe: Probe) -> u8 {
    if probe.stop_within(Duration::from_millis(500)).is_err() {
        let played = probe.played.load(Ordering::Acquire);
        let submitted = probe.submitted.load(Ordering::Acquire);
        emit(&format!(
            "BRIDGE_STOPPED reason=stop_timeout played={played} submitted={submitted}"
        ));
        // Drop would join the worker without a bound. Exit skips that join;
        // the process teardown closes the audio client.
        std::process::exit(1);
    }
    let played = probe.played.load(Ordering::Acquire);
    let submitted = probe.submitted.load(Ordering::Acquire);
    thread::sleep(Duration::from_millis(50));
    let played_after = probe.played.load(Ordering::Acquire);
    let submitted_after = probe.submitted.load(Ordering::Acquire);
    if played_after != played || submitted_after != submitted {
        reason = "progress_after_stop";
    }
    emit(&format!(
        "BRIDGE_STOPPED reason={reason} played={played_after} submitted={submitted_after}"
    ));
    match reason {
        "clip_end" | "stop" => 0,
        "bad_wav" | "protocol" | "go_timeout" | "go_closed" | "stopped_before_go"
        | "fixture_rejected" | "device_unavailable" => 2,
        _ => 1,
    }
}

fn run() -> Result<(), u8> {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: research_clock_bridge <wav>");
        emit("BRIDGE_STOPPED reason=usage played=0 submitted=0");
        return Err(2);
    };
    if args.next().is_some() {
        eprintln!("usage: research_clock_bridge <wav>");
        emit("BRIDGE_STOPPED reason=usage played=0 submitted=0");
        return Err(2);
    }
    let pcm = match load_wav(Path::new(&path)) {
        Ok(pcm) => pcm,
        Err(reason) => {
            emit(&format!(
                "BRIDGE_STOPPED reason={reason} played=0 submitted=0"
            ));
            return Err(2);
        }
    };
    let samples = pcm.len() as u64;
    let (gate, bundle, preparation) = match fixture(samples) {
        Ok(value) => value,
        Err(()) => {
            emit("BRIDGE_STOPPED reason=fixture_rejected played=0 submitted=0");
            return Err(2);
        }
    };
    let base = Instant::now();
    let fence = gate.callback_fence();
    let published = Arc::new(AtomicBool::new(false));
    let experimental = Arc::new(ExperimentalFence::externally_controlled());
    let published_audio = Arc::clone(&published);
    let probe = match open_pcm_controlled(
        Arc::clone(&experimental),
        pcm,
        Arc::new(move || {
            let allowed = fence.allows(elapsed_ms(base));
            published_audio.store(allowed, Ordering::Release);
            allowed
        }),
    ) {
        Ok(probe) => probe,
        Err(code) => {
            eprintln!("device open: {code}");
            emit("BRIDGE_STOPPED reason=device_unavailable played=0 submitted=0");
            return Err(2);
        }
    };
    emit(&format!("BRIDGE_READY samples={samples} rate=24000"));
    let (lines_tx, lines_rx) = mpsc::channel();
    thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if lines_tx.send(line).is_err() {
                break;
            }
        }
    });
    match lines_rx.recv_timeout(Duration::from_secs(8)) {
        Ok(line) if line.trim() == "GO" => {}
        Ok(line) if line.trim() == "STOP" => return Err(finish("stopped_before_go", probe)),
        Ok(_) => return Err(finish("protocol", probe)),
        Err(mpsc::RecvTimeoutError::Timeout) => return Err(finish("go_timeout", probe)),
        Err(mpsc::RecvTimeoutError::Disconnected) => return Err(finish("go_closed", probe)),
    }
    let (running_tx, running_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let experimental_for_gate = Arc::clone(&experimental);
    let worker = thread::spawn(move || {
        let reason = renew(
            gate,
            bundle,
            preparation,
            base,
            experimental_for_gate,
            running_tx,
            cmd_rx,
        );
        let _ = done_tx.send(reason);
    });
    if running_rx.recv_timeout(Duration::from_secs(2)).is_err() {
        let reason = done_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap_or("activate_failed");
        let _ = worker.join();
        return Err(finish(reason, probe));
    }
    let mut heard = false;
    let mut paused = false;
    let mut thread_done = false;
    let mut chosen = "watchdog";
    let mut last_clock = None;
    let watchdog = Instant::now() + Duration::from_secs(30);
    loop {
        if Instant::now() >= watchdog {
            break;
        }
        if let Ok(reason) = done_rx.try_recv() {
            chosen = reason;
            thread_done = true;
            break;
        }
        let played = probe.played.load(Ordering::Acquire);
        let submitted = probe.submitted.load(Ordering::Acquire);
        let allowed = published.load(Ordering::Acquire);
        let clock = (played, submitted, allowed);
        if last_clock != Some(clock) {
            emit(&format!(
                "CLOCK played={played} submitted={submitted} allowed={}",
                u8::from(allowed)
            ));
            last_clock = Some(clock);
        }
        if let Ok(line) = lines_rx.try_recv() {
            match line.trim() {
                "STOP" => {
                    chosen = "stop";
                    break;
                }
                "PAUSE" => {
                    if paused {
                        chosen = "protocol";
                        break;
                    }
                    match acknowledge_pause(&probe) {
                        Some((played_at_pause, submitted_at_pause)) => {
                            emit(&format!(
                                "PAUSED played={played_at_pause} submitted={submitted_at_pause}"
                            ));
                            paused = true;
                        }
                        None => {
                            chosen = "pause_failed";
                            break;
                        }
                    }
                }
                "RESUME" => {
                    if !paused {
                        chosen = "protocol";
                        break;
                    }
                    match acknowledge_resume(&probe) {
                        Some((played_at_resume, submitted_at_resume)) => {
                            emit(&format!(
                                "RESUMED played={played_at_resume} submitted={submitted_at_resume}"
                            ));
                            paused = false;
                        }
                        None => {
                            chosen = "resume_failed";
                            break;
                        }
                    }
                }
                "GO" => {}
                _ => {
                    chosen = "protocol";
                    break;
                }
            }
        }
        if allowed {
            heard = true;
        }
        if probe.failed.load(Ordering::Acquire) {
            chosen = "device_failed";
            break;
        }
        if played >= samples {
            chosen = "clip_end";
            break;
        }
        if heard && !allowed {
            chosen = "fence_closed";
            break;
        }
        if probe.is_finished() {
            chosen = "device_stopped";
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    if !thread_done {
        let _ = cmd_tx.send(chosen);
        if let Ok(reason) = done_rx.recv_timeout(Duration::from_secs(1)) {
            chosen = reason;
        }
    }
    let _ = worker.join();
    let code = finish(chosen, probe);
    if code == 0 { Ok(()) } else { Err(code) }
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(code) => std::process::ExitCode::from(code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_second_fixture_accepts_and_renews() {
        let (mut gate, bundle, preparation) = fixture(96_000).unwrap();
        let base = Instant::now();
        gate.initialization_barrier().unwrap();
        let mut lease = initial_lease(
            &bundle,
            &preparation,
            gate.challenge(elapsed_ms(base)).unwrap(),
        );
        gate.accept(&lease, elapsed_ms(base), &Operation::new(1000).unwrap())
            .unwrap();
        lease.challenge = gate.challenge(elapsed_ms(base)).unwrap();
        lease.seq = 2;
        gate.accept(&lease, elapsed_ms(base), &Operation::new(1000).unwrap())
            .unwrap();
        assert!(gate.callback_fence().allows(elapsed_ms(base)));
    }

    #[test]
    fn rejects_a_wav_that_is_not_mono_s16_24k() {
        let dir = std::env::temp_dir();
        let path = dir.join("nai-research-clock-bridge-empty.wav");
        std::fs::write(&path, b"RIFF----WAVE").unwrap();
        assert_eq!(load_wav(&path).unwrap_err(), "bad_wav");
        let _ = std::fs::remove_file(path);
    }
}
