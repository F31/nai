//! Pure RF02 policy. No actor/approval/guard decision is accepted from a model.
use nai_contracts::workbench::{
    DeliveryScope, FoundationRole, FoundationTarget, WorkbenchError as Error,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, Instant};
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone)]
pub struct Operation {
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    ancestors: Vec<Arc<AtomicBool>>,
}
impl Operation {
    pub fn new(ttl_ms: u32) -> Result<Self> {
        Self::bounded(ttl_ms, 4000)
    }
    /// Explicit model I/O budget; never use this constructor for store/control work.
    pub fn model_task(ttl_ms: u32) -> Result<Self> {
        Self::bounded(ttl_ms, 45000)
    }
    fn bounded(ttl_ms: u32, maximum: u32) -> Result<Self> {
        if !(1..=maximum).contains(&ttl_ms) {
            return Err(Error::InvalidArgument);
        }
        Ok(Self {
            deadline: Instant::now() + Duration::from_millis(ttl_ms.into()),
            cancelled: Arc::new(AtomicBool::new(false)),
            ancestors: Vec::new(),
        })
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    /// Child cancellation is local; ancestor cancellation and deadlines cannot be reset.
    pub fn child(&self, ttl_ms: u32) -> Result<Self> {
        self.child_bounded(ttl_ms, 4000)
    }
    pub fn model_child(&self, ttl_ms: u32) -> Result<Self> {
        self.child_bounded(ttl_ms, 45000)
    }
    fn child_bounded(&self, ttl_ms: u32, maximum: u32) -> Result<Self> {
        self.check()?;
        if self.ancestors.len() >= 8 {
            return Err(Error::ResourceExhausted);
        }
        let mut child = Self::bounded(ttl_ms, maximum)?;
        child.deadline = child.deadline.min(self.deadline);
        child.ancestors = self.ancestors.clone();
        child.ancestors.push(self.cancelled.clone());
        Ok(child)
    }
    /// Remaining monotonic budget, rounded down so a child cannot gain time.
    pub fn remaining_ms(&self) -> Result<u32> {
        self.check()?;
        let remaining = self
            .deadline
            .saturating_duration_since(Instant::now())
            .as_millis();
        if remaining == 0 {
            return Err(Error::DeadlineExceeded);
        }
        u32::try_from(remaining).map_err(|_| Error::InvalidArgument)
    }
    pub fn check(&self) -> Result<()> {
        if self.cancelled.load(Ordering::Acquire)
            || self.ancestors.iter().any(|v| v.load(Ordering::Acquire))
        {
            Err(Error::Cancelled)
        } else if Instant::now() >= self.deadline {
            Err(Error::DeadlineExceeded)
        } else {
            Ok(())
        }
    }
}
#[derive(Default)]
pub struct StopFence {
    revoked: std::sync::RwLock<std::collections::BTreeSet<String>>,
    stopped: AtomicBool,
    generation: AtomicU64,
}
impl StopFence {
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.generation.fetch_add(1, Ordering::SeqCst);
    }
    /// Ordinary scoped revocation never holds a lock during I/O; P0 stop remains atomic.
    pub fn revoke_scope(&self, key: &str) -> Result<()> {
        let mut set = self.revoked.write().map_err(|_| Error::PermissionDenied)?;
        if set.len() >= 256 && !set.contains(key) {
            self.stop();
            return Err(Error::ResourceExhausted);
        }
        set.insert(key.into());
        Ok(())
    }
    pub fn check_scope(&self, key: &str) -> Result<()> {
        if self
            .revoked
            .read()
            .map_err(|_| Error::PermissionDenied)?
            .contains(key)
        {
            Err(Error::PermissionDenied)
        } else {
            Ok(())
        }
    }
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }
    /// Bounded sorted scope watermark; cloning never holds this lock during I/O.
    pub fn revocation_scopes(&self) -> Result<Vec<String>> {
        Ok(self
            .revoked
            .read()
            .map_err(|_| Error::PermissionDenied)?
            .iter()
            .cloned()
            .collect())
    }
    pub fn check(&self, expected: u64) -> Result<()> {
        if self.stopped.load(Ordering::Acquire) || expected != self.generation() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
}
pub fn uuid(id: &str) -> Result<()> {
    if id.len() != 36
        || !id.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
    {
        Err(Error::InvalidArgument)
    } else {
        Ok(())
    }
}
pub fn manual_guard(body: &str) -> Result<()> {
    if body.trim().is_empty()
        || body.len() > 8192
        || body
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        Err(Error::InvalidArgument)
    } else {
        Ok(())
    }
}
pub struct GateInput<'a> {
    pub role_scope: &'a str,
    pub object_scope: &'a str,
    pub role: FoundationRole,
    pub expected_role_revision: u64,
    pub current_role_revision: u64,
    pub epoch: u64,
    pub current_epoch: u64,
    pub stopped: bool,
    pub object_revision: u64,
    pub current_object_revision: u64,
    pub object_sha256: &'a str,
    pub actual_sha256: &'a str,
    pub bundle_sha256: &'a str,
    pub approved_sha256: Option<&'a str>,
    pub target: FoundationTarget,
    pub scope: DeliveryScope,
    pub expires_tick: u64,
    pub fence_generation: u64,
    pub body: &'a str,
}
/// A grant is process-local and cannot survive restart or be used by another target.
pub struct Permit {
    generation: u64,
    expires_tick: u64,
    hash: String,
}
impl Permit {
    pub fn recheck(&self, fence: &StopFence, now: u64, hash: &str) -> Result<()> {
        fence.check(self.generation)?;
        if now >= self.expires_tick {
            Err(Error::DeadlineExceeded)
        } else if hash != self.hash {
            Err(Error::IntegrityFailed)
        } else {
            Ok(())
        }
    }
}
pub fn delivery_gate(
    i: &GateInput<'_>,
    fence: &StopFence,
    now: u64,
    require_approval: bool,
) -> Result<Permit> {
    fence.check(i.fence_generation)?;
    fence.check_scope(i.role_scope)?;
    fence.check_scope(i.object_scope)?;
    if !matches!(i.role, FoundationRole::Admin | FoundationRole::Operator)
        || i.expected_role_revision != i.current_role_revision
    {
        return Err(Error::PermissionDenied);
    }
    if i.stopped || i.epoch != i.current_epoch {
        return Err(Error::Cancelled);
    }
    if i.target != FoundationTarget::PrivateCard || i.scope != DeliveryScope::Private {
        return Err(Error::CapabilityUnavailable);
    }
    if now >= i.expires_tick {
        return Err(Error::DeadlineExceeded);
    }
    if i.object_revision != i.current_object_revision || i.object_sha256 != i.actual_sha256 {
        return Err(Error::RevisionConflict);
    }
    // Trusted deterministic input/evidence/output checks for local manually authored text only.
    manual_guard(i.body)?;
    if i.object_sha256.len() != 64 || i.bundle_sha256.len() != 64 {
        return Err(Error::IntegrityFailed);
    }
    if require_approval && i.approved_sha256 != Some(i.bundle_sha256) {
        return Err(Error::PermissionDenied);
    }
    Ok(Permit {
        generation: i.fence_generation,
        expires_tick: i.expires_tick,
        hash: i.bundle_sha256.into(),
    })
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Settlement {
    Known(u64),
    Unknown,
    Released,
}
/// Monetary values are currency + integer minor units. Provider calls remain RF04.
pub trait BudgetPort {
    fn reserve(
        &mut self,
        op: &Operation,
        principal: &str,
        workspace: &str,
        id: &str,
        currency: &str,
        maximum_minor: u64,
    ) -> Result<()>;
    fn settle(
        &mut self,
        op: &Operation,
        principal: &str,
        workspace: &str,
        id: &str,
        outcome: Settlement,
    ) -> Result<()>;
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_and_deadline_are_distinct() {
        let op = Operation::new(4000).unwrap();
        op.cancel();
        assert_eq!(op.check(), Err(Error::Cancelled));
        assert!(Operation::new(4001).is_err());
        let mut short = Operation::new(4000).unwrap();
        short.deadline = Instant::now();
        assert_eq!(short.check(), Err(Error::DeadlineExceeded));
    }
    #[test]
    fn stop_is_lock_free_and_old_permits_never_revive() {
        let fence = Arc::new(StopFence::default());
        let permit = Permit {
            generation: 0,
            expires_tick: 100,
            hash: "bundle".into(),
        };
        assert!(permit.recheck(&fence, 1, "bundle").is_ok());
        let worker = fence.clone();
        std::thread::spawn(move || worker.stop()).join().unwrap();
        assert_eq!(permit.recheck(&fence, 1, "bundle"), Err(Error::Cancelled));
        assert!(fence.check(fence.generation()).is_err());
    }
    #[test]
    fn malformed_ids_and_control_text_are_rejected() {
        assert!(uuid("actor-supplied-by-client").is_err());
        assert!(uuid("00000000-0000-0000-0000-000000000001").is_ok());
        assert!(manual_guard("\0").is_err());
        assert!(manual_guard(" ").is_err());
        assert!(manual_guard("人工编写的私有说明").is_ok());
    }
}

#[cfg(test)]
mod child_operation_tests {
    use super::*;
    #[test]
    fn parent_cancel_propagates_but_child_does_not_cancel_siblings() {
        let parent = Operation::new(4000).unwrap();
        let a = parent.child(4000).unwrap();
        let b = parent.child(4000).unwrap();
        let descendant = a.child(4000).unwrap();
        a.cancel();
        assert_eq!(a.check(), Err(Error::Cancelled));
        assert_eq!(descendant.check(), Err(Error::Cancelled));
        assert!(parent.check().is_ok());
        assert!(b.check().is_ok());
        parent.cancel();
        assert_eq!(b.check(), Err(Error::Cancelled));
        assert!(parent.child(4000).is_err());
    }
    #[test]
    fn child_deadline_and_depth_cannot_expand_parent_budget() {
        let parent = Operation::new(4000).unwrap();
        let child = parent.child(4000).unwrap();
        assert_eq!(child.deadline, parent.deadline);
        let mut expired = Operation::new(4000).unwrap();
        expired.deadline = Instant::now() - Duration::from_millis(1);
        assert!(matches!(expired.child(4000), Err(Error::DeadlineExceeded)));
        let mut current = Operation::new(4000).unwrap();
        for _ in 0..8 {
            current = current.child(4000).unwrap();
        }
        assert!(matches!(current.child(4000), Err(Error::ResourceExhausted)));
    }
}

#[cfg(test)]
mod model_operation_tests {
    use super::*;
    #[test]
    fn model_scope_keeps_control_bound_and_inherits_model_parent_deadline() {
        assert!(Operation::new(4001).is_err());
        assert!(Operation::model_task(45001).is_err());
        assert!(Operation::model_task(0).is_err());
        let parent = Operation::model_task(15000).unwrap();
        let child = parent.model_child(45000).unwrap();
        assert_eq!(child.deadline, parent.deadline);
        assert!(child.remaining_ms().unwrap() <= 15000);
        let sibling = parent.model_child(15000).unwrap();
        child.cancel();
        assert!(parent.check().is_ok());
        assert!(sibling.check().is_ok());
        parent.cancel();
        assert_eq!(sibling.check(), Err(Error::Cancelled));
        assert!(parent.model_child(15000).is_err());
    }
}
