//! Session mechanisms. Actor/preflight decisions must come from the trusted root.
use crate::duty::{DUTY_REVOKE_DELAY, DutyEnforcement, DutyLease};
use nai_contracts::v1::{CommandAck, CommandAction, ControlCommand};
use nai_contracts::v1::{SessionLifecycle as Life, SessionMode, SessionSnapshot};
use prost::Message;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    Unauthorized,
    InvalidTransition,
    Stale,
    Conflict,
    Capacity,
    Expired,
    Exhausted,
}

#[derive(Clone)]
pub struct TaskToken {
    epoch: Arc<AtomicU64>,
    born: u64,
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
}
impl TaskToken {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
    pub fn valid(&self, now: Instant) -> bool {
        now < self.deadline
            && !self.cancelled.load(Ordering::SeqCst)
            && self.epoch.load(Ordering::SeqCst) == self.born
    }
}

pub struct Session {
    snapshot: SessionSnapshot,
    epoch: Arc<AtomicU64>,
    commands: CommandLedger<CommandAck>,
    duty: Option<DutyLease>,
    duty_revoked: bool,
}
impl Session {
    pub fn new(session: &str, workspace: &str) -> Result<Self, SessionError> {
        if session.is_empty() || workspace.is_empty() {
            return Err(SessionError::Stale);
        }
        Ok(Self {
            snapshot: SessionSnapshot {
                session_id: session.into(),
                workspace_id: workspace.into(),
                lifecycle: Life::Created as i32,
                mode: SessionMode::Assisted as i32,
                media_state: nai_contracts::v1::MediaState::Standby as i32,
                ..Default::default()
            },
            epoch: Arc::new(AtomicU64::new(0)),
            commands: CommandLedger::new(256),
            duty: None,
            duty_revoked: false,
        })
    }
    /// Recovery never resumes playback or inherits approvals/bindings.
    pub fn recover(saved: SessionSnapshot) -> Result<Self, SessionError> {
        let floor = saved.generation_epoch;
        Self::recover_after(saved, floor)
    }
    /// Include surviving media's epoch floor when the engine restarted after
    /// a revoke that could not be persisted.
    pub fn recover_after(saved: SessionSnapshot, media_epoch: u64) -> Result<Self, SessionError> {
        let mut session = Self::new(&saved.session_id, &saved.workspace_id)?;
        session.snapshot.generation_epoch = saved
            .generation_epoch
            .max(media_epoch)
            .checked_add(1)
            .ok_or(SessionError::Exhausted)?;
        session.snapshot.state_revision = saved
            .state_revision
            .checked_add(1)
            .ok_or(SessionError::Exhausted)?;
        session.snapshot.lifecycle = Life::Stopped as i32;
        session
            .epoch
            .store(session.snapshot.generation_epoch, Ordering::SeqCst);
        Ok(session)
    }
    pub fn snapshot(&self) -> &SessionSnapshot {
        &self.snapshot
    }
    /// Authenticated control route. Mute/Takeover/Stop must signal media BEFORE
    /// calling this method; executed refers to state, not a hardware stop ACK.
    pub fn command(
        &mut self,
        command: &ControlCommand,
        actor_allowed: bool,
        preflight: bool,
        received: Instant,
        now: Instant,
        retry_until: Instant,
    ) -> Result<CommandAck, SessionError> {
        if !actor_allowed {
            return Err(SessionError::Unauthorized);
        }
        let context = command.context.as_ref().ok_or(SessionError::Stale)?;
        if context.session_id != self.snapshot.session_id
            || context.workspace_id != self.snapshot.workspace_id
        {
            return Err(SessionError::Stale);
        }
        let fingerprint = command.encode_to_vec();
        let mut ledger = std::mem::replace(&mut self.commands, CommandLedger::new(256));
        let result = ledger.execute(&command.command_id, &fingerprint, now, retry_until, || {
            let deadline = received
                .checked_add(std::time::Duration::from_millis(command.ttl_ms.into()))
                .ok_or(SessionError::Expired)?;
            if command.ttl_ms == 0
                || context.remaining_budget_ms == 0
                || now >= deadline
                || now
                    >= received
                        + std::time::Duration::from_millis(context.remaining_budget_ms.into())
            {
                return Err(SessionError::Expired);
            }
            if context.generation_epoch != Some(self.snapshot.generation_epoch)
                || command.expected_state_revision != self.snapshot.state_revision
            {
                return Err(SessionError::Stale);
            }
            match CommandAction::try_from(command.action)
                .map_err(|_| SessionError::InvalidTransition)?
            {
                CommandAction::Prepare => self.transition(true, Life::Preparing, preflight)?,
                CommandAction::Start => self.transition(true, Life::Running, preflight)?,
                CommandAction::Stop => {
                    self.transition(true, Life::Draining, preflight)?;
                    self.close_auto_admission();
                }
                CommandAction::Mute | CommandAction::Takeover => {
                    self.revoke(true)?;
                    self.close_auto_admission();
                }
                CommandAction::ChangeMode => self.change_mode(
                    true,
                    SessionMode::try_from(command.requested_mode)
                        .map_err(|_| SessionError::Unauthorized)?,
                    preflight,
                    now,
                )?,
                _ => return Err(SessionError::InvalidTransition),
            }
            Ok(CommandAck {
                command_id: command.command_id.clone(),
                executed: true,
                state_revision: self.snapshot.state_revision,
                generation_epoch: self.snapshot.generation_epoch,
                ..Default::default()
            })
        });
        self.commands = ledger;
        result
    }
    fn close_auto_admission(&mut self) {
        self.duty = None;
        self.duty_revoked = true;
        if self.snapshot.mode == SessionMode::AutoSupervised as i32 {
            self.snapshot.mode = SessionMode::Assisted as i32;
        }
    }
    /// Supervised automatic mode additionally requires a valid duty lease
    /// (V3.6 §21.4); every mode change revokes current output authority first.
    pub fn change_mode(
        &mut self,
        actor_allowed: bool,
        mode: SessionMode,
        mode_authorized: bool,
        now: Instant,
    ) -> Result<(), SessionError> {
        if !actor_allowed || !mode_authorized || mode == SessionMode::ModeUnspecified {
            return Err(SessionError::Unauthorized);
        }
        if mode == SessionMode::AutoSupervised && !self.duty_valid(now) {
            return Err(SessionError::Unauthorized);
        }
        self.revoke(true)?;
        self.snapshot.mode = mode as i32;
        Ok(())
    }
    /// Operator-only grant or renewal. Heartbeats and reconnects never reach
    /// this path, so an expired lease stays closed until an explicit confirm.
    pub fn confirm_duty(
        &mut self,
        actor_allowed: bool,
        operator: &str,
        ttl: Duration,
        now: Instant,
    ) -> Result<(), SessionError> {
        if !actor_allowed || operator.trim().is_empty() || ttl.is_zero() {
            return Err(SessionError::Unauthorized);
        }
        let expires_at = now.checked_add(ttl).ok_or(SessionError::Expired)?;
        self.duty = Some(DutyLease {
            operator: operator.into(),
            granted_at: now,
            expires_at,
        });
        self.duty_revoked = false;
        Ok(())
    }
    /// Observes liveness only. Deliberately does not extend the duty lease.
    pub fn note_heartbeat(&self, _now: Instant) {}
    pub fn duty_valid(&self, now: Instant) -> bool {
        !self.duty_revoked && self.duty.as_ref().is_some_and(|lease| lease.valid(now))
    }
    /// Automatic replies need a running session, supervised automatic mode and
    /// a valid duty lease.
    pub fn auto_reply_permitted(&self, now: Instant) -> bool {
        self.snapshot.lifecycle == Life::Running as i32
            && self.snapshot.mode == SessionMode::AutoSupervised as i32
            && self.duty_valid(now)
    }
    /// Applies duty expiry. New automatic replies stop at `expires_at`; output
    /// authorization is revoked no later than `DUTY_REVOKE_DELAY` after it.
    pub fn enforce_duty(&mut self, now: Instant) -> DutyEnforcement {
        if self.duty_revoked {
            return DutyEnforcement::Revoked;
        }
        let Some(lease) = self.duty.clone() else {
            return DutyEnforcement::Valid;
        };
        if lease.valid(now) {
            return DutyEnforcement::Valid;
        }
        if now < lease.expires_at + DUTY_REVOKE_DELAY {
            return DutyEnforcement::HaltedPendingRevoke;
        }
        // System enforcement, not an actor request: the trusted root drives it.
        if self.revoke(true).is_err() {
            // A renewed lease must not reopen the old Auto mode after counter
            // exhaustion. The root must still revoke media independently.
            if self.snapshot.mode == SessionMode::AutoSupervised as i32 {
                self.snapshot.mode = SessionMode::Assisted as i32;
            }
            return DutyEnforcement::HaltedPendingRevoke;
        }
        self.duty_revoked = true;
        DutyEnforcement::Revoked
    }
    pub fn transition(
        &mut self,
        actor_allowed: bool,
        target: Life,
        preflight: bool,
    ) -> Result<(), SessionError> {
        if !actor_allowed {
            return Err(SessionError::Unauthorized);
        }
        let current = Life::try_from(self.snapshot.lifecycle).map_err(|_| SessionError::Stale)?;
        let allowed = matches!(
            (current, target),
            (
                Life::Created | Life::Stopped | Life::Faulted,
                Life::Preparing
            ) | (Life::Preparing, Life::Ready | Life::ReadyDegraded)
                | (Life::Ready | Life::ReadyDegraded, Life::Running)
                | (Life::Running, Life::Draining)
                | (Life::Draining, Life::Stopped)
        );
        if !allowed && target != Life::Faulted {
            return Err(SessionError::InvalidTransition);
        }
        if matches!(target, Life::Ready | Life::ReadyDegraded | Life::Running) && !preflight {
            return Err(SessionError::Unauthorized);
        }
        if matches!(target, Life::Draining | Life::Faulted | Life::Preparing) {
            self.revoke(actor_allowed)?;
        } else {
            self.snapshot.state_revision = self
                .snapshot
                .state_revision
                .checked_add(1)
                .ok_or(SessionError::Exhausted)?;
        }
        self.snapshot.lifecycle = target as i32;
        Ok(())
    }
    /// Invoke only AFTER the dedicated media emergency handle has muted output.
    pub fn revoke(&mut self, actor_allowed: bool) -> Result<(), SessionError> {
        if !actor_allowed {
            return Err(SessionError::Unauthorized);
        }
        let epoch = self
            .snapshot
            .generation_epoch
            .checked_add(1)
            .ok_or(SessionError::Exhausted)?;
        let revision = self
            .snapshot
            .state_revision
            .checked_add(1)
            .ok_or(SessionError::Exhausted)?;
        self.epoch.store(epoch, Ordering::SeqCst);
        self.snapshot.generation_epoch = epoch;
        self.snapshot.state_revision = revision;
        self.snapshot.media_state = nai_contracts::v1::MediaState::Muted as i32;
        self.snapshot.pending_approvals.clear();
        Ok(())
    }
    pub fn task(&self, deadline: Instant) -> Result<TaskToken, SessionError> {
        if self.snapshot.lifecycle != Life::Running as i32 {
            return Err(SessionError::InvalidTransition);
        }
        Ok(TaskToken {
            epoch: self.epoch.clone(),
            born: self.snapshot.generation_epoch,
            cancelled: Arc::new(AtomicBool::new(false)),
            deadline,
        })
    }
}

