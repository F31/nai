//! Converts observations from the same WASAPI stream to the V5.6.1 clock.
//! This does not open a device, grant output permission or authenticate IPC.
use crate::clocked_experiment::{ClockReading, Probe};
use nai_contracts::v561_media::{ClockState, PlaybackClockObservation};

/// The existing WASAPI backend consumes mono PCM at this source rate.
pub const WASAPI_SOURCE_RATE: u32 = 24_000;

pub struct DeviceClock {
    epoch: u64,
    generation: u64,
    sequence: u64,
    last_played: Option<u64>,
}

impl DeviceClock {
    pub fn new(epoch: u64, generation: u64) -> Option<Self> {
        (epoch != 0 && generation != 0).then_some(Self {
            epoch,
            generation,
            sequence: 0,
            last_played: None,
        })
    }

    /// None means no usable clock: the caller must show the safe frame.
    pub fn observe(&mut self, probe: &Probe) -> Option<PlaybackClockObservation> {
        if !probe.sentence_clock_running() {
            return None;
        }
        let reading = probe.clock_reading();
        if !probe.sentence_clock_running() {
            return None;
        }
        self.observe_reading(reading)
    }

    /// Verified stream identity while the source fence is still unarmed.
    pub fn observe_prepared(&mut self, probe: &Probe) -> Option<PlaybackClockObservation> {
        let mut clock = self.observe_stopped(probe)?;
        if clock.played_sample != Some(0) {
            return None;
        }
        clock.state = ClockState::Unavailable as i32;
        clock.played_sample = None;
        clock.sample_rate = 0;
        Some(clock)
    }

    /// Source timeline ended/revoked; not a physical device-stop acknowledgement.
    pub fn observe_stopped(&mut self, probe: &Probe) -> Option<PlaybackClockObservation> {
        if !probe.confirmed_at_zero() || probe.failed.load(std::sync::atomic::Ordering::Acquire) {
            return None;
        }
        let mut clock = self.observe_reading(probe.clock_reading())?;
        clock.state = ClockState::Stopped as i32;
        Some(clock)
    }

    /// The state and count come from one source snapshot. Observe running first,
    /// then the final counter so normal thread completion cannot lose its endpoint.
    pub fn observe_sentence(
        &mut self,
        probe: &Probe,
        samples: u64,
    ) -> Option<PlaybackClockObservation> {
        if samples == 0
            || !probe.confirmed_at_zero()
            || probe.failed.load(std::sync::atomic::Ordering::Acquire)
        {
            return None;
        }
        let running = probe.sentence_clock_running();
        let mut clock = self.observe_reading(probe.clock_reading())?;
        let played = clock.played_sample?;
        if played > samples {
            return None;
        }
        if played == samples {
            clock.state = ClockState::Stopped as i32;
        } else if !running {
            return None;
        }
        Some(clock)
    }

