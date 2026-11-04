#![cfg(windows)]
//! Isolated silent integration; generated fixture is not output authorization.
use nai_audio_cpal::clocked_experiment::{
    ClockReading, ExperimentalFence, fresh_stream_generation, open_pcm_controlled,
    open_pcm_controlled_generation,
};
use nai_contracts::{device_clock_bound, prepared_media_hash, public_media_hash, workbench::*};
use nai_runtime_core::{
    foundation::Operation,
    public_execution::{PURPOSE, PublicExecutionGate},
};
use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

fn id(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012}")
}
fn setup() -> (PublicExecutionGate, PublicMediaBundle, String) {
    let mut b = PublicMediaBundle {
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
            sample_count: 24000,
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
    let d = PreparedMediaDescriptor {
        protocol_major: 1,
        workspace_id: b.workspace_id.clone(),
        task_id: b.audio.as_ref().unwrap().artifact_id.clone(),
        task_sha256: "a".repeat(64),
        audio: b.audio.clone(),
        visual: b.visual.clone(),
        expected_frames: 30,
        samples_per_frame: 800,
        first_frame_sha256: "e".repeat(64),
        pcm_format: "s16le-mono-24000".into(),
        pixel_format: "rgb32le".into(),
        renderer_id: "rms-sample-v1".into(),
    };
    b.prepared_media_sha256 = prepared_media_hash(&d);
    b.approved_sha256 = public_media_hash(&b);
    let r = PreparedMediaReceipt {
        protocol_major: 1,
        descriptor_sha256: b.prepared_media_sha256.clone(),
        consumer_instance_id: b.output_id.clone(),
        pcm_sha256: b.audio.as_ref().unwrap().pcm_sha256.clone(),
        sample_count: 24000,
        expected_frames: 30,
        first_frame_sha256: d.first_frame_sha256.clone(),
        tail_frame: Some(PublicMediaFrameInfo {
            frame_index: 30,
            sample_start: 23200,
            sample_count: 800,
            sample_rate: 24000,
            width: 512,
            height: 288,
            pixels_sha256: "f".repeat(64),
            openness_q16: 1,
        }),
    };
    let prep = id(60);
    let g = PublicExecutionGate::new("f".repeat(64), b.clone(), &d, &r, prep.clone()).unwrap();
    (g, b, prep)
}
#[test]
#[ignore = "explicit Windows silent device and fixture integration"]
fn native_lease_fence_controls_same_stream_silence() {
    let (mut gate, b, prep) = setup();
    let base = Instant::now();
    let fence = gate.callback_fence();
    let experimental = Arc::new(ExperimentalFence::externally_controlled());
    let mut probe = open_pcm_controlled(
        experimental.clone(),
        zeroize::Zeroizing::new(vec![0; 24000]),
        Arc::new(move || fence.allows(base.elapsed().as_millis() as u64)),
    )
    .unwrap();
    gate.initialization_barrier().unwrap();
    let now = base.elapsed().as_millis() as u64;
    let challenge = gate.challenge(now).unwrap();
    let lease = PublicExecutionLease {
        authorization: "f".repeat(64),
        output_id: b.output_id.clone(),
        bundle_sha256: b.approved_sha256.clone(),
        prepared_media_sha256: b.prepared_media_sha256.clone(),
        session_id: b.session_id.clone(),
        epoch: b.epoch,
        challenge,
        seq: 1,
        lease_ms: 350,
        purpose: PURPOSE.into(),
        preparation_id: prep,
    };
    gate.accept(
        &lease,
        base.elapsed().as_millis() as u64,
        &Operation::new(1000).unwrap(),
    )
    .unwrap();
    assert!(experimental.activate());
    let until = Instant::now() + Duration::from_millis(250);
    while probe.submitted.load(Ordering::Acquire) == 0 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(probe.submitted.load(Ordering::Acquire) > 0);
    // Repeated renewals keep the same stream active beyond the old 500ms ceiling.
    let mut renewal = lease.clone();
    let before = probe.submitted.load(Ordering::Acquire);
    for seq in 2..=7 {
        std::thread::sleep(Duration::from_millis(100));
        let now = base.elapsed().as_millis() as u64;
        renewal.challenge = gate.challenge(now).unwrap();
        renewal.seq = seq;
        gate.accept(
            &renewal,
            base.elapsed().as_millis() as u64,
            &Operation::new(1000).unwrap(),
        )
        .unwrap();
        assert!(!probe.is_finished());
    }
    assert!(probe.submitted.load(Ordering::Acquire) > before + 10000);
    // A replay is terminal and must stop the same audio stream.
    assert!(
        gate.accept(
            &lease,
            base.elapsed().as_millis() as u64,
            &Operation::new(1000).unwrap()
        )
        .is_err()
    );
    let until = Instant::now() + Duration::from_millis(200);
    while !probe.is_finished() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(probe.is_finished());
    let submitted = probe.submitted.load(Ordering::Acquire);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(probe.submitted.load(Ordering::Acquire), submitted);
    assert!(probe.stop_within(Duration::from_millis(500)).is_ok());
}

#[test]
#[ignore = "explicit Windows silent device and fixture integration"]
fn missed_native_renewal_stops_same_stream() {
    let (mut gate, b, prep) = setup();
    let base = Instant::now();
    let fence = gate.callback_fence();
    let experimental = Arc::new(ExperimentalFence::externally_controlled());
    let mut probe = open_pcm_controlled(
        experimental.clone(),
        zeroize::Zeroizing::new(vec![0; 24000]),
        Arc::new(move || fence.allows(base.elapsed().as_millis() as u64)),
    )
    .unwrap();
    gate.initialization_barrier().unwrap();
    let challenge = gate.challenge(base.elapsed().as_millis() as u64).unwrap();
    let lease = PublicExecutionLease {
        authorization: "f".repeat(64),
        output_id: b.output_id.clone(),
        bundle_sha256: b.approved_sha256.clone(),
        prepared_media_sha256: b.prepared_media_sha256.clone(),
        session_id: b.session_id.clone(),
        epoch: b.epoch,
        challenge,
        seq: 1,
        lease_ms: 200,
        purpose: PURPOSE.into(),
        preparation_id: prep,
    };
    gate.accept(
        &lease,
        base.elapsed().as_millis() as u64,
        &Operation::new(1000).unwrap(),
    )
    .unwrap();
    assert!(experimental.activate());
    let deadline = Instant::now() + Duration::from_millis(180);
    while probe.submitted.load(Ordering::Acquire) == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(probe.submitted.load(Ordering::Acquire) > 0);
    let deadline = Instant::now() + Duration::from_millis(450);
    while !probe.is_finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(probe.is_finished());
    let after = probe.submitted.load(Ordering::Acquire);
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(probe.submitted.load(Ordering::Acquire), after);
    assert!(gate.challenge(base.elapsed().as_millis() as u64).is_err());
    assert!(probe.stop_within(Duration::from_millis(500)).is_ok());
}

fn observation(
    bundle: &PublicMediaBundle,
    preparation_id: &str,
    reading: ClockReading,
) -> PublicDeviceClockObservation {
    PublicDeviceClockObservation {
        output_id: bundle.output_id.clone(),
        preparation_id: preparation_id.into(),
        session_id: bundle.session_id.clone(),
        epoch: bundle.epoch,
        stream_generation: reading.stream_generation,
        device_frequency: reading.device_frequency,
        device_position: reading.device_position,
        sample_rate: 24000,
        played_samples: reading.played_samples,
    }
}

fn accepts(
    bundle: &PublicMediaBundle,
    preparation_id: &str,
    reading: ClockReading,
    epoch: u64,
) -> bool {
    let samples = bundle.audio.as_ref().unwrap().sample_count;
    device_clock_bound(
        &observation(bundle, preparation_id, reading),
        &bundle.output_id,
        preparation_id,
        reading.submitted_samples,
        samples,
        epoch,
        reading.submitted_samples,
    )
}

#[test]
#[ignore = "explicit Windows silent device and fixture integration"]
fn same_stream_clock_fills_bound_observation() {
    let (mut gate, bundle, preparation) = setup();
    let base = Instant::now();
    let fence = gate.callback_fence();
    let experimental = Arc::new(ExperimentalFence::externally_controlled());
    let mut probe = open_pcm_controlled(
        experimental.clone(),
        zeroize::Zeroizing::new(vec![0; 24000]),
        Arc::new(move || fence.allows(base.elapsed().as_millis() as u64)),
    )
    .unwrap();
    let opened = probe.clock_reading();
    assert_eq!(opened.stream_generation, 1);
    assert!(opened.device_frequency > 0);
    assert_eq!(fresh_stream_generation(opened.stream_generation), 2);
    gate.initialization_barrier().unwrap();
    let now = base.elapsed().as_millis() as u64;
    let challenge = gate.challenge(now).unwrap();
    let lease = PublicExecutionLease {
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
        preparation_id: preparation.clone(),
    };
    gate.accept(&lease, now, &Operation::new(1000).unwrap())
        .unwrap();
    assert!(experimental.activate());
    let mut renewal = lease.clone();
    let mut seq = 2u64;
    let deadline = Instant::now() + Duration::from_millis(1500);
    let mut reading = probe.clock_reading();
    while reading.played_samples == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
        let now = base.elapsed().as_millis() as u64;
        renewal.challenge = gate.challenge(now).unwrap();
        renewal.seq = seq;
        gate.accept(&renewal, now, &Operation::new(1000).unwrap())
            .unwrap();
        seq += 1;
        assert!(!probe.is_finished());
        reading = probe.clock_reading();
    }
    assert!(reading.played_samples > 0);
    assert!(reading.played_samples <= reading.submitted_samples);
    assert!(reading.device_position > 0);
    assert!(reading.device_frequency > 0);
    assert_eq!(reading.stream_generation, 1);
    assert!(accepts(&bundle, &preparation, reading, lease.epoch));
    let mut rejected = observation(&bundle, &preparation, reading);
    rejected.stream_generation = 0;
    let samples = bundle.audio.as_ref().unwrap().sample_count;
    assert!(!device_clock_bound(
        &rejected,
        &bundle.output_id,
        &preparation,
        reading.submitted_samples,
        samples,
        lease.epoch,
        reading.submitted_samples,
    ));
    rejected.stream_generation = reading.stream_generation;
    rejected.played_samples = reading.submitted_samples.saturating_add(1);
    assert!(!device_clock_bound(
        &rejected,
        &bundle.output_id,
        &preparation,
        reading.submitted_samples,
        samples,
        lease.epoch,
        reading.submitted_samples,
    ));
    println!(
        "same-stream played={} submitted={} position={} frequency={} generation={}",
        reading.played_samples,
        reading.submitted_samples,
        reading.device_position,
        reading.device_frequency,
        reading.stream_generation
    );
    assert!(
        gate.accept(
            &lease,
            base.elapsed().as_millis() as u64,
            &Operation::new(1000).unwrap()
        )
        .is_err()
    );
    let until = Instant::now() + Duration::from_millis(500);
    while !probe.is_finished() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(probe.is_finished());
    assert!(!probe.failed.load(Ordering::Acquire));
    let frozen = probe.clock_reading();
    assert!(frozen.played_samples >= reading.played_samples);
    assert!(accepts(&bundle, &preparation, frozen, lease.epoch));
    assert_eq!(frozen.stream_generation, 1);
    std::thread::sleep(Duration::from_millis(50));
    let held = probe.clock_reading();
    assert_eq!(held.submitted_samples, frozen.submitted_samples);
    assert_eq!(held.played_samples, frozen.played_samples);
    assert_eq!(held.device_position, frozen.device_position);
    assert_eq!(held.stream_generation, frozen.stream_generation);
    assert!(accepts(&bundle, &preparation, held, lease.epoch));
    println!(
        "same-stream frozen played={} submitted={} position={} frequency={} generation={}",
        held.played_samples,
        held.submitted_samples,
        held.device_position,
        held.device_frequency,
        held.stream_generation
    );
    assert!(probe.stop_within(Duration::from_millis(500)).is_ok());
}