/// One ledger per session. Full ledgers reject rather than evict retry records.
pub struct CommandLedger<T> {
    capacity: usize,
    entries: HashMap<String, (Vec<u8>, Instant, T)>,
}
impl<T: Clone> CommandLedger<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: HashMap::new(),
        }
    }
    pub fn execute(
        &mut self,
        id: &str,
        fingerprint: &[u8],
        now: Instant,
        retry_until: Instant,
        run: impl FnOnce() -> Result<T, SessionError>,
    ) -> Result<T, SessionError> {
        if id.is_empty()
            || id.len() > 256
            || fingerprint.len() > 4096
            || retry_until <= now
            || retry_until.duration_since(now) > std::time::Duration::from_secs(300)
        {
            return Err(SessionError::Expired);
        }
        self.entries.retain(|_, (_, expiry, _)| *expiry > now);
        if let Some((old, _, result)) = self.entries.get(id) {
            return if old == fingerprint {
                Ok(result.clone())
            } else {
                Err(SessionError::Conflict)
            };
        }
        if self.entries.len() >= self.capacity {
            return Err(SessionError::Capacity);
        }
        let result = run()?;
        self.entries
            .insert(id.into(), (fingerprint.into(), retry_until, result.clone()));
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn running() -> Session {
        let mut s = Session::new("s", "w").unwrap();
        for state in [Life::Preparing, Life::Ready, Life::Running] {
            s.transition(true, state, true).unwrap();
        }
        s
    }
    #[test]
    fn authenticated_commands_dedupe_before_stale_epoch_and_reject_replay_changes() {
        let now = Instant::now();
        let mut s = running();
        let command = ControlCommand {
            command_id: "mute-1".into(),
            action: CommandAction::Mute as i32,
            expected_state_revision: s.snapshot().state_revision,
            ttl_ms: 1000,
            context: Some(nai_contracts::v1::RequestContext {
                session_id: "s".into(),
                workspace_id: "w".into(),
                generation_epoch: Some(s.snapshot().generation_epoch),
                remaining_budget_ms: 1000,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            s.command(
                &command,
                false,
                true,
                now,
                now,
                now + Duration::from_secs(5)
            ),
            Err(SessionError::Unauthorized)
        );
        let first = s
            .command(&command, true, true, now, now, now + Duration::from_secs(5))
            .unwrap();
        assert_eq!(
            s.command(
                &command,
                true,
                true,
                now,
                now + Duration::from_secs(2),
                now + Duration::from_secs(5)
            ),
            Ok(first.clone())
        );
        assert_eq!(s.snapshot().generation_epoch, first.generation_epoch);
        let mut changed = command.clone();
        changed.action = CommandAction::Start as i32;
        assert_eq!(
            s.command(&changed, true, true, now, now, now + Duration::from_secs(5)),
            Err(SessionError::Conflict)
        );
        changed.command_id = "different".into();
        assert_eq!(
            s.command(&changed, true, true, now, now, now + Duration::from_secs(5)),
            Err(SessionError::Stale)
        );
    }
    #[test]
    fn lifecycle_requires_actor_and_preflight() {
        let mut s = Session::new("s", "w").unwrap();
        assert_eq!(
            s.transition(true, Life::Running, true),
            Err(SessionError::InvalidTransition)
        );
        assert_eq!(
            s.transition(false, Life::Preparing, true),
            Err(SessionError::Unauthorized)
        );
        s.transition(true, Life::Preparing, true).unwrap();
        assert_eq!(
            s.transition(true, Life::Ready, false),
            Err(SessionError::Unauthorized)
        );
    }
    #[test]
    fn two_layer_cancellation_and_session_isolation() {
        let now = Instant::now();
        let mut s = running();
        let other = running();
        let a = s.task(now + Duration::from_secs(1)).unwrap();
        let b = s.task(now + Duration::from_secs(1)).unwrap();
        let c = other.task(now + Duration::from_secs(1)).unwrap();
        a.cancel();
        assert!(!a.valid(now));
        assert!(b.valid(now));
        s.revoke(true).unwrap();
        assert!(!b.valid(now));
        assert!(c.valid(now));
        assert!(!c.valid(now + Duration::from_secs(1)));
    }
    #[test]
    fn recovery_does_not_inherit_authority() {
        let s = running();
        let restored = Session::recover(s.snapshot().clone()).unwrap();
        assert!(restored.snapshot().generation_epoch > s.snapshot().generation_epoch);
        assert_eq!(restored.snapshot().lifecycle, Life::Stopped as i32);
        assert!(restored.task(Instant::now()).is_err());
        let reconciled = Session::recover_after(s.snapshot().clone(), 100).unwrap();
        assert_eq!(reconciled.snapshot().generation_epoch, 101);
    }
    #[test]
    fn duty_counter_exhaustion_does_not_reopen_auto_after_confirmation() {
        for exhaust_epoch in [true, false] {
            let now = Instant::now();
            let mut session = running();
            session
                .confirm_duty(true, "operator", Duration::from_secs(1), now)
                .unwrap();
            session
                .change_mode(true, SessionMode::AutoSupervised, true, now)
                .unwrap();
            if exhaust_epoch {
                session.snapshot.generation_epoch = u64::MAX;
            } else {
                session.snapshot.state_revision = u64::MAX;
            }
            let later = now + Duration::from_secs(1) + DUTY_REVOKE_DELAY;
            assert_eq!(
                session.enforce_duty(later),
                DutyEnforcement::HaltedPendingRevoke
            );
            session
                .confirm_duty(true, "operator", Duration::from_secs(10), later)
                .unwrap();
            assert!(!session.auto_reply_permitted(later));
            assert_eq!(
                session.change_mode(true, SessionMode::AutoSupervised, true, later),
                Err(SessionError::Exhausted)
            );
        }
    }
    #[test]
    fn bounded_retry_ledger_never_reexecutes_or_evicts() {
        let now = Instant::now();
        let expiry = now + Duration::from_secs(1);
        let mut ledger = CommandLedger::new(1);
        assert_eq!(ledger.execute("a", b"mute", now, expiry, || Ok(7)), Ok(7));
        assert_eq!(
            ledger.execute("a", b"mute", now, expiry, || panic!("reexecuted")),
            Ok(7)
        );
        assert_eq!(
            ledger.execute("a", b"start", now, expiry, || Ok(8)),
            Err(SessionError::Conflict)
        );
        assert_eq!(
            ledger.execute("b", b"mute", now, expiry, || Ok(8)),
            Err(SessionError::Capacity)
        );
        assert_eq!(
            ledger.execute(
                "b",
                b"mute",
                expiry,
                expiry + Duration::from_secs(1),
                || Ok(8)
            ),
            Ok(8)
        );
    }
}
