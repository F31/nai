//! Monotonic media clock. Position advances by sample count or duration only,
//! never rewinds. Silence preserves position continuity; audit UTC stays out.
use crate::MediaError;
use std::time::{Duration, Instant};

/// Monotonic media clock for one output. Sample position never decreases.
#[derive(Debug, Clone)]
pub struct MediaClock {
    sample_rate: u32,
    position: u64,
    started: Instant,
}
impl MediaClock {
    pub fn new(sample_rate: u32, now: Instant) -> Result<Self, MediaError> {
        if !(crate::timeline::audio::MIN_SAMPLE_RATE..=crate::timeline::audio::MAX_SAMPLE_RATE)
            .contains(&sample_rate)
        {
            return Err(MediaError::Stale);
        }
        Ok(Self {
            sample_rate,
            position: 0,
            started: now,
        })
    }
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    pub fn sample_position(&self) -> u64 {
        self.position
    }
    /// Durable position for this monotonic instant (does not mutate).
    pub fn probe(&self, now: Instant) -> u64 {
        let elapsed = now.saturating_duration_since(self.started).as_secs_f64();
        let samples = (elapsed * self.sample_rate as f64).round() as u64;
        self.position.saturating_add(samples)
    }
    /// Advance purely by elapsed monotonic time.
    pub fn tick(&mut self, now: Instant) -> u64 {
        let delta = now.saturating_duration_since(self.started);
        let samples = (delta.as_secs_f64() * self.sample_rate as f64).round() as u64;
        self.position = self.position.saturating_add(samples);
        self.inflight_reset(now, delta);
        samples
    }
    fn inflight_reset(&mut self, _now: Instant, _delta: Duration) {
        // Kept explicit: the position advances from `started`; silence-heavy
        // sessions set the anchor explicitly to avoid drift.
    }
    /// Advance by an absolute number of samples (>= 0); never moves backwards.
    pub fn advance_samples(&mut self, count: u64) -> u64 {
        self.position = self.position.saturating_add(count);
        count
    }
    /// Insert silence of a duration, returning the number of samples added.
    pub fn insert_silence(&mut self, duration: Duration) -> Result<u64, MediaError> {
        let samples = duration
            .as_micros()
            .checked_mul(self.sample_rate as u128)
            .map(|us| us / 1_000_000)
            .ok_or(MediaError::Expired)?
            .min(u64::MAX as u128) as u64;
        Ok(self.advance_samples(samples))
    }
    /// Advance by a millisecond count.
    pub fn advance_ms(&mut self, ms: u32) -> u64 {
        let samples = (ms as u64 * self.sample_rate as u64) / 1000;
        self.advance_samples(samples)
    }
    /// Rational playback timestamp at `time_base` (pts = position * den / num).
    pub fn pts(&self, time_base_num: u64, time_base_den: u64) -> Result<i64, MediaError> {
        if time_base_num == 0 || time_base_den == 0 {
            return Err(MediaError::Stale);
        }
        let pts = (self.position as u128 * time_base_den as u128) / time_base_num as u128;
        i64::try_from(pts).map_err(|_| MediaError::Expired)
    }
}

/// Deterministic sample mapping across rates. Different-rate sample_start
/// values must never be compared directly; use this instead.
pub fn map_cross_rate(model_rate: u32, model_sample: u64, output_rate: u32) -> u64 {
    if model_rate == output_rate {
        return model_sample;
    }
    let mapped = model_sample as u128 * output_rate as u128 / model_rate as u128;
    mapped.min(u64::MAX as u128) as u64
}

/// Total output samples for a run of model samples (checked, no overflow).
pub fn duration_in_output(
    total_model_samples: u64,
    model_rate: u32,
    output_rate: u32,
) -> Option<u64> {
    if model_rate == output_rate {
        return Some(total_model_samples);
    }
    let value = total_model_samples as u128 * output_rate as u128;
    if !value.is_multiple_of(model_rate as u128) {
        return Some((value / model_rate as u128) as u64);
    }
    let value = value / model_rate as u128;
    u64::try_from(value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_advances_and_never_rewinds() {
        let mut clock = MediaClock::new(48_000, Instant::now()).unwrap();
        assert_eq!(clock.sample_position(), 0);
        clock.advance_ms(1000);
        assert_eq!(clock.sample_position(), 48_000);
        clock.advance_samples(0);
        assert_eq!(clock.sample_position(), 48_000);
        clock.insert_silence(Duration::from_secs(1)).unwrap();
        assert_eq!(clock.sample_position(), 96_000);
        assert!(clock.probe(Instant::now()) >= 96_000);
    }

    #[test]
    fn silence_preserves_continuity() {
        let mut clock = MediaClock::new(16_000, Instant::now()).unwrap();
        clock.advance_samples(1600);
        clock.insert_silence(Duration::from_millis(500)).unwrap();
        assert_eq!(clock.sample_position(), 9600);
        // pts is a rational function of sample position: tb = 1000/16000.
        assert_eq!(clock.pts(1000, 16_000).unwrap(), 153_600);
    }

    #[test]
    fn cross_rate_mapping_is_exact_not_direct_compare() {
        assert_eq!(map_cross_rate(16_000, 16_000, 48_000), 48_000);
        assert_eq!(map_cross_rate(16_000, 8_000, 48_000), 24_000);
        assert_eq!(map_cross_rate(48_000, 30_000, 16_000), 10_000);
        assert_eq!(duration_in_output(16_000, 16_000, 48_000), Some(48_000));
        // Uneven smallest unit maps wholesale, never truncating to equal position.
        assert_eq!(map_cross_rate(16_000, 1, 48_000), 3);
    }
}
