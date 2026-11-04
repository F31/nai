#![cfg(windows)]
//! Silent default-device evidence. Fixture hashes are not output authorization.
use nai_audio_cpal::clocked_experiment::{ExperimentalFence, open_pcm_controlled};
use nai_contracts::{device_clock_bound, prepared_media_hash, public_media_hash, workbench::*};
use nai_media_core::{
    played_frame::{PlayedFrameBoard, Shown},
    public_avatar::BuiltinAvatar,
    public_clip::{PublicClipRenderer, pcm_hash},
    public_cue::{CueClockGate, CueReading, map_public_cue},
};
use nai_runtime_core::{
    foundation::Operation,
    public_execution::{PURPOSE, PublicExecutionGate},
};
use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

fn id(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012}")
}

#[test]
#[ignore = "explicit Windows silent device; not output authorization"]
fn silent_device_selects_frames_from_played_samples() {
    let pcm = vec![0i16; 24_000];
    let avatar = BuiltinAvatar::new(&|| Ok(())).unwrap();
    let audio = PublicMediaAudio {
        artifact_id: id(8),
        pcm_sha256: pcm_hash(&pcm, &|| Ok(())).unwrap(),
        sample_count: pcm.len() as u64,
        sample_rate: 24_000,
        channels: 1,
    };
    let mut renderer = PublicClipRenderer::new(&pcm, &audio, &avatar, &|| Ok(())).unwrap();
    let first = renderer
        .frame_at(0, &|| Ok(()))
        .unwrap()
        .unwrap()
        .info
        .pixels_sha256
        .clone();
    let frames = audio.sample_count.div_ceil(800);
    let visual = PublicMediaVisual {
        artifact_id: id(9),
        package_sha256: "e".repeat(64),
        license_sha256: "f".repeat(64),
        width: 512,
        height: 288,
        fps: 30,
        ..Default::default()
    };
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
        audio: Some(audio.clone()),
        visual: Some(visual.clone()),
        lease_ms: 500,
        scope: DeliveryScope::Public as i32,
        ..Default::default()
    };
    let descriptor = PreparedMediaDescriptor {
        protocol_major: 1,
        workspace_id: bundle.workspace_id.clone(),
        task_id: audio.artifact_id.clone(),
        task_sha256: "a".repeat(64),
        audio: Some(audio.clone()),
        visual: Some(visual),
        expected_frames: frames,
        samples_per_frame: 800,
        first_frame_sha256: first,
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
        pcm_sha256: audio.pcm_sha256.clone(),
        sample_count: audio.sample_count,
        expected_frames: frames,
        first_frame_sha256: descriptor.first_frame_sha256.clone(),
        tail_frame: Some(PublicMediaFrameInfo {
            frame_index: frames,
            sample_start: (frames - 1) * 800,
            sample_count: u32::try_from(audio.sample_count - (frames - 1) * 800).unwrap(),
            sample_rate: 24_000,
            width: 512,
            height: 288,
            pixels_sha256: "f".repeat(64),
            openness_q16: 1,
        }),
        ..Default::default()
    };
    let prep = id(60);
    let mut gate = PublicExecutionGate::new(
        "f".repeat(64),
        bundle.clone(),
        &descriptor,
        &receipt,
        prep.clone(),
    )
    .unwrap();
    let base = Instant::now();
    let fence = gate.callback_fence();
    let experimental = Arc::new(ExperimentalFence::externally_controlled());
    let mut probe = open_pcm_controlled(
        experimental.clone(),
        Zeroizing::new(pcm.clone()),
        Arc::new(move || fence.allows(base.elapsed().as_millis() as u64)),
    )
    .unwrap();
    gate.initialization_barrier().unwrap();
    let lease = PublicExecutionLease {
        authorization: "f".repeat(64),
        output_id: bundle.output_id.clone(),
        bundle_sha256: bundle.approved_sha256.clone(),
        prepared_media_sha256: bundle.prepared_media_sha256.clone(),
        session_id: bundle.session_id.clone(),
        epoch: bundle.epoch,
        challenge: gate.challenge(base.elapsed().as_millis() as u64).unwrap(),
        seq: 1,
        lease_ms: 350,
        purpose: PURPOSE.into(),
        preparation_id: prep.clone(),
    };
    gate.accept(
        &lease,
        base.elapsed().as_millis() as u64,
        &Operation::new(1000).unwrap(),
    )
    .unwrap();
    assert!(experimental.activate());
    let mut board = PlayedFrameBoard::open(bundle.epoch, 8).unwrap();
    let mut last = None;
    let mut submitted_ahead = false;
    let mut renewal = lease.clone();
    for seq in 2..=6 {
        std::thread::sleep(Duration::from_millis(80));
        let now = base.elapsed().as_millis() as u64;
        renewal.challenge = gate.challenge(now).unwrap();
        renewal.seq = seq;
        gate.accept(&renewal, now, &Operation::new(1000).unwrap())
            .unwrap();
        let reading = probe.clock_reading();
        if reading.stream_generation < 1 || reading.device_frequency == 0 {
            continue;
        }
        let mapped = map_public_cue(
            bundle.epoch,
            CueReading {
                stream_generation: reading.stream_generation,
                device_frequency: reading.device_frequency,
                device_position: reading.device_position,
                cue_device_start: reading.cue_device_start,
                cue_device_end: reading.cue_device_end,
                cue_span_count: reading.cue_span_count,
            },
            CueClockGate::Live,
        )
        .expect("live cue mapping");
        let Some(played) = mapped else {
            continue;
        };
        assert!(device_clock_bound(
            &PublicDeviceClockObservation {
                output_id: bundle.output_id.clone(),
                preparation_id: prep.clone(),
                session_id: bundle.session_id.clone(),
                epoch: bundle.epoch,
                stream_generation: reading.stream_generation,
                device_frequency: reading.device_frequency,
                device_position: reading.device_position,
                sample_rate: 24_000,
                played_samples: played,
            },
            &bundle.output_id,
            &prep,
            reading.submitted_samples,
            audio.sample_count,
            bundle.epoch,
            reading.submitted_samples,
        ));
        board.offer_windows(played, audio.sample_count, now);
        if let Shown::Frame {
            frame_id,
            source_sample,
        } = board.present(bundle.epoch, played)
        {
            assert!(source_sample <= played);
            let rendered = renderer
                .frame_at(source_sample.min(audio.sample_count - 1), &|| Ok(()))
                .unwrap()
                .unwrap();
            assert_eq!(rendered.info.frame_index, frame_id);
            if reading.submitted_samples >= played.saturating_add(800)
                && source_sample / 800 != reading.submitted_samples / 800
            {
                submitted_ahead = true;
            }
            last = Some((
                frame_id,
                source_sample,
                played,
                reading.submitted_samples,
                reading.stream_generation,
                reading.device_frequency,
            ));
        }
        if probe.failed.load(Ordering::Acquire) {
            break;
        }
    }
    let last = last.expect("a frame from played samples");
    assert!(last.2 > 0, "{last:?}");
    println!(
        "played_av frame_id={} source_sample={} played_samples={} submitted_samples={} stream_generation={} device_frequency={} submitted_window_ahead={}",
        last.0, last.1, last.2, last.3, last.4, last.5, submitted_ahead
    );
    experimental.revoke();
    board.revoke();
    gate.stop();
    assert!(probe.stop_within(Duration::from_millis(500)).is_ok());
}
