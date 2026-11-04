//! Health lease only. This gate cannot authorize content, audio or frames.
use crate::audition::LeaseGate;
use nai_contracts::workbench::{
    PublicOutputLease, PublicOutputReason as Reason, PublicOutputStatus,
};
use zeroize::Zeroizing;
pub struct StandbyGate {
    lease: LeaseGate,
    token: Zeroizing<String>,
    seq: u64,
    reason: Reason,
}
impl StandbyGate {
    pub fn new(token: String) -> Option<Self> {
        if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        Some(Self {
            lease: LeaseGate::default(),
            token: Zeroizing::new(token),
            seq: 0,
            reason: Reason::StandbyOnly,
        })
    }
    pub fn terminal(&self) -> bool {
        self.reason != Reason::StandbyOnly
    }
    pub fn stop(&mut self, reason: Reason) {
        if !self.terminal() {
            self.reason = reason;
            self.lease.stop();
        }
    }
    pub fn poll(&mut self, now: u64) {
        if !self.terminal()
            && ((!self.lease.started() && now >= 1000)
                || (self.lease.started() && !self.lease.valid(now)))
        {
            self.stop(Reason::LeaseExpired);
        }
    }
    pub fn accept(&mut self, r: &PublicOutputLease, now: u64) -> bool {
        self.poll(now);
        if self.terminal() {
            return false;
        }
        if r.authorization != self.token.as_str()
            || r.seq <= self.seq
            || !self.lease.accept(r.challenge, r.seq, r.lease_ms, now)
        {
            self.stop(Reason::ProtocolRejected);
            return false;
        }
        self.seq = r.seq;
        true
    }
    pub fn status(&mut self, now: u64, frames: u64) -> PublicOutputStatus {
        self.poll(now);
        let challenge = if self.terminal() {
            0
        } else {
            self.lease.challenge(now).unwrap_or(0)
        };
        PublicOutputStatus {
            protocol_major: 1,
            seq: self.seq,
            challenge,
            window_open: true,
            standby: true,
            live_ready: false,
            audio_active: false,
            standby_frames: frames,
            reason: self.reason as i32,
            terminal: self.terminal(),
            ..Default::default()
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn gate() -> StandbyGate {
        StandbyGate::new("a".repeat(64)).unwrap()
    }
    fn lease(challenge: u64, seq: u64) -> PublicOutputLease {
        PublicOutputLease {
            authorization: "a".repeat(64),
            seq,
            challenge,
            lease_ms: 500,
            ..Default::default()
        }
    }
    #[test]
    fn expiration_and_stop_cannot_be_revived() {
        for stop in [false, true] {
            let mut g = gate();
            let s = g.status(0, 1);
            assert!(g.accept(&lease(s.challenge, 1), 1));
            if stop {
                g.stop(Reason::Stopped)
            } else {
                g.poll(500)
            };
            assert!(g.terminal());
            assert!(!g.accept(&lease(s.challenge, 2), 501));
            let s = g.status(502, 2);
            assert!(s.standby && !s.live_ready && !s.audio_active && s.terminal);
        }
    }
    #[test]
    fn stale_challenge_token_replay_and_oversized_lease_fail_closed() {
        for scenario in 0..4 {
            let mut g = gate();
            let s = g.status(0, 1);
            let mut r = lease(s.challenge, 1);
            match scenario {
                0 => r.authorization = "b".repeat(64),
                1 => r.challenge += 1,
                2 => r.lease_ms = 501,
                _ => r.seq = 0,
            };
            assert!(!g.accept(&r, 1));
            assert!(g.terminal());
        }
    }
    #[test]
    fn delayed_response_does_not_extend_from_arrival_time() {
        let mut g = gate();
        let s = g.status(0, 1);
        assert!(!g.accept(&lease(s.challenge, 1), 251));
        assert!(g.terminal());
    }
    #[test]
    fn healthy_status_never_grants_public_media() {
        let mut g = gate();
        let s = g.status(0, 1);
        assert!(g.accept(&lease(s.challenge, 1), 10));
        let s = g.status(100, 2);
        assert_eq!(s.seq, 1);
        assert!(s.challenge > 1);
        assert!(!s.terminal && !s.live_ready && !s.audio_active);
        assert!(g.accept(&lease(s.challenge, 2), 101));
    }
}
