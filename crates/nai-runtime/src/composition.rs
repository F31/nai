//! The composition root. It owns the single storage owner, the session and its
//! duty lease, the media gate, the approval backend and the decision log, and it
//! drives the supervised media subprocesses. It never opens a second store and
//! never exposes owner administration to models or plugins.
use crate::{Result, RuntimeError, err, supervisor::Supervisor};
use nai_contracts::v1::{ErrorCode, PlaybackLease, SessionMode};
use nai_control_api::ApprovalBackend;
use nai_kernel::duty::DutyEnforcement;
use nai_kernel::session::Session;
use nai_kernel::{Limits, Registry};
use nai_media_core::{EmergencyHandle, MediaGate};
use nai_storage::{ConfigKind, DataPaths, DecisionAck, DecisionKind, Operation, Store};
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// One short operation per call: `Operation` caps at 30s and has no long-lease
/// expression, so every decision-log or store access is bounded and re-created.
/// This is the budget for a call that touches rows, **not** for a bring-up that
/// runs the schema migration chain -- see `BRINGUP_OPERATION`.
const SHORT_OPERATION: Duration = Duration::from_secs(5);
/// The budget for a one-shot bring-up whose work **includes the schema migration
/// chain** (`Store::open` on a fresh data root). The definition lives in
/// `nai_storage::BRINGUP_OPERATION`, next to the `Store::open` that performs the work;
/// it is re-exported here because the engine is a consumer, and the number is **not**
/// repeated -- so the engine cannot hold a second, drifting copy.
pub use nai_storage::BRINGUP_OPERATION;
/// How long a **revocation** path waits for the shared session lock. A snapshot
/// or a command holds it for microseconds; a bounded wait turns a transient
/// contention into a short pause instead of a lost safety action. Exhausting the
/// budget is still an explicit conflict -- the action is reported, never skipped.
const SESSION_REVOKE_BUDGET: Duration = Duration::from_millis(250);
/// How long the **periodic** liveness read waits. It runs once per watch tick
/// (`DRILL_POLL`, 5ms) and must not stall lease renewal, so it waits at most a
/// tick and then reports "not observed this tick" rather than a fault.
const SESSION_POLL_BUDGET: Duration = Duration::from_millis(5);

/// Why a non-blocking session-lock attempt produced no guard. The two are kept
/// apart because they call for opposite responses: `Busy` is self-clearing and
/// worth waiting for, `Poisoned` is a real fault that waiting cannot fix.
enum SessionLockMiss {
    Busy,
    Poisoned,
}
/// Bounded media-gate queue (utterance/approval/fact references per scope).
const GATE_CAPACITY: usize = 64;
/// The media gate only accepts lease TTLs up to 600ms.
const MAX_LEASE_MS: u64 = 600;

/// A control action is applied as "media mute first, then session revoke, then a
/// critical decision log". A successful ACK never means the device stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlAction {
    Mute,
    Takeover,
}
impl ControlAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Mute => "mute",
            Self::Takeover => "takeover",
        }
    }
}

/// Duty-lease transitions that are always recorded as critical.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DutyEvent {
    Granted,
    Expired,
    Revoked,
}
impl DutyEvent {
    fn as_str(self) -> &'static str {
        match self {
            Self::Granted => "granted",
            Self::Expired => "expired",
            Self::Revoked => "revoked",
        }
    }
}

/// Fault outcome after the ordered halt has been applied.
#[derive(Debug)]
pub struct ObservedFault {
    pub fault: crate::supervisor::SupervisorFault,
    pub ack: DecisionAck,
}

pub struct Composition {
    store: Arc<Mutex<Store>>,
    workspace: String,
    actor: String,
    profile: String,
    session_id: String,
    session: Arc<Mutex<Session>>,
    gate: MediaGate,
    registry: Registry,
    approvals: Arc<ApprovalBackend>,
    config: Value,
    lease_renew_ms: u64,
    lease_expire_ms: u64,
    boot_id: String,
    duty_active: bool,
    duty_expiry_logged: bool,
    trace_seq: u64,
}

