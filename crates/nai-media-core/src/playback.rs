//! Rehearsal playback lane: one utterance's audio and frames driven to a
//! water-level commit, then judged only by an actual local playback
//! acknowledgement.
//!
//! The lane exists so "committed" and "played" cannot be confused. A commit is
//! a local water-level fact: enough audio and enough frames are buffered to
//! start. Completion is a remote fact reported by the output: the audience
//! actually heard the whole utterance. Only the second one is completion, and
//! the lane refuses an acknowledgement that claims playback before the lane
//! itself committed, because an output cannot have played what it was never
//! given.
//!
//! Frame boundaries are computed with the same integer arithmetic the software
//! renderer uses for its own frame spans, so the declared frame total and the
//! produced frames agree by construction instead of by convention.
use crate::MediaError;
use crate::timeline::UtteranceTimeline;
use crate::timeline::ack::{AckTracker, PlaybackState};
use crate::timeline::audio::AudioBlock;
use nai_contracts::v1::OutputAck;

/// Upper bound on declared frames per utterance. A longer utterance must be
/// chunked by the caller, not declared as one unbounded span.
pub const MAX_FRAMES: u32 = 4096;

/// Exclusive sample boundary of frame `seq` (1-based) at the given rate and
/// frame rate. Matches the renderer's own `sample_rate * fps_den / fps_num`
/// span arithmetic, including its integer truncation.
pub fn frame_boundary(seq: u32, sample_rate: u32, fps_num: u32, fps_den: u32) -> u64 {
    if fps_num == 0 {
        return 0;
    }
    (u128::from(seq) * u128::from(sample_rate) * u128::from(fps_den) / u128::from(fps_num)) as u64
}

/// How many frames an utterance of `samples` produces: the smallest frame count
/// whose exclusive boundary reaches the sample total.
pub fn frame_count(samples: u64, sample_rate: u32, fps_num: u32, fps_den: u32) -> u32 {
    if samples == 0 || sample_rate == 0 || fps_num == 0 || fps_den == 0 {
        return 0;
    }
    let mut frames = 0u32;
    while frame_boundary(frames, sample_rate, fps_num, fps_den) < samples {
        frames += 1;
        if frames >= MAX_FRAMES {
            return MAX_FRAMES;
        }
    }
    frames
}

/// What one utterance must satisfy before the lane can commit and be judged.
#[derive(Debug, Clone, Copy)]
pub struct LaneSpec<'a> {
    pub output_id: &'a str,
    pub utterance_id: &'a str,
    /// Rate the audio blocks are carried at.
    pub sample_rate: u32,
    /// First audio water level, in milliseconds. Both this and the default
    /// video water level must be reached before a commit.
    pub audio_water_ms: u32,
    /// The exact sample total an actual-playback acknowledgement must match. It
    /// is declared up front, never read back from whatever was produced.
    pub expected_samples: u64,
    /// The declared frame total, from [`frame_count`].
    pub expected_frames: u32,
}

/// One utterance's playback lane. Commit and completion are separate states and
/// are never inferred from each other.
pub struct PlaybackLane {
    output_id: String,
    utterance_id: String,
    expected_samples: u64,
    expected_frames: u32,
    timeline: UtteranceTimeline,
    tracker: AckTracker,
    committed: bool,
}

impl PlaybackLane {
    pub fn new(spec: &LaneSpec<'_>) -> Result<Self, MediaError> {
        if spec.sample_rate == 0
            || spec.expected_frames == 0
            || spec.expected_frames > MAX_FRAMES
            || spec.expected_samples == 0
        {
            return Err(MediaError::Stale);
        }
        Ok(Self {
            output_id: spec.output_id.to_string(),
            utterance_id: spec.utterance_id.to_string(),
            expected_samples: spec.expected_samples,
            expected_frames: spec.expected_frames,
            timeline: UtteranceTimeline::new(
                spec.utterance_id,
                spec.sample_rate,
                spec.sample_rate,
                spec.audio_water_ms,
            )?,
            tracker: AckTracker::for_utterance(
                spec.output_id,
                spec.utterance_id,
                spec.expected_samples,
            )?,
            committed: false,
        })
    }

    pub fn output_id(&self) -> &str {
        &self.output_id
    }
    pub fn utterance_id(&self) -> &str {
        &self.utterance_id
    }
    pub fn expected_samples(&self) -> u64 {
        self.expected_samples
    }
    pub fn expected_frames(&self) -> u32 {
        self.expected_frames
    }
    pub fn committed(&self) -> bool {
        self.committed
    }
    pub fn state(&self) -> PlaybackState {
        self.tracker.state()
    }
    pub fn completed(&self) -> bool {
        self.tracker.state() == PlaybackState::Completed
    }

