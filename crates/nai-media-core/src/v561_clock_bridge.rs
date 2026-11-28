//! Validate an already-authenticated audio source. Does not authenticate an IPC peer.
use nai_contracts::{
    v561_media::{ClockState, PlaybackClockObservation},
    v561_media_validation::validate_clock,
};
use std::time::{Duration, Instant};

pub struct ClockBridge {
    epoch: u64,
    samples: u64,
    rate: u32,
    max_age: Duration,
    latest: Option<(PlaybackClockObservation, Instant)>,
    revoked: bool,
}
impl ClockBridge {
    pub fn new(epoch: u64, samples: u64, rate: u32, max_age: Duration) -> Option<Self> {
        (epoch != 0
            && samples != 0
            && rate != 0
            && rate <= 96000
            && !max_age.is_zero()
            && max_age <= Duration::from_millis(200))
        .then_some(Self {
            epoch,
            samples,
            rate,
            max_age,
            latest: None,
            revoked: false,
        })
    }
    pub fn stop(&mut self) {
        self.revoked = true;
    }
    pub fn accept(&mut self, clock: PlaybackClockObservation) -> Result<(), &'static str> {
        self.accept_at(clock, Instant::now())
    }
    /// Trusted transport supplies a conservative observation time including
    /// device snapshot age and RPC delay. It never changes the sample counter.
    pub fn accept_at(
        &mut self,
        clock: PlaybackClockObservation,
        at: Instant,
    ) -> Result<(), &'static str> {
        let now = Instant::now();
        let valid = !self.revoked
            && at <= now
            && now.duration_since(at) <= self.max_age
            && validate_clock(&clock, self.rate).is_ok()
            && clock.epoch == self.epoch
            && clock
                .played_sample
                .is_none_or(|played| played <= self.samples)
            && (clock.state == ClockState::Unavailable as i32 || clock.sample_rate == self.rate)
            && match &self.latest {
                None => clock.state == ClockState::Unavailable as i32,
                Some((old, _)) => {
                    clock.device_generation == old.device_generation
                        && clock.observation_sequence > old.observation_sequence
                        && old.state != ClockState::Stopped as i32
                        && clock.state != ClockState::Unavailable as i32
                        && clock.played_sample >= Some(old.played_sample.unwrap_or(0))
                }
            };
        if !valid {
            self.stop();
            return Err("audio clock rejected");
        }
        self.latest = Some((clock, at));
        Ok(())
    }
    /// Wall time is used only to reject stale evidence, never to advance samples.
    pub fn running(&self) -> Option<&PlaybackClockObservation> {
        let (clock, at) = self.latest.as_ref()?;
        (!self.revoked && at.elapsed() <= self.max_age && clock.state == ClockState::Running as i32)
            .then_some(clock)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn clock(seq: u64, played: Option<u64>, state: ClockState) -> PlaybackClockObservation {
        PlaybackClockObservation {
            epoch: 7,
            device_generation: 3,
            observation_sequence: Some(seq),
            state: state as i32,
            played_sample: played,
            sample_rate: if state == ClockState::Unavailable {
                0
            } else {
                24000
            },
        }
    }
    fn bridge() -> ClockBridge {
        ClockBridge::new(7, 24000, 24000, Duration::from_millis(200)).unwrap()
    }
    #[test]
    fn source_identity_sequence_and_counter_faults_are_terminal() {
        for mode in 0..5 {
            let mut bridge = bridge();
            bridge
                .accept(clock(0, None, ClockState::Unavailable))
                .unwrap();
            bridge
                .accept(clock(1, Some(100), ClockState::Running))
                .unwrap();
            let mut bad = clock(2, Some(200), ClockState::Running);
            match mode {
                0 => bad.epoch = 8,
                1 => bad.device_generation = 4,
                2 => bad.observation_sequence = Some(1),
                3 => bad.played_sample = Some(99),
                _ => bad.played_sample = Some(24001),
            }
            assert!(bridge.accept(bad).is_err());
            assert!(bridge.running().is_none());
            assert!(
                bridge
                    .accept(clock(3, Some(300), ClockState::Running))
                    .is_err()
            );
        }
    }
    #[test]
    fn prepared_and_stopped_are_never_running() {
        let mut bridge = bridge();
        bridge
            .accept(clock(0, None, ClockState::Unavailable))
            .unwrap();
        assert!(bridge.running().is_none());
        bridge
            .accept(clock(1, Some(0), ClockState::Running))
            .unwrap();
        assert_eq!(bridge.running().unwrap().played_sample, Some(0));
        bridge
            .accept(clock(2, Some(24000), ClockState::Stopped))
            .unwrap();
        assert!(bridge.running().is_none());
        assert!(
            bridge
                .accept(clock(3, Some(24000), ClockState::Running))
                .is_err()
        );
    }
    #[test]
    fn silence_or_time_does_not_invent_progress() {
        let mut bridge = ClockBridge::new(7, 24000, 24000, Duration::from_millis(1)).unwrap();
        bridge
            .accept(clock(0, None, ClockState::Unavailable))
            .unwrap();
        bridge
            .accept(clock(1, Some(123), ClockState::Running))
            .unwrap();
        std::thread::sleep(Duration::from_millis(5));
        assert!(bridge.running().is_none());
        assert_eq!(bridge.latest.as_ref().unwrap().0.played_sample, Some(123));
    }
}
