//! Independent output state. The host must call `frame` and `permits_audio`
//! before each output callback and clear its own downstream buffers on denial.
use crate::{
    v561_frame_lease::{FrameLease, Identity},
    v561_frame_scheduler::OwnedVerifiedFrame,
};
use std::time::{Duration, Instant};

pub struct LeasedOutput {
    identity: Identity,
    lease: FrameLease,
    latest: Option<OwnedVerifiedFrame>,
}

impl LeasedOutput {
    pub fn new(identity: Identity, now: Instant, timeout: Duration) -> Option<Self> {
        Some(Self {
            identity,
            lease: FrameLease::new(identity, now, timeout)?,
            latest: None,
        })
    }

    pub fn accept(&mut self, identity: Identity, frame: OwnedVerifiedFrame, now: Instant) -> bool {
        if identity != self.identity
            || !self
                .lease
                .observe_verified(identity.generation, &frame, now)
        {
            self.expire_if_needed(now);
            return false;
        }
        self.latest = Some(frame);
        true
    }

    pub fn frame(&mut self, now: Instant) -> Option<&OwnedVerifiedFrame> {
        self.expire_if_needed(now);
        self.latest.as_ref()
    }

    pub fn permits_audio(&mut self, identity: Identity, now: Instant) -> bool {
        self.expire_if_needed(now);
        identity == self.identity && self.latest.is_some()
    }

    /// Fill the entire callback buffer, including when no frame is admissible.
    pub fn write_video(&mut self, destination: &mut [u8], now: Instant) -> bool {
        if let Some(frame) = self.frame(now)
            && frame.pixels().len() == destination.len()
        {
            destination.copy_from_slice(frame.pixels());
            return true;
        }
        destination.fill(0);
        false
    }

    /// PCM is supplied by the clocked consumer for this callback only.
    pub fn write_audio(
        &mut self,
        identity: Identity,
        samples: &[i16],
        destination: &mut [i16],
        now: Instant,
    ) -> bool {
        if self.permits_audio(identity, now) && samples.len() == destination.len() {
            destination.copy_from_slice(samples);
            return true;
        }
        destination.fill(0);
        false
    }

    pub fn stop(&mut self) {
        self.lease.revoke();
        self.latest = None;
    }

    fn expire_if_needed(&mut self, now: Instant) {
        if !self.lease.permits(now) {
            self.latest = None;
        }
    }
}
