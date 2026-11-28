//! Validate authenticated current-wire stream receipts. Authentication is the caller's job.
use crate::v561_clock_bridge::ClockBridge;
use nai_contracts::{
    v561_media::{ClockState, PlaybackClockObservation},
    v561_stream_audio::{Receipt, State, WireVersion},
};
use std::time::{Duration, Instant};

pub struct StreamClock {
    bridge: ClockBridge,
    revoked: bool,
}
impl StreamClock {
    pub fn new(epoch: u64, max_samples: u64) -> Option<Self> {
        Some(Self {
            bridge: ClockBridge::new(epoch, max_samples, 24000, Duration::from_millis(200))?,
            revoked: false,
        })
    }
    pub fn accept(
        &mut self,
        r: &Receipt,
        request_at: Instant,
    ) -> Result<(PlaybackClockObservation, Instant), &'static str> {
        let result = (|| {
            let clock = r.clock.ok_or("missing device clock")?;
            if self.revoked
                || r.version != WireVersion::Current as u32
                || r.clock_age_ms > 200
                || clock.epoch != r.epoch
                || clock.played_sample.unwrap_or(0) != r.played_samples
                || clock.observation_sequence != Some(r.sequence)
                || r.played_samples > r.submitted_samples
                || r.submitted_samples > r.received_samples
            {
                return Err("stream clock envelope rejected");
            }
            let prepared = r.state == State::Prepared as i32;
            let valid = match (State::try_from(r.state), ClockState::try_from(clock.state)) {
                (Ok(State::Prepared), Ok(ClockState::Unavailable)) => {
                    r.sequence == 1 && r.received_samples == 0 && r.played_samples == 0
                }
                (Ok(State::Running), Ok(ClockState::Running)) => true,
                (Ok(State::Completed), Ok(ClockState::Stopped)) => {
                    r.sealed && r.played_samples == r.received_samples
                }
                (Ok(State::Stopped), Ok(ClockState::Stopped)) => true,
                _ => false,
            };
            if !valid {
                return Err("stream clock state rejected");
            }
            let base = if prepared { Instant::now() } else { request_at };
            let observed_at = base
                .checked_sub(Duration::from_millis(r.clock_age_ms.into()))
                .ok_or("clock time overflow")?;
            self.bridge.accept_at(clock, observed_at)?;
            Ok((clock, observed_at + Duration::from_millis(200)))
        })();
        if result.is_err() {
            self.revoked = true;
            self.bridge.stop();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn receipt(seq: u64, played: u64, state: State) -> Receipt {
        Receipt {
            segments: Vec::new(),
            version: WireVersion::Current as u32,
            epoch: 7,
            sequence: seq,
            state: state as i32,
            received_samples: if state == State::Prepared { 0 } else { 24000 },
            submitted_samples: if state == State::Prepared { 0 } else { 24000 },
            played_samples: played,
            free_samples: 0,
            sealed: state == State::Completed,
            mac: vec![],
            clock_age_ms: 0,
            clock: Some(PlaybackClockObservation {
                epoch: 7,
                device_generation: 3,
                observation_sequence: Some(seq),
                state: if state == State::Prepared {
                    ClockState::Unavailable
                } else if state == State::Completed {
                    ClockState::Stopped
                } else {
                    ClockState::Running
                } as i32,
                played_sample: if state == State::Prepared {
                    None
                } else {
                    Some(played)
                },
                sample_rate: if state == State::Prepared { 0 } else { 24000 },
            }),
        }
    }
    #[test]
    fn fresh_control_does_not_refresh_stale_device_poll_or_slow_roundtrip() {
        for (age, lag) in [(201, 0), (100, 110)] {
            let mut c = StreamClock::new(7, 24000).unwrap();
            c.accept(&receipt(1, 0, State::Prepared), Instant::now())
                .unwrap();
            let mut r = receipt(2, 100, State::Running);
            r.clock_age_ms = age;
            assert!(
                c.accept(&r, Instant::now() - Duration::from_millis(lag))
                    .is_err()
            );
            assert!(
                c.accept(&receipt(3, 200, State::Running), Instant::now())
                    .is_err()
            );
        }
    }
    #[test]
    fn missing_old_forged_identity_and_counter_are_terminal() {
        for fault in 0..7 {
            let mut c = StreamClock::new(7, 24000).unwrap();
            c.accept(&receipt(1, 0, State::Prepared), Instant::now())
                .unwrap();
            let mut r = receipt(2, 100, State::Running);
            match fault {
                0 => r.version = 1,
                1 => r.clock = None,
                2 => r.clock.as_mut().unwrap().epoch = 8,
                3 => r.clock.as_mut().unwrap().device_generation = 4,
                4 => r.clock.as_mut().unwrap().played_sample = Some(101),
                5 => r.clock.as_mut().unwrap().observation_sequence = Some(1),
                _ => r.submitted_samples = 99,
            }
            assert!(c.accept(&r, Instant::now()).is_err());
        }
    }
    #[test]
    fn stopped_does_not_reopen_and_wall_time_cannot_advance_source() {
        let mut c = StreamClock::new(7, 24000).unwrap();
        c.accept(&receipt(1, 0, State::Prepared), Instant::now())
            .unwrap();
        let (r, deadline) = c
            .accept(&receipt(2, 123, State::Running), Instant::now())
            .unwrap();
        assert_eq!(r.played_sample, Some(123));
        assert!(deadline > Instant::now());
        c.accept(&receipt(3, 24000, State::Completed), Instant::now())
            .unwrap();
        assert!(
            c.accept(&receipt(4, 24000, State::Running), Instant::now())
                .is_err()
        );
    }
}
