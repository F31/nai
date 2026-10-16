//! Trusted credential-owner boundary. Native process sandboxing belongs to D05.
use nai_contracts::v1::ErrorCode;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityError {
    pub code: ErrorCode,
}
impl fmt::Display for SecurityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "security: {}", self.code.as_str_name())
    }
}
impl std::error::Error for SecurityError {}
fn err(code: ErrorCode) -> SecurityError {
    SecurityError { code }
}

/// A local cooperative deadline, never sent as an absolute monotonic timestamp.
#[derive(Clone)]
pub struct Operation {
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
}
impl Operation {
    /// The ADR 0004 ceiling: `new` rejects anything longer, so this value is both the
    /// largest acceptable budget and the number a caller passes when it wants "the
    /// whole ceiling" -- which is what a one-shot bring-up (a fresh data root plus the
    /// full schema migration chain) and an upgrade rehearsal actually do.
    ///
    /// Published so a caller can **reference** the ceiling instead of repeating the
    /// number: a second literal would let the two drift apart without anything
    /// noticing. `new` reads this constant, so "the ceiling" and "the rejection
    /// threshold" can never disagree.
    pub const CEILING: Duration = Duration::from_secs(30);

    pub fn new(timeout: Duration) -> Result<Self, SecurityError> {
        if timeout > Self::CEILING {
            return Err(err(ErrorCode::InvalidArgument));
        }
        Ok(Self {
            deadline: Instant::now() + timeout,
            cancelled: Arc::new(AtomicBool::new(false)),
        })
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn check(&self) -> Result<(), SecurityError> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(err(ErrorCode::Cancelled));
        }
        if Instant::now() >= self.deadline {
            return Err(err(ErrorCode::DeadlineExceeded));
        }
        Ok(())
    }
    pub fn stopped(&self) -> bool {
        self.check().is_err()
    }
    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CredentialRef(String);
