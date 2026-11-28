#![cfg(all(windows, feature = "clocked-experiment"))]
use nai_audio_cpal::clocked_experiment::ExperimentalFence;
use std::sync::{Arc, atomic::Ordering};
use std::time::{Duration, Instant};

#[test]
#[ignore = "explicit Windows silent device experiment"]
fn silent_same_stream_clock_stops_on_gate_revocation() {
    let gate = Arc::new(ExperimentalFence::default());
    let mut probe = nai_audio_cpal::clocked_experiment::open_silent(gate.clone()).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(probe.submitted.load(Ordering::Acquire), 0);
    assert_eq!(probe.played.load(Ordering::Acquire), 0);
    assert!(gate.activate());
    let end = Instant::now() + Duration::from_secs(2);
    while probe.played.load(Ordering::Acquire) < 2400 && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(10));
    }
    let played = probe.played.load(Ordering::Acquire);
    assert!(played >= 2400);
    assert!(played <= probe.submitted.load(Ordering::Acquire));
    gate.revoke();
    assert!(!gate.activate());
    let begin = Instant::now();
    assert!(probe.stop_within(Duration::from_millis(500)).is_ok());
    assert!(begin.elapsed() < Duration::from_millis(550));
    let stopped = probe.played.load(Ordering::Acquire);
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(probe.played.load(Ordering::Acquire), stopped);
    assert!(!probe.failed.load(Ordering::Acquire));
}

#[test]
fn invalid_pcm_is_rejected_before_opening_a_device() {
    let gate = Arc::new(ExperimentalFence::default());
    assert!(
        nai_audio_cpal::clocked_experiment::open_pcm(gate.clone(), zeroize::Zeroizing::new(vec![]))
            .is_err()
    );
    assert!(
        nai_audio_cpal::clocked_experiment::open_pcm(
            gate,
            zeroize::Zeroizing::new(vec![0; 480001])
        )
        .is_err()
    );
}

#[test]
fn long_diagnostic_rejects_unbounded_or_oversized_input_before_device_open() {
    use nai_audio_cpal::clocked_experiment::open_pcm_diagnostic;
    let gate = Arc::new(ExperimentalFence::default());
    let permitted = Arc::new(|| true);
    assert!(
        open_pcm_diagnostic(
            gate.clone(),
            zeroize::Zeroizing::new(vec![0; 2]),
            permitted.clone(),
            1
        )
        .is_err()
    );
    assert!(
        open_pcm_diagnostic(
            gate,
            zeroize::Zeroizing::new(vec![0; 2]),
            permitted,
            24_000 * 600 + 1
        )
        .is_err()
    );
}

#[test]
#[ignore = "explicit Windows silent device experiment"]
fn silent_stream_expires_without_control_renewal() {
    let gate = Arc::new(ExperimentalFence::default());
    let mut probe = nai_audio_cpal::clocked_experiment::open_silent(gate.clone()).unwrap();
    assert!(gate.activate_for(Duration::from_millis(300)));
    std::thread::sleep(Duration::from_millis(500));
    assert!(!gate.activate());
    assert!(probe.submitted.load(Ordering::Acquire) > 0);
    let stopped = probe.submitted.load(Ordering::Acquire);
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(probe.submitted.load(Ordering::Acquire), stopped);
    assert!(probe.stop_within(Duration::from_millis(500)).is_ok());
    assert!(!probe.failed.load(Ordering::Acquire));
}

#[test]
#[ignore = "explicit Windows silent device experiment"]
fn independent_fence_revocation_stops_submissions() {
    use std::sync::atomic::AtomicBool;
    let gate = Arc::new(ExperimentalFence::default());
    let permitted = Arc::new(AtomicBool::new(true));
    let permission_copy = permitted.clone();
    let mut probe = nai_audio_cpal::clocked_experiment::open_pcm_controlled(
        gate.clone(),
        zeroize::Zeroizing::new(vec![0; 24000]),
        Arc::new(move || permission_copy.load(Ordering::Acquire)),
    )
    .unwrap();
    assert!(gate.activate());
    let deadline = Instant::now() + Duration::from_millis(400);
    while probe.submitted.load(Ordering::Acquire) == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(probe.submitted.load(Ordering::Acquire) > 0);
    permitted.store(false, Ordering::Release);
    let exit_by = Instant::now() + Duration::from_millis(180);
    while !probe.is_finished() && Instant::now() < exit_by {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(probe.is_finished(), "external fence did not stop worker");
    let submitted = probe.submitted.load(Ordering::Acquire);
    std::thread::sleep(Duration::from_millis(120));
    assert_eq!(probe.submitted.load(Ordering::Acquire), submitted);
    assert!(!gate.activate());
    assert!(!probe.failed.load(Ordering::Acquire));
    assert!(probe.stop_within(Duration::from_millis(500)).is_ok());
}