#[test]
#[ignore = "explicit Windows silent device and fixture integration"]
fn rebuilt_silent_stream_starts_at_its_own_zero() {
    let (mut gate, bundle, preparation) = setup();
    let base = Instant::now();
    let fence = gate.callback_fence();
    let experimental = Arc::new(ExperimentalFence::externally_controlled());
    let mut probe = open_pcm_controlled(
        experimental.clone(),
        zeroize::Zeroizing::new(vec![0; 24000]),
        Arc::new(move || fence.allows(base.elapsed().as_millis() as u64)),
    )
    .unwrap();
    assert!(probe.confirmed_at_zero());
    assert_eq!(probe.position_before_start(), 0);
    assert_eq!(probe.clock_reading().stream_generation, 1);
    gate.initialization_barrier().unwrap();
    let now = base.elapsed().as_millis() as u64;
    let challenge = gate.challenge(now).unwrap();
    let lease = PublicExecutionLease {
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
        preparation_id: preparation.clone(),
    };
    gate.accept(&lease, now, &Operation::new(1000).unwrap())
        .unwrap();
    assert!(experimental.activate());
    let mut renewal = lease.clone();
    let mut seq = 2u64;
    let deadline = Instant::now() + Duration::from_millis(1500);
    let mut reading = probe.clock_reading();
    while reading.device_position < 30_000 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
        let now = base.elapsed().as_millis() as u64;
        renewal.challenge = gate.challenge(now).unwrap();
        renewal.seq = seq;
        gate.accept(&renewal, now, &Operation::new(1000).unwrap())
            .unwrap();
        seq += 1;
        assert!(!probe.is_finished());
        reading = probe.clock_reading();
    }
    assert!(reading.device_position >= 30_000, "{reading:?}");
    assert!(reading.played_samples > 0, "{reading:?}");
    assert_eq!(reading.stream_generation, 1);
    assert!(
        gate.accept(
            &lease,
            base.elapsed().as_millis() as u64,
            &Operation::new(1000).unwrap()
        )
        .is_err()
    );
    let until = Instant::now() + Duration::from_millis(500);
    while !probe.is_finished() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(probe.is_finished());
    assert!(!probe.failed.load(Ordering::Acquire));
    let frozen = probe.clock_reading();
    std::thread::sleep(Duration::from_millis(50));
    let held = probe.clock_reading();
    assert_eq!(held.device_position, frozen.device_position);
    assert_eq!(held.played_samples, frozen.played_samples);
    assert_eq!(held.submitted_samples, frozen.submitted_samples);
    assert_eq!(held.stream_generation, frozen.stream_generation);
    assert!(held.device_position >= 30_000, "{held:?}");
    assert_eq!(held.stream_generation, 1);
    assert!(probe.stop_within(Duration::from_millis(500)).is_ok());
    println!(
        "rebuild first frozen played={} submitted={} position={} frequency={} generation={}",
        held.played_samples,
        held.submitted_samples,
        held.device_position,
        held.device_frequency,
        held.stream_generation
    );

    let (mut next_gate, next_bundle, next_preparation) = setup();
    let next_base = Instant::now();
    let next_fence = next_gate.callback_fence();
    let next_experimental = Arc::new(ExperimentalFence::externally_controlled());
    let mut next = open_pcm_controlled_generation(
        next_experimental.clone(),
        zeroize::Zeroizing::new(vec![0; 24000]),
        Arc::new(move || next_fence.allows(next_base.elapsed().as_millis() as u64)),
        held.stream_generation,
    )
    .unwrap();
    assert!(next.confirmed_at_zero());
    assert_eq!(next.position_before_start(), 0);
    assert_ne!(next.position_before_start(), held.device_position);
    let opened = next.clock_reading();
    println!(
        "rebuild second opened played={} submitted={} position={} frequency={} generation={} start={}",
        opened.played_samples,
        opened.submitted_samples,
        opened.device_position,
        opened.device_frequency,
        opened.stream_generation,
        next.position_before_start()
    );
    assert_eq!(
        opened.stream_generation,
        fresh_stream_generation(held.stream_generation)
    );
    assert_eq!(opened.stream_generation, 2);
    assert_eq!(opened.played_samples, 0);
    assert_eq!(opened.submitted_samples, 0);
    assert!(opened.device_frequency > 0);
    assert!(
        opened.device_position < held.device_position,
        "opened={opened:?} held={held:?}"
    );
    next_gate.initialization_barrier().unwrap();
    let now = next_base.elapsed().as_millis() as u64;
    let challenge = next_gate.challenge(now).unwrap();
    let next_lease = PublicExecutionLease {
        authorization: "f".repeat(64),
        output_id: next_bundle.output_id.clone(),
        bundle_sha256: next_bundle.approved_sha256.clone(),
        prepared_media_sha256: next_bundle.prepared_media_sha256.clone(),
        session_id: next_bundle.session_id.clone(),
        epoch: next_bundle.epoch,
        challenge,
        seq: 1,
        lease_ms: 350,
        purpose: PURPOSE.into(),
        preparation_id: next_preparation.clone(),
    };
    next_gate
        .accept(&next_lease, now, &Operation::new(1000).unwrap())
        .unwrap();
    assert!(next_experimental.activate());
    let mut next_renewal = next_lease.clone();
    let mut next_seq = 2u64;
    let next_deadline = Instant::now() + Duration::from_millis(1500);
    let mut live = next.clock_reading();
    while live.played_samples == 0 && Instant::now() < next_deadline {
        std::thread::sleep(Duration::from_millis(50));
        let now = next_base.elapsed().as_millis() as u64;
        next_renewal.challenge = next_gate.challenge(now).unwrap();
        next_renewal.seq = next_seq;
        next_gate
            .accept(&next_renewal, now, &Operation::new(1000).unwrap())
            .unwrap();
        next_seq += 1;
        assert!(!next.is_finished());
        live = next.clock_reading();
    }
    assert!(live.played_samples > 0, "{live:?}");
    assert!(live.played_samples <= live.submitted_samples, "{live:?}");
    assert!(live.device_position > 0, "{live:?}");
    assert_eq!(live.stream_generation, 2);
    assert!(accepts(
        &next_bundle,
        &next_preparation,
        live,
        next_lease.epoch
    ));
    let mut rejected = observation(&next_bundle, &next_preparation, live);
    rejected.stream_generation = 0;
    let samples = next_bundle.audio.as_ref().unwrap().sample_count;
    assert!(!device_clock_bound(
        &rejected,
        &next_bundle.output_id,
        &next_preparation,
        live.submitted_samples,
        samples,
        next_lease.epoch,
        live.submitted_samples,
    ));
    println!(
        "rebuild second live played={} submitted={} position={} frequency={} generation={}",
        live.played_samples,
        live.submitted_samples,
        live.device_position,
        live.device_frequency,
        live.stream_generation
    );
    assert!(next.stop_within(Duration::from_millis(500)).is_ok());
}

