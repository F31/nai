//! Bounded sentence identities on one absolute source timeline. No device I/O.
use nai_contracts::v561_continuous_audio::{
    SegmentBegin, SegmentIdentity, SegmentProgress, SegmentSeal,
};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;

pub const MAX_SCRIPT_SEGMENTS: u32 = 128;
pub const MAX_SCRIPT_SAMPLES: u64 = 90 * 24000;

#[derive(Debug, PartialEq, Eq)]
pub struct InvalidTimeline;
struct Segment {
    identity: SegmentIdentity,
    maximum: u64,
    received: u64,
    hash: Sha256,
    sealed: bool,
    chunk: u64,
}
pub struct Timeline {
    epoch: u64,
    maximum: u64,
    sentences: u32,
    next: u32,
    generation: u64,
    endpoint: u64,
    submitted: u64,
    played: u64,
    revoked: bool,
    segments: VecDeque<Segment>,
}
impl Timeline {
    pub fn new(epoch: u64, maximum: u64, sentences: u32) -> Result<Self, InvalidTimeline> {
        // Matches current bounded device transport, not an unlimited script.
        if epoch == 0
            || !(1..=MAX_SCRIPT_SAMPLES).contains(&maximum)
            || !(1..=MAX_SCRIPT_SEGMENTS).contains(&sentences)
        {
            return Err(InvalidTimeline);
        }
        Ok(Self {
            epoch,
            maximum,
            sentences,
            next: 1,
            generation: 0,
            endpoint: 0,
            submitted: 0,
            played: 0,
            revoked: false,
            segments: VecDeque::with_capacity(2),
        })
    }
    pub fn revoke(&mut self) {
        self.revoked = true;
        self.segments.clear();
    }
    fn apply<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, InvalidTimeline>,
    ) -> Result<T, InvalidTimeline> {
        if self.revoked {
            return Err(InvalidTimeline);
        }
        let result = f(self);
        if result.is_err() {
            self.revoke();
        }
        result
    }
    pub fn begin(&mut self, begin: &SegmentBegin) -> Result<(), InvalidTimeline> {
        self.apply(|this| {
            let id = begin.identity.as_ref().ok_or(InvalidTimeline)?;
            if this.segments.len() == 2
                || id.script_epoch != this.epoch
                || id.sentence_id != this.next
                || id.sentence_id > this.sentences
                || id.generation_epoch <= this.generation
                || id.source_start_sample != this.endpoint
                || !(1..=144000).contains(&begin.max_samples)
                || this.endpoint >= this.maximum
                || this.segments.back().is_some_and(|s| !s.sealed)
            {
                return Err(InvalidTimeline);
            }
            this.segments.push_back(Segment {
                identity: *id,
                maximum: begin.max_samples.min(this.maximum - this.endpoint),
                received: 0,
                hash: Sha256::new(),
                sealed: false,
                chunk: 0,
            });
            this.next += 1;
            this.generation = id.generation_epoch;
            Ok(())
        })
    }
    pub fn push(
        &mut self,
        id: &SegmentIdentity,
        sequence: u64,
        pcm: &[u8],
    ) -> Result<(), InvalidTimeline> {
        self.apply(|this| {
            let segment = this.segments.back_mut().ok_or(InvalidTimeline)?;
            if segment.identity != *id
                || segment.sealed
                || sequence != segment.chunk.checked_add(1).ok_or(InvalidTimeline)?
                || pcm.is_empty()
                || pcm.len() > 9600
                || !pcm.len().is_multiple_of(2)
                || pcm.len() as u64 / 2 > segment.maximum - segment.received
            {
                return Err(InvalidTimeline);
            }
            segment.hash.update(pcm);
            segment.received += pcm.len() as u64 / 2;
            segment.chunk = sequence;
            Ok(())
        })
    }
    pub fn seal(&mut self, seal: &SegmentSeal) -> Result<(), InvalidTimeline> {
        self.apply(|this| {
            let segment = this.segments.back_mut().ok_or(InvalidTimeline)?;
            if seal.identity.as_ref() != Some(&segment.identity)
                || segment.sealed
                || seal.samples == 0
                || seal.samples != segment.received
                || seal.sha256.len() != 32
                || segment.hash.clone().finalize().as_slice() != seal.sha256
            {
                return Err(InvalidTimeline);
            }
            this.endpoint = segment
                .identity
                .source_start_sample
                .checked_add(seal.samples)
                .ok_or(InvalidTimeline)?;
            if this.endpoint > this.maximum {
                return Err(InvalidTimeline);
            }
            segment.sealed = true;
            Ok(())
        })
    }
    /// Caller must first authenticate and validate device clock freshness/generation.
    /// Values are absolute source samples, excluding any silent device gaps.
    pub fn observe(
        &mut self,
        submitted: u64,
        played: u64,
    ) -> Result<Vec<SegmentProgress>, InvalidTimeline> {
        self.apply(|this| {
            let received = this.segments.back().map_or(this.endpoint, |s| {
                s.identity.source_start_sample + s.received
            });
            if submitted < this.submitted
                || played < this.played
                || played > submitted
                || submitted > received
            {
                return Err(InvalidTimeline);
            }
            this.submitted = submitted;
            this.played = played;
            Ok(this
                .segments
                .iter()
                .map(|s| SegmentProgress {
                    identity: Some(s.identity),
                    received_samples: s.received,
                    submitted_samples: submitted
                        .saturating_sub(s.identity.source_start_sample)
                        .min(s.received),
                    played_samples: played
                        .saturating_sub(s.identity.source_start_sample)
                        .min(s.received),
                    sealed: s.sealed,
                    completed: s.sealed && played >= s.identity.source_start_sample + s.received,
                })
                .collect())
        })
    }
    /// Completion observations must be consumed before freeing the bounded slot.
    pub fn retire_completed(&mut self) -> Result<u32, InvalidTimeline> {
        self.apply(|this| {
            let mut count = 0;
            while this.segments.front().is_some_and(|s| {
                s.sealed && this.played >= s.identity.source_start_sample + s.received
            }) {
                this.segments.pop_front();
                count += 1;
            }
            Ok(count)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(n: u32, start: u64) -> SegmentIdentity {
        SegmentIdentity {
            script_epoch: 7,
            sentence_id: n,
            generation_epoch: n as u64,
            source_start_sample: start,
        }
    }
    fn source(t: &mut Timeline, n: u32, start: u64) {
        let id = id(n, start);
        t.begin(&SegmentBegin {
            identity: Some(id),
            max_samples: 4,
        })
        .unwrap();
        t.push(&id, 1, &[1, 0, 2, 0]).unwrap();
        t.seal(&SegmentSeal {
            identity: Some(id),
            samples: 2,
            sha256: Sha256::digest([1, 0, 2, 0]).to_vec(),
        })
        .unwrap();
    }
    #[test]
    fn actual_device_crossing_not_seal_completes_sentence() {
        let mut t = Timeline::new(7, 10, 3).unwrap();
        source(&mut t, 1, 0);
        source(&mut t, 2, 2);
        let p = t.observe(4, 1).unwrap();
        assert!(p.iter().all(|p| p.sealed && !p.completed));
        let p = t.observe(4, 3).unwrap();
        assert!(p[0].completed);
        assert!(!p[1].completed);
        assert_eq!(p[1].played_samples, 1);
        assert_eq!(t.retire_completed().unwrap(), 1);
        source(&mut t, 3, 4);
        let p = t.observe(6, 6).unwrap();
        assert!(p.iter().all(|p| p.completed));
    }
    #[test]
    fn third_outstanding_segment_is_terminal_not_unbounded() {
        let mut t = Timeline::new(7, 10, 3).unwrap();
        source(&mut t, 1, 0);
        source(&mut t, 2, 2);
        assert!(
            t.begin(&SegmentBegin {
                identity: Some(id(3, 4)),
                max_samples: 2
            })
            .is_err()
        );
        assert!(t.observe(4, 4).is_err());
    }
    #[test]
    fn wrong_hash_and_old_identity_revoke_all_future_progress() {
        let mut t = Timeline::new(7, 10, 2).unwrap();
        source(&mut t, 1, 0);
        t.begin(&SegmentBegin {
            identity: Some(id(2, 2)),
            max_samples: 2,
        })
        .unwrap();
        assert!(t.push(&id(1, 0), 1, &[1, 0]).is_err());
        assert!(t.observe(2, 2).is_err());
        let mut t = Timeline::new(7, 10, 1).unwrap();
        t.begin(&SegmentBegin {
            identity: Some(id(1, 0)),
            max_samples: 2,
        })
        .unwrap();
        t.push(&id(1, 0), 1, &[1, 0]).unwrap();
        assert!(
            t.seal(&SegmentSeal {
                identity: Some(id(1, 0)),
                samples: 1,
                sha256: vec![0; 32]
            })
            .is_err()
        );
        assert!(t.observe(1, 1).is_err());
    }
    #[test]
    fn replay_generation_and_unverified_boundary_cannot_admit_future() {
        let mut t = Timeline::new(7, 10, 2).unwrap();
        t.begin(&SegmentBegin {
            identity: Some(id(1, 0)),
            max_samples: 2,
        })
        .unwrap();
        t.push(&id(1, 0), 1, &[1, 0]).unwrap();
        assert!(
            t.begin(&SegmentBegin {
                identity: Some(id(2, 1)),
                max_samples: 2
            })
            .is_err()
        );
        for (epoch, sentence, generation) in [(8, 2, 2), (7, 1, 2), (7, 2, 1)] {
            let mut t = Timeline::new(7, 10, 2).unwrap();
            source(&mut t, 1, 0);
            let mut invalid = id(sentence, 2);
            invalid.script_epoch = epoch;
            invalid.generation_epoch = generation;
            assert!(
                t.begin(&SegmentBegin {
                    identity: Some(invalid),
                    max_samples: 2
                })
                .is_err()
            );
            assert!(t.observe(2, 2).is_err());
        }
    }
    #[test]
    fn sample_cap_chunk_replay_and_duplicate_seal_are_terminal() {
        let mut t = Timeline::new(7, 1, 1).unwrap();
        t.begin(&SegmentBegin {
            identity: Some(id(1, 0)),
            max_samples: 2,
        })
        .unwrap();
        assert!(t.push(&id(1, 0), 1, &[1, 0, 2, 0]).is_err());
        let mut t = Timeline::new(7, 10, 1).unwrap();
        t.begin(&SegmentBegin {
            identity: Some(id(1, 0)),
            max_samples: 2,
        })
        .unwrap();
        t.push(&id(1, 0), 1, &[1, 0]).unwrap();
        assert!(t.push(&id(1, 0), 1, &[1, 0]).is_err());
        let mut t = Timeline::new(7, 10, 1).unwrap();
        source(&mut t, 1, 0);
        assert!(
            t.seal(&SegmentSeal {
                identity: Some(id(1, 0)),
                samples: 2,
                sha256: Sha256::digest([1, 0, 2, 0]).to_vec()
            })
            .is_err()
        );
        assert!(t.observe(2, 2).is_err());
    }
    #[test]
    fn clock_regression_cancel_and_wrong_start_are_terminal() {
        let mut t = Timeline::new(7, 10, 2).unwrap();
        source(&mut t, 1, 0);
        t.observe(2, 1).unwrap();
        assert!(t.observe(2, 0).is_err());
        let mut t = Timeline::new(7, 10, 2).unwrap();
        source(&mut t, 1, 0);
        t.revoke();
        assert!(t.observe(2, 2).is_err());
        let mut t = Timeline::new(7, 10, 2).unwrap();
        source(&mut t, 1, 0);
        assert!(
            t.begin(&SegmentBegin {
                identity: Some(id(2, 3)),
                max_samples: 2
            })
            .is_err()
        );
    }
}
