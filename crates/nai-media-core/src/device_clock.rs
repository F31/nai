//! Stream-local device-position validation; never a PlaybackAck producer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockError {
    Identity,
    Frequency,
    Regression,
    BeyondSubmitted,
    Overflow,
    InvalidCue,
}

pub struct DeviceClockCursor {
    stream: u64,
    frequency: u64,
    rate: u32,
    position: u64,
    submitted: u64,
}

impl DeviceClockCursor {
    /// A new stream must be observed before start, at position zero.
    pub fn new(stream: u64, frequency: u64, rate: u32) -> Result<Self, ClockError> {
        if stream == 0 {
            return Err(ClockError::Identity);
        }
        if frequency == 0 || rate == 0 {
            return Err(ClockError::Frequency);
        }
        Ok(Self {
            stream,
            frequency,
            rate,
            position: 0,
            submitted: 0,
        })
    }

    /// Call only after the backend has successfully released this many frames.
    pub fn commit(&mut self, frames: u64) -> Result<(), ClockError> {
        self.submitted = self
            .submitted
            .checked_add(frames)
            .ok_or(ClockError::Overflow)?;
        Ok(())
    }

    /// Convert a verified device position to a source sample position.
    /// This only applies to a stream starting at source sample zero with no
    /// inserted silence; cue offsets and gaps require a separate timeline.
    pub fn observe_source(
        &mut self,
        stream: u64,
        frequency: u64,
        position: u64,
        source_rate: u32,
    ) -> Result<u64, ClockError> {
        if source_rate == 0 {
            return Err(ClockError::Frequency);
        }
        let frames = self.observe(stream, frequency, position)?;
        let source = u128::from(frames) * u128::from(source_rate) / u128::from(self.rate);
        u64::try_from(source).map_err(|_| ClockError::Overflow)
    }

    /// Map a committed, contiguous cue's half-open device-frame interval to
    /// cue-local source samples. Silence outside the interval is not a cue.
    /// The caller must independently bind the interval to the authorized cue
    /// and stream epoch. A device position is not a hardware completion ACK.
    pub fn observe_cue_source(
        &mut self,
        stream: u64,
        frequency: u64,
        position: u64,
        cue_start: u64,
        cue_end: u64,
        source_rate: u32,
    ) -> Result<Option<u64>, ClockError> {
        if source_rate == 0 {
            return Err(ClockError::Frequency);
        }
        if cue_start >= cue_end || cue_end > self.submitted {
            return Err(ClockError::InvalidCue);
        }
        let frames = self.observe(stream, frequency, position)?;
        if frames < cue_start || frames >= cue_end {
            return Ok(None);
        }
        let source =
            u128::from(frames - cue_start) * u128::from(source_rate) / u128::from(self.rate);
        Ok(Some(
            u64::try_from(source).map_err(|_| ClockError::Overflow)?,
        ))
    }