/// Source samples the lead would represent if the device clock had been the sentence.
/// The experiment maps device frames as position * 48000 / frequency, then halves them.
fn source_samples_for_device_position(position: u64, frequency: u64) -> u64 {
    u64::try_from(u128::from(position).saturating_mul(24_000) / u128::from(frequency))
        .unwrap_or(u64::MAX)
}

fn device_frames_for_position(position: u64, frequency: u64) -> u64 {
    u64::try_from(u128::from(position).saturating_mul(48_000) / u128::from(frequency))
        .unwrap_or(u64::MAX)
}

#[test]
#[ignore = "explicit Windows silent device and fixture integration"]
fn leading_silence_does_not_advance_the_sentence() {
    let (mut gate, bundle, preparation) = setup();
    let base = Instant::now();
    let fence = gate.callback_fence();
    let experimental = Arc::new(ExperimentalFence::externally_controlled());
    let mut probe = open_pcm_controlled(
        experimental.clone(),
        zeroize::Zeroizing::new(vec![0; 24000]),
        Arc::new(move || fence.allows(base.elapsed().as_millis() as u64)),
    )
    .unwrap();
    assert!(probe.confirmed_at_zero());
    assert_eq!(probe.position_before_start(), 0);
    let deadline = Instant::now() + Duration::from_millis(1500);
    let mut reading = probe.clock_reading();
    let mut previous_position = probe.position_before_start();
    let mut advanced = false;
    while Instant::now() < deadline {
        assert_eq!(reading.played_samples, 0, "{reading:?}");
        assert_eq!(reading.submitted_samples, 0, "{reading:?}");
        assert!(!probe.is_finished(), "{reading:?}");
        if reading.device_position > previous_position {
            advanced = true;
        }
        previous_position = reading.device_position;
        if reading.device_frequency > 0
            && source_samples_for_device_position(reading.device_position, reading.device_frequency)
                >= 8_000
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
        reading = probe.clock_reading();
    }
    assert!(advanced, "{reading:?}");
    assert!(reading.device_frequency > 0, "{reading:?}");
    let lead_samples =
        source_samples_for_device_position(reading.device_position, reading.device_frequency);
    assert!(lead_samples >= 8_000, "{reading:?}");
    assert_eq!(reading.played_samples, 0);
    assert_eq!(reading.submitted_samples, 0);
    assert_eq!(reading.stream_generation, 1);
    let marked = reading.device_position;
    std::thread::sleep(Duration::from_millis(50));
    let held = probe.clock_reading();
    assert!(held.device_position >= marked, "{held:?}");
    assert_eq!(held.played_samples, 0);
    assert_eq!(held.submitted_samples, 0);
    assert_eq!(held.stream_generation, 1);
    assert!(!probe.failed.load(Ordering::Acquire));
    assert!(accepts(&bundle, &preparation, held, bundle.epoch));
    println!(
        "leading silence position={} played={} submitted={} frequency={} generation={} lead_samples={}",
        held.device_position,
        held.played_samples,
        held.submitted_samples,
        held.device_frequency,
        held.stream_generation,
        source_samples_for_device_position(held.device_position, held.device_frequency)
    );

    gate.initialization_barrier().unwrap();
    let now = base.elapsed().as_millis() as u64;
    let challenge = gate.challenge(now).unwrap();
    let lease = PublicExecutionLease {
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
        preparation_id: preparation.clone(),
    };
    gate.accept(&lease, now, &Operation::new(1000).unwrap())
        .unwrap();
    assert!(experimental.activate());
    let mut renewal = lease.clone();
    let mut seq = 2u64;
    let cue_deadline = Instant::now() + Duration::from_millis(1500);
    let mut live = probe.clock_reading();
    while live.played_samples == 0 && Instant::now() < cue_deadline {
        std::thread::sleep(Duration::from_millis(20));
        let now = base.elapsed().as_millis() as u64;
        renewal.challenge = gate.challenge(now).unwrap();
        renewal.seq = seq;
        gate.accept(&renewal, now, &Operation::new(1000).unwrap())
            .unwrap();
        seq += 1;
        assert!(!probe.is_finished(), "{live:?}");
        live = probe.clock_reading();
    }
    let ceiling = source_samples_for_device_position(held.device_position, held.device_frequency);
    println!(
        "leading silence cue played={} submitted={} position={} frequency={} generation={} ceiling={}",
        live.played_samples,
        live.submitted_samples,
        live.device_position,
        live.device_frequency,
        live.stream_generation,
        ceiling
    );
    assert!(live.played_samples > 0, "{live:?}");
    assert!(
        live.played_samples < ceiling,
        "played={} ceiling={} held={held:?} live={live:?}",
        live.played_samples,
        ceiling
    );
    assert!(live.played_samples <= live.submitted_samples, "{live:?}");
    assert_eq!(live.stream_generation, 1);
    assert!(probe.stop_within(Duration::from_millis(500)).is_ok());
}