    /// Accumulates one validated audio block. Blocks are the caller's; the lane
    /// only tracks their contribution to the water level.
    pub fn push_audio(&mut self, block: &AudioBlock) -> Result<(), MediaError> {
        if block.utterance_id != self.utterance_id {
            return Err(MediaError::Stale);
        }
        self.timeline.push_audio(block)
    }

    /// Accumulates one rendered frame into the video water level.
    pub fn push_frame(&mut self, frame_seq: u32) -> Result<(), MediaError> {
        self.timeline.push_video(frame_seq, self.expected_frames)
    }

    /// Commits when both water levels are met. A commit is a local buffering
    /// fact: it is not playback, and it never completes the lane.
    pub fn commit(&mut self, now_ms: u64, started_ms: u64) -> Result<PlaybackState, MediaError> {
        let state = self.timeline.commit(now_ms, started_ms)?;
        if state == crate::timeline::CommitState::Committed {
            self.committed = true;
            self.tracker.commit()?;
        }
        Ok(self.tracker.state())
    }

    /// Applies one acknowledgement. An acknowledgement that reports progress or
    /// completion before this lane committed is refused: the output cannot have
    /// played audio it was never handed.
    pub fn acknowledge(&mut self, ack: &OutputAck) -> Result<PlaybackState, MediaError> {
        let claims_progress =
            ack.committed || ack.actual_local_playback_completed || ack.played_sample_count != 0;
        if !self.committed && claims_progress {
            return Err(MediaError::InvalidAck);
        }
        self.tracker.acknowledge(ack)
    }

    /// Cancels the lane. A cancelled lane is never completed afterwards, so an
    /// unknown post-fault state cannot cause a repeated playback.
    pub fn cancel(&mut self) {
        self.tracker.cancel();
    }
}

/// The declared rehearsal playback outcome. It is a scenario input, not a fault
/// injection: every variant below is a *normal* ending that the lane must
/// survive honestly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RehearsalSink {
    /// Transport state, partial progress, then the actual completion.
    Complete,
    /// Transport state and partial progress only: never completes.
    Partial,
    /// Acknowledgements that contradict the lane: a wrong identity and a
    /// non-matching sample total that claims completion.
    Invalid,
    /// The lane is cancelled first; the completing acknowledgement that follows
    /// must be refused.
    Cancelled,
}

impl RehearsalSink {
    /// Rehearsal-only, like the stub generators: this stands in for a local
    /// output so the completion rule can be exercised without a device. It is
    /// not a device, produces no output and proves nothing about hardware.
    pub const OUTPUT_KIND: &'static str = "rehearsal-sink";

    pub fn parse(text: &str) -> Result<Self, String> {
        match text {
            "complete" => Ok(Self::Complete),
            "partial" => Ok(Self::Partial),
            "invalid" => Ok(Self::Invalid),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(format!("unknown playback scenario {other}")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Invalid => "invalid",
            Self::Cancelled => "cancelled",
        }
    }
}

/// What a rehearsal playback pass observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SinkReport {
    /// The state after each accepted acknowledgement.
    pub accepted: Vec<PlaybackState>,
    /// Acknowledgements the lane refused.
    pub rejected: usize,
    pub completed: bool,
}