impl CredentialRef {
    pub fn parse(value: &str) -> Result<Self, SecurityError> {
        let id = value
            .strip_prefix("credential:")
            .ok_or_else(|| err(ErrorCode::InvalidArgument))?;
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
        {
            return Err(err(ErrorCode::InvalidArgument));
        }
        Ok(Self(value.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub struct Secret(Zeroizing<Vec<u8>>);
impl Secret {
    pub fn new(bytes: Vec<u8>) -> Result<Self, SecurityError> {
        let bytes = Zeroizing::new(bytes);
        if bytes.is_empty() || bytes.len() > 2560 {
            return Err(err(ErrorCode::InvalidArgument));
        }
        Ok(Self(bytes))
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}
impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([REDACTED])")
    }
}

pub trait VaultBackend {
    fn store(&self, reference: &CredentialRef, secret: &Secret) -> Result<(), SecurityError>;
    fn load(&self, reference: &CredentialRef) -> Result<Secret, SecurityError>;
    fn delete(&self, reference: &CredentialRef) -> Result<(), SecurityError>;
}

pub struct NativeVault {
    workspace: String,
}
impl NativeVault {
    pub fn new(workspace: &str) -> Result<Self, SecurityError> {
        if workspace.is_empty()
            || workspace.len() > 128
            || !workspace
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
        {
            return Err(err(ErrorCode::InvalidArgument));
        }
        Ok(Self {
            workspace: workspace.into(),
        })
    }
    pub fn workspace(&self) -> &str {
        &self.workspace
    }
    #[cfg(windows)]
    fn entry(&self, reference: &CredentialRef) -> Result<keyring::Entry, SecurityError> {
        keyring::Entry::new(&format!("nai/{}", self.workspace), reference.as_str())
            .map_err(native_error)
    }
}
#[cfg(windows)]
fn native_error(error: keyring::Error) -> SecurityError {
    // Never embed provider errors, usernames or secret contents in diagnostics.
    err(match error {
        keyring::Error::NoEntry => ErrorCode::CapabilityAbsent,
        _ => ErrorCode::CapabilityNotReady,
    })
}
impl VaultBackend for NativeVault {
    fn store(&self, _reference: &CredentialRef, _secret: &Secret) -> Result<(), SecurityError> {
        #[cfg(windows)]
        {
            self.entry(_reference)?
                .set_secret(_secret.as_bytes())
                .map_err(native_error)
        }
        #[cfg(not(windows))]
        {
            Err(err(ErrorCode::CapabilityNotReady))
        }
    }
    fn load(&self, _reference: &CredentialRef) -> Result<Secret, SecurityError> {
        #[cfg(windows)]
        {
            Secret::new(self.entry(_reference)?.get_secret().map_err(native_error)?)
        }
        #[cfg(not(windows))]
        {
            Err(err(ErrorCode::CapabilityNotReady))
        }
    }
    fn delete(&self, _reference: &CredentialRef) -> Result<(), SecurityError> {
        #[cfg(windows)]
        {
            self.entry(_reference)?
                .delete_credential()
                .map_err(native_error)
        }
        #[cfg(not(windows))]
        {
            Err(err(ErrorCode::CapabilityNotReady))
        }
    }
}

/// Owner methods belong only to trusted composition/configuration code. The
/// authenticated principal must be supplied by the host, never model payloads.
pub struct CredentialBroker<B: VaultBackend> {
    backend: B,
    grants: BTreeMap<String, BTreeSet<CredentialRef>>,
}
impl<B: VaultBackend> CredentialBroker<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            grants: BTreeMap::new(),
        }
    }
    pub fn set_grant(
        &mut self,
        principal: &str,
        references: impl IntoIterator<Item = CredentialRef>,
    ) {
        self.grants
            .insert(principal.into(), references.into_iter().collect());
    }
    pub fn revoke(&mut self, principal: &str) {
        self.grants.remove(principal);
    }
    pub fn provision(
        &self,
        op: &Operation,
        reference: &CredentialRef,
        secret: &Secret,
    ) -> Result<(), SecurityError> {
        op.check()?;
        self.backend.store(reference, secret)?;
        op.check()
    }
    pub fn delete(&self, op: &Operation, reference: &CredentialRef) -> Result<(), SecurityError> {
        op.check()?;
        self.backend.delete(reference)?;
        op.check()
    }
    pub fn read(
        &self,
        op: &Operation,
        principal: &str,
        reference: &CredentialRef,
    ) -> Result<Secret, SecurityError> {
        op.check()?;
        if !self
            .grants
            .get(principal)
            .is_some_and(|allowed| allowed.contains(reference))
        {
            return Err(err(ErrorCode::PermissionDenied));
        }
        let secret = self.backend.load(reference)?;
        op.check()?;
        Ok(secret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    #[derive(Default)]
    struct TestVault {
        values: RefCell<BTreeMap<CredentialRef, Vec<u8>>>,
        reads: Cell<u32>,
    }
    impl VaultBackend for TestVault {
        fn store(&self, r: &CredentialRef, s: &Secret) -> Result<(), SecurityError> {
            self.values
                .borrow_mut()
                .insert(r.clone(), s.as_bytes().to_vec());
            Ok(())
        }
        fn load(&self, r: &CredentialRef) -> Result<Secret, SecurityError> {
            self.reads.set(self.reads.get() + 1);
            Secret::new(
                self.values
                    .borrow()
                    .get(r)
                    .cloned()
                    .ok_or_else(|| err(ErrorCode::CapabilityAbsent))?,
            )
        }
        fn delete(&self, r: &CredentialRef) -> Result<(), SecurityError> {
            self.values.borrow_mut().remove(r);
            Ok(())
        }
    }
    fn op() -> Operation {
        Operation::new(Duration::from_secs(5)).unwrap()
    }
    /// The ceiling is an **external** fact (ADR 0004), so the number is written down
    /// here on purpose. Asserting only `new(CEILING).is_ok()` would be
    /// self-referential: shrink the constant and the rejection threshold shrinks with
    /// it, so the check would still pass while the real bound had moved.
    #[test]
    fn ceiling_is_the_adr_0004_bound_and_the_rejection_threshold() {
        assert_eq!(Operation::CEILING, Duration::from_secs(30));
        assert!(Operation::new(Operation::CEILING).is_ok());
        assert_eq!(
            Operation::new(Operation::CEILING + Duration::from_millis(1))
                .err()
                .map(|error| error.code),
            Some(ErrorCode::InvalidArgument)
        );
    }
    #[test]
    fn references_and_debug_never_expose_secret() {
        for bad in [
            "sk-test",
            "credential:",
            "credential:../x",
            "credential:x/y",
        ] {
            assert!(CredentialRef::parse(bad).is_err());
        }
        let secret = Secret::new(b"test-only-sensitive-bytes".to_vec()).unwrap();
        assert!(!format!("{secret:?}").contains("sensitive"));
        assert!(Secret::new(vec![1; 2561]).is_err());
    }
    #[test]
    fn broker_checks_current_grants_before_touching_backend() {
        let reference = CredentialRef::parse("credential:test").unwrap();
        let mut broker = CredentialBroker::new(TestVault::default());
        broker
            .provision(
                &op(),
                &reference,
                &Secret::new(b"test-only".to_vec()).unwrap(),
            )
            .unwrap();
        assert_eq!(
            broker.read(&op(), "other", &reference).unwrap_err().code,
            ErrorCode::PermissionDenied
        );
        assert_eq!(broker.backend.reads.get(), 0);
        broker.set_grant("provider", [reference.clone()]);
        assert_eq!(
            broker
                .read(&op(), "provider", &reference)
                .unwrap()
                .as_bytes(),
            b"test-only"
        );
        broker.revoke("provider");
        assert_eq!(
            broker.read(&op(), "provider", &reference).unwrap_err().code,
            ErrorCode::PermissionDenied
        );
        assert_eq!(broker.backend.reads.get(), 1);
    }
    #[test]
    fn cancelled_or_expired_operations_do_not_call_backend() {
        let reference = CredentialRef::parse("credential:test").unwrap();
        let mut broker = CredentialBroker::new(TestVault::default());
        broker.set_grant("provider", [reference.clone()]);
        let cancelled = op();
        cancelled.cancel();
        assert_eq!(
            broker
                .read(&cancelled, "provider", &reference)
                .unwrap_err()
                .code,
            ErrorCode::Cancelled
        );
        assert_eq!(
            broker
                .read(
                    &Operation::new(Duration::ZERO).unwrap(),
                    "provider",
                    &reference
                )
                .unwrap_err()
                .code,
            ErrorCode::DeadlineExceeded
        );
        assert_eq!(broker.backend.reads.get(), 0);
    }
    #[test]
    #[cfg(not(windows))]
    fn production_backend_is_explicitly_unavailable() {
        let vault = NativeVault::new("test").unwrap();
        let r = CredentialRef::parse("credential:test").unwrap();
        assert_eq!(
            vault.load(&r).unwrap_err().code,
            ErrorCode::CapabilityNotReady
        );
        assert_eq!(
            vault
                .store(&r, &Secret::new(b"test-only".to_vec()).unwrap())
                .unwrap_err()
                .code,
            ErrorCode::CapabilityNotReady
        );
    }
    #[test]
    #[cfg(windows)]
    fn native_windows_roundtrip_and_cleanup() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let vault = NativeVault::new(&format!("test-{}-{nonce}", std::process::id())).unwrap();
        let r = CredentialRef::parse("credential:test-only").unwrap();
        let result = (|| {
            vault.store(&r, &Secret::new(b"not-a-production-secret".to_vec())?)?;
            let secret = vault.load(&r)?;
            assert_eq!(secret.as_bytes(), b"not-a-production-secret");
            Ok::<_, SecurityError>(())
        })();
        let cleanup = vault.delete(&r);
        result.unwrap();
        cleanup.unwrap();
        assert_eq!(
            vault.load(&r).unwrap_err().code,
            ErrorCode::CapabilityAbsent
        );
    }

    // W121：Operation 有界预算的**运行期终止语义**闭世界守卫
    // nai-security/src/lib.rs:46-72。既有测试只钉了**构造期**（`new(CEILING).is_ok()` 与越界被拒），
    // 运行期终止语义整段零判据：
    //   (a) `check()` **先判 cancel 再判 deadline** —— 既取消又超期的 Operation MUST 报
    //       `Cancelled`（而非 `DeadlineExceeded`）；否则上层把「主动取消」误读成「超时」；
    //   (b) `remaining()` 单调不增（不会因等待而变长），超期后为 0；
    //   (c) `stopped()` MUST 与 `check().is_err()` **严格一致**（判据与读取面不得分家）；
    //   (d) `cancel()` 经 `Arc<AtomicBool>` 共享 —— 任一克隆句柄上的取消 MUST 让全部句柄都 stopped。
    #[test]
    fn check_reports_cancellation_before_the_deadline() {
        // (a) 先确认「超期」条件成立，再确认 cancel 覆盖它。
        let expired = Operation::new(Duration::ZERO).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(
            expired.remaining(),
            Duration::ZERO,
            "a zero budget must already be exhausted"
        );
        assert_eq!(
            expired.check().unwrap_err().code,
            ErrorCode::DeadlineExceeded
        );
        expired.cancel();
        assert_eq!(
            expired.check().unwrap_err().code,
            ErrorCode::Cancelled,
            "cancellation MUST take precedence over the deadline"
        );
        // 仅取消（预算充足）时同样是 Cancelled。
        let cancelled = Operation::new(Duration::from_secs(5)).unwrap();
        cancelled.cancel();
        assert_eq!(cancelled.check().unwrap_err().code, ErrorCode::Cancelled);
    }

    #[test]
    fn remaining_never_grows_as_time_passes() {
        // (b) 预算单调不增：等待后剩余 MUST 严格变少，且不超过初始预算。
        let op = Operation::new(Duration::from_secs(5)).unwrap();
        let first = op.remaining();
        std::thread::sleep(Duration::from_millis(120));
        let second = op.remaining();
        assert!(first <= Duration::from_secs(5));
        assert!(
            second < first,
            "the remaining budget MUST shrink as time passes: {second:?} vs {first:?}"
        );
    }

    #[test]
    fn stopped_is_exactly_check_failure() {
        // (c) stopped() 与 check().is_err() 在 live/cancelled/expired 三种情形下都必须一致。
        let live = Operation::new(Duration::from_secs(5)).unwrap();
        assert!(!live.stopped());
        assert_eq!(live.stopped(), live.check().is_err());

        let cancelled = Operation::new(Duration::from_secs(5)).unwrap();
        cancelled.cancel();
        assert!(cancelled.stopped());
        assert_eq!(cancelled.stopped(), cancelled.check().is_err());

        let expired = Operation::new(Duration::ZERO).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        assert!(expired.stopped());
        assert_eq!(
            expired.stopped(),
            expired.check().is_err(),
            "stopped() MUST NOT drift from check().is_err()"
        );
    }

    #[test]
    fn cancellation_is_shared_across_cloned_handles() {
        // (d) cancel() 经 Arc 共享：克隆句柄 MUST 观测到同一停止状态
        // （若改成每个句柄独立的 bool，本断言会红）。
        let op = Operation::new(Duration::from_secs(5)).unwrap();
        let clone = op.clone();
        assert!(!clone.stopped(), "a live clone is not stopped");
        op.cancel();
        assert!(
            clone.stopped(),
            "cancellation MUST be shared: the clone observes the cancel"
        );
        assert!(op.stopped() && clone.stopped());
    }
}