    pub fn observe(
        &mut self,
        stream: u64,
        frequency: u64,
        position: u64,
    ) -> Result<u64, ClockError> {
        if stream != self.stream {
            return Err(ClockError::Identity);
        }
        if frequency != self.frequency {
            return Err(ClockError::Frequency);
        }
        if position < self.position {
            return Err(ClockError::Regression);
        }
        let frames = u128::from(position) * u128::from(self.rate) / u128::from(frequency);
        let frames = u64::try_from(frames).map_err(|_| ClockError::Overflow)?;
        if frames > self.submitted {
            return Err(ClockError::BeyondSubmitted);
        }
        self.position = position;
        Ok(frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn device_clock_uses_frequency_not_qpc_or_channel_values() {
        let mut c = DeviceClockCursor::new(1, 384000, 48000).unwrap();
        c.commit(480).unwrap();
        assert_eq!(c.observe(1, 384000, 7), Ok(0));
        assert_eq!(c.observe(1, 384000, 3840), Ok(480));
        assert_eq!(c.observe(1, 384000, 3840), Ok(480));
    }
    #[test]
    fn device_clock_rejects_reset_identity_frequency_and_unwritten_progress() {
        let mut c = DeviceClockCursor::new(1, 48000, 48000).unwrap();
        c.commit(100).unwrap();
        assert_eq!(c.observe(1, 48000, 50), Ok(50));
        assert_eq!(c.observe(2, 48000, 50), Err(ClockError::Identity));
        assert_eq!(c.observe(1, 96000, 50), Err(ClockError::Frequency));
        assert_eq!(c.observe(1, 48000, 49), Err(ClockError::Regression));
        assert_eq!(c.observe(1, 48000, 101), Err(ClockError::BeyondSubmitted));
        assert_eq!(c.observe(1, 48000, 100), Ok(100));
    }
    #[test]
    fn source_rate_mapping_preserves_stream_identity_and_submission_bounds() {
        let mut c = DeviceClockCursor::new(7, 384000, 48000).unwrap();
        c.commit(48000).unwrap();
        assert_eq!(c.observe_source(7, 384000, 192000, 24000), Ok(12000));
        assert_eq!(
            c.observe_source(8, 384000, 192000, 24000),
            Err(ClockError::Identity)
        );
        assert_eq!(
            c.observe_source(7, 384000, 384008, 24000),
            Err(ClockError::BeyondSubmitted)
        );
        assert_eq!(
            c.observe_source(7, 384000, 192000, 0),
            Err(ClockError::Frequency)
        );
        assert_eq!(c.observe_source(7, 384000, 384000, 24000), Ok(24000));
    }

    #[test]
    fn cue_clock_excludes_silence_and_rejects_uncommitted_or_stale_streams() {
        let mut c = DeviceClockCursor::new(7, 48000, 48000).unwrap();
        c.commit(48000).unwrap();
        assert_eq!(
            c.observe_cue_source(7, 48000, 100, 4800, 28800, 24000),
            Ok(None)
        );
        assert_eq!(
            c.observe_cue_source(7, 48000, 4800, 4800, 28800, 24000),
            Ok(Some(0))
        );
        assert_eq!(
            c.observe_cue_source(7, 48000, 16800, 4800, 28800, 24000),
            Ok(Some(6000))
        );
        assert_eq!(
            c.observe_cue_source(7, 48000, 28800, 4800, 28800, 24000),
            Ok(None)
        );
        assert_eq!(
            c.observe_cue_source(7, 48000, 30000, 28800, 50000, 24000),
            Err(ClockError::InvalidCue)
        );
        assert_eq!(
            c.observe_cue_source(8, 48000, 30000, 28800, 48000, 24000),
            Err(ClockError::Identity)
        );
        assert_eq!(
            c.observe_cue_source(7, 48000, 28799, 4800, 28800, 24000),
            Err(ClockError::Regression)
        );
        assert_eq!(
            c.observe_cue_source(7, 48000, 30000, 28800, 48000, 24000),
            Ok(Some(600))
        );
    }

    #[test]
    fn cue_device_position_handles_fractional_rate_and_exact_boundaries() {
        let mut c = DeviceClockCursor::new(3, 44100, 44100).unwrap();
        c.commit(88200).unwrap();
        assert_eq!(
            c.observe_cue_source(3, 44100, 4409, 4410, 48510, 24000),
            Ok(None)
        );
        assert_eq!(
            c.observe_cue_source(3, 44100, 4410, 4410, 48510, 24000),
            Ok(Some(0))
        );
        assert_eq!(
            c.observe_cue_source(3, 44100, 26460, 4410, 48510, 24000),
            Ok(Some(12000))
        );
        assert_eq!(
            c.observe_cue_source(3, 44100, 48509, 4410, 48510, 24000),
            Ok(Some(23999))
        );
        assert_eq!(
            c.observe_cue_source(3, 44100, 48510, 4410, 48510, 24000),
            Ok(None)
        );
        assert_eq!(
            c.observe_cue_source(3, 44100, 50000, 48510, 48510, 24000),
            Err(ClockError::InvalidCue)
        );
    }

    #[test]
    fn device_clock_checked_large_values_and_zero_parameters() {
        assert!(DeviceClockCursor::new(0, 1, 1).is_err());
        assert!(DeviceClockCursor::new(1, 0, 1).is_err());
        assert!(DeviceClockCursor::new(1, 1, 0).is_err());
        let mut c = DeviceClockCursor::new(1, 1, u32::MAX).unwrap();
        c.commit(u64::MAX).unwrap();
        assert_eq!(c.commit(1), Err(ClockError::Overflow));
        assert_eq!(c.observe(1, 1, u64::MAX), Err(ClockError::Overflow));
    }
}