    fn observe_reading(&mut self, reading: ClockReading) -> Option<PlaybackClockObservation> {
        if reading.stream_generation != self.generation
            || reading.device_frequency == 0
            || reading.played_samples > reading.submitted_samples
            || self
                .last_played
                .is_some_and(|last| reading.played_samples < last)
        {
            return None;
        }
        let next = self.sequence.checked_add(1)?;
        let result = PlaybackClockObservation {
            epoch: self.epoch,
            device_generation: self.generation,
            observation_sequence: Some(self.sequence),
            state: ClockState::Running as i32,
            played_sample: Some(reading.played_samples),
            sample_rate: WASAPI_SOURCE_RATE,
        };
        self.sequence = next;
        self.last_played = Some(reading.played_samples);
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reading() -> ClockReading {
        ClockReading {
            submitted_samples: 100,
            played_samples: 50,
            device_frequency: 48000,
            device_position: 100,
            stream_generation: 2,
            cue_device_start: 0,
            cue_device_end: 0,
            cue_span_count: 0,
        }
    }
    #[test]
    fn binds_clock_to_stream_and_rejects_unwritten_or_regressed_progress() {
        let mut c = DeviceClock::new(1, 2).unwrap();
        let first = c.observe_reading(reading()).unwrap();
        assert_eq!(
            (
                first.epoch,
                first.device_generation,
                first.played_sample,
                first.observation_sequence
            ),
            (1, 2, Some(50), Some(0))
        );
        let mut bad = reading();
        bad.played_samples = 101;
        assert!(c.observe_reading(bad).is_none());
        let mut bad = reading();
        bad.played_samples = 49;
        assert!(c.observe_reading(bad).is_none());
        let mut bad = reading();
        bad.stream_generation = 3;
        assert!(c.observe_reading(bad).is_none());
        let mut bad = reading();
        bad.device_frequency = 0;
        assert!(c.observe_reading(bad).is_none());
        assert_eq!(
            c.observe_reading(reading()).unwrap().observation_sequence,
            Some(1)
        );
    }

    #[test]
    #[ignore = "requires a real Windows output device; plays silence"]
    fn real_wasapi_observation_advances_and_stops() {
        use crate::clocked_experiment::{ExperimentalFence, open_silent};
        use std::{
            sync::Arc,
            time::{Duration, Instant},
        };
        let gate = Arc::new(ExperimentalFence::default());
        let mut probe = open_silent(gate.clone()).unwrap();
        assert!(gate.activate());
        let mut clock = DeviceClock::new(1, 1).unwrap();
        let end = Instant::now() + Duration::from_millis(400);
        let mut maximum = 0;
        while Instant::now() < end {
            if let Some(value) = clock.observe(&probe) {
                maximum = maximum.max(value.played_sample.unwrap());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        probe.set_hold(true);
        assert!(clock.observe(&probe).is_none());
        gate.revoke();
        assert!(clock.observe(&probe).is_none());
        probe.stop_within(Duration::from_millis(500)).unwrap();
        assert!(maximum > 0);
        assert!(clock.observe(&probe).is_none());
    }

    #[test]
    #[ignore = "requires a real Windows output device; plays silence"]
    fn real_device_clock_drives_v561_frame_selection() {
        use crate::clocked_experiment::{ExperimentalFence, open_silent};
        use nai_contracts::{
            v561_media::{FrameRef, PixelFormat},
            v561_media_validation::FrameLimits,
        };
        use nai_media_core::v561_frame_scheduler::Scheduler;
        use std::{
            sync::Arc,
            time::{Duration, Instant},
        };

        let gate = Arc::new(ExperimentalFence::default());
        let mut probe = open_silent(gate.clone()).unwrap();
        assert!(gate.activate());
        let mut clock = DeviceClock::new(1, 1).unwrap();
        let limits = FrameLimits {
            slots: 1,
            max_width: 1,
            max_height: 1,
            max_payload_bytes: 4,
            max_sample_rate: 48000,
        };
        let mut scheduler = Scheduler::new("test-pool".into(), 1, 1, limits, 1, 24000).unwrap();
        scheduler
            .offer(
                FrameRef {
                    pool_id: "test-pool".into(),
                    epoch: 1,
                    sequence: Some(0),
                    source_sample: Some(0),
                    source_sample_rate: 24000,
                    pts_ns: Some(0),
                    width: 1,
                    height: 1,
                    stride_bytes: 4,
                    pixel_format: PixelFormat::Bgra8 as i32,
                    slot_index: Some(0),
                    slot_generation: 1,
                    payload_bytes: 4,
                    content_sha256: vec![],
                },
                vec![1, 2, 3, 255],
            )
            .unwrap();
        let deadline = Instant::now() + Duration::from_millis(400);
        let mut selected = false;
        while Instant::now() < deadline {
            if let Some(observation) = clock.observe(&probe) {
                selected = scheduler
                    .present(&observation)
                    .unwrap()
                    .is_some_and(|frame| frame.pixels == [1, 2, 3, 255]);
                if selected {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        gate.revoke();
        scheduler.stop();
        probe.stop_within(Duration::from_millis(500)).unwrap();
        assert!(selected);
    }
}
