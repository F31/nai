//! Recording sink and continuity. Recording happens on the media clock's sample
//! position (monotonic), never on arrival time; gaps between utterances are
//! filled with deterministic silence so the recorded timeline stays continuous.
use crate::MediaError;
use crate::timeline::audio::{AudioBlock, SampleFormat};

pub trait RecordingSink {
    /// Append one validated block at an explicit media-clock sample position.
    fn push_audio(
        &mut self,
        block: &AudioBlock,
        sample_position: u64,
        samples: &[u8],
    ) -> Result<u64, MediaError>;
    fn close(&mut self);
}

/// In-memory mono i16 recorder used for deterministic tests.
#[derive(Debug, Default)]
pub struct MemoryRecorder {
    pub sample_rate: u32,
    pub samples: Vec<i16>,
    pub position: u64,
    pub gaps_written: u64,
    closed: bool,
}
impl MemoryRecorder {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            samples: Vec::new(),
            position: 0,
            gaps_written: 0,
            closed: false,
        }
    }
}
impl RecordingSink for MemoryRecorder {
    fn push_audio(
        &mut self,
        block: &AudioBlock,
        sample_position: u64,
        samples: &[u8],
    ) -> Result<u64, MediaError> {
        if self.closed || block.sample_rate != self.sample_rate || block.end_of_stream {
            return Err(MediaError::Stale);
        }
        if sample_position < self.position {
            return Err(MediaError::Expired);
        }
        let expected_frames = block.sample_count as usize * block.channels as usize;
        let frame_bytes = if block.format == SampleFormat::I16 {
            2
        } else {
            4
        };
        let bytes_needed = expected_frames * frame_bytes;
        if samples.len() < bytes_needed {
            return Err(MediaError::Stale);
        }
        let gap = (sample_position as usize).saturating_sub(self.position as usize);
        for _ in 0..gap {
            self.samples.push(0);
            self.gaps_written = self.gaps_written.saturating_add(1);
        }
        for frame in 0..expected_frames {
            let offset = frame * frame_bytes;
            let sample = if block.format == SampleFormat::I16 {
                i16::from_le_bytes([samples[offset], samples[offset + 1]])
            } else {
                crate::timeline::resample::f32_to_i16(f32::from_le_bytes(
                    samples[offset..offset + 4].try_into().unwrap(),
                ))
            };
            self.samples.push(sample);
        }
        self.position = sample_position.saturating_add(expected_frames as u64);
        Ok(expected_frames as u64)
    }
    fn close(&mut self) {
        self.closed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::audio::AudioBlock;

    fn block(start: u64, count: u32) -> AudioBlock {
        AudioBlock {
            session_id: "s".into(),
            utterance_id: "u".into(),
            generation_epoch: 7,
            seq: 0,
            sample_rate: 16_000,
            channels: 1,
            format: SampleFormat::I16,
            sample_start: start,
            sample_count: count,
            end_of_stream: false,
        }
    }
    fn frames(count: usize) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(count * 2);
        for frame in 0..count {
            bytes.extend_from_slice(&(frame as i16).to_le_bytes());
        }
        bytes
    }

    #[test]
    fn recording_lays_silence_between_utterances_and_keeps_monotonic_pts() {
        let mut recorder = MemoryRecorder::new(16_000);
        assert_eq!(
            recorder.push_audio(&block(0, 160), 0, &frames(160)),
            Ok(160)
        );
        // Second utterance starts 160 samples later -> silence bridges gap.
        assert_eq!(
            recorder.push_audio(&block(160, 160), 320, &frames(160)),
            Ok(160)
        );
        assert_eq!(recorder.gaps_written, 160);
        assert_eq!(recorder.samples.len(), 480);
        assert_eq!(recorder.samples[160], 0);
        assert_eq!(recorder.samples[161], 0);
        assert_eq!(recorder.position, 480);
        // Rewind is rejected.
        assert!(recorder.push_audio(&block(0, 10), 0, &frames(10)).is_err());
    }

    #[test]
    fn wrong_rate_or_after_close_is_rejected() {
        let mut recorder = MemoryRecorder::new(16_000);
        let mut bad = block(0, 10);
        bad.sample_rate = 48_000;
        assert!(recorder.push_audio(&bad, 0, &frames(10)).is_err());
        recorder.close();
        assert!(recorder.push_audio(&block(0, 10), 20, &frames(10)).is_err());
    }

    #[test]
    fn float_frames_are_converted_to_i16() {
        let mut recorder = MemoryRecorder::new(16_000);
        let block = AudioBlock {
            format: SampleFormat::F32,
            ..block(0, 2)
        };
        let bytes = [0f32.to_le_bytes(), 0.5f32.to_le_bytes()].concat();
        recorder.push_audio(&block, 0, &bytes).unwrap();
        assert!(recorder.samples[1] > 16000);
    }
}
