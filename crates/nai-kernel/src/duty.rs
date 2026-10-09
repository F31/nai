//! Duty lease for supervised automatic mode (V3.6 §21.4, INV-08). Only an
//! explicit operator confirmation grants or renews it; network heartbeats and
//! reconnects never extend or restore it.
use std::time::{Duration, Instant};

/// Initial value from ADR 0015: output authorization is revoked no later than
/// this delay after the duty lease expired.
pub const DUTY_REVOKE_DELAY: Duration = Duration::from_millis(600);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DutyLease {
    pub operator: String,
    pub granted_at: Instant,
    pub expires_at: Instant,
}
impl DutyLease {
    pub fn valid(&self, now: Instant) -> bool {
        now < self.expires_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DutyEnforcement {
    /// No expiry action is needed (no lease, or a valid lease). This is NOT an
    /// automatic-reply grant; callers must use Session::auto_reply_permitted.
    Valid,
    /// Expired and new automatic replies are already refused, but the bounded
    /// output revocation delay has not elapsed yet.
    HaltedPendingRevoke,
    /// Output authorization revoked; idempotent on repeated calls.
    Revoked,
}
