//! Single-writer lease renewal, lock-free audio callback checks, terminal revoke.
use std::{
    sync::atomic::{AtomicU8, AtomicU64, Ordering},
    time::Instant,
};
pub struct AudioLease {
    epoch: u64,
    sequence: AtomicU64,
    deadline_ns: AtomicU64,
    state: AtomicU8, // 0 prepared, 1 live, 2 revoked
    base: Instant,
}
impl AudioLease {
    pub fn new(epoch: u64) -> Option<Self> {
        (epoch != 0).then(|| Self {
            epoch,
            sequence: AtomicU64::new(1),
            deadline_ns: AtomicU64::new(0),
            state: AtomicU8::new(0),
            base: Instant::now(),
        })
    }
    pub fn revoke(&self) {
        self.state.store(2, Ordering::Release);
    }
    /// Must be called by exactly one control reader, never by the audio callback.
    pub fn renew(&self, epoch: u64, sequence: u64, ttl_ms: u32) -> bool {
        let state = self.state.load(Ordering::Acquire);
        if state == 2
            || epoch != self.epoch
            || sequence <= self.sequence.load(Ordering::Acquire)
            || !(1..=500).contains(&ttl_ms)
            || (state == 1 && !self.allowed())
        {
            self.revoke();
            return false;
        }
        let Ok(now) = u64::try_from(self.base.elapsed().as_nanos()) else {
            self.revoke();
            return false;
        };
        let Some(deadline) = now.checked_add(u64::from(ttl_ms) * 1_000_000) else {
            self.revoke();
            return false;
        };
        self.sequence.store(sequence, Ordering::Release);
        self.deadline_ns.store(deadline, Ordering::Release);
        self.state
            .compare_exchange(state, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    pub fn allowed(&self) -> bool {
        if self.state.load(Ordering::Acquire) != 1 {
            return false;
        }
        if self.base.elapsed().as_nanos() >= u128::from(self.deadline_ns.load(Ordering::Acquire)) {
            self.revoke();
            return false;
        }
        self.state.load(Ordering::Acquire) == 1
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prepared_is_silent_and_expiry_is_terminal() {
        let lease = AudioLease::new(1).unwrap();
        assert!(!lease.allowed());
        assert!(lease.renew(1, 2, 1));
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(!lease.allowed());
        assert!(!lease.renew(1, 3, 500));
    }
    #[test]
    fn epoch_sequence_and_ttl_faults_revoke() {
        for (epoch, seq, ttl) in [(2, 3, 500), (1, 2, 500), (1, 3, 501), (1, 3, 0)] {
            let lease = AudioLease::new(1).unwrap();
            assert!(lease.renew(1, 2, 500));
            assert!(!lease.renew(epoch, seq, ttl));
            assert!(!lease.allowed());
            assert!(!lease.renew(1, 4, 500));
        }
    }
    #[test]
    fn explicit_revoke_does_not_reopen() {
        let lease = AudioLease::new(1).unwrap();
        lease.revoke();
        assert!(!lease.renew(1, 2, 500));
        assert!(!lease.allowed());
    }
}
