//! Same-stream cue interval to cue-local source samples.
//!
//! The caller binds the interval to the authorized cue and lease epoch.
//! Silence outside the interval is not a cue. Cancel and device loss refuse
//! mapping. A mapped sample is not a playback permit.

use crate::device_clock::{ClockError, DeviceClockCursor};

pub const PUBLIC_DEVICE_RATE: u32 = 48_000;
pub const PUBLIC_SOURCE_RATE: u32 = 24_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CueClockGate {
    Live,
    Cancelled,
    DeviceLost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CueClockRefusal {
    Epoch,
    Stream,
    Frequency,
    InvalidCue,
    Cancelled,
    DeviceLost,
    Overflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CueReading {
    pub stream_generation: u64,
    pub device_frequency: u64,
    pub device_position: u64,
    pub cue_device_start: u64,
    pub cue_device_end: u64,
    pub cue_span_count: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicCueBinding {
    epoch: u64,
    stream_generation: u64,
    device_frequency: u64,
    cue_start: u64,
    cue_end: u64,
}

impl PublicCueBinding {
    pub fn bind(
        epoch: u64,
        stream_generation: u64,
        device_frequency: u64,
        cue_start: u64,
        cue_end: u64,
    ) -> Result<Self, CueClockRefusal> {
        if epoch == 0 {
            return Err(CueClockRefusal::Epoch);
        }
        if stream_generation == 0 {
            return Err(CueClockRefusal::Stream);
        }
        if device_frequency == 0 {
            return Err(CueClockRefusal::Frequency);
        }
        if cue_start >= cue_end {
            return Err(CueClockRefusal::InvalidCue);
        }
        Ok(Self {
            epoch,
            stream_generation,
            device_frequency,
            cue_start,
            cue_end,
        })
    }

    pub fn source_at(
        &self,
        epoch: u64,
        device_position: u64,
        gate: CueClockGate,
    ) -> Result<Option<u64>, CueClockRefusal> {
        match gate {
            CueClockGate::Cancelled => return Err(CueClockRefusal::Cancelled),
            CueClockGate::DeviceLost => return Err(CueClockRefusal::DeviceLost),
            CueClockGate::Live => {}
        }
        if epoch != self.epoch {
            return Err(CueClockRefusal::Epoch);
        }
        let mut cursor = DeviceClockCursor::new(
            self.stream_generation,
            self.device_frequency,
            PUBLIC_DEVICE_RATE,
        )
        .map_err(map_clock_error)?;
        cursor.commit(u64::MAX).map_err(map_clock_error)?;
        cursor
            .observe_cue_source(
                self.stream_generation,
                self.device_frequency,
                device_position,
                self.cue_start,
                self.cue_end,
                PUBLIC_SOURCE_RATE,
            )
            .map_err(map_clock_error)
    }
}

/// Maps a published first-cue interval to cue-local source samples.
/// `cue_span_count == 0` is leading silence, not a cue.
pub fn map_public_cue(
    epoch: u64,
    reading: CueReading,
    gate: CueClockGate,
) -> Result<Option<u64>, CueClockRefusal> {
    match gate {
        CueClockGate::Cancelled => return Err(CueClockRefusal::Cancelled),
        CueClockGate::DeviceLost => return Err(CueClockRefusal::DeviceLost),
        CueClockGate::Live => {}
    }
    if reading.cue_span_count == 0 {
        return Ok(None);
    }
    PublicCueBinding::bind(
        epoch,
        reading.stream_generation,
        reading.device_frequency,
        reading.cue_device_start,
        reading.cue_device_end,
    )?
    .source_at(epoch, reading.device_position, CueClockGate::Live)
}

fn map_clock_error(error: ClockError) -> CueClockRefusal {
    match error {
        ClockError::Identity => CueClockRefusal::Stream,
        ClockError::Frequency => CueClockRefusal::Frequency,
        ClockError::InvalidCue => CueClockRefusal::InvalidCue,
        ClockError::Overflow => CueClockRefusal::Overflow,
        ClockError::Regression | ClockError::BeyondSubmitted => CueClockRefusal::InvalidCue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live(position: u64, start: u64, end: u64, spans: u64) -> CueReading {
        CueReading {
            stream_generation: 1,
            device_frequency: 384_000,
            device_position: position,
            cue_device_start: start,
            cue_device_end: end,
            cue_span_count: spans,
        }
    }

    #[test]
    fn leading_silence_is_not_a_cue() {
        assert_eq!(
            map_public_cue(3, live(51_128, 0, 0, 0), CueClockGate::Live),
            Ok(None)
        );
    }

    #[test]
    fn position_before_the_interval_does_not_advance() {
        assert_eq!(
            map_public_cue(3, live(100, 4_800, 28_800, 1), CueClockGate::Live),
            Ok(None)
        );
    }

    #[test]
    fn position_inside_the_interval_maps_cue_local_samples() {
        assert_eq!(
            map_public_cue(3, live(38_400, 4_800, 28_800, 1), CueClockGate::Live),
            Ok(Some(0))
        );
        assert_eq!(
            map_public_cue(3, live(134_400, 4_800, 28_800, 1), CueClockGate::Live),
            Ok(Some(6_000))
        );
    }

    #[test]
    fn position_at_or_after_the_end_does_not_advance() {
        assert_eq!(
            map_public_cue(3, live(230_400, 4_800, 28_800, 1), CueClockGate::Live),
            Ok(None)
        );
        assert_eq!(
            map_public_cue(3, live(205_120, 8_832, 12_192, 1), CueClockGate::Live),
            Ok(None)
        );
    }

    #[test]
    fn cancelled_and_device_lost_refuse_mapping() {
        let reading = live(134_400, 4_800, 28_800, 1);
        assert_eq!(
            map_public_cue(3, reading, CueClockGate::Cancelled),
            Err(CueClockRefusal::Cancelled)
        );
        assert_eq!(
            map_public_cue(3, reading, CueClockGate::DeviceLost),
            Err(CueClockRefusal::DeviceLost)
        );
        let bound = PublicCueBinding::bind(3, 1, 384_000, 4_800, 28_800).unwrap();
        assert_eq!(
            bound.source_at(4, 134_400, CueClockGate::Live),
            Err(CueClockRefusal::Epoch)
        );
    }

    #[test]
    fn bind_rejects_an_empty_or_unidentified_interval() {
        assert_eq!(
            PublicCueBinding::bind(0, 1, 384_000, 100, 200),
            Err(CueClockRefusal::Epoch)
        );
        assert_eq!(
            PublicCueBinding::bind(3, 0, 384_000, 100, 200),
            Err(CueClockRefusal::Stream)
        );
        assert_eq!(
            PublicCueBinding::bind(3, 1, 0, 100, 200),
            Err(CueClockRefusal::Frequency)
        );
        assert_eq!(
            PublicCueBinding::bind(3, 1, 384_000, 200, 200),
            Err(CueClockRefusal::InvalidCue)
        );
        assert_eq!(
            map_public_cue(3, live(100, 200, 200, 1), CueClockGate::Live),
            Err(CueClockRefusal::InvalidCue)
        );
    }

    #[test]
    fn leading_none_is_not_played_zero_for_selection() {
        let mapped = map_public_cue(3, live(51_128, 0, 0, 0), CueClockGate::Live).unwrap();
        assert!(mapped.is_none());
        assert_ne!(mapped, Some(0));
    }
}
