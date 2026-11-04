//! Independently expiring public execution consumer; health alone never authorizes media.
use crate::{
    audition::LeaseGate,
    foundation::{Operation, Result, uuid},
    public_media::{sha, structure},
};
use nai_contracts::{prepared_media_hash, public_media_hash, workbench::*};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use zeroize::Zeroizing;
pub const PURPOSE: &str = "nai-public-execution-v1";
pub struct IssueRequest<'a> {
    pub authorization: &'a str,
    pub preparation_id: &'a str,
    pub challenge: u64,
    pub seq: u64,
    pub max_lease_ms: u32,
}
/// Callback-only view: no locks, DB, channels, or ability to arm a deadline.
#[derive(Default)]
pub struct ExecutionFence {
    stopped: AtomicBool,
    started: AtomicBool,
    until: AtomicU64,
    last: AtomicU64,
}
impl ExecutionFence {
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        self.until.store(0, Ordering::Release);
    }
    pub fn allows(&self, now: u64) -> bool {
        if self.stopped.load(Ordering::Acquire) || !self.started.load(Ordering::Acquire) {
            return false;
        }
        if now < self.last.fetch_max(now, Ordering::AcqRel)
            || now >= self.until.load(Ordering::Acquire)
        {
            self.stop();
            return false;
        }
        !self.stopped.load(Ordering::Acquire)
    }
    pub fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }
    fn arm(&self, until: u64) {
        self.until.store(until, Ordering::Release);
        self.started.store(true, Ordering::Release);
    }
}
pub struct PublicExecutionGate {
    key: Zeroizing<String>,
    bundle: PublicMediaBundle,
    preparation: String,
    lease: LeaseGate,
    fence: Arc<ExecutionFence>,
    last: Option<u64>,
}
impl PublicExecutionGate {
    /// Must be called with locally verified media/receipt after actual device preparation.
    pub fn new(
        key: String,
        bundle: PublicMediaBundle,
        d: &PreparedMediaDescriptor,
        r: &PreparedMediaReceipt,
        preparation: String,
    ) -> Result<Self> {
        let key = Zeroizing::new(key);
        structure(&bundle)?;
        uuid(&preparation)?;
        let a = bundle
            .audio
            .as_ref()
            .ok_or(WorkbenchError::IntegrityFailed)?;
        let v = bundle
            .visual
            .as_ref()
            .ok_or(WorkbenchError::IntegrityFailed)?;
        let tail = r
            .tail_frame
            .as_ref()
            .ok_or(WorkbenchError::IntegrityFailed)?;
        if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(WorkbenchError::InvalidArgument);
        }
        if public_media_hash(&bundle) != bundle.approved_sha256
            || prepared_media_hash(d) != bundle.prepared_media_sha256
            || d.protocol_major != 1
            || d.workspace_id != bundle.workspace_id
            || d.task_id != a.artifact_id
            || !sha(&d.task_sha256)
            || d.audio != bundle.audio
            || d.visual != bundle.visual
            || d.renderer_id != "rms-sample-v1"
            || d.pcm_format != "s16le-mono-24000"
            || d.pixel_format != "rgb32le"
            || d.samples_per_frame != 800
            || d.expected_frames != a.sample_count.div_ceil(800)
            || !sha(&d.first_frame_sha256)
            || v.width != 512
            || v.height != 288
            || v.fps != 30
            || r.protocol_major != 1
            || r.consumer_instance_id != bundle.output_id
            || r.descriptor_sha256 != bundle.prepared_media_sha256
            || r.pcm_sha256 != a.pcm_sha256
            || r.sample_count != a.sample_count
            || r.expected_frames != d.expected_frames
            || r.first_frame_sha256 != d.first_frame_sha256
            || tail.frame_index != d.expected_frames
            || tail.sample_start != (d.expected_frames - 1) * 800
            || u64::from(tail.sample_count) != a.sample_count - tail.sample_start
            || tail.sample_rate != 24000
            || tail.width != 512
            || tail.height != 288
            || !sha(&tail.pixels_sha256)
            || tail.openness_q16 > 65535
        {
            return Err(WorkbenchError::IntegrityFailed);
        }
        Ok(Self {
            key,
            bundle,
            preparation,
            lease: LeaseGate::default(),
            fence: Arc::default(),
            last: None,
        })
    }
    pub fn initialization_barrier(&mut self) -> Result<()> {
        if self.fence.started.load(Ordering::Acquire)
            || self.fence.stopped()
            || !self.lease.initialization_barrier()
        {
            self.stop();
            return Err(WorkbenchError::Cancelled);
        }
        Ok(())
    }
    pub fn active(&mut self, now: u64) -> bool {
        self.poll(now).is_ok() && self.lease.started() && self.lease.valid(now)
    }
    pub fn callback_fence(&self) -> Arc<ExecutionFence> {
        self.fence.clone()
    }
    pub fn deadline(&self) -> u64 {
        if self.fence.stopped() {
            0
        } else {
            self.lease.deadline()
        }
    }
    pub fn stop(&mut self) {
        self.fence.stop();
        self.lease.stop();
    }
    fn poll(&mut self, now: u64) -> Result<()> {
        if self.fence.stopped()
            || self.last.is_some_and(|last| now < last)
            || self.lease.started() && !self.lease.valid(now)
        {
            self.stop();
            return Err(WorkbenchError::Cancelled);
        }
        self.last = Some(now);
        Ok(())
    }
    pub fn challenge(&mut self, now: u64) -> Result<u64> {
        self.poll(now)?;
        self.lease.challenge(now).ok_or(WorkbenchError::Cancelled)
    }
    pub fn accept(&mut self, x: &PublicExecutionLease, now: u64, op: &Operation) -> Result<()> {
        let result = (|| {
            self.poll(now)?;
            op.check()?;
            let valid_key = x.authorization.len() == self.key.len()
                && x.authorization
                    .bytes()
                    .zip(self.key.bytes())
                    .fold(0u8, |v, (a, b)| v | (a ^ b))
                    == 0;
            if !valid_key
                || x.purpose != PURPOSE
                || x.output_id != self.bundle.output_id
                || x.bundle_sha256 != self.bundle.approved_sha256
                || x.prepared_media_sha256 != self.bundle.prepared_media_sha256
                || x.session_id != self.bundle.session_id
                || x.epoch != self.bundle.epoch
                || x.preparation_id != self.preparation
            {
                return Err(WorkbenchError::PermissionDenied);
            }
            if !self.lease.accept(x.challenge, x.seq, x.lease_ms, now) {
                return Err(WorkbenchError::DeadlineExceeded);
            }
            op.check()?;
            if self.fence.stopped() {
                return Err(WorkbenchError::Cancelled);
            }
            self.fence.arm(self.lease.deadline());
            Ok(())
        })();
        if result.is_err() {
            self.stop();
        }
        result
    }
}

impl Drop for PublicExecutionGate {
    fn drop(&mut self) {
        self.stop();
    }
}
