//! Bounded native-buffer assembly. No per-packet zero padding in continuous speech.
use nai_media_core::device_callback::{DeviceChunk, DrainFence};
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub(crate) struct Packet {
    pub chunk: DeviceChunk,
    pub copied: Arc<AtomicU64>,
    pub failure: Arc<AtomicU64>,
    pub drained: Arc<AtomicU64>,
    pub played: Arc<AtomicU64>,
}
struct PendingDrain {
    at: Instant,
    deadline: Instant,
    frames: u64,
    fence: DrainFence,
    drained: Arc<AtomicU64>,
    failure: Arc<AtomicU64>,
    played: Arc<AtomicU64>,
    buffer_frames: u64,
}
struct DeviceTail {
    end: u64,
    frames: u64,
    deadline: Instant,
    fence: DrainFence,
    played: Arc<AtomicU64>,
    failure: Arc<AtomicU64>,
}
pub(crate) struct Queue {
    receiver: mpsc::Receiver<Packet>,
    active: Option<Packet>,
    channels: usize,
    pending: Option<PendingDrain>,
    device_tails: VecDeque<DeviceTail>,
    staged: bool,
}
impl Queue {
    pub fn new(receiver: mpsc::Receiver<Packet>, channels: usize) -> Self {
        Self {
            receiver,
            active: None,
            channels,
            pending: None,
            device_tails: VecDeque::with_capacity(32),
            staged: false,
        }
    }
    /// Err is a backend timing/format fault; cue failures use that cue's signal.
    pub fn fill(
        &mut self,
        data: &mut [f32],
        now: Instant,
        latency: Option<Duration>,
    ) -> Result<(), u64> {
        self.staged = false;
        data.fill(0.0);
        if data.is_empty()
            || self.channels == 0
            || !data.len().is_multiple_of(self.channels)
            || data.len() / self.channels > 4800
        {
            self.pending = None;
            return Err(2);
        }
        // Only another callback may observe this tail. A wall-clock sleep in
        // the producer must never create a receipt when callbacks have stopped.
        if self.pending.is_some() {
            if latency.is_none_or(|v| v > Duration::from_millis(100)) {
                self.pending = None;
                return Err(1);
            }
            let pending = self.pending.as_ref().unwrap();
            if !pending.fence.still_valid(now)
                || now >= pending.deadline
                || pending.failure.load(Ordering::SeqCst) != 0
            {
                self.pending = None;
            } else if now >= pending.at {
                pending.drained.store(pending.frames, Ordering::SeqCst);
                self.pending = None;
            }
        }
        if self
            .active
            .as_ref()
            .is_some_and(|p| p.chunk.revoked() || p.failure.load(Ordering::SeqCst) != 0)
        {
            self.active = None;
            return Ok(());
        }
        if self
            .active
            .as_ref()
            .is_none_or(|p| p.chunk.remaining_samples() == 0)
        {
            self.active = self.receiver.try_recv().ok();
        }
        let Some(first) = self.active.as_ref() else {
            return Ok(());
        };
        let counter = first.copied.clone();
        let failure = first.failure.clone();
        let drained = first.drained.clone();
        let played = first.played.clone();
        let Some(latency) = latency.filter(|v| *v <= Duration::from_millis(100)) else {
            return Err(1);
        };
        let mut offset = 0;
        let mut earliest = None::<Instant>;
        // A bounded native buffer may span packet boundaries. Keep each 10ms
        // gate slice's predicted playback offset; commit counts only at the end.
        for _ in 0..8 {
            if offset == data.len() {
                break;
            }
            if self
                .active
                .as_ref()
                .is_none_or(|p| p.chunk.remaining_samples() == 0)
            {
                let Ok(next) = self.receiver.try_recv() else {
                    break;
                };
                if !Arc::ptr_eq(&next.copied, &counter) {
                    // The producer must finish one cue before submitting another.
                    next.failure.store(3, Ordering::SeqCst);
                    failure.store(3, Ordering::SeqCst);
                    data.fill(0.0);
                    return Ok(());
                }
                self.active = Some(next);
            }
            let Some(packet) = self.active.as_mut() else {
                break;
            };
            if packet.chunk.revoked() || packet.failure.load(Ordering::SeqCst) != 0 {
                data.fill(0.0);
                self.active = None;
                return Ok(());
            }
            earliest =
                Some(earliest.map_or(packet.chunk.deadline(), |v| v.min(packet.chunk.deadline())));
            let take = packet.chunk.remaining_samples().min(data.len() - offset);
            let position = Duration::from_nanos(
                ((offset / self.channels) as u64 * 1_000_000_000).div_ceil(48000),
            );
            let n = crate::device_smoke::fill_native_buffer(
                &mut packet.chunk,
                &mut data[offset..offset + take],
                48000,
                self.channels,
                now + position,
                Some(latency),
            );
            if packet.chunk.revoked() {
                data.fill(0.0);
                self.active = None;
                return Ok(());
            }
            if !matches!(n, Ok(v) if v == take) {
                data.fill(0.0);
                failure.store(3, Ordering::SeqCst);
                return Ok(());
            }
            offset += take;
        }
        let span =
            Duration::from_nanos(((offset / self.channels) as u64 * 1_000_000_000).div_ceil(48000));
        if self.active.as_ref().is_some_and(|p| p.chunk.revoked()) {
            data.fill(0.0);
            return Ok(());
        }
        if earliest.is_some_and(|v| now.max(Instant::now()) + latency + span >= v) {
            data.fill(0.0);
            failure.store(4, Ordering::SeqCst);
            return Ok(());
        }
        let frames = (offset / self.channels) as u64;
        let total = counter.fetch_add(frames, Ordering::SeqCst) + frames;
        if frames > 0 {
            let packet = self.active.as_ref().ok_or(3u64)?;
            self.pending = Some(PendingDrain {
                at: now.max(Instant::now()) + latency + span,
                deadline: earliest.ok_or(3u64)?,
                frames: total,
                fence: packet.chunk.drain_fence(),
                drained,
                failure,
                played,
                buffer_frames: frames,
            });
            self.staged = true;
        }
        Ok(())
    }

