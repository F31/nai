//! Challenge-anchored worker-local lease. No clock synchronization or arrival-time renewal.
#[derive(Default)]
pub struct LeaseGate {
    seq: u64,
    issued: u64,
    epoch: u64,
    until: u64,
    active: bool,
    stopped: bool,
}
impl LeaseGate {
    pub fn challenge(&mut self, now: u64) -> Option<u64> {
        if self.stopped || self.active && now >= self.until {
            self.stop();
            return None;
        }
        self.seq = self.seq.checked_add(1)?;
        self.issued = now;
        Some(self.seq)
    }
    pub fn accept(&mut self, seq: u64, epoch: u64, ttl: u32, now: u64) -> bool {
        if self.stopped || self.active && now >= self.until {
            self.stop();
            return false;
        }
        if seq != self.seq
            || seq == 0
            || epoch <= self.epoch
            || ttl == 0
            || ttl > 500
            || now < self.issued
            || now - self.issued > 250
        {
            return false;
        }
        let Some(until) = self.issued.checked_add(ttl.into()) else {
            return false;
        };
        if now >= until {
            return false;
        }
        self.epoch = epoch;
        self.until = until;
        self.active = true;
        true
    }
    pub fn deadline(&self) -> u64 {
        if self.stopped { 0 } else { self.until }
    }
    pub fn valid(&self, now: u64) -> bool {
        !self.stopped && self.active && now < self.until
    }
    pub fn started(&self) -> bool {
        self.active
    }
    /// Device initialization may outlive a lease. Require a new challenge after
    /// initialization without reusing a sequence or accepting buffered replies.
    pub fn initialization_barrier(&mut self) -> bool {
        if self.stopped {
            return false;
        }
        let Some(seq) = self.seq.checked_add(1) else {
            self.stop();
            return false;
        };
        self.seq = seq;
        self.active = false;
        self.until = 0;
        true
    }
    pub fn stop(&mut self) {
        self.stopped = true;
        self.until = 0;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn device_initialization_cannot_reuse_a_buffered_lease() {
        let mut g = LeaseGate::default();
        let old = g.challenge(1).unwrap();
        assert!(g.accept(old, 1, 500, 2));
        assert!(g.initialization_barrier());
        assert!(!g.valid(700));
        let fresh = g.challenge(700).unwrap();
        assert!(fresh > old);
        assert!(!g.accept(old, 2, 500, 701));
        assert!(g.accept(fresh, 3, 500, 702));
        g.stop();
        assert!(!g.initialization_barrier());
    }
    #[test]
    fn delayed_and_replayed_lease_cannot_extend_permission() {
        let mut g = LeaseGate::default();
        let c = g.challenge(100).unwrap();
        assert!(g.accept(c, 1, 500, 300));
        assert_eq!(g.deadline(), 600);
        assert!(!g.accept(c, 1, 500, 350));
        let n = g.challenge(400).unwrap();
        assert!(!g.accept(c, 2, 500, 410));
        assert!(g.accept(n, 3, 500, 450));
        assert!(g.valid(899));
        assert!(!g.valid(900));
        assert!(!g.accept(n, 4, 500, 900));
        assert!(g.challenge(901).is_none());
    }
    #[test]
    fn stop_and_invalid_ttl_fail_closed() {
        let mut g = LeaseGate::default();
        let c = g.challenge(0).unwrap();
        assert!(!g.accept(c, 1, 501, 1));
        assert!(!g.accept(c, 1, 500, 251));
        assert!(g.accept(c, 2, 500, 10));
        g.stop();
        assert!(!g.valid(11));
        assert!(!g.accept(c, 3, 500, 12));
    }
}
