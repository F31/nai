//! Receiver-side freshness gate. The caller supplies an authenticated task
//! identity and calls observe only after validating and accepting media.
use std::{
    num::NonZeroU64,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    pub epoch: NonZeroU64,
    pub generation: NonZeroU64,
}

pub struct FrameLease {
    identity: Identity,
    timeout: Duration,
    last: Option<NonZeroU64>,
    expires: Instant,
    revoked: bool,
}

impl FrameLease {
    /// Bind freshness to an owned, verified frame. The receiver supplies the
    /// trusted task generation, distinct from the frame's recycled slot generation.
    pub fn observe_verified(
        &mut self,
        generation: NonZeroU64,
        frame: &crate::v561_frame_scheduler::OwnedVerifiedFrame,
        now: Instant,
    ) -> bool {
        let metadata = frame.metadata();
        let Some(epoch) = NonZeroU64::new(metadata.epoch) else {
            return false;
        };
        let Some(sequence) = metadata
            .sequence
            .and_then(|s| s.checked_add(1))
            .and_then(NonZeroU64::new)
        else {
            return false;
        };
        self.observe(Identity { epoch, generation }, sequence, now)
    }
    pub fn new(identity: Identity, now: Instant, timeout: Duration) -> Option<Self> {
        if timeout.is_zero() {
            return None;
        }
        Some(Self {
            identity,
            timeout,
            last: None,
            expires: now.checked_add(timeout)?,
            revoked: false,
        })
    }

    pub fn observe(&mut self, identity: Identity, sequence: NonZeroU64, now: Instant) -> bool {
        if self.revoked || now >= self.expires {
            self.revoke();
            return false;
        }
        if identity != self.identity || self.last.is_some_and(|last| sequence <= last) {
            return false;
        }
        let Some(expires) = now.checked_add(self.timeout) else {
            self.revoke();
            return false;
        };
        self.last = Some(sequence);
        self.expires = expires;
        true
    }

    /// Evaluate before every presentation or audio dispatch, including idle callbacks.
    pub fn permits(&mut self, now: Instant) -> bool {
        if now >= self.expires {
            self.revoke();
        }
        !self.revoked && self.last.is_some()
    }

    pub fn revoke(&mut self) {
        self.revoked = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(value: u64) -> NonZeroU64 {
        NonZeroU64::new(value).unwrap()
    }

    #[test]
    fn stale_frames_do_not_extend_an_active_lease() {
        let now = Instant::now();
        let ttl = Duration::from_millis(500);
        let id = Identity {
            epoch: n(1),
            generation: n(2),
        };
        let mut lease = FrameLease::new(id, now, ttl).unwrap();
        assert!(!lease.permits(now));
        assert!(lease.observe(id, n(2), now));
        assert!(lease.permits(now));
        let later = now + Duration::from_millis(400);
        assert!(!lease.observe(id, n(2), later));
        assert!(!lease.observe(id, n(1), later));
        assert!(!lease.observe(
            Identity {
                generation: n(1),
                ..id
            },
            n(3),
            later
        ));
        assert!(!lease.observe(Identity { epoch: n(2), ..id }, n(3), later));
        assert!(!lease.permits(now + ttl));
        assert!(!lease.observe(id, n(4), now + ttl));
    }

    #[test]
    fn fresh_frames_extend_but_explicit_revoke_is_terminal() {
        let now = Instant::now();
        let ttl = Duration::from_millis(500);
        let id = Identity {
            epoch: n(7),
            generation: n(8),
        };
        let mut lease = FrameLease::new(id, now, ttl).unwrap();
        assert!(lease.observe(id, n(1), now));
        assert!(lease.observe(id, n(2), now + Duration::from_millis(250)));
        assert!(lease.permits(now + ttl));
        lease.revoke();
        assert!(!lease.permits(now + ttl));
        assert!(!lease.observe(id, n(3), now + ttl));
    }
}