#[test]
#[ignore = "explicit Windows silent device and fixture integration"]
fn gap_silence_does_not_advance_the_sentence() {
    let (mut gate, bundle, preparation) = setup();
    let base = Instant::now();
    let fence = gate.callback_fence();
    let experimental = Arc::new(ExperimentalFence::externally_controlled());
    let mut probe = open_pcm_controlled(
        experimental.clone(),
        zeroize::Zeroizing::new(vec![0; 24000]),
        Arc::new(move || fence.allows(base.elapsed().as_millis() as u64)),
    )
    .unwrap();
    gate.initialization_barrier().unwrap();
    let now = base.elapsed().as_millis() as u64;
    let challenge = gate.challenge(now).unwrap();
    let mut renewal = PublicExecutionLease {
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
        preparation_id: preparation.clone(),
    };
    gate.accept(&renewal, now, &Operation::new(1000).unwrap())
        .unwrap();
    assert!(experimental.activate());
    let mut seq = 2u64;
    let mut renew = || {
        let now = base.elapsed().as_millis() as u64;
        renewal.challenge = gate.challenge(now).unwrap();
        renewal.seq = seq;
        gate.accept(&renewal, now, &Operation::new(1000).unwrap())
            .unwrap();
        seq += 1;
    };
    let started = Instant::now() + Duration::from_millis(1500);
    let mut live = probe.clock_reading();
    while Instant::now() < started {
        renew();
        live = probe.clock_reading();
        if live.played_samples >= 400 && live.submitted_samples > live.played_samples {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
        assert!(!probe.is_finished(), "{live:?}");
    }
    assert!(live.played_samples >= 400, "{live:?}");
    assert!(live.submitted_samples > live.played_samples, "{live:?}");
    assert_eq!(live.stream_generation, 1, "{live:?}");
    probe.set_gap(true);
    assert!(!probe.is_holding());
    let notice = Instant::now() + Duration::from_millis(500);
    while !probe.is_gapping() && Instant::now() < notice {
        renew();
        std::thread::sleep(Duration::from_millis(20));
        assert!(!probe.is_finished(), "{live:?}");
    }
    assert!(probe.is_gapping(), "{live:?}");
    let cap = probe.sentence_cap();
    assert!(cap >= 400, "cap={cap} live={live:?}");
    live = probe.clock_reading();
    let submitted_at_gap = live.submitted_samples;
    assert!(
        live.played_samples <= cap,
        "played={} cap={cap} {live:?}",
        live.played_samples
    );
    println!(
        "gap entered cap={} played={} submitted={} position={} frequency={} generation={}",
        cap,
        live.played_samples,
        live.submitted_samples,
        live.device_position,
        live.device_frequency,
        live.stream_generation
    );
    let drain = Instant::now() + Duration::from_millis(1500);
    while live.played_samples != cap && Instant::now() < drain {
        assert!(
            live.played_samples <= cap,
            "played={} cap={cap} {live:?}",
            live.played_samples
        );
        assert_eq!(live.submitted_samples, submitted_at_gap, "{live:?}");
        renew();
        std::thread::sleep(Duration::from_millis(20));
        assert!(!probe.is_finished(), "{live:?}");
        live = probe.clock_reading();
    }
    assert_eq!(
        live.played_samples, cap,
        "played={} cap={cap} {live:?}",
        live.played_samples
    );
    assert_eq!(live.submitted_samples, submitted_at_gap, "{live:?}");
    assert_eq!(live.stream_generation, 1, "{live:?}");
    assert!(!probe.is_holding());
    assert!(!probe.failed.load(Ordering::Acquire));
    let silence_start = live.device_position;
    let frequency = live.device_frequency;
    let window = Instant::now() + Duration::from_millis(250);
    while Instant::now() < window {
        renew();
        std::thread::sleep(Duration::from_millis(20));
        live = probe.clock_reading();
        assert_eq!(live.played_samples, cap, "{live:?}");
        assert_eq!(live.submitted_samples, submitted_at_gap, "{live:?}");
        assert_eq!(live.device_frequency, frequency, "{live:?}");
        assert_eq!(live.stream_generation, 1, "{live:?}");
        assert!(!probe.is_finished(), "{live:?}");
        assert!(!probe.is_holding());
        assert!(!probe.failed.load(Ordering::Acquire));
    }
    let delta_samples = source_samples_for_device_position(
        live.device_position.saturating_sub(silence_start),
        frequency,
    );
    println!(
        "gap silence played={} submitted={} position={} start_position={} delta_samples={} frequency={} generation={}",
        live.played_samples,
        live.submitted_samples,
        live.device_position,
        silence_start,
        delta_samples,
        live.device_frequency,
        live.stream_generation
    );
    assert!(
        delta_samples >= 2_000,
        "delta={delta_samples} start={silence_start} live={live:?}"
    );
    assert!(live.device_position > silence_start, "{live:?}");
    probe.set_gap(false);
    let resume = Instant::now() + Duration::from_millis(1500);
    while live.played_samples <= cap && Instant::now() < resume {
        renew();
        std::thread::sleep(Duration::from_millis(20));
        assert!(!probe.is_finished(), "{live:?}");
        live = probe.clock_reading();
    }
    println!(
        "gap resumed played={} submitted={} position={} frequency={} generation={}",
        live.played_samples,
        live.submitted_samples,
        live.device_position,
        live.device_frequency,
        live.stream_generation
    );
    assert!(
        live.played_samples > cap,
        "played={} cap={cap} {live:?}",
        live.played_samples
    );
    assert!(live.submitted_samples > submitted_at_gap, "{live:?}");
    assert_eq!(live.stream_generation, 1, "{live:?}");
    assert!(probe.stop_within(Duration::from_millis(500)).is_ok());
}

#[test]
#[ignore = "explicit Windows silent device and fixture integration"]
fn same_stream_clock_rejects_a_different_lease_epoch() {
    let (mut gate, bundle, preparation) = setup();
    let base = Instant::now();
    let fence = gate.callback_fence();
    let experimental = Arc::new(ExperimentalFence::externally_controlled());
    let mut probe = open_pcm_controlled(
        experimental.clone(),
        zeroize::Zeroizing::new(vec![0; 24000]),
        Arc::new(move || fence.allows(base.elapsed().as_millis() as u64)),
    )
    .unwrap();
    gate.initialization_barrier().unwrap();
    let now = base.elapsed().as_millis() as u64;
    let challenge = gate.challenge(now).unwrap();
    let mut renewal = PublicExecutionLease {
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
        preparation_id: preparation.clone(),
    };
    gate.accept(&renewal, now, &Operation::new(1000).unwrap())
        .unwrap();
    assert_eq!(renewal.epoch, bundle.epoch);
    assert!(experimental.activate());
    let mut seq = 2u64;
    let deadline = Instant::now() + Duration::from_millis(1500);
    let mut reading = probe.clock_reading();
    while reading.played_samples == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
        let now = base.elapsed().as_millis() as u64;
        renewal.challenge = gate.challenge(now).unwrap();
        renewal.seq = seq;
        gate.accept(&renewal, now, &Operation::new(1000).unwrap())
            .unwrap();
        seq += 1;
        assert!(!probe.is_finished(), "{reading:?}");
        reading = probe.clock_reading();
    }
    assert!(reading.played_samples > 0, "{reading:?}");
    assert!(
        reading.played_samples <= reading.submitted_samples,
        "{reading:?}"
    );
    assert!(reading.device_position > 0, "{reading:?}");
    assert!(reading.device_frequency > 0, "{reading:?}");
    assert_eq!(reading.stream_generation, 1, "{reading:?}");
    assert!(accepts(&bundle, &preparation, reading, renewal.epoch));
    let samples = bundle.audio.as_ref().unwrap().sample_count;
    let mut other_epoch = observation(&bundle, &preparation, reading);
    other_epoch.epoch = renewal.epoch.saturating_add(1);
    assert!(!device_clock_bound(
        &other_epoch,
        &bundle.output_id,
        &preparation,
        reading.submitted_samples,
        samples,
        renewal.epoch,
        reading.submitted_samples,
    ));
    let mut zero_epoch = observation(&bundle, &preparation, reading);
    zero_epoch.epoch = 0;
    assert!(!device_clock_bound(
        &zero_epoch,
        &bundle.output_id,
        &preparation,
        reading.submitted_samples,
        samples,
        renewal.epoch,
        reading.submitted_samples,
    ));
    println!(
        "epoch bound played={} submitted={} position={} frequency={} generation={} lease_epoch={} rejected_epoch={}",
        reading.played_samples,
        reading.submitted_samples,
        reading.device_position,
        reading.device_frequency,
        reading.stream_generation,
        renewal.epoch,
        other_epoch.epoch
    );
    assert!(probe.stop_within(Duration::from_millis(500)).is_ok());
}