    /// Last denial check immediately before releasing the encoded native buffer.
    pub fn validate_device_write(&mut self, now: Instant) -> bool {
        if self.staged
            && self.pending.as_ref().is_none_or(|p| {
                now >= p.deadline
                    || !p.fence.still_valid(now)
                    || p.failure.load(Ordering::SeqCst) != 0
            })
        {
            self.staged = false;
            return false;
        }
        true
    }

    /// Must follow a successful backend write. Silence never rebinds a cue.
    pub fn commit_device_write(&mut self, start: u64) -> Result<(), u64> {
        if !std::mem::take(&mut self.staged) {
            return Ok(());
        }
        let p = self.pending.as_ref().ok_or(6u64)?;
        if self.device_tails.len() >= 32 {
            return Err(6);
        }
        self.device_tails.push_back(DeviceTail {
            end: start.checked_add(p.buffer_frames).ok_or(6u64)?,
            frames: p.frames,
            deadline: p.deadline,
            fence: p.fence.clone(),
            played: p.played.clone(),
            failure: p.failure.clone(),
        });
        Ok(())
    }

    /// Input is validated stream-relative IAudioClock frames, never elapsed time.
    pub fn observe_device(&mut self, position: u64, now: Instant) {
        // Preserve each in-flight buffer's deadline across renewed leases.
        while let Some(p) = self.device_tails.front() {
            if now >= p.deadline {
                p.failure.store(4, Ordering::SeqCst);
                self.device_tails.pop_front();
            } else if !p.fence.still_valid(now) || p.failure.load(Ordering::SeqCst) != 0 {
                self.device_tails.pop_front();
            } else if position >= p.end {
                p.played.store(p.frames, Ordering::SeqCst);
                self.device_tails.pop_front();
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nai_contracts::{candidate_hash, v1::*};
    use nai_media_core::{MediaGate, Preplay};
    fn packet(
        gate: &mut MediaGate,
        cue: &PerformanceCue,
        approval: &Approval,
        now: Instant,
        value: f32,
        count: Arc<AtomicU64>,
        failure: Arc<AtomicU64>,
    ) -> Packet {
        let proof = Preplay {
            cue,
            approval,
            facts: &[],
            mode: SessionMode::Assisted,
            guard_passed: true,
            permission_granted: true,
            supervision_until_utc_ms: None,
        };
        gate.enqueue(&proof, 1000, now).unwrap();
        Packet {
            chunk: gate
                .device_chunk(&proof, vec![value; 600], 48000, 1, 1000, now)
                .unwrap(),
            copied: count,
            failure,
            drained: Arc::new(AtomicU64::new(0)),
            played: Arc::new(AtomicU64::new(0)),
        }
    }
    fn setup(epoch: u64, now: Instant) -> (MediaGate, PerformanceCue, Approval) {
        let mut cue = PerformanceCue {
            session_id: "s".into(),
            utterance_id: format!("u-{epoch}"),
            generation_epoch: epoch,
            hash_algorithm: "sha256-canonical-v1".into(),
            approval_ref: "a".into(),
            policy_version: "p".into(),
            expires_at_utc_ms: 11000,
            ..Default::default()
        };
        cue.candidate_hash = candidate_hash(&cue);
        let approval = Approval {
            approval_id: "a".into(),
            session_id: "s".into(),
            generation_epoch: epoch,
            candidate_hash: cue.candidate_hash.clone(),
            hash_algorithm: cue.hash_algorithm.clone(),
            actor_id: "test".into(),
            action_scope: "speak".into(),
            expires_at_utc_ms: 11000,
            ..Default::default()
        };
        let mut gate = MediaGate::with_epoch_floor("s", 1, epoch - 1);
        gate.arm(
            PlaybackLease {
                lease_id: format!("l-{epoch}"),
                session_id: "s".into(),
                generation_epoch: epoch,
                allowed_utterance_ids: vec![cue.utterance_id.clone()],
                mode: SessionMode::Assisted as i32,
                approval_refs: vec!["a".into()],
                expires_at_utc_ms: 1600,
                policy_revision: "p".into(),
                candidate_hashes: [(cue.utterance_id.clone(), cue.candidate_hash.clone())].into(),
                ..Default::default()
            },
            1000,
            now,
        )
        .unwrap();
        (gate, cue, approval)
    }
    #[test]
    fn copying_and_sleeping_cannot_observe_tail_without_a_later_callback() {
        let now = Instant::now();
        let (mut gate, cue, approval) = setup(1, now);
        let count = Arc::new(AtomicU64::new(0));
        let failure = Arc::new(AtomicU64::new(0));
        let p = packet(&mut gate, &cue, &approval, now, 0.2, count.clone(), failure);
        let drained = p.drained.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        let mut q = Queue::new(rx, 1);
        tx.try_send(p).ok().unwrap();
        let mut data = [0.0; 600];
        q.fill(&mut data, now, Some(Duration::from_millis(20)))
            .unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 600);
        assert_eq!(drained.load(Ordering::SeqCst), 0);
        let tail = q.pending.as_ref().unwrap().at;
        q.fill(
            &mut data,
            now + Duration::from_millis(1),
            Some(Duration::from_millis(20)),
        )
        .unwrap();
        assert_eq!(drained.load(Ordering::SeqCst), 0);
        // No real-time sleep is used to manufacture a receipt: explicitly drive
        // a subsequent backend callback, past the recorded predicted horizon.
        q.fill(
            &mut data,
            tail + Duration::from_nanos(1),
            Some(Duration::from_millis(20)),
        )
        .unwrap();
        assert_eq!(drained.load(Ordering::SeqCst), 600);
        assert!(data.iter().all(|v| *v == 0.0));
        assert_eq!(count.load(Ordering::SeqCst), 600);
    }
    #[test]
    fn actual_device_tail_requires_write_and_exact_position_not_predicted_time() {
        let now = Instant::now();
        let (mut gate, cue, approval) = setup(1, now);
        let p = packet(
            &mut gate,
            &cue,
            &approval,
            now,
            0.2,
            Arc::new(AtomicU64::new(0)),
            Arc::new(AtomicU64::new(0)),
        );
        let played = p.played.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        let mut q = Queue::new(rx, 1);
        tx.try_send(p).ok().unwrap();
        q.fill(&mut [0.0; 900], now, Some(Duration::from_millis(20)))
            .unwrap();
        q.observe_device(10000, now);
        assert_eq!(played.load(Ordering::SeqCst), 0); // no backend release yet
        assert!(q.validate_device_write(now));
        q.commit_device_write(1000).unwrap(); // voice occupies 600 of 900 frames
        q.observe_device(1599, now);
        assert_eq!(played.load(Ordering::SeqCst), 0);
        q.observe_device(1600, now);
        assert_eq!(played.load(Ordering::SeqCst), 600);
        // Idle zero writes cannot inflate or rebind the previous cue.
        q.fill(&mut [0.0; 900], now, Some(Duration::from_millis(20)))
            .unwrap();
        q.commit_device_write(1900).unwrap();
        q.observe_device(2800, now);
        assert_eq!(played.load(Ordering::SeqCst), 600);
    }
    #[test]
    fn actual_device_tail_refuses_mute_epoch_expiry_and_error() {
        for scenario in ["mute", "epoch", "expiry", "error"] {
            let now = Instant::now();
            let (mut gate, cue, approval) = setup(1, now);
            let failure = Arc::new(AtomicU64::new(0));
            let p = packet(
                &mut gate,
                &cue,
                &approval,
                now,
                0.2,
                Arc::new(AtomicU64::new(0)),
                failure.clone(),
            );
            let played = p.played.clone();
            let (tx, rx) = mpsc::sync_channel(1);
            let mut q = Queue::new(rx, 1);
            tx.try_send(p).ok().unwrap();
            q.fill(&mut [0.0; 600], now, Some(Duration::from_millis(20)))
                .unwrap();
            assert!(q.validate_device_write(now));
            q.commit_device_write(1000).unwrap();
            if scenario == "mute" {
                gate.emergency().mute();
            }
            if scenario == "epoch" {
                let (_, next, approval) = setup(2, now);
                gate.arm(
                    PlaybackLease {
                        lease_id: "next-device-epoch".into(),
                        session_id: "s".into(),
                        generation_epoch: 2,
                        allowed_utterance_ids: vec![next.utterance_id.clone()],
                        approval_refs: vec![approval.approval_id],
                        candidate_hashes: [(next.utterance_id, next.candidate_hash)].into(),
                        mode: SessionMode::Assisted as i32,
                        policy_revision: "p".into(),
                        expires_at_utc_ms: 1600,
                        ..Default::default()
                    },
                    1000,
                    now,
                )
                .unwrap();
            }
            if scenario == "error" {
                failure.store(5, Ordering::SeqCst);
            }
            q.observe_device(
                1600,
                if scenario == "expiry" {
                    now + Duration::from_secs(1)
                } else {
                    now
                },
            );
            assert_eq!(played.load(Ordering::SeqCst), 0, "{scenario}");
        }
    }
    #[test]
    fn actual_device_inflight_queue_is_bounded_and_keeps_earlier_deadlines() {
        for bounded in [false, true] {
            let now = Instant::now();
            let (mut gate, cue, approval) = setup(1, now);
            let failure = Arc::new(AtomicU64::new(0));
            let p = packet(
                &mut gate,
                &cue,
                &approval,
                now,
                0.2,
                Arc::new(AtomicU64::new(0)),
                failure.clone(),
            );
            let played = p.played.clone();
            let (tx, rx) = mpsc::sync_channel(1);
            let mut q = Queue::new(rx, 1);
            tx.try_send(p).ok().unwrap();
            if bounded {
                for start in 0..32 {
                    q.fill(&mut [0.0; 1], now, Some(Duration::from_millis(20)))
                        .unwrap();
                    q.commit_device_write(start).unwrap();
                }
                q.fill(&mut [0.0; 1], now, Some(Duration::from_millis(20)))
                    .unwrap();
                assert_eq!(q.commit_device_write(32), Err(6));
                assert_eq!(q.device_tails.len(), 32);
            } else {
                q.fill(&mut [0.0; 300], now, Some(Duration::from_millis(20)))
                    .unwrap();
                q.pending.as_mut().unwrap().deadline = now + Duration::from_millis(100);
                q.commit_device_write(0).unwrap();
                q.fill(&mut [0.0; 300], now, Some(Duration::from_millis(20)))
                    .unwrap();
                q.pending.as_mut().unwrap().deadline = now + Duration::from_millis(500);
                q.commit_device_write(300).unwrap();
                q.observe_device(600, now + Duration::from_millis(200));
                assert_eq!(failure.load(Ordering::SeqCst), 4);
            }
            assert_eq!(played.load(Ordering::SeqCst), 0);
        }
    }
    #[test]
    fn revoke_after_encoding_prevents_voice_commit() {
        let now = Instant::now();
        let (mut gate, cue, approval) = setup(1, now);
        let p = packet(
            &mut gate,
            &cue,
            &approval,
            now,
            0.2,
            Arc::new(AtomicU64::new(0)),
            Arc::new(AtomicU64::new(0)),
        );
        let played = p.played.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        let mut q = Queue::new(rx, 1);
        tx.try_send(p).ok().unwrap();
        q.fill(&mut [0.0; 600], now, Some(Duration::from_millis(20)))
            .unwrap();
        gate.emergency().mute();
        assert!(!q.validate_device_write(now));
        q.commit_device_write(0).unwrap();
        q.observe_device(600, now);
        assert_eq!(played.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn stereo_receipt_counts_frames_not_interleaved_values() {
        let now = Instant::now();
        let (mut gate, cue, approval) = setup(1, now);
        let proof = Preplay {
            cue: &cue,
            approval: &approval,
            facts: &[],
            mode: SessionMode::Assisted,
            guard_passed: true,
            permission_granted: true,
            supervision_until_utc_ms: None,
        };
        gate.enqueue(&proof, 1000, now).unwrap();
        let copied = Arc::new(AtomicU64::new(0));
        let drained = Arc::new(AtomicU64::new(0));
        let p = Packet {
            chunk: gate
                .device_chunk(&proof, vec![0.2; 1200], 48000, 2, 1000, now)
                .unwrap(),
            copied: copied.clone(),
            drained: drained.clone(),
            played: Arc::new(AtomicU64::new(0)),
            failure: Arc::new(AtomicU64::new(0)),
        };
        let (tx, rx) = mpsc::sync_channel(1);
        let mut q = Queue::new(rx, 2);
        let mut data = [0.0; 1200];
        tx.try_send(p).ok().unwrap();
        q.fill(&mut data, now, Some(Duration::from_millis(20)))
            .unwrap();
        assert_eq!(copied.load(Ordering::SeqCst), 600);
        assert_eq!(drained.load(Ordering::SeqCst), 0);
        let tail = q.pending.as_ref().unwrap().at;
        q.fill(
            &mut data,
            tail + Duration::from_nanos(1),
            Some(Duration::from_millis(20)),
        )
        .unwrap();
        assert_eq!(drained.load(Ordering::SeqCst), 600);
    }
    #[test]
    fn tail_observation_refuses_revoke_expiry_error_and_missing_timing() {
        for scenario in ["mute", "epoch", "expiry", "error", "timing", "empty"] {
            let now = Instant::now();
            let (mut gate, cue, approval) = setup(1, now);
            let count = Arc::new(AtomicU64::new(0));
            let failure = Arc::new(AtomicU64::new(0));
            let p = packet(&mut gate, &cue, &approval, now, 0.2, count, failure.clone());
            let drained = p.drained.clone();
            let (tx, rx) = mpsc::sync_channel(1);
            let mut q = Queue::new(rx, 1);
            tx.try_send(p).ok().unwrap();
            let mut data = [0.0; 600];
            q.fill(&mut data, now, Some(Duration::from_millis(20)))
                .unwrap();
            let tail = q.pending.as_ref().unwrap().at + Duration::from_nanos(1);
            let deadline = q.pending.as_ref().unwrap().deadline;
            match scenario {
                "mute" => gate.emergency().mute(),
                "epoch" => {
                    let (_, next, approval) = setup(2, now);
                    gate.arm(
                        PlaybackLease {
                            lease_id: "next-epoch".into(),
                            session_id: "s".into(),
                            generation_epoch: 2,
                            allowed_utterance_ids: vec![next.utterance_id.clone()],
                            approval_refs: vec![approval.approval_id],
                            candidate_hashes: [(next.utterance_id, next.candidate_hash)].into(),
                            mode: SessionMode::Assisted as i32,
                            policy_revision: "p".into(),
                            expires_at_utc_ms: 1600,
                            ..Default::default()
                        },
                        1000,
                        now,
                    )
                    .unwrap();
                }
                "error" => {
                    failure.store(5, Ordering::SeqCst);
                }
                _ => {}
            }
            let result = q.fill(
                if scenario == "empty" {
                    &mut []
                } else {
                    &mut data
                },
                if scenario == "expiry" { deadline } else { tail },
                if scenario == "timing" {
                    None
                } else {
                    Some(Duration::from_millis(20))
                },
            );
            if scenario == "timing" {
                assert_eq!(result, Err(1));
            } else if scenario == "empty" {
                assert_eq!(result, Err(2));
            } else {
                result.unwrap();
            }
            assert_eq!(drained.load(Ordering::SeqCst), 0, "{scenario}");
        }
    }
    #[test]
    fn cancelled_cue_cannot_donate_its_tail_receipt_to_the_next_epoch() {
        let now = Instant::now();
        let (mut gate, cue, approval) = setup(1, now);
        let old = packet(
            &mut gate,
            &cue,
            &approval,
            now,
            0.2,
            Arc::new(AtomicU64::new(0)),
            Arc::new(AtomicU64::new(0)),
        );
        let old_drained = old.drained.clone();
        let old_played = old.played.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        let mut q = Queue::new(rx, 1);
        let mut data = [0.0; 600];
        tx.try_send(old).ok().unwrap();
        q.fill(&mut data, now, Some(Duration::from_millis(20)))
            .unwrap();
        q.commit_device_write(0).unwrap();
        gate.emergency().mute();
        q.observe_device(600, now);
        assert_eq!(old_played.load(Ordering::SeqCst), 0);
        let (mut gate2, cue2, approval2) = setup(2, now);
        let new = packet(
            &mut gate2,
            &cue2,
            &approval2,
            now,
            0.4,
            Arc::new(AtomicU64::new(0)),
            Arc::new(AtomicU64::new(0)),
        );
        let new_drained = new.drained.clone();
        let new_played = new.played.clone();
        tx.try_send(new).ok().unwrap();
        q.fill(
            &mut data,
            now + Duration::from_millis(1),
            Some(Duration::from_millis(20)),
        )
        .unwrap();
        q.fill(
            &mut data,
            now + Duration::from_millis(2),
            Some(Duration::from_millis(20)),
        )
        .unwrap();
        let tail = q.pending.as_ref().unwrap().at;
        q.commit_device_write(1000).unwrap();
        q.observe_device(1599, now);
        assert_eq!(new_played.load(Ordering::SeqCst), 0);
        q.observe_device(1600, now);
        assert_eq!(new_played.load(Ordering::SeqCst), 600);
        assert_eq!(old_played.load(Ordering::SeqCst), 0);
        assert_eq!(new_drained.load(Ordering::SeqCst), 0);
        q.fill(
            &mut data,
            tail + Duration::from_nanos(1),
            Some(Duration::from_millis(20)),
        )
        .unwrap();
        assert_eq!(old_drained.load(Ordering::SeqCst), 0);
        assert_eq!(new_drained.load(Ordering::SeqCst), 600);
    }
    #[test]
    fn packet_boundary_has_no_inserted_silence_and_idle_does_not_advance_cue() {
        let now = Instant::now();
        let (mut gate, cue, approval) = setup(1, now);
        let count = Arc::new(AtomicU64::new(0));
        let failure = Arc::new(AtomicU64::new(0));
        let (tx, rx) = mpsc::sync_channel(1);
        let mut queue = Queue::new(rx, 1);
        tx.try_send(packet(
            &mut gate,
            &cue,
            &approval,
            now,
            0.2,
            count.clone(),
            failure.clone(),
        ))
        .ok()
        .unwrap();
        let mut first = vec![0.0; 300];
        queue
            .fill(&mut first, now, Some(Duration::from_millis(20)))
            .unwrap();
        assert!(first.iter().all(|v| *v == 0.2));
        tx.try_send(packet(
            &mut gate,
            &cue,
            &approval,
            now,
            0.4,
            count.clone(),
            failure.clone(),
        ))
        .ok()
        .unwrap();
        let mut output = vec![0.0; 900];
        queue
            .fill(&mut output, now, Some(Duration::from_millis(20)))
            .unwrap();
        assert!(output[..300].iter().all(|v| *v == 0.2));
        assert!(output[300..].iter().all(|v| *v == 0.4));
        assert_eq!(count.load(Ordering::SeqCst), 1200);
        assert_eq!(failure.load(Ordering::SeqCst), 0);
        queue
            .fill(&mut output, now, Some(Duration::from_millis(20)))
            .unwrap();
        assert!(output.iter().all(|v| *v == 0.0));
        assert_eq!(count.load(Ordering::SeqCst), 1200);
        tx.try_send(packet(
            &mut gate,
            &cue,
            &approval,
            now,
            0.5,
            count.clone(),
            failure.clone(),
        ))
        .ok()
        .unwrap();
        queue
            .fill(&mut output[..100], now, Some(Duration::from_millis(20)))
            .unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 1300);
        gate.emergency().mute();
        let (mut second, cue2, approval2) = setup(2, now);
        let count2 = Arc::new(AtomicU64::new(0));
        let failure2 = Arc::new(AtomicU64::new(0));
        tx.try_send(packet(
            &mut second,
            &cue2,
            &approval2,
            now,
            0.7,
            count2.clone(),
            failure2.clone(),
        ))
        .ok()
        .unwrap();
        queue
            .fill(&mut output, now, Some(Duration::from_millis(20)))
            .unwrap();
        assert!(output.iter().all(|v| *v == 0.0));
        queue
            .fill(&mut output[..600], now, Some(Duration::from_millis(20)))
            .unwrap();
        assert!(output[..600].iter().all(|v| *v == 0.7));
        assert_eq!(count2.load(Ordering::SeqCst), 600);
        assert_eq!(count.load(Ordering::SeqCst), 1300);
    }
}
