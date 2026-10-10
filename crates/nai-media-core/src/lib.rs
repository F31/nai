//! Portable media authorization and media-basics timeline. No PCM/device
//! implementation or hardware ACK synthesis.
pub mod frame_slots;
pub mod playback;
pub mod timeline;
use nai_contracts::v1::{
    Approval, OutputAck, PerformanceCue, PlaybackLease, RevisionRef, SessionMode,
};
pub use playback::{
    LaneSpec, MAX_FRAMES, PlaybackLane, RehearsalSink, SinkReport, frame_boundary, frame_count,
    rehearse,
};
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
pub use timeline::ack::{AckTracker, PlaybackState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaError {
    Muted,
    Expired,
    Stale,
    Unauthorized,
    Capacity,
    InvalidAck,
}

/// Dedicated P0 signal: no business queue, model lock, network or database access.
#[derive(Clone, Default)]
pub struct EmergencyHandle(Arc<AtomicU64>);
impl EmergencyHandle {
    pub fn mute(&self) {
        let _ = self
            .0
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| v.checked_add(1));
    }
    fn revision(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

/// This must be reconstructed from current trusted guard/storage/permission state,
/// never from a candidate's self-reported approval or a provider response.
pub struct Preplay<'a> {
    pub cue: &'a PerformanceCue,
    pub approval: &'a Approval,
    pub facts: &'a [RevisionRef],
    pub mode: SessionMode,
    pub guard_passed: bool,
    pub permission_granted: bool,
    pub supervision_until_utc_ms: Option<u64>,
}

pub struct MediaGate {
    session: String,
    epoch: u64,
    emergency: EmergencyHandle,
    armed_revision: u64,
    lease: Option<PlaybackLease>,
    deadline: Option<Instant>,
    queue: VecDeque<String>,
    capacity: usize,
}
impl MediaGate {
    /// Diagnostic rehearsal frame admission only; no guard/approval bypass for
    /// production output. Device output remains unavailable in the drill.
    pub fn rehearsal_frame_permitted(
        &mut self,
        frame: &nai_contracts::v1::FrameMetadata,
        now: Instant,
    ) -> Result<(), MediaError> {
        self.poll(now)?;
        let lease = self.lease.as_ref().ok_or(MediaError::Muted)?;
        if lease.mode != SessionMode::Rehearsal as i32
            || frame.session_id != self.session
            || frame.generation_epoch != self.epoch
            || !lease.allowed_utterance_ids.contains(&frame.utterance_id)
        {
            return Err(MediaError::Unauthorized);
        }
        Ok(())
    }
    pub fn new(session: &str, capacity: usize) -> Self {
        Self::with_epoch_floor(session, capacity, 0)
    }
    /// Trusted supervisor supplies the surviving/recovered epoch floor when a
    /// keeper restarts. This sets no lease or playback permission.
    pub fn with_epoch_floor(session: &str, capacity: usize, epoch_floor: u64) -> Self {
        Self {
            session: session.into(),
            epoch: epoch_floor,
            emergency: EmergencyHandle::default(),
            armed_revision: 0,
            lease: None,
            deadline: None,
            queue: VecDeque::new(),
            capacity,
        }
    }
    pub fn emergency(&self) -> EmergencyHandle {
        self.emergency.clone()
    }
    pub fn queued(&self) -> usize {
        self.queue.len()
    }
    pub fn epoch_floor(&self) -> u64 {
        self.epoch
    }
    /// Trusted control path only. After a revoke/expiry, require a higher epoch.
    pub fn arm(
        &mut self,
        lease: PlaybackLease,
        utc_ms: u64,
        now: Instant,
    ) -> Result<(), MediaError> {
        let arm_revision = self.emergency.revision();
        if self.session.is_empty()
            || lease.session_id != self.session
            || lease.lease_id.is_empty()
            || lease.generation_epoch <= self.epoch
            || self.emergency.revision() == u64::MAX
        {
            return Err(MediaError::Stale);
        }
        if lease.allowed_utterance_ids.is_empty()
            || lease.allowed_utterance_ids.len() > self.capacity
            || lease.approval_refs.len() > self.capacity
            || lease.candidate_hashes.len() > self.capacity
            || lease.fact_versions.len() > 256
        {
            return Err(MediaError::Capacity);
        }
        let ttl = lease
            .expires_at_utc_ms
            .checked_sub(utc_ms)
            .filter(|v| *v > 0 && *v <= 600)
            .ok_or(MediaError::Expired)?;
        if SessionMode::try_from(lease.mode).is_err()
            || lease.mode == SessionMode::ModeUnspecified as i32
            || lease.policy_revision.is_empty()
        {
            return Err(MediaError::Unauthorized);
        }
        self.epoch = lease.generation_epoch;
        self.armed_revision = arm_revision;
        self.deadline = Some(now + Duration::from_millis(ttl));
        self.lease = Some(lease);
        self.queue.clear();
        Ok(())
    }
    pub fn poll(&mut self, now: Instant) -> Result<(), MediaError> {
        let error = if self.emergency.revision() != self.armed_revision {
            Some(MediaError::Muted)
        } else if self.deadline.is_none_or(|deadline| now >= deadline) {
            Some(MediaError::Expired)
        } else {
            None
        };
        if let Some(error) = error {
            self.lease = None;
            self.deadline = None;
            self.queue.clear();
            return Err(error);
        }
        Ok(())
    }
    pub fn renew(
        &mut self,
        lease: PlaybackLease,
        utc_ms: u64,
        now: Instant,
    ) -> Result<(), MediaError> {
        self.poll(now)?;
        let old = self.lease.as_ref().ok_or(MediaError::Muted)?;
        let mut same = lease.clone();
        same.expires_at_utc_ms = old.expires_at_utc_ms;
        if same != *old || lease.expires_at_utc_ms <= old.expires_at_utc_ms {
            return Err(MediaError::Stale);
        }
        let ttl = lease
            .expires_at_utc_ms
            .checked_sub(utc_ms)
            .filter(|v| *v > 0 && *v <= 600)
            .ok_or(MediaError::Expired)?;
        self.deadline = Some(now + Duration::from_millis(ttl));
        self.lease = Some(lease);
        Ok(())
    }
    fn validate(
        &mut self,
        proof: &Preplay<'_>,
        utc_ms: u64,
        now: Instant,
    ) -> Result<(), MediaError> {
        self.poll(now)?;
        let lease = self.lease.as_ref().ok_or(MediaError::Muted)?;
        let cue = proof.cue;
        let approval = proof.approval;
        if cue.session_id != self.session
            || cue.generation_epoch != self.epoch
            || approval.session_id != self.session
            || approval.generation_epoch != self.epoch
        {
            return Err(MediaError::Stale);
        }
        if nai_contracts::candidate_hash(cue) != cue.candidate_hash {
            return Err(MediaError::Unauthorized);
        }
        let facts_match = lease.fact_versions.len() == proof.facts.len()
            && lease.fact_versions.iter().all(|f| {
                !f.object_id.is_empty()
                    && !f.revision.is_empty()
                    && proof.facts.contains(f)
                    && cue.fact_versions.get(&f.object_id) == Some(&f.revision)
            });
        if !proof.guard_passed
            || !proof.permission_granted
            || proof.mode as i32 != lease.mode
            || (proof.mode == SessionMode::AutoSupervised
                && proof
                    .supervision_until_utc_ms
                    .is_none_or(|expiry| utc_ms >= expiry))
            || !lease.allowed_utterance_ids.contains(&cue.utterance_id)
            || cue.candidate_hash.is_empty()
            || cue.hash_algorithm != "sha256-canonical-v1"
            || lease.candidate_hashes.get(&cue.utterance_id) != Some(&cue.candidate_hash)
            || cue.expires_at_utc_ms <= utc_ms
            || lease.expires_at_utc_ms <= utc_ms
            || cue.policy_version != lease.policy_revision
            || approval.approval_id.is_empty()
            || cue.approval_ref != approval.approval_id
            || !lease.approval_refs.contains(&approval.approval_id)
            || approval.candidate_hash != cue.candidate_hash
            || approval.hash_algorithm != cue.hash_algorithm
            || approval.actor_id.is_empty()
            || approval.action_scope != "speak"
            || approval.revoked_at_utc_ms.is_some()
            || approval.expires_at_utc_ms <= utc_ms
            || approval.fact_versions != lease.fact_versions
            || !facts_match
            || cue.fact_versions.len() != proof.facts.len()
        {
            return Err(MediaError::Unauthorized);
        }
        Ok(())
    }
    pub fn enqueue(
        &mut self,
        proof: &Preplay<'_>,
        utc_ms: u64,
        now: Instant,
    ) -> Result<(), MediaError> {
        self.validate(proof, utc_ms, now)?;
        if self.queue.len() >= self.capacity {
            return Err(MediaError::Capacity);
        }
        self.queue.push_back(proof.cue.utterance_id.clone());
        Ok(())
    }
    /// Each consumption rechecks current proof; queued data is not authorization.
    pub fn consume(
        &mut self,
        proof: &Preplay<'_>,
        utc_ms: u64,
        now: Instant,
    ) -> Result<String, MediaError> {
        self.validate(proof, utc_ms, now)?;
        if self.queue.front() != Some(&proof.cue.utterance_id) {
            return Err(MediaError::Stale);
        }
        // Final P0 check immediately before crossing the consumption boundary.
        self.poll(now)?;
        self.queue.pop_front().ok_or(MediaError::Stale)
    }
}

/// Owned by an output invocation, not an automatic replay queue.
pub struct PlaybackProgress {
    output: String,
    utterance: String,
    expected_samples: u64,
    completed: bool,
    cancelled: bool,
}
impl PlaybackProgress {
    pub fn new(output: &str, utterance: &str, expected_samples: u64) -> Self {
        Self {
            output: output.into(),
            utterance: utterance.into(),
            expected_samples,
            completed: false,
            cancelled: false,
        }
    }
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }
    pub fn acknowledge(&mut self, ack: &OutputAck) -> Result<bool, MediaError> {
        if self.cancelled
            || self.output.is_empty()
            || self.utterance.is_empty()
            || ack.output_id != self.output
            || ack.utterance_id != self.utterance
            || ack.played_sample_count > self.expected_samples
        {
            return Err(MediaError::InvalidAck);
        }
        if ack.actual_local_playback_completed {
            if !ack.committed || ack.played_sample_count != self.expected_samples {
                return Err(MediaError::InvalidAck);
            }
            self.completed = true;
        }
        Ok(self.completed)
    }
}