impl Composition {
    /// Assembles the runtime for one workspace/profile.
    ///
    /// `bootstrap` performs trusted owner administration: it creates the
    /// workspace and grants the permissions the runtime needs. It MUST only be
    /// requested by the trusted owner (never a model or plugin), and it is
    /// intentionally one-shot — the workspace must not already exist. The
    /// runtime profile itself must be provisioned by the owner before assembly.
    pub fn prepare(
        paths: DataPaths,
        workspace: &str,
        actor: &str,
        profile: &str,
        bootstrap: bool,
    ) -> Result<Self> {
        let (mut store, operation) = open_bringup_store(paths)?;
        if bootstrap {
            bootstrap_owner(&mut store, &operation, workspace, actor, profile)?;
        }
        let config = store
            .load_config(&operation, workspace, actor, ConfigKind::Runtime, profile)
            .map_err(RuntimeError::from)?;
        let media = &config["media"];
        let lease_renew_ms = media["lease_renew_ms"].as_u64().unwrap_or(0);
        let lease_expire_ms = media["lease_expire_ms"].as_u64().unwrap_or(0);
        if lease_renew_ms == 0 || lease_expire_ms <= lease_renew_ms {
            return Err(err(
                ErrorCode::InvalidArgument,
                "runtime media lease intervals invalid",
            ));
        }
        let max_sessions = config["runtime"]["max_active_sessions"]
            .as_u64()
            .unwrap_or(0);
        if !(1..=1024).contains(&max_sessions) {
            return Err(err(
                ErrorCode::InvalidArgument,
                "runtime max_active_sessions out of range",
            ));
        }
        // Single-session composition: the workspace id is the session id.
        let session_id = workspace.to_owned();
        let session = Session::new(&session_id, workspace).map_err(RuntimeError::from)?;
        let epoch_floor = session.snapshot().generation_epoch;
        let session = Arc::new(Mutex::new(session));
        let gate = MediaGate::with_epoch_floor(&session_id, GATE_CAPACITY, epoch_floor);
        let boot_id = format!("boot-{}-{}", std::process::id(), utc_ms()?);
        let registry = Registry::new(
            &boot_id,
            Limits {
                plugins: 256,
                sessions: max_sessions as usize,
                dependencies: 64,
                depth: 32,
            },
        )
        .map_err(RuntimeError::from)?;
        let store = Arc::new(Mutex::new(store));
        let approvals = ApprovalBackend::new(store.clone(), workspace, actor)
            .map_err(|message| err(ErrorCode::InvalidArgument, message))?;
        Ok(Self {
            store,
            workspace: workspace.into(),
            actor: actor.into(),
            profile: profile.into(),
            session_id,
            session,
            gate,
            registry,
            approvals: Arc::new(approvals),
            config,
            lease_renew_ms,
            lease_expire_ms,
            boot_id,
            duty_active: false,
            duty_expiry_logged: false,
            trace_seq: 0,
        })
    }

