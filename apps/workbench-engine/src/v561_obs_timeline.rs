//! Sample alignment for the optional independent output consumer.
use nai_contracts::v561_media::{ClockState, FrameRef, PlaybackClockObservation};
use zeroize::{Zeroize, Zeroizing};

// Match the existing device input capacity so its admission prefill can be mirrored.
const PCM_LIMIT_SAMPLES: usize = crate::v561_stream_audio::DEFAULT_CAPACITY_SAMPLES as usize;
const PCM_SAMPLE_RATE: u32 = 24_000;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Identity,
    Discontinuous,
    Capacity,
    Unavailable,
    Revoked,
}

pub struct Timeline {
    epoch: u64,
    generation: u64,
    start: u64,
    pcm: Zeroizing<Vec<i16>>,
    revoked: bool,
}

impl Timeline {
    pub fn new(epoch: u64, generation: u64) -> Option<Self> {
        (epoch != 0 && generation != 0).then_some(Self {
            epoch,
            generation,
            start: 0,
            pcm: Zeroizing::new(Vec::new()),
            revoked: false,
        })
    }

    pub fn append(
        &mut self,
        epoch: u64,
        generation: u64,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), Error> {
        if self.revoked {
            return Err(Error::Revoked);
        }
        if (epoch, generation) != (self.epoch, self.generation) {
            return self.reject(Error::Identity);
        }
        if self.start.checked_add(self.pcm.len() as u64) != Some(offset)
            || bytes.is_empty()
            || !bytes.len().is_multiple_of(2)
        {
            return self.reject(Error::Discontinuous);
        }
        if bytes.len() / 2 > PCM_LIMIT_SAMPLES - self.pcm.len() {
            return self.reject(Error::Capacity);
        }
        self.pcm.extend(
            bytes
                .chunks_exact(2)
                .map(|pair| i16::from_le_bytes([pair[0], pair[1]])),
        );
        Ok(())
    }

    pub fn frame_start(
        &mut self,
        frame: &FrameRef,
        clock: &PlaybackClockObservation,
    ) -> Result<u64, Error> {
        if self.revoked {
            return Err(Error::Revoked);
        }
        if frame.epoch != self.epoch
            || (clock.epoch, clock.device_generation) != (self.epoch, self.generation)
        {
            return self.reject(Error::Identity);
        }
        let Some(sample) = frame.source_sample else {
            return self.reject(Error::Discontinuous);
        };
        if clock.state != ClockState::Running as i32
            || clock.sample_rate != PCM_SAMPLE_RATE
            || frame.source_sample_rate == 0
        {
            return self.reject(Error::Discontinuous);
        }
        let scaled =
            u128::from(sample) * u128::from(PCM_SAMPLE_RATE) / u128::from(frame.source_sample_rate);
        let Ok(scaled) = u64::try_from(scaled) else {
            return self.reject(Error::Discontinuous);
        };
        if clock.played_sample.is_none_or(|played| scaled > played) {
            return self.reject(Error::Discontinuous);
        }
        Ok(scaled)
    }

    /// Consume a contiguous prefix through the supplied playback sample, exclusive.
    pub fn take_until(&mut self, end: u64) -> Result<Zeroizing<Vec<i16>>, Error> {
        if self.revoked {
            return Err(Error::Revoked);
        }
        let Some(count) = end
            .checked_sub(self.start)
            .and_then(|delta| usize::try_from(delta).ok())
        else {
            return self.reject(Error::Discontinuous);
        };
        if count > self.pcm.len() {
            return Err(Error::Unavailable);
        }
        let taken = Zeroizing::new(self.pcm[..count].to_vec());
        let len = self.pcm.len();
        self.pcm.copy_within(count.., 0);
        self.pcm[len - count..].zeroize();
        self.pcm.truncate(len - count);
        self.start = end;
        Ok(taken)
    }

    pub fn revoke(&mut self) {
        self.revoked = true;
        self.pcm.zeroize();
    }

    fn reject<T>(&mut self, error: Error) -> Result<T, Error> {
        self.revoke();
        Err(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_16khz_frames_to_24khz_pcm() {
        let mut timeline = Timeline::new(7, 3).unwrap();
        timeline.append(7, 3, 0, &vec![0; 1920]).unwrap();
        let frame = FrameRef {
            epoch: 7,
            source_sample: Some(640),
            source_sample_rate: 16000,
            ..Default::default()
        };
        let clock = PlaybackClockObservation {
            epoch: 7,
            device_generation: 3,
            state: ClockState::Running as i32,
            sample_rate: 24000,
            played_sample: Some(960),
            ..Default::default()
        };
        assert_eq!(timeline.frame_start(&frame, &clock), Ok(960));
        assert_eq!(timeline.take_until(960).unwrap().len(), 960);
        assert_eq!(timeline.take_until(961).err(), Some(Error::Unavailable));
        timeline.append(7, 3, 960, &[1, 0]).unwrap();
        assert_eq!(timeline.take_until(961).unwrap().as_slice(), &[1]);
    }

    #[test]
    fn gap_wrong_generation_and_overflow_revoke_the_stream() {
        for (generation, offset, bytes, expected) in [
            (4, 0, vec![0; 2], Error::Identity),
            (3, 1, vec![0; 2], Error::Discontinuous),
            (3, 0, vec![0; (PCM_LIMIT_SAMPLES + 1) * 2], Error::Capacity),
        ] {
            let mut timeline = Timeline::new(7, 3).unwrap();
            assert_eq!(
                timeline.append(7, generation, offset, &bytes),
                Err(expected)
            );
            assert_eq!(timeline.append(7, 3, 0, &[0; 2]), Err(Error::Revoked));
            assert_eq!(timeline.take_until(0).err(), Some(Error::Revoked));
        }
    }
}
