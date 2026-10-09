//! Portable media timeline primitives (V3.5 §21.3/§27, dh-media-core): validated
//! audio blocks, a monotonic media clock, bounded PCM buffering, explicit
//! cross-rate sample mapping, water-level commit, recording continuity and
//! actual-playback ACK completion. Pure logic, no device, network or hardware
//! ACK synthesis. Windows window/audio-tracks are a device milestone, not here.

pub mod ack;
pub mod audio;
pub mod clock;
pub mod record;
pub mod resample;
pub mod ring;
pub use crate::MediaError;

// Utterance timeline accumulates validated audio and video frames and only
// commits once both the approved first-audio and first-video water levels are
// reached; persistent lateness cancels instead of running sound without motion.
use crate::timeline::audio::AudioBlock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitState {
    NeedsAudio,
    NeedsVideo,
    Ready,
    Committed,
    Late,
}

/// First batch to consider "ready" for video (frame-count based).
pub const DEFAULT_VIDEO_WATER_FRAMES: u32 = 8;

pub struct UtteranceTimeline {
    utterance_id: String,
    model_rate: u32,
    output_rate: u32,
    audio_water_samples: u64,
    committed: bool,
    total_model_samples: u64,
    total_frames: u32,
    last_end_of_stream: bool,
    persistent_late_ms: u64,
}
impl UtteranceTimeline {
    pub fn new(
        utterance_id: &str,
        model_rate: u32,
        output_rate: u32,
        audio_water_ms: u32,
    ) -> Result<Self, MediaError> {
        if utterance_id.is_empty() || output_rate == 0 || model_rate == 0 {
            return Err(MediaError::Stale);
        }
        Ok(Self {
            utterance_id: utterance_id.into(),
            model_rate,
            output_rate,
            audio_water_samples: (audio_water_ms as u64 * output_rate as u64) / 1000,
            committed: false,
            total_model_samples: 0,
            total_frames: 0,
            last_end_of_stream: false,
            persistent_late_ms: 0,
        })
    }

    pub fn utterance_id(&self) -> &str {
        &self.utterance_id
    }
    pub fn committed(&self) -> bool {
        self.committed
    }
    pub fn total_model_samples(&self) -> u64 {
        self.total_model_samples
    }
    pub fn total_frames(&self) -> u32 {
        self.total_frames
    }

    pub fn push_audio(&mut self, block: &AudioBlock) -> Result<(), MediaError> {
        if block.utterance_id != self.utterance_id {
            return Err(MediaError::Stale);
        }
        if self.last_end_of_stream {
            return Err(MediaError::Stale);
        }
        self.total_model_samples = self
            .total_model_samples
            .saturating_add(block.sample_count as u64);
        self.last_end_of_stream = block.end_of_stream;
        Ok(())
    }

    pub fn push_video(&mut self, frame_seq: u32, expected: u32) -> Result<(), MediaError> {
        if expected == 0 || frame_seq == 0 || frame_seq > expected {
            return Err(MediaError::Stale);
        }
        let expected_next = self.total_frames.saturating_add(1);
        if frame_seq != expected_next {
            return Err(MediaError::Stale);
        }
        self.total_frames = frame_seq;
        Ok(())
    }

    pub fn state(&mut self) -> CommitState {
        if self.committed {
            return CommitState::Committed;
        }
        let audio_ready = self.output_samples() >= self.audio_water_samples;
        let video_ready = self.total_frames >= DEFAULT_VIDEO_WATER_FRAMES;
        match (audio_ready, video_ready) {
            (false, _) => CommitState::NeedsAudio,
            (true, false) => CommitState::NeedsVideo,
            (true, true) => CommitState::Ready,
        }
    }

    fn output_samples(&self) -> u64 {
        crate::timeline::clock::map_cross_rate(
            self.model_rate,
            self.total_model_samples,
            self.output_rate,
        )
    }

    /// Commit only when both water levels are met. Persistent lateness (no
    /// readiness within `persistent_late_ms`) returns Late instead of Commit.
    pub fn commit(&mut self, now_ms: u64, started_ms: u64) -> Result<CommitState, MediaError> {
        let state = self.state();
        if state == CommitState::Committed {
            return Ok(CommitState::Committed);
        }
        let since_start = now_ms.saturating_sub(started_ms);
        if self.persistent_late_ms > 0 && since_start >= self.persistent_late_ms {
            return Ok(CommitState::Late);
        }
        if state != CommitState::Ready {
            if since_start >= 3_000 {
                return Ok(CommitState::Late);
            }
            return Ok(state);
        }
        self.committed = true;
        Ok(CommitState::Committed)
    }

    pub fn mark_persistent_late(&mut self, persistent_late_ms: u64) {
        self.persistent_late_ms = persistent_late_ms;
    }

    /// Deterministic silence for an output-realized gap while keeping the
    /// media clock continuous.
    pub fn silence_between(&self, previous_output_end: u64, next_output_start: u64) -> u64 {
        next_output_start.saturating_sub(previous_output_end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::audio::{AudioBlock, SampleFormat};

    fn block(utterance: &str, seq: u32, start: u64, count: u32, eos: bool) -> AudioBlock {
        AudioBlock {
            session_id: "s".into(),
            utterance_id: utterance.into(),
            generation_epoch: 7,
            seq,
            sample_rate: 16_000,
            channels: 1,
            format: SampleFormat::I16,
            sample_start: start,
            sample_count: count,
            end_of_stream: eos,
        }
    }

    #[test]
    fn commit_requires_both_audio_and_video_water_levels() {
        let mut timeline = UtteranceTimeline::new("u", 16_000, 48_000, 200).unwrap();
        timeline.mark_persistent_late(60_000);
        assert_eq!(timeline.state(), CommitState::NeedsAudio);
        timeline.push_audio(&block("u", 0, 0, 3200, false)).unwrap(); // 100ms model = 300ms output
        assert_eq!(timeline.state(), CommitState::NeedsVideo);
        for frame in 1..=DEFAULT_VIDEO_WATER_FRAMES {
            timeline
                .push_video(frame, DEFAULT_VIDEO_WATER_FRAMES)
                .unwrap();
        }
        assert_eq!(timeline.state(), CommitState::Ready);
        assert_eq!(timeline.commit(0, 0).unwrap(), CommitState::Committed);
    }

    #[test]
    fn persistent_lateness_cancels_instead_of_committing() {
        let mut timeline = UtteranceTimeline::new("u", 16_000, 48_000, 200).unwrap();
        timeline.mark_persistent_late(500);
        timeline.push_audio(&block("u", 0, 0, 3200, false)).unwrap();
        for frame in 1..=DEFAULT_VIDEO_WATER_FRAMES {
            timeline
                .push_video(frame, DEFAULT_VIDEO_WATER_FRAMES)
                .unwrap();
        }
        assert_eq!(timeline.commit(3000, 0).unwrap(), CommitState::Late);
    }

    #[test]
    fn data_after_end_of_stream_is_rejected() {
        let mut timeline = UtteranceTimeline::new("u", 16_000, 48_000, 200).unwrap();
        timeline.push_audio(&block("u", 0, 0, 3200, true)).unwrap();
        assert!(
            timeline
                .push_audio(&block("u", 1, 3200, 3200, false))
                .is_err()
        );
    }
}
