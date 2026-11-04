//! Callback-side software boundary; copied samples are never a device ACK.
use super::*;

pub struct DeviceChunk {
    pcm: Vec<f32>,
    channels: usize,
    sample_rate: u32,
    cursor: usize,
    emergency: EmergencyHandle,
    revision: u64,
    epoch_signal: Arc<AtomicU64>,
    epoch: u64,
    deadline: Instant,
    stopped: bool,
}

/// Denial-only snapshot for a later callback observation. This cannot enqueue,
/// copy audio or authorize output, and is never a playback acknowledgement.
#[derive(Clone)]
pub struct DrainFence {
    emergency: EmergencyHandle,
    revision: u64,
    epoch_signal: Arc<AtomicU64>,
    epoch: u64,
    deadline: Instant,
    eligible: bool,
}
impl DrainFence {
    pub fn still_valid(&self, now: Instant) -> bool {
        self.eligible
            && now < self.deadline
            && self.emergency.revision() == self.revision
            && self.epoch_signal.load(Ordering::SeqCst) == self.epoch
    }
}

impl MediaGate {
    /// Admit one queued chunk. Callers must supply current trusted proof;
    /// neither provider PCM nor an earlier approval snapshot grants permission.
    pub fn device_chunk(
        &mut self,
        proof: &Preplay<'_>,
        pcm: Vec<f32>,
        sample_rate: u32,
        channels: usize,
        utc_ms: u64,
        now: Instant,
    ) -> Result<DeviceChunk, MediaError> {
        if !(8_000..=192_000).contains(&sample_rate)
            || !(1..=2).contains(&channels)
            || pcm.is_empty()
            || !pcm.len().is_multiple_of(channels)
            || pcm.len() / channels > sample_rate as usize * 600 / 1000
            || pcm.iter().any(|v| !v.is_finite() || v.abs() > 1.0)
        {
            return Err(MediaError::Capacity);
        }
        self.validate(proof, utc_ms, now)?;
        let mut expiry = proof
            .cue
            .expires_at_utc_ms
            .min(proof.approval.expires_at_utc_ms);
        if proof.mode == SessionMode::AutoSupervised {
            expiry = expiry.min(
                proof
                    .supervision_until_utc_ms
                    .ok_or(MediaError::Unauthorized)?,
            );
        }
        let deadline = self
            .deadline
            .ok_or(MediaError::Expired)?
            .min(now + Duration::from_millis(expiry.saturating_sub(utc_ms)));
        self.consume(proof, utc_ms, now)?;
        Ok(DeviceChunk {
            pcm,
            channels,
            sample_rate,
            cursor: 0,
            emergency: self.emergency.clone(),
            revision: self.armed_revision,
            epoch_signal: self.callback_epoch.clone(),
            epoch: self.epoch,
            deadline,
            stopped: false,
        })
    }
}

impl DeviceChunk {
    pub fn drain_fence(&self) -> DrainFence {
        DrainFence {
            emergency: self.emergency.clone(),
            revision: self.revision,
            epoch_signal: self.epoch_signal.clone(),
            epoch: self.epoch,
            deadline: self.deadline,
            eligible: !self.stopped,
        }
    }
    pub fn remaining_samples(&self) -> usize {
        self.pcm.len() - self.cursor
    }
    /// Read-only scheduling limit, not an output grant.
    pub fn deadline(&self) -> Instant {
        self.deadline
    }
    /// A denial-only check for adapters assembling several bounded slices into
    /// one native buffer. False is not authorization; `fill` still checks time.
    pub fn revoked(&self) -> bool {
        self.emergency.revision() != self.revision
            || self.epoch_signal.load(Ordering::SeqCst) != self.epoch
    }
    /// Fill at most 10ms of interleaved output. No allocation, locks, IO or
    /// completion ACK. `now` is the backend's current monotonic callback time.
    /// The backend must also account for hardware buffering before submission.
    pub fn fill(&mut self, output: &mut [f32], now: Instant) -> usize {
        output.fill(0.0);
        let frames = output.len() / self.channels;
        if self.stopped
            || output.is_empty()
            || !output.len().is_multiple_of(self.channels)
            || frames > self.sample_rate as usize / 100
        {
            self.stopped = true;
            return 0;
        }
        let span = Duration::from_nanos(
            (frames as u64 * 1_000_000_000).div_ceil(u64::from(self.sample_rate)),
        );
        if now.checked_add(span).is_none_or(|end| end >= self.deadline) {
            self.stopped = true;
            return 0;
        }
        let mut copied = 0;
        for frame in output.chunks_exact_mut(self.channels) {
            if self.revoked() {
                // Also discard samples copied earlier in this invocation.
                output.fill(0.0);
                self.stopped = true;
                return 0;
            }
            if self.cursor == self.pcm.len() {
                break;
            }
            frame.copy_from_slice(&self.pcm[self.cursor..self.cursor + self.channels]);
            self.cursor += self.channels;
            copied += self.channels;
        }
        if self.revoked() {
            output.fill(0.0);
            self.stopped = true;
            return 0;
        }
        copied
    }
}