#[test]
#[ignore = "explicit Windows silent device and fixture integration"]
fn same_stream_clock_rejects_played_past_the_cue() {
    let (mut gate, bundle, preparation) = setup();
    let samples = bundle.audio.as_ref().unwrap().sample_count;
    let base = Instant::now();
    let fence = gate.callback_fence();
    let experimental = Arc::new(ExperimentalFence::externally_controlled());
    let mut probe = open_pcm_controlled(
        experimental.clone(),
        zeroize::Zeroizing::new(vec![0; 24000]),
        Arc::new(move || fence.allows(base.elapsed().as_millis() as u64)),
    )
    .unwrap();
    gate.initialization_barrier().unwrap();
    let now = base.elapsed().as_millis() as u64;
    let challenge = gate.challenge(now).unwrap();
    let mut renewal = PublicExecutionLease {
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
        preparation_id: preparation.clone(),
    };
    gate.accept(&renewal, now, &Operation::new(1000).unwrap())
        .unwrap();
    assert!(experimental.activate());
    let mut seq = 2u64;
    let deadline = Instant::now() + Duration::from_millis(1500);
    let mut reading = probe.clock_reading();
    while Instant::now() < deadline {
        if reading.played_samples > 0
            && reading.submitted_samples > reading.played_samples
            && reading.submitted_samples < samples
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
        let now = base.elapsed().as_millis() as u64;
        renewal.challenge = gate.challenge(now).unwrap();
        renewal.seq = seq;
        gate.accept(&renewal, now, &Operation::new(1000).unwrap())
            .unwrap();
        seq += 1;
        assert!(!probe.is_finished(), "{reading:?}");
        reading = probe.clock_reading();
    }
    let cue_samples = reading.submitted_samples;
    assert!(reading.played_samples > 0, "{reading:?}");
    assert!(reading.played_samples <= cue_samples, "{reading:?}");
    assert!(
        cue_samples < samples,
        "cue={cue_samples} clip={samples} {reading:?}"
    );
    assert_eq!(reading.stream_generation, 1, "{reading:?}");
    assert!(accepts(&bundle, &preparation, reading, renewal.epoch));
    assert!(device_clock_bound(
        &observation(&bundle, &preparation, reading),
        &bundle.output_id,
        &preparation,
        samples,
        samples,
        renewal.epoch,
        cue_samples,
    ));
    let mut over = observation(&bundle, &preparation, reading);
    over.played_samples = cue_samples.saturating_add(1);
    assert!(
        over.played_samples <= samples,
        "played={} clip={samples}",
        over.played_samples
    );
    assert!(device_clock_bound(
        &over,
        &bundle.output_id,
        &preparation,
        samples,
        samples,
        renewal.epoch,
        samples,
    ));
    assert!(!device_clock_bound(
        &over,
        &bundle.output_id,
        &preparation,
        samples,
        samples,
        renewal.epoch,
        cue_samples,
    ));
    println!(
        "cue bound played={} submitted={} cue_samples={} sample_count={} rejected_played={} position={} frequency={} generation={}",
        reading.played_samples,
        reading.submitted_samples,
        cue_samples,
        samples,
        over.played_samples,
        reading.device_position,
        reading.device_frequency,
        reading.stream_generation
    );
    assert!(probe.stop_within(Duration::from_millis(500)).is_ok());
}

