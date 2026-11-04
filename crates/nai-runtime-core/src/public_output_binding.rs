//! Root-clock, single-use freshness challenge. Health observation is not media authority.
use crate::foundation::{Operation, Result, uuid};
use nai_contracts::workbench::{
    PublicOutputBindingChallenge, PublicOutputObservation, PublicOutputReason, PublicOutputStatus,
    WorkbenchError as Error,
};
pub const TTL_MS: u64 = 250;
#[derive(Default)]
pub struct OutputBindingGate {
    next: u64,
    pending: Option<(u64, u64)>,
}
pub struct BoundPublicOutput {
    status: PublicOutputStatus,
    issued: u64,
    until: u64,
    nonce: String,
}
/// Health consistency only; never an execution permission.
pub fn healthy(s: &PublicOutputStatus) -> bool {
    let playing = s.window_prepared.as_ref().is_some_and(|p| {
        p.playing
            && !p.execution_stopped
            && !p.submission_complete
            && p.device_ready
            && p.execution_seq > 0
    });
    s.protocol_major == 1
        && s.window_open
        && !s.terminal
        && s.seq > 0
        && s.challenge > 0
        && s.standby_frames > 0
        && s.visual.is_some()
        && s.standby != playing
        && s.live_ready == playing
        && s.audio_active == playing
        && s.reason
            == if playing {
                PublicOutputReason::MediaActive as i32
            } else {
                PublicOutputReason::StandbyOnly as i32
            }
}
impl OutputBindingGate {
    pub fn prepare(&mut self, now: u64) -> Result<PublicOutputBindingChallenge> {
        self.next = self.next.checked_add(1).ok_or(Error::ResourceExhausted)?;
        self.pending = Some((self.next, now));
        Ok(PublicOutputBindingChallenge {
            challenge: self.next,
            ttl_ms: TTL_MS as u32,
        })
    }
    pub fn consume(
        &mut self,
        observation: Option<&PublicOutputObservation>,
        now: u64,
        nonce: &str,
        op: &Operation,
    ) -> Result<Option<BoundPublicOutput>> {
        let pending = self.pending.take();
        op.check()?;
        let Some(o) = observation else {
            return Ok(None);
        };
        let (challenge, issued) = pending.ok_or(Error::PermissionDenied)?;
        if challenge != o.challenge {
            return Err(Error::RevisionConflict);
        }
        let s = o.status.as_ref().ok_or(Error::InvalidArgument)?;
        uuid(&s.output_id)?;
        if !nonce.is_empty() {
            uuid(nonce)?;
        }
        if !healthy(s) {
            return Err(Error::CapabilityUnavailable);
        }
        let bound = BoundPublicOutput {
            status: s.clone(),
            issued,
            until: issued.checked_add(TTL_MS).ok_or(Error::DeadlineExceeded)?,
            nonce: o.stop_nonce.clone(),
        };
        bound.recheck(now, nonce, op)?;
        Ok(Some(bound))
    }
}
impl BoundPublicOutput {
    pub fn recheck(&self, now: u64, nonce: &str, op: &Operation) -> Result<()> {
        op.check()?;
        if now < self.issued || now >= self.until {
            return Err(Error::DeadlineExceeded);
        }
        if nonce != self.nonce {
            return Err(Error::Cancelled);
        }
        Ok(())
    }
    pub fn valid_until_tick(&self) -> u64 {
        self.until
    }
    pub fn status(&self) -> &PublicOutputStatus {
        &self.status
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn observation(challenge: u64) -> PublicOutputObservation {
        PublicOutputObservation {
            challenge,
            stop_nonce: String::new(),
            status: Some(PublicOutputStatus {
                protocol_major: 1,
                seq: 1,
                challenge: 1,
                window_open: true,
                standby: true,
                standby_frames: 1,
                reason: PublicOutputReason::StandbyOnly as i32,
                output_id: "00000000-0000-4000-8000-000000000001".into(),
                visual: Some(Default::default()),
                ..Default::default()
            }),
        }
    }
    #[test]
    fn challenge_is_single_use_and_deadline_starts_at_root_issue() {
        let mut gate = OutputBindingGate::default();
        let op = Operation::new(4000).unwrap();
        let c = gate.prepare(100).unwrap();
        let b = gate
            .consume(Some(&observation(c.challenge)), 200, "", &op)
            .unwrap()
            .unwrap();
        b.recheck(349, "", &op).unwrap();
        assert_eq!(b.recheck(350, "", &op), Err(Error::DeadlineExceeded));
        assert!(
            gate.consume(Some(&observation(c.challenge)), 201, "", &op)
                .is_err()
        );
        let c = gate.prepare(100).unwrap();
        assert!(
            gate.consume(Some(&observation(c.challenge)), 350, "", &op)
                .is_err()
        );
    }
    #[test]
    fn replacement_missing_observation_and_clock_regression_do_not_revive_old_challenges() {
        let mut gate = OutputBindingGate::default();
        let op = Operation::new(4000).unwrap();
        let old = gate.prepare(100).unwrap();
        gate.prepare(110).unwrap();
        assert!(
            gate.consume(Some(&observation(old.challenge)), 120, "", &op)
                .is_err()
        );
        let c = gate.prepare(100).unwrap();
        assert!(
            gate.consume(Some(&observation(c.challenge)), 99, "", &op)
                .is_err()
        );
        let c = gate.prepare(100).unwrap();
        assert!(gate.consume(None, 100, "", &op).unwrap().is_none());
        assert!(
            gate.consume(Some(&observation(c.challenge)), 101, "", &op)
                .is_err()
        );
    }
    #[test]
    fn terminal_or_unknown_instance_stop_and_cancellation_refuse_binding() {
        for n in 0..5 {
            let mut gate = OutputBindingGate::default();
            let c = gate.prepare(100).unwrap();
            let mut o = observation(c.challenge);
            let op = Operation::new(4000).unwrap();
            match n {
                0 => o.status.as_mut().unwrap().terminal = true,
                1 => o.status.as_mut().unwrap().output_id = "unknown".into(),
                2 => o.stop_nonce = "changed".into(),
                3 => op.cancel(),
                _ => o.status.as_mut().unwrap().live_ready = true,
            }
            assert!(gate.consume(Some(&o), 101, "", &op).is_err());
        }
    }
}
