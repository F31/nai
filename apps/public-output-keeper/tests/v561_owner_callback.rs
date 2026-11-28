#![cfg(windows)]
//! Explicit silent device test, not avatar or microphone acceptance.
use nai_audio_cpal::clocked_experiment::{ExperimentalFence, open_pcm_diagnostic};
use nai_kernel::output_owner::{OutputOwner, Speaker};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

#[test]
#[ignore = "requires a Windows audio output device; silent fixture"]
fn takeover_stops_old_audio_callback() {
    let root = Arc::new(OutputOwner::default());
    let permitted = root.acquire_callback(Speaker::Narration).unwrap();
    let fence = Arc::new(ExperimentalFence::externally_controlled());
    let mut probe = open_pcm_diagnostic(
        fence.clone(),
        Zeroizing::new(vec![0; 24000 * 4]),
        Arc::new(permitted),
        24000 * 4,
    )
    .unwrap();
    assert!(fence.activate());
    let start = Instant::now();
    while probe.clock_reading().played_samples == 0 && start.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        probe.clock_reading().played_samples > 0,
        "silent playback never advanced"
    );
    let _human = root.acquire(Speaker::Human).unwrap();
    let revoked = Instant::now();
    while !probe.is_finished() && revoked.elapsed() < Duration::from_millis(500) {
        std::thread::sleep(Duration::from_millis(5));
    }
    let stopped = probe.is_finished();
    fence.revoke();
    probe.stop_within(Duration::from_millis(500)).unwrap();
    assert!(
        stopped,
        "old audio callback did not stop within diagnostic bound"
    );
}