#[test]
#[ignore = "explicit Windows silent device and fixture integration"]
fn cancelled_lease_stops_the_sentence_clock() {
    let (mut gate, bundle, preparation) = setup();
    let samples = bundle.audio.as_ref().unwrap().sample_count;
    let base = Instant::now();
    let fence = gate.callback_fence();
    let experimental = Arc::new(ExperimentalFence::externally_controlled());
    let mut probe = open_pcm_controlled(
        experimental.clone(),
        zeroize::Zeroizing::new(vec![0; 24000]),
        Arc::new(move || fence.allows(base.elapsed().as_millis() as u64)),
    )
    .unwrap();
    gate.initialization_barrier().unwrap();
    let now = base.elapsed().as_millis() as u64;
    let challenge = gate.challenge(now).unwrap();
    let lease = PublicExecutionLease {
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
        preparation_id: preparation.clone(),
    };
    gate.accept(&lease, now, &Operation::new(1000).unwrap())
        .unwrap();
    assert!(experimental.activate());
    let mut renewal = lease.clone();
    let mut seq = 2u64;
    let deadline = Instant::now() + Duration::from_millis(1500);
    let mut reading = probe.clock_reading();
    while Instant::now() < deadline {
        if reading.played_samples > 0
            && reading.submitted_samples > reading.played_samples
            && reading.submitted_samples < samples
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
        let now = base.elapsed().as_millis() as u64;
        renewal.challenge = gate.challenge(now).unwrap();
        renewal.seq = seq;
        gate.accept(&renewal, now, &Operation::new(1000).unwrap())
            .unwrap();
        seq += 1;
        assert!(!probe.is_finished(), "{reading:?}");
        reading = probe.clock_reading();
    }
    assert!(reading.played_samples > 0, "{reading:?}");
    assert!(
        reading.submitted_samples > reading.played_samples,
        "{reading:?}"
    );
    assert!(reading.submitted_samples < samples, "{reading:?}");
    assert_eq!(reading.stream_generation, 1, "{reading:?}");
    assert!(!probe.is_holding());
    println!(
        "cancel before played={} submitted={} position={} frequency={} generation={}",
        reading.played_samples,
        reading.submitted_samples,
        reading.device_position,
        reading.device_frequency,
        reading.stream_generation
    );
    let submitted_before = reading.submitted_samples;
    assert!(
        gate.accept(
            &lease,
            base.elapsed().as_millis() as u64,
            &Operation::new(1000).unwrap()
        )
        .is_err()
    );
    assert!(gate.challenge(base.elapsed().as_millis() as u64).is_err());
    let until = Instant::now() + Duration::from_millis(500);
    while !probe.is_finished() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(probe.is_finished(), "{reading:?}");
    assert!(!probe.failed.load(Ordering::Acquire));
    let stopped = probe.clock_reading();
    let cue_samples = stopped.submitted_samples;
    assert!(
        stopped.played_samples <= cue_samples,
        "played={} cue={cue_samples} {stopped:?}",
        stopped.played_samples
    );
    assert!(stopped.submitted_samples >= submitted_before, "{stopped:?}");
    assert!(
        stopped.submitted_samples <= submitted_before.saturating_add(2400),
        "before={submitted_before} stopped={stopped:?}"
    );
    assert_eq!(stopped.stream_generation, 1, "{stopped:?}");
    assert!(
        stopped.device_position >= reading.device_position,
        "{stopped:?}"
    );
    assert!(accepts(&bundle, &preparation, stopped, lease.epoch));
    println!(
        "cancel stopped played={} submitted={} cue_samples={} position={} frequency={} generation={}",
        stopped.played_samples,
        stopped.submitted_samples,
        cue_samples,
        stopped.device_position,
        stopped.device_frequency,
        stopped.stream_generation
    );
    std::thread::sleep(Duration::from_millis(50));
    let held = probe.clock_reading();
    assert_eq!(held.submitted_samples, cue_samples, "{held:?}");
    assert_eq!(held.played_samples, stopped.played_samples, "{held:?}");
    assert_eq!(held.device_position, stopped.device_position, "{held:?}");
    assert_eq!(held.stream_generation, 1, "{held:?}");
    assert!(accepts(&bundle, &preparation, held, lease.epoch));
    println!(
        "cancel held played={} submitted={} position={} frequency={} generation={}",
        held.played_samples,
        held.submitted_samples,
        held.device_position,
        held.device_frequency,
        held.stream_generation
    );
    assert!(probe.stop_within(Duration::from_millis(500)).is_ok());
}