/// Drives one lane through the declared scenario. Deterministic: the same lane
/// state and scenario always produce the same report. The caller must have
/// committed the lane first; a transport-state report before commit is legal but
/// carries no progress.
pub fn rehearse(sink: RehearsalSink, lane: &mut PlaybackLane) -> SinkReport {
    fn apply(
        lane: &mut PlaybackLane,
        ack: &OutputAck,
        accepted: &mut Vec<PlaybackState>,
        rejected: &mut usize,
    ) {
        match lane.acknowledge(ack) {
            Ok(state) => accepted.push(state),
            Err(_) => *rejected += 1,
        }
    }
    let output = lane.output_id().to_string();
    let utterance = lane.utterance_id().to_string();
    let expected = lane.expected_samples();
    let transport = OutputAck {
        output_id: output.clone(),
        utterance_id: utterance.clone(),
        ..Default::default()
    };
    let partial = OutputAck {
        output_id: output.clone(),
        utterance_id: utterance.clone(),
        committed: true,
        actual_local_playback_completed: false,
        played_sample_count: expected / 2,
        ..Default::default()
    };
    let complete = OutputAck {
        output_id: output.clone(),
        utterance_id: utterance.clone(),
        committed: true,
        actual_local_playback_completed: true,
        played_sample_count: expected,
        ..Default::default()
    };
    let mut accepted = Vec::new();
    let mut rejected = 0usize;
    match sink {
        RehearsalSink::Complete => {
            apply(lane, &transport, &mut accepted, &mut rejected);
            apply(lane, &partial, &mut accepted, &mut rejected);
            apply(lane, &complete, &mut accepted, &mut rejected);
        }
        RehearsalSink::Partial => {
            apply(lane, &transport, &mut accepted, &mut rejected);
            apply(lane, &partial, &mut accepted, &mut rejected);
        }
        RehearsalSink::Invalid => {
            apply(lane, &transport, &mut accepted, &mut rejected);
            let mut wrong_total = complete.clone();
            wrong_total.played_sample_count = expected.saturating_add(1);
            apply(lane, &wrong_total, &mut accepted, &mut rejected);
            let mut wrong_identity = complete.clone();
            wrong_identity.output_id = format!("{output}-other");
            apply(lane, &wrong_identity, &mut accepted, &mut rejected);
            let mut uncommitted_claim = complete.clone();
            uncommitted_claim.committed = false;
            apply(lane, &uncommitted_claim, &mut accepted, &mut rejected);
        }
        RehearsalSink::Cancelled => {
            lane.cancel();
            apply(lane, &complete, &mut accepted, &mut rejected);
        }
    }
    let completed = lane.completed();
    SinkReport {
        accepted,
        rejected,
        completed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::CommitState;
    use crate::timeline::audio::SampleFormat;

    const RATE: u32 = 8_000;
    const FPS_NUM: u32 = 25;
    const FPS_DEN: u32 = 1;
    const WINDOW: u64 = 320;
    const TOTAL: u64 = 3_200;

    fn spec<'a>(output: &'a str, utterance: &'a str) -> LaneSpec<'a> {
        LaneSpec {
            output_id: output,
            utterance_id: utterance,
            sample_rate: RATE,
            audio_water_ms: 40,
            expected_samples: TOTAL,
            expected_frames: frame_count(TOTAL, RATE, FPS_NUM, FPS_DEN),
        }
    }

    fn block(seq: u32, start: u64, count: u32, eos: bool) -> AudioBlock {
        AudioBlock {
            session_id: "s".into(),
            utterance_id: "utt".into(),
            generation_epoch: 7,
            seq,
            sample_rate: RATE,
            channels: 1,
            format: SampleFormat::F32,
            sample_start: start,
            sample_count: count,
            end_of_stream: eos,
        }
    }

    /// Audio and frames as the engine feeds them: one frame span per block.
    fn fill(lane: &mut PlaybackLane) {
        let frames = lane.expected_frames();
        let mut consumed = 0u64;
        for seq in 0..frames {
            let end = frame_boundary(seq + 1, RATE, FPS_NUM, FPS_DEN);
            let take = ((end - consumed).min(TOTAL - consumed)) as u32;
            let eos = consumed + u64::from(take) >= TOTAL;
            lane.push_audio(&block(seq, consumed, take, eos)).unwrap();
            lane.push_frame(seq + 1).unwrap();
            consumed += u64::from(take);
        }
        assert_eq!(consumed, TOTAL);
    }

    #[test]
    fn frame_total_is_declared_before_any_frame_exists() {
        assert_eq!(frame_count(TOTAL, RATE, FPS_NUM, FPS_DEN), 10);
        assert_eq!(frame_boundary(1, RATE, FPS_NUM, FPS_DEN), WINDOW);
        // A non-integral window must not drift: boundaries stay monotonic and
        // the declared count still covers the last partial frame.
        let odd = frame_count(1_000, RATE, 30, 1);
        assert_eq!(odd, 4);
        assert!(frame_boundary(odd, RATE, 30, 1) >= 1_000);
        assert!(frame_boundary(odd - 1, RATE, 30, 1) < 1_000);
    }

    #[test]
    fn audio_water_level_alone_does_not_commit() {
        let mut lane = PlaybackLane::new(&spec("out", "utt")).unwrap();
        lane.push_audio(&block(0, 0, WINDOW as u32, false)).unwrap();
        assert_eq!(lane.commit(0, 0).unwrap(), PlaybackState::Pending);
        assert!(!lane.committed());
        // Frames alone must not commit either: both water levels are required.
        let mut frames_only = PlaybackLane::new(&spec("out", "utt")).unwrap();
        for seq in 1..=10 {
            frames_only.push_frame(seq).unwrap();
        }
        assert_eq!(frames_only.commit(0, 0).unwrap(), PlaybackState::Pending);
    }

    #[test]
    fn a_commit_is_not_a_completion_and_partial_progress_never_completes() {
        let mut lane = PlaybackLane::new(&spec("out", "utt")).unwrap();
        fill(&mut lane);
        assert_eq!(lane.commit(0, 0).unwrap(), PlaybackState::Committed);
        assert!(lane.committed());
        assert!(!lane.completed());
        let report = rehearse(RehearsalSink::Partial, &mut lane);
        assert_eq!(report.rejected, 0);
        assert_eq!(
            report.accepted,
            vec![PlaybackState::Committed, PlaybackState::Playing]
        );
        assert!(!report.completed);
        assert_eq!(lane.state(), PlaybackState::Playing);
    }

    #[test]
    fn only_the_matching_complete_acknowledgement_completes() {
        let mut lane = PlaybackLane::new(&spec("out", "utt")).unwrap();
        fill(&mut lane);
        lane.commit(0, 0).unwrap();
        let report = rehearse(RehearsalSink::Complete, &mut lane);
        assert_eq!(report.rejected, 0);
        assert_eq!(
            report.accepted,
            vec![
                PlaybackState::Committed,
                PlaybackState::Playing,
                PlaybackState::Completed
            ]
        );
        assert!(report.completed);
        assert!(lane.completed());
    }

    #[test]
    fn contradictory_acknowledgements_are_refused_without_completing() {
        let mut lane = PlaybackLane::new(&spec("out", "utt")).unwrap();
        fill(&mut lane);
        lane.commit(0, 0).unwrap();
        let report = rehearse(RehearsalSink::Invalid, &mut lane);
        assert_eq!(report.rejected, 3, "a wrong total, identity and claim");
        assert_eq!(report.accepted, vec![PlaybackState::Committed]);
        assert!(!report.completed);
        assert!(!lane.completed());
    }

    #[test]
    fn a_cancelled_lane_is_never_completed() {
        let mut lane = PlaybackLane::new(&spec("out", "utt")).unwrap();
        fill(&mut lane);
        lane.commit(0, 0).unwrap();
        let report = rehearse(RehearsalSink::Cancelled, &mut lane);
        assert_eq!(report.rejected, 1);
        assert!(report.accepted.is_empty());
        assert!(!report.completed);
        assert_eq!(lane.state(), PlaybackState::Cancelled);
    }

    #[test]
    fn progress_before_a_commit_is_refused() {
        let mut lane = PlaybackLane::new(&spec("out", "utt")).unwrap();
        let ack = OutputAck {
            output_id: "out".into(),
            utterance_id: "utt".into(),
            committed: true,
            actual_local_playback_completed: true,
            played_sample_count: TOTAL,
            ..Default::default()
        };
        assert_eq!(lane.acknowledge(&ack).unwrap_err(), MediaError::InvalidAck);
        // A pure transport state report is accepted and keeps the lane Pending.
        let transport = OutputAck {
            output_id: "out".into(),
            utterance_id: "utt".into(),
            ..Default::default()
        };
        assert_eq!(
            lane.acknowledge(&transport).unwrap(),
            PlaybackState::Pending
        );
    }

    #[test]
    fn a_late_commit_after_water_levels_is_idempotent_and_late_is_reported() {
        let mut lane = PlaybackLane::new(&spec("out", "utt")).unwrap();
        fill(&mut lane);
        assert_eq!(lane.commit(0, 0).unwrap(), PlaybackState::Committed);
        assert_eq!(lane.commit(0, 0).unwrap(), PlaybackState::Committed);
        // Audio water level met but no frames: past the lateness window the
        // timeline reports Late and the lane must not commit.
        let mut starved = PlaybackLane::new(&spec("out", "utt")).unwrap();
        starved
            .push_audio(&block(0, 0, WINDOW as u32, false))
            .unwrap();
        let timeline_state = {
            let mut probe = UtteranceTimeline::new("utt", RATE, RATE, 40).unwrap();
            probe
                .push_audio(&block(0, 0, WINDOW as u32, false))
                .unwrap();
            probe.commit(5_000, 0).unwrap()
        };
        assert_eq!(timeline_state, CommitState::Late);
        assert_eq!(starved.commit(5_000, 0).unwrap(), PlaybackState::Pending);
        assert!(!starved.committed());
    }
}