    pub fn workspace(&self) -> &str {
        &self.workspace
    }
    pub fn actor(&self) -> &str {
        &self.actor
    }
    pub fn profile(&self) -> &str {
        &self.profile
    }
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
    pub fn boot_id(&self) -> &str {
        &self.boot_id
    }
    pub fn config(&self) -> &Value {
        &self.config
    }
    pub fn lease_renew_ms(&self) -> u64 {
        self.lease_renew_ms
    }
    pub fn lease_expire_ms(&self) -> u64 {
        self.lease_expire_ms
    }
    /// The single session, shared with the control surface. Both sides observe
    /// the same epoch; there is no private copy.
    pub fn session(&self) -> Arc<Mutex<Session>> {
        self.session.clone()
    }
    /// A single non-blocking attempt. The composition shares this mutex with the
    /// control surface, so a miss is expected occasionally and is not by itself
    /// an error.
    fn try_session(
        &self,
    ) -> std::result::Result<std::sync::MutexGuard<'_, Session>, SessionLockMiss> {
        match self.session.try_lock() {
            Ok(guard) => Ok(guard),
            Err(std::sync::TryLockError::WouldBlock) => Err(SessionLockMiss::Busy),
            Err(std::sync::TryLockError::Poisoned(_)) => Err(SessionLockMiss::Poisoned),
        }
    }
    /// Fail-fast acquisition for queries and commands: the caller learns at once
    /// that the session was busy and may retry at its own cadence.
    pub fn session_lock(&self) -> Result<std::sync::MutexGuard<'_, Session>> {
        self.try_session()
            .map_err(|_| err(ErrorCode::Conflict, "session state busy"))
    }
    /// Acquisition for a **revocation** path: waits up to `budget` for a
    /// transient contention to clear. Exhausting the budget is still reported as
    /// an explicit conflict, so a revoke is never silently skipped -- it is
    /// either applied or reported.
    fn session_lock_within(&self, budget: Duration) -> Result<std::sync::MutexGuard<'_, Session>> {
        let deadline = Instant::now() + budget;
        loop {
            match self.try_session() {
                Ok(guard) => return Ok(guard),
                Err(SessionLockMiss::Poisoned) => {
                    return Err(err(ErrorCode::Conflict, "session state poisoned"));
                }
                Err(SessionLockMiss::Busy) => {
                    if Instant::now() >= deadline {
                        return Err(err(
                            ErrorCode::Conflict,
                            "session state busy beyond the lock budget",
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
    }
    /// Reads the live generation epoch for a **periodic** reader. `Ok(None)` means
    /// the lock stayed busy for the whole poll budget: "not observed this tick".
    /// That is neither evidence of a change nor a fault, and a watch loop must
    /// treat it as neither -- the next tick reads again. Only a poisoned lock is
    /// an error, because waiting cannot repair it.
    pub fn live_epoch(&self) -> Result<Option<u64>> {
        let deadline = Instant::now() + SESSION_POLL_BUDGET;
        loop {
            match self.try_session() {
                Ok(session) => return Ok(Some(session.snapshot().generation_epoch)),
                Err(SessionLockMiss::Poisoned) => {
                    return Err(err(ErrorCode::Internal, "session state poisoned"));
                }
                Err(SessionLockMiss::Busy) if Instant::now() >= deadline => return Ok(None),
                Err(SessionLockMiss::Busy) => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    }
    pub fn gate(&self) -> &MediaGate {
        &self.gate
    }
    pub fn gate_mut(&mut self) -> &mut MediaGate {
        &mut self.gate
    }
    /// Dedicated P0 mute handle. Muting never waits for the database or a model.
    pub fn emergency(&self) -> EmergencyHandle {
        self.gate.emergency()
    }
    pub fn registry(&self) -> &Registry {
        &self.registry
    }
    pub fn registry_mut(&mut self) -> &mut Registry {
        &mut self.registry
    }
    pub fn approvals(&self) -> &Arc<ApprovalBackend> {
        &self.approvals
    }
    /// The single storage owner. Reading it never reopens the store.
    pub fn store(&self) -> &Arc<Mutex<Store>> {
        &self.store
    }
    pub fn decision_buffer_stats(&self) -> Result<(usize, u64)> {
        let store = self
            .store
            .try_lock()
            .map_err(|_| err(ErrorCode::Conflict, "decision store busy"))?;
        Ok(store.decision_buffer_stats())
    }

    /// Appends one decision-log entry, graded by the storage owner. Critical
    /// kinds return the storage error on failure; telemetry may be buffered.
    pub fn record(
        &mut self,
        kind: DecisionKind,
        session: Option<&str>,
        payload: &Value,
        trace_id: &str,
    ) -> Result<DecisionAck> {
        let operation = short()?;
        let mut store = self
            .store
            .try_lock()
            .map_err(|_| err(ErrorCode::Conflict, "decision store busy"))?;
        store
            .append_decision(
                &operation,
                &self.workspace,
                session,
                kind,
                payload,
                trace_id,
            )
            .map_err(RuntimeError::from)
    }

    /// Session control. Mute/Takeover mute the media side first (before the
    /// database), then revoke the session (epoch increments), then log an
    /// explicit cancellation. The returned epoch reflects the state change only;
    /// it is not evidence that a device stopped.
    pub fn control(&mut self, action: ControlAction) -> Result<u64> {
        self.gate.emergency().mute();
        let epoch = {
            let mut session = self.session_lock_within(SESSION_REVOKE_BUDGET)?;
            session.revoke(true).map_err(RuntimeError::from)?;
            session.snapshot().generation_epoch
        };
        self.duty_active = false;
        self.duty_expiry_logged = false;
        let session = self.session_id.clone();
        let trace = self.next_trace("control");
        self.record(
            DecisionKind::Cancellation,
            Some(&session),
            &json!({
                "action": action.as_str(),
                "epoch": epoch,
                "note": "state ack is not a hardware stop",
            }),
            &trace,
        )?;
        Ok(epoch)
    }

    /// Operator-only duty grant (V3.6 §21.4). Recorded as critical.
    pub fn confirm_duty(&mut self, operator: &str, ttl: Duration, now: Instant) -> Result<()> {
        let epoch = {
            let mut session = self.session_lock()?;
            session
                .confirm_duty(true, operator, ttl, now)
                .map_err(RuntimeError::from)?;
            session.snapshot().generation_epoch
        };
        self.duty_active = true;
        self.duty_expiry_logged = false;
        let session = self.session_id.clone();
        let trace = self.next_trace("duty");
        self.record(
            DecisionKind::DutyLease,
            Some(&session),
            &json!({
                "event": DutyEvent::Granted.as_str(),
                "operator": operator,
                "ttl_ms": ttl.as_millis() as u64,
                "epoch": epoch,
            }),
            &trace,
        )?;
        Ok(())
    }

    /// Applies duty expiry. On the halt/revoke path the media side is muted first,
    /// then the session enforces revocation, then the transition is recorded as a
    /// critical `duty_lease` entry. Log failure is returned, never swallowed.
    pub fn enforce_duty(&mut self, now: Instant) -> Result<DutyEnforcement> {
        if !self.duty_active {
            return Ok(DutyEnforcement::Valid);
        }
        if self.session_lock()?.duty_valid(now) {
            return Ok(DutyEnforcement::Valid);
        }
        self.gate.emergency().mute();
        let (enforcement, epoch) = {
            let mut session = self.session_lock_within(SESSION_REVOKE_BUDGET)?;
            let enforcement = session.enforce_duty(now);
            (enforcement, session.snapshot().generation_epoch)
        };
        let event = match enforcement {
            DutyEnforcement::Valid => return Ok(DutyEnforcement::Valid),
            DutyEnforcement::HaltedPendingRevoke => DutyEvent::Expired,
            DutyEnforcement::Revoked => DutyEvent::Revoked,
        };
        if event == DutyEvent::Expired && self.duty_expiry_logged {
            return Ok(enforcement);
        }
        let session = self.session_id.clone();
        let trace = self.next_trace("duty");
        self.record(
            DecisionKind::DutyLease,
            Some(&session),
            &json!({"event": event.as_str(), "epoch": epoch}),
            &trace,
        )?;
        match event {
            DutyEvent::Expired => self.duty_expiry_logged = true,
            _ => {
                self.duty_active = false;
                self.duty_expiry_logged = false;
            }
        }
        Ok(enforcement)
    }

    /// Periodic duty driver. Identical to [`Self::enforce_duty`]; kept as the
    /// named tick the engine will call.
    pub fn tick_duty(&mut self, now: Instant) -> Result<DutyEnforcement> {
        self.enforce_duty(now)
    }

    /// Builds a bounded rehearsal lease for the supervised keeper. Production
    /// playback leases require approvals and facts and are not produced here.
    pub fn rehearsal_lease(
        &self,
        lease_id: &str,
        utterance_ids: &[String],
        epoch: u64,
        now_utc_ms: u64,
    ) -> Result<PlaybackLease> {
        if lease_id.is_empty() || utterance_ids.is_empty() || epoch == 0 {
            return Err(err(
                ErrorCode::InvalidArgument,
                "rehearsal lease requires a lease id, epoch and at least one utterance",
            ));
        }
        let ttl = self.lease_expire_ms.clamp(1, MAX_LEASE_MS);
        let expires = now_utc_ms
            .checked_add(ttl)
            .ok_or_else(|| err(ErrorCode::InvalidArgument, "lease expiry overflow"))?;
        let session_epoch = self.session_lock()?.snapshot().generation_epoch;
        if epoch < session_epoch {
            return Err(err(
                ErrorCode::StaleEpoch,
                "rehearsal lease epoch precedes the live session epoch",
            ));
        }
        Ok(PlaybackLease {
            lease_id: lease_id.into(),
            session_id: self.session_id.clone(),
            generation_epoch: epoch,
            allowed_utterance_ids: utterance_ids.to_vec(),
            mode: SessionMode::Rehearsal as i32,
            expires_at_utc_ms: expires,
            policy_revision: "rehearsal".into(),
            ..Default::default()
        })
    }

    /// Ordered fault handling for a supervised media child. Detects the fault,
    /// mutes media first, revokes output authority (epoch increments), records a
    /// critical `cancellation` entry and puts the supervisor on standby. The log
    /// failure is reported but never rolls back the mute or the revoke.
    pub fn observe(&mut self, supervisor: &mut Supervisor) -> Result<Option<ObservedFault>> {
        let Some(fault) = supervisor.detect_fault() else {
            return Ok(None);
        };
        self.gate.emergency().mute();
        let epoch = {
            let mut session = self.session_lock_within(SESSION_REVOKE_BUDGET)?;
            session.revoke(true).map_err(RuntimeError::from)?;
            session.snapshot().generation_epoch
        };
        self.duty_active = false;
        self.duty_expiry_logged = false;
        let session = self.session_id.clone();
        let trace = self.next_trace("supervisor");
        let payload = json!({
            "child": fault.child,
            "exit": fault.exit_text(),
            "epoch": epoch,
            "reason": fault.reason.as_str(),
            "detail": fault.detail.clone(),
        });
        let logged = self.record(DecisionKind::Cancellation, Some(&session), &payload, &trace);
        // Standby is set regardless: delivery must not resume even if the log failed.
        supervisor.set_standby(fault.reason);
        let ack = logged?;
        Ok(Some(ObservedFault { fault, ack }))
    }

    /// The **one** trace-id generator for this composition. Widened to
    /// `pub(crate)` so the observation surface reuses it rather than minting a
    /// second, drifting format (ADR 0027, one definition).
    pub(crate) fn next_trace(&mut self, prefix: &str) -> String {
        self.trace_seq += 1;
        format!("{prefix}-{}", self.trace_seq)
    }
}

/// Trusted owner administration for a fresh data root. Creating the workspace
/// and granting permissions must never be reachable from a model/plugin path.
fn bootstrap_owner(
    store: &mut Store,
    operation: &Operation,
    workspace: &str,
    actor: &str,
    profile: &str,
) -> Result<()> {
    store
        .create_workspace(operation, workspace, workspace)
        .map_err(RuntimeError::from)?;
    for (permission, resource) in [
        ("config.read", profile),
        ("config.write", profile),
        ("approval.read", "*"),
        ("approval.write", "*"),
        ("session.manage", "*"),
    ] {
        store
            .set_permission(operation, workspace, actor, permission, resource, true)
            .map_err(RuntimeError::from)?;
    }
    Ok(())
}

/// One short read-or-write operation for a call that touches rows. Widened to
/// `pub(crate)` so the assembly-log read surface reuses this budget rather than
/// minting a second, drifting one (ADR 0027, one definition).
pub(crate) fn short() -> Result<Operation> {
    Operation::new(SHORT_OPERATION)
        .map_err(|_| err(ErrorCode::Internal, "operation budget rejected"))
}

/// The bring-up budget, for work that includes the schema migration chain. The
/// engine's `--provision` reuses this constant rather than holding its own copy.
fn bringup() -> Result<Operation> {
    Operation::new(BRINGUP_OPERATION)
        .map_err(|_| err(ErrorCode::Internal, "bring-up budget rejected"))
}

/// Opens the store for a bring-up and hands back the operation that bounded it.
/// The budget decision and the open live here together so a test can exercise the
/// path `Composition::prepare` actually uses, instead of pinning only the
/// constants and leaving a good constant wired to a wrong call site.
fn open_bringup_store(paths: DataPaths) -> Result<(Store, Operation)> {
    let operation = bringup()?;
    let store = Store::open(paths, &operation).map_err(RuntimeError::from)?;
    Ok((store, operation))
}

fn utc_ms() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_millis().min(u64::MAX as u128) as u64)
        .map_err(|_| err(ErrorCode::Internal, "invalid UTC clock"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The budgets are a policy table, so every value **and** every relation is
    /// written down. A later edit that quietly shrinks the bring-up budget back
    /// to the per-call one, or grows it past the security ceiling, must fail here
    /// instead of surfacing as an unexplained deadline on a loaded host.
    #[test]
    fn the_lock_and_operation_budgets_match_the_policy_table() {
        assert_eq!(SHORT_OPERATION, Duration::from_secs(5));
        assert_eq!(BRINGUP_OPERATION, Duration::from_secs(30));
        assert_eq!(SESSION_REVOKE_BUDGET, Duration::from_millis(250));
        assert_eq!(SESSION_POLL_BUDGET, Duration::from_millis(5));
        // A bring-up that runs the migration chain must not be squeezed into the
        // per-call budget; the poll must not outlive a watch tick; and neither
        // may exceed the ADR 0004 ceiling.
        assert!(BRINGUP_OPERATION > SHORT_OPERATION);
        assert!(SESSION_POLL_BUDGET <= SESSION_REVOKE_BUDGET);
        assert!(SESSION_REVOKE_BUDGET <= BRINGUP_OPERATION);
        // `Operation::new` really does refuse more than the ceiling, so the bound
        // above is the working limit and not a coincidence of this table.
        assert!(Operation::new(Duration::from_secs(31)).is_err());
        assert!(Operation::new(BRINGUP_OPERATION).is_ok());
        // And the two constructors really do carry the two budgets, so a body
        // swapped between them fails here even when the constants are untouched.
        let bringup_left = bringup().unwrap().remaining();
        let short_left = short().unwrap().remaining();
        assert!(bringup_left > short_left);
        assert!(bringup_left >= BRINGUP_OPERATION - Duration::from_secs(1));
        assert!(short_left <= SHORT_OPERATION);
    }

    /// The table above cannot catch a correct constant wired to the wrong call
    /// site, so the bring-up open is exercised through the very function
    /// `Composition::prepare` calls and the operation it was given is measured.
    /// A fresh data root means the full migration chain really runs here.
    #[test]
    fn the_bring_up_open_is_bounded_by_the_bring_up_budget() {
        let dir = tempfile::tempdir().unwrap();
        let (store, operation) = open_bringup_store(DataPaths::at(dir.path()).unwrap()).unwrap();
        assert!(
            operation.remaining() > SHORT_OPERATION,
            "the migration chain must not be squeezed into the per-call budget"
        );
        drop(store);
    }
}