#[test]
#[ignore = "explicit Windows silent device and fixture integration"]
fn cue_device_span_excludes_silence_outside_the_interval() {
    let (mut gate, bundle, preparation) = setup();
    let base = Instant::now();
    let fence = gate.callback_fence();
    let experimental = Arc::new(ExperimentalFence::externally_controlled());
    let mut probe = open_pcm_controlled(
        experimental.clone(),
        zeroize::Zeroizing::new(vec![0; 24000]),
        Arc::new(move || fence.allows(base.elapsed().as_millis() as u64)),
    )
    .unwrap();
    assert!(probe.confirmed_at_zero());
    assert_eq!(probe.position_before_start(), 0);
    let lead_deadline = Instant::now() + Duration::from_millis(1500);
    let mut lead = probe.clock_reading();
    while Instant::now() < lead_deadline {
        assert_eq!(probe.cue_span_count(), 0, "{lead:?}");
        assert_eq!(probe.cue_device_start(), 0);
        assert_eq!(probe.cue_device_end(), 0);
        assert_eq!(lead.played_samples, 0, "{lead:?}");
        assert_eq!(lead.submitted_samples, 0, "{lead:?}");
        assert!(!probe.is_finished(), "{lead:?}");
        if lead.device_frequency > 0
            && source_samples_for_device_position(lead.device_position, lead.device_frequency)
                >= 2_000
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
        lead = probe.clock_reading();
    }
    assert!(lead.device_frequency > 0, "{lead:?}");
    assert!(
        source_samples_for_device_position(lead.device_position, lead.device_frequency) >= 2_000,
        "{lead:?}"
    );
    std::thread::sleep(Duration::from_millis(50));
    let held_lead = probe.clock_reading();
    assert!(
        held_lead.device_position >= lead.device_position,
        "{held_lead:?}"
    );
    assert_eq!(held_lead.played_samples, 0, "{held_lead:?}");
    assert_eq!(held_lead.submitted_samples, 0, "{held_lead:?}");
    assert_eq!(probe.cue_span_count(), 0);
    assert_eq!(held_lead.stream_generation, 1, "{held_lead:?}");
    let lead_frames =
        device_frames_for_position(held_lead.device_position, held_lead.device_frequency);
    println!(
        "cue span lead position={} lead_frames={} played=0 span_count=0 frequency={} generation={}",
        held_lead.device_position,
        lead_frames,
        held_lead.device_frequency,
        held_lead.stream_generation
    );
    gate.initialization_barrier().unwrap();
    let now = base.elapsed().as_millis() as u64;
    let challenge = gate.challenge(now).unwrap();
    let mut renewal = PublicExecutionLease {
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
        preparation_id: preparation.clone(),
    };
    gate.accept(&renewal, now, &Operation::new(1000).unwrap())
        .unwrap();
    assert!(experimental.activate());
    let mut seq = 2u64;
    let mut renew = || {
        let now = base.elapsed().as_millis() as u64;
        renewal.challenge = gate.challenge(now).unwrap();
        renewal.seq = seq;
        gate.accept(&renewal, now, &Operation::new(1000).unwrap())
            .unwrap();
        seq += 1;
    };
    let open_deadline = Instant::now() + Duration::from_millis(1500);
    let mut live = probe.clock_reading();
    while Instant::now() < open_deadline {
        renew();
        live = probe.clock_reading();
        if probe.cue_span_count() == 1
            && live.played_samples > 0
            && probe.cue_device_end() > probe.cue_device_start()
            && live.submitted_samples > live.played_samples
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
        assert!(!probe.is_finished(), "{live:?}");
    }
    let start = probe.cue_device_start();
    let end = probe.cue_device_end();
    let mapped = end.saturating_sub(start) / 2;
    assert_eq!(
        probe.cue_span_count(),
        1,
        "start={start} end={end} {live:?}"
    );
    assert!(start > 0, "start={start}");
    assert!(end > start, "start={start} end={end}");
    assert!(
        start >= lead_frames,
        "start={start} lead_frames={lead_frames} {live:?}"
    );
    assert!(live.played_samples > 0, "{live:?}");
    assert!(
        live.played_samples <= mapped,
        "played={} mapped={mapped} {live:?}",
        live.played_samples
    );
    assert_eq!(
        probe.sentence_cap(),
        mapped,
        "cap={} mapped={mapped}",
        probe.sentence_cap()
    );
    assert_eq!(
        live.submitted_samples,
        end.saturating_sub(start).div_ceil(2),
        "{live:?}"
    );
    assert_eq!(live.stream_generation, 1, "{live:?}");
    println!(
        "cue span open start={} end={} mapped={} played={} submitted={} position={} frequency={} generation={}",
        start,
        end,
        mapped,
        live.played_samples,
        live.submitted_samples,
        live.device_position,
        live.device_frequency,
        live.stream_generation
    );
    probe.set_gap(true);
    let notice = Instant::now() + Duration::from_millis(500);
    while !probe.is_gapping() && Instant::now() < notice {
        renew();
        std::thread::sleep(Duration::from_millis(20));
        assert!(!probe.is_finished(), "{live:?}");
    }
    assert!(probe.is_gapping());
    let frozen_start = probe.cue_device_start();
    let frozen_end = probe.cue_device_end();
    let frozen_mapped = frozen_end.saturating_sub(frozen_start) / 2;
    assert_eq!(probe.cue_span_count(), 1);
    assert_eq!(frozen_start, start);
    assert!(frozen_end >= end, "end={end} frozen_end={frozen_end}");
    assert_eq!(
        probe.sentence_cap(),
        frozen_mapped,
        "cap={} mapped={frozen_mapped}",
        probe.sentence_cap()
    );
    let submitted_at_gap = probe.clock_reading().submitted_samples;
    assert_eq!(
        submitted_at_gap,
        frozen_end.saturating_sub(frozen_start).div_ceil(2)
    );
    let drain = Instant::now() + Duration::from_millis(1500);
    live = probe.clock_reading();
    while live.played_samples != frozen_mapped && Instant::now() < drain {
        assert!(
            live.played_samples <= frozen_mapped,
            "played={} mapped={frozen_mapped} {live:?}",
            live.played_samples
        );
        assert_eq!(probe.cue_device_end(), frozen_end);
        assert_eq!(probe.cue_span_count(), 1);
        assert_eq!(live.submitted_samples, submitted_at_gap, "{live:?}");
        renew();
        std::thread::sleep(Duration::from_millis(20));
        assert!(!probe.is_finished(), "{live:?}");
        live = probe.clock_reading();
    }
    assert_eq!(
        live.played_samples, frozen_mapped,
        "played={} mapped={frozen_mapped} {live:?}",
        live.played_samples
    );
    let silence_start = live.device_position;
    let frequency = live.device_frequency;
    let window = Instant::now() + Duration::from_millis(250);
    while Instant::now() < window {
        renew();
        std::thread::sleep(Duration::from_millis(20));
        live = probe.clock_reading();
        assert_eq!(live.played_samples, frozen_mapped, "{live:?}");
        assert_eq!(live.submitted_samples, submitted_at_gap, "{live:?}");
        assert_eq!(probe.cue_device_start(), frozen_start);
        assert_eq!(probe.cue_device_end(), frozen_end);
        assert_eq!(probe.cue_span_count(), 1);
        assert_eq!(live.device_frequency, frequency, "{live:?}");
        assert_eq!(live.stream_generation, 1, "{live:?}");
        assert!(!probe.is_finished(), "{live:?}");
    }
    let device_frames = device_frames_for_position(live.device_position, frequency);
    let delta_samples = source_samples_for_device_position(
        live.device_position.saturating_sub(silence_start),
        frequency,
    );
    println!(
        "cue span after end played={} start={} end={} position={} device_frames={} delta_samples={} frequency={} generation={}",
        live.played_samples,
        frozen_start,
        frozen_end,
        live.device_position,
        device_frames,
        delta_samples,
        live.device_frequency,
        live.stream_generation
    );
    assert!(
        device_frames >= frozen_end,
        "frames={device_frames} end={frozen_end} {live:?}"
    );
    assert!(delta_samples >= 2_000, "delta={delta_samples} {live:?}");
    assert!(probe.stop_within(Duration::from_millis(500)).is_ok());
}
