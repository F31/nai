//! Upgrade verification, revocation and preflight core.
//!
//! The upgrade orchestrator owns *decisions*; every fact about the world it
//! depends on enters through an injectable seam (signature verification,
//! revocation source, disk-space probe, active-session gate). That is why the
//! crate has no host, platform-SDK or network dependency and stays in the
//! `shared` layer: a decision that hard-codes how to ask the world a question
//! stops being testable off the target machine.
//!
//! Nothing here certifies a real signature chain, a Windows install, or a real
//! downloader. `DeterministicVerifier` is a rehearsal stand-in, and a
//! `SignatureVerifier` implementation that chains to a real CA key is a
//! deliberate later injection point.
mod manifest;
mod plan;
mod preflight;
mod rehearsal;
mod revocation;
mod signature;
mod verify;

use nai_contracts::v1::ErrorCode;
use std::{
    fmt,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

pub use manifest::{FileDecl, Requirement, RuntimeEntry, UpgradeKind, UpgradePackage};
pub use plan::{
    BackupRef, ExportedWrites, Outcome, Prepared, RecoveryAction, RollbackReport, StagingId,
    UpgradeHost, UpgradeJournal, UpgradeRequest, UpgradeTransaction,
};
pub use preflight::{
    CheckResult, HostFacts, NoSpaceProbe, PreflightReport, Probe, ResourceState, SpaceProbe,
    preflight,
};
pub use rehearsal::{EffectCounts, FaultPoint, RehearsalHost, RehearsalJournal};
pub use revocation::{RevocationEntry, RevocationList, RevocationSource};
pub use signature::{DeterministicVerifier, SignatureVerifier};
pub use verify::{PackageFile, VerifiedPackage, VerifyLimits, verify_package};

pub use nai_security::Operation;
pub use nai_storage::{UpgradeRecord, UpgradeStep};

/// A refusal or a failure. `reason` is a stable, portable token (never a
/// localised OS string) so a report can classify outcomes without a debug build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeError {
    pub code: ErrorCode,
    pub reason: &'static str,
}
impl fmt::Display for UpgradeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "upgrade: {} ({})", self.code.as_str_name(), self.reason)
    }
}
impl std::error::Error for UpgradeError {}
impl From<nai_storage::StorageError> for UpgradeError {
    fn from(storage_error: nai_storage::StorageError) -> Self {
        error(storage_error.code, storage_error.reason)
    }
}
impl From<nai_security::SecurityError> for UpgradeError {
    fn from(security_error: nai_security::SecurityError) -> Self {
        error(security_error.code, "operation stopped")
    }
}
pub type Result<T> = std::result::Result<T, UpgradeError>;

pub(crate) fn error(code: ErrorCode, reason: &'static str) -> UpgradeError {
    UpgradeError { code, reason }
}

/// Rejects work once the deadline passes or cancellation is signalled. Every
/// entry point, per-file loop and exit checks through here, so a cancelled or
/// expired run cannot produce a success value or a partial staging.
pub(crate) fn checkpoint(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Acquire) {
        return Err(error(ErrorCode::Cancelled, "upgrade work cancelled"));
    }
    if Instant::now() >= deadline {
        return Err(error(ErrorCode::DeadlineExceeded, "upgrade work deadline"));
    }
    Ok(())
}
