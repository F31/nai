//! Audio block metadata and validation. Duration is a function of sample count,
//! and arrival time is never content time; audit UTC is not a media clock.
use crate::MediaError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleFormat {
    I16,
    F32,
}

pub const MIN_SAMPLE_RATE: u32 = 8000;
pub const MAX_SAMPLE_RATE: u32 = 96_000;
pub const MAX_SAMPLE_COUNT: u32 = 1 << 20;
pub const MAX_CHANNELS: u8 = 2;

/// One PCM block for an utterance. `sample_start` is in this block's own rate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioBlock {
    pub session_id: String,
    pub utterance_id: String,
    pub generation_epoch: u64,
    pub seq: u32,
    pub sample_rate: u32,
    pub channels: u8,
    pub format: SampleFormat,
    pub sample_start: u64,
    pub sample_count: u32,
    pub end_of_stream: bool,
}

impl AudioBlock {
    pub fn duration_samples(&self) -> u64 {
        self.sample_count as u64
    }

    /// Validate a block against its immediate predecessor within one utterance.
    /// The end-of-stream block is final; no data may follow it.
    pub fn validate(&self, previous: Option<&Self>) -> Result<(), MediaError> {
        if self.session_id.is_empty()
            || self.utterance_id.is_empty()
            || self.sample_rate < MIN_SAMPLE_RATE
            || self.sample_rate > MAX_SAMPLE_RATE
            || self.channels == 0
            || self.channels > MAX_CHANNELS
            || self.sample_count == 0
            || self.sample_count > MAX_SAMPLE_COUNT
        {
            return Err(MediaError::Stale);
        }
        if let Some(previous) = previous {
            if previous.utterance_id != self.utterance_id
                || previous.generation_epoch != self.generation_epoch
                || previous.sample_rate != self.sample_rate
                || previous.channels != self.channels
                || previous.format != self.format
                || self.seq != previous.seq.saturating_add(1)
                || previous.end_of_stream
            {
                return Err(MediaError::Stale);
            }
            let expected = previous
                .sample_start
                .saturating_add(previous.sample_count as u64);
            if self.sample_start != expected {
                return Err(MediaError::Stale);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(seq: u32, start: u64, count: u32) -> AudioBlock {
        AudioBlock {
            session_id: "s".into(),
            utterance_id: "utt".into(),
            generation_epoch: 7,
            seq,
            sample_rate: 16_000,
            channels: 1,
            format: SampleFormat::I16,
            sample_start: start,
            sample_count: count,
            end_of_stream: false,
        }
    }

    #[test]
    fn contiguous_blocks_pass_and_end_of_stream_is_final() {
        let first = block(0, 0, 160);
        assert_eq!(first.validate(None), Ok(()));
        assert_eq!(block(1, 160, 160).validate(Some(&first)), Ok(()));
        let mut eos = block(1, 160, 80);
        eos.end_of_stream = true;
        assert_eq!(eos.validate(Some(&first)), Ok(()));
        assert!(block(2, 240, 80).validate(Some(&eos)).is_err());
    }

    #[test]
    fn sequence_gap_or_format_change_is_rejected() {
        let first = block(0, 0, 160);
        let skipped = block(2, 160, 160);
        assert_eq!(skipped.validate(Some(&first)), Err(MediaError::Stale));
        let duplicate = block(0, 160, 160);
        assert_eq!(duplicate.validate(Some(&first)), Err(MediaError::Stale));
        let mut other_rate = block(1, 160, 160);
        other_rate.sample_rate = 48_000;
        assert_eq!(other_rate.validate(Some(&first)), Err(MediaError::Stale));
        let mut same_start = block(1, 0, 160);
        same_start.sample_rate = 16_000;
        assert_eq!(same_start.validate(Some(&first)), Err(MediaError::Stale));
    }

    #[test]
    fn empty_or_out_of_range_block_is_rejected() {
        let mut empty = block(0, 0, 0);
        empty.sample_count = 0;
        assert_eq!(empty.validate(None), Err(MediaError::Stale));
        let huge = block(0, 0, MAX_SAMPLE_COUNT + 1);
        assert_eq!(huge.validate(None), Err(MediaError::Stale));
        let mut bad_channels = block(0, 0, 10);
        bad_channels.channels = 3;
        assert_eq!(bad_channels.validate(None), Err(MediaError::Stale));
    }
}
