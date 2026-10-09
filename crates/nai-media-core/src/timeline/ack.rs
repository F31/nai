//! Playback ACK completion semantics (V3.5 §21.3). Only an actual local
//! playback completion ACK that matches the expected sample count marks a
//! task Completed; generation end, full download or acceptance never imply the
//! audience has heard it. In-flight partial ACKs track progress but do not
//! complete, and an unknown-state recovery must not double-play.
use crate::MediaError;
use crate::timeline::audio::AudioBlock;
use nai_contracts::v1::OutputAck;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Pending,
    Committed,
    Playing,
    Completed,
    Cancelled,
}

pub struct AckTracker {
    utterance_id: String,
    output_id: String,
    expected_samples: u64,
    acked_samples: u64,
    state: PlaybackState,
}
impl AckTracker {
    pub fn new(
        output: &str,
        block: &AudioBlock,
        expected_output_samples: u64,
    ) -> Result<Self, MediaError> {
        if output.is_empty() || block.utterance_id.is_empty() || expected_output_samples == 0 {
            return Err(MediaError::Stale);
        }
        Ok(Self {
            utterance_id: block.utterance_id.clone(),
            output_id: output.into(),
            expected_samples: expected_output_samples,
            acked_samples: 0,
            state: PlaybackState::Pending,
        })
    }

    pub fn utterance_id(&self) -> &str {
        &self.utterance_id
    }
    pub fn state(&self) -> PlaybackState {
        self.state
    }

    pub fn commit(&mut self) -> Result<(), MediaError> {
        match self.state {
            PlaybackState::Pending => {
                self.state = PlaybackState::Committed;
                Ok(())
            }
            PlaybackState::Committed | PlaybackState::Playing => Ok(()),
            PlaybackState::Completed | PlaybackState::Cancelled => Err(MediaError::InvalidAck),
        }
    }

    /// Apply one playback ACK. Partial progress while Playing is allowed; only
    /// committed + actual_local_playback_completed + matching sample count is a
    /// Completed state.
    pub fn acknowledge(&mut self, ack: &OutputAck) -> Result<PlaybackState, MediaError> {
        if self.state == PlaybackState::Cancelled {
            return Err(MediaError::InvalidAck);
        }
        if ack.output_id != self.output_id || ack.utterance_id != self.utterance_id {
            return Err(MediaError::InvalidAck);
        }
        if ack.played_sample_count > self.expected_samples
            || !ack.committed && ack.actual_local_playback_completed
        {
            return Err(MediaError::InvalidAck);
        }
        if !ack.committed {
            // Not committed yet: merely reporting transport state stays Pending.
            if ack.played_sample_count != 0 {
                return Err(MediaError::InvalidAck);
            }
            return Ok(self.state);
        }
        self.state = PlaybackState::Committed;
        if self.acked_samples < ack.played_sample_count {
            self.acked_samples = ack.played_sample_count;
        }
        if self.acked_samples > 0 {
            self.state = PlaybackState::Playing;
        }
        if ack.actual_local_playback_completed {
            if ack.played_sample_count != self.expected_samples {
                return Err(MediaError::InvalidAck);
            }
            self.acked_samples = self.expected_samples;
            self.state = PlaybackState::Completed;
        }
        Ok(self.state)
    }

    pub fn cancel(&mut self) {
        if self.state != PlaybackState::Completed {
            self.state = PlaybackState::Cancelled;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::audio::SampleFormat;
    use nai_contracts::v1::OutputAck;

    fn block() -> AudioBlock {
        AudioBlock {
            session_id: "s".into(),
            utterance_id: "u".into(),
            generation_epoch: 7,
            seq: 0,
            sample_rate: 48_000,
            channels: 1,
            format: SampleFormat::I16,
            sample_start: 0,
            sample_count: 48_000,
            end_of_stream: true,
        }
    }
    fn ack(committed: bool, completed: bool, played: u64) -> OutputAck {
        OutputAck {
            output_id: "out".into(),
            utterance_id: "u".into(),
            committed,
            actual_local_playback_completed: completed,
            played_sample_count: played,
            ..Default::default()
        }
    }

    #[test]
    fn only_actual_full_playback_ack_completes() {
        let mut tracker = AckTracker::new("out", &block(), 48_000).unwrap();
        assert_eq!(tracker.state(), PlaybackState::Pending);
        assert_eq!(
            tracker.acknowledge(&ack(false, false, 0)).unwrap(),
            PlaybackState::Pending
        );
        tracker.commit().unwrap();
        assert_eq!(
            tracker.acknowledge(&ack(true, false, 24_000)).unwrap(),
            PlaybackState::Playing
        );
        assert_eq!(
            tracker.acknowledge(&ack(false, true, 48_000)).unwrap_err(),
            MediaError::InvalidAck
        );
        assert_eq!(
            tracker.acknowledge(&ack(true, false, 48_000)).unwrap(),
            PlaybackState::Playing
        );
        assert_eq!(
            tracker.acknowledge(&ack(true, true, 48_000)).unwrap(),
            PlaybackState::Completed
        );
    }

    #[test]
    fn invalid_ack_or_post_completion_is_rejected() {
        let mut tracker = AckTracker::new("out", &block(), 48_000).unwrap();
        tracker.commit().unwrap();
        assert_eq!(
            tracker.acknowledge(&ack(true, true, 47_999)).unwrap_err(),
            MediaError::InvalidAck
        );
        let mut tracker = AckTracker::new("out", &block(), 48_000).unwrap();
        tracker.commit().unwrap();
        assert_eq!(
            tracker.acknowledge(&ack(true, true, 48_000)).unwrap(),
            PlaybackState::Completed
        );
        assert_eq!(
            tracker.acknowledge(&ack(false, true, 48_000)).unwrap_err(),
            MediaError::InvalidAck
        );
        let mut mismatch = ack(true, true, 48_000);
        mismatch.utterance_id = "other".into();
        assert_eq!(
            tracker.acknowledge(&mismatch).unwrap_err(),
            MediaError::InvalidAck
        );
    }

    #[test]
    fn cancel_never_completes_and_blocks_late_acks() {
        let mut tracker = AckTracker::new("out", &block(), 48_000).unwrap();
        tracker.commit().unwrap();
        tracker.cancel();
        assert_eq!(tracker.state(), PlaybackState::Cancelled);
        assert!(tracker.acknowledge(&ack(true, true, 48_000)).is_err());
    }
}
