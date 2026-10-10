//! The upgrade transaction: journalled, atomic and crash-recoverable.
//!
//! The core owns the *order* of steps and the *decisions* between them. Every
//! world effect and every fact about the world enters through [`UpgradeHost`],
//! and the record of how far a transaction got enters through
//! [`UpgradeJournal`]. That keeps the transaction off the filesystem, off the
//! platform and off a session registry, so it runs — and is tested — on any
//! machine, and a real installer is a later injection rather than a rewrite.
//!
//! The journal is **write-ahead**: a step is recorded *before* the effect it
//! describes. A crash therefore leaves a record at most one step ahead of
//! reality, and recovery always converges *backwards* to a consistent state
//! (discard the staging, or restore the backup). Recording after the fact would
//! let a crash mid-migration leave a `prepared` record over a half-migrated
//! tree — the one state from which nothing can be safely restored.
//!
//! ## Shape
//!
//! ```text
//! prepare : verify → preflight → stage            → journal(prepared)
//! activate: backup → journal(migrated) → migrate
//!           → self-check → journal(checked) → swap → journal(committed)
//! recover : read journal → discard | restore | confirm-committed
//! ```
//!
//! Activation is a separate call from preparation on purpose: a live session may
//! permit staging but not activation, and the caller — not this core — owns the
//! session fact. The gate that decides when to call `activate` is a later slice.
use crate::{
    Result, UpgradeError, error,
    preflight::{HostFacts, PreflightReport, Probe, SpaceProbe, preflight},
    revocation::RevocationList,
    signature::SignatureVerifier,
    verify::{PackageFile, VerifiedPackage, VerifyLimits, verify_package},
};
use nai_contracts::v1::ErrorCode;
use nai_security::Operation;
use nai_storage::{Store, UpgradeRecord, UpgradeStep};
use std::{sync::atomic::AtomicBool, time::Instant};

/// Stable tokens for the journal's free-form `error_code`, used on recovery
/// paths where there is no originating error to name.
const ABANDONED: &str = "abandoned";
const RECOVERED_AFTER_MIGRATION: &str = "recovered_after_migration";
const RECOVERED_BEFORE_SWAP: &str = "recovered_before_swap";

fn bounded_token(value: &str, what: &'static str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        Err(error(ErrorCode::InvalidArgument, what))
    } else {
        Ok(())
    }
}

/// Opaque handle to a staged package. Validated so a host cannot hand back an
/// empty or unbounded reference that would later be written into the journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagingId(String);
impl StagingId {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        bounded_token(&value, "invalid staging reference")?;
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque handle to a consistency backup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupRef(String);
impl BackupRef {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        bounded_token(&value, "invalid backup reference")?;
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What a rollback exported instead of discarding: the facts and audit records
/// written since the upgrade began. Zeroes mean "nothing was written", which is
/// a claim the host can actually support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExportedWrites {
    pub facts: usize,
    pub audit: usize,
}

/// The outcome of a rollback, carried back to the caller so a failed upgrade is
/// diagnosable rather than a bare error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackReport {
    pub cause_code: ErrorCode,
    pub cause_reason: &'static str,
    pub exported: ExportedWrites,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Committed,
    RolledBack(RollbackReport),
}

/// What recovery found and did, read purely from the journal's current step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryAction {
    /// No record: nothing had begun.
    NotStarted,
    /// `prepared`: the staging was discarded and the record closed. The running
    /// version was never touched.
    Abandoned,
    /// `migrated`/`checked` without a completed swap: restored from backup,
    /// exporting post-upgrade writes.
    RolledBack(RollbackReport),
    /// A terminal success, re-entered without repeating any effect.
    AlreadyCommitted,
    /// A terminal failure, re-entered without repeating any effect.
    AlreadyRolledBack,
}

/// Everything `prepare` needs. Borrowed, so the caller keeps ownership of the
/// package bytes and of the deadline/cancellation it handed to verification.
pub struct UpgradeRequest<'a> {
    pub transaction_id: String,
    pub from_manifest: String,
    pub manifest: &'a [u8],
    pub files: Vec<PackageFile<'a>>,
    pub signature: &'a [u8],
    pub verifier: &'a dyn SignatureVerifier,
    pub revocations: &'a RevocationList,
    pub now_ms: i64,
    pub limits: VerifyLimits,
    pub deadline: Instant,
    pub cancelled: &'a AtomicBool,
}

/// The effects the transaction needs, supplied by the caller.
///
/// Every method states the contract the core relies on. The core decides *what*
/// happens and *in what order*; it never learns how a staging area, a backup or
/// a manifest is actually represented.
pub trait UpgradeHost {
    /// Facts the preflight inspects (platform, kernel, capabilities, runtime,
    /// credentials).
    fn facts(&self) -> HostFacts;
    /// Free bytes at the install target, or `Unknown` — which the preflight
    /// refuses rather than treating as room.
    fn free_bytes(&self) -> Probe;
    /// The identity (hash) of the currently installed version manifest. Recovery
    /// compares it with the recorded target to tell "the swap landed but was not
    /// journalled" apart from "the swap never happened".
    fn current_manifest(&self, op: &Operation) -> Result<String>;
    /// Materialise a verified package into a staging area. MUST be atomic: a
    /// failure leaves nothing staged. MAY be called again for the same package
    /// without duplicating bytes.
    fn stage(
        &mut self,
        op: &Operation,
        transaction_id: &str,
        package: &VerifiedPackage,
    ) -> Result<StagingId>;
    /// Remove a transaction's staging area. Called when a `prepared` transaction
    /// is abandoned. MUST be idempotent.
    fn discard_staging(&mut self, op: &Operation, transaction_id: &str) -> Result<()>;
    /// Take a consistency backup of the running version; returns a durable
    /// reference the journal records before it is needed.
    fn backup(&mut self, op: &Operation, staging: &StagingId) -> Result<BackupRef>;
    /// Migrate the data directory to the new layout.
    fn migrate(&mut self, op: &Operation, staging: &StagingId) -> Result<()>;
    /// Isolated self-check of the migrated state. A failure here is a decision
    /// to roll back, not a partial success.
    fn self_check(&mut self, op: &Operation) -> Result<()>;
    /// Atomically replace the current version manifest: write the new manifest
    /// to a fresh file, make it durable, then rename it over the current one, so
    /// a reader sees the old manifest or the new one and never a partial write.
    fn commit(&mut self, op: &Operation, staging: &StagingId, to_manifest: &str) -> Result<()>;
    /// Restore the previous program and the backup, *exporting* the facts and
    /// audit records written since the upgrade began rather than discarding
    /// them.
    fn rollback(
        &mut self,
        op: &Operation,
        backup: &BackupRef,
        cause: &'static str,
    ) -> Result<ExportedWrites>;
}

/// How far a transaction got. The core defines the trait so a test can record
/// into memory; production records into the storage journal.
pub trait UpgradeJournal {
    fn record(&mut self, op: &Operation, record: &UpgradeRecord) -> Result<()>;
    fn current(&self, op: &Operation, transaction_id: &str) -> Result<Option<UpgradeRecord>>;
}

impl UpgradeJournal for Store {
    fn record(&mut self, op: &Operation, record: &UpgradeRecord) -> Result<()> {
        self.record_upgrade(op, record).map_err(UpgradeError::from)
    }
    fn current(&self, op: &Operation, transaction_id: &str) -> Result<Option<UpgradeRecord>> {
        self.upgrade_record(op, transaction_id)
            .map_err(UpgradeError::from)
    }
}

/// A prepared, not-yet-activated transaction. Carries the identity a later
/// `activate` or `recover` needs plus the evidence that verification and
/// preflight passed.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub transaction_id: String,
    pub from_manifest: String,
    pub to_manifest: String,
    pub staging: StagingId,
    pub content_bytes: u64,
    pub preflight: PreflightReport,
}

/// Drives one transaction through the two seams. Holds no per-transaction state:
/// the identity lives in the [`Prepared`] value and in the journal record, so a
/// restarted process recovers from the journal rather than from memory.
pub struct UpgradeTransaction<'a> {
    host: &'a mut dyn UpgradeHost,
    journal: &'a mut dyn UpgradeJournal,
}

impl<'a> UpgradeTransaction<'a> {
    pub fn new(host: &'a mut dyn UpgradeHost, journal: &'a mut dyn UpgradeJournal) -> Self {
        Self { host, journal }
    }

    /// Verify, preflight and stage. Nothing is journalled until the package is
    /// staged, so a refusal here leaves no record, no staging and an untouched
    /// running version.
    pub fn prepare(&mut self, op: &Operation, request: &UpgradeRequest<'_>) -> Result<Prepared> {
        op.check()?;
        bounded_token(&request.transaction_id, "invalid transaction id")?;
        bounded_token(&request.from_manifest, "invalid current manifest identity")?;

        // 1. Verification: schema, exact content set, per-file hash, signature
        //    and revocation — all before anything is materialised.
        let verified = verify_package(
            request.manifest,
            &request.files,
            request.signature,
            request.verifier,
            request.revocations,
            request.now_ms,
            request.limits,
            request.deadline,
            request.cancelled,
        )?;
        let to_manifest = verified.manifest_hash.clone();
        if to_manifest == request.from_manifest {
            return Err(error(
                ErrorCode::InvalidArgument,
                "upgrade target equals current version",
            ));
        }
        op.check()?;

        // 2. Preflight before staging: a refusal costs no bytes and cannot leave
        //    the running version disturbed.
        let report = preflight(
            &verified.package,
            &self.host.facts(),
            &MeasuredSpace(self.host.free_bytes()),
        )?;
        if let Some(refusal) = report.refusal_error() {
            return Err(refusal);
        }
        op.check()?;

        // 3. Stage the verified content.
        let staging = self.host.stage(op, &request.transaction_id, &verified)?;
        op.check()?;

        // 4. Record `prepared`. Only now does the transaction exist.
        let base = UpgradeRecord {
            transaction_id: request.transaction_id.clone(),
            from_manifest: request.from_manifest.clone(),
            to_manifest: to_manifest.clone(),
            step: UpgradeStep::Prepared,
            backup_ref: None,
            error_code: None,
        };
        self.record_step(op, &base, UpgradeStep::Prepared, None, None)?;

        Ok(Prepared {
            transaction_id: request.transaction_id.clone(),
            from_manifest: request.from_manifest.clone(),
            to_manifest,
            staging,
            content_bytes: verified.content_bytes,
            preflight: report,
        })
    }

    /// Back up, migrate, self-check and swap the manifest. Any failure after the
    /// backup converges to `rolled_back` with the backup restored and the
    /// post-upgrade writes exported. A cancellation or deadline is propagated
    /// after that convergence, because it is the caller's stop signal rather
    /// than a business outcome.
    pub fn activate(&mut self, op: &Operation, prepared: &Prepared) -> Result<Outcome> {
        op.check()?;
        let base = UpgradeRecord {
            transaction_id: prepared.transaction_id.clone(),
            from_manifest: prepared.from_manifest.clone(),
            to_manifest: prepared.to_manifest.clone(),
            step: UpgradeStep::Prepared,
            backup_ref: None,
            error_code: None,
        };

        // The backup precedes the migration, and `migrated` is written ahead of
        // it, so the reference recovery needs is durable before migration starts.
        let backup = match self.host.backup(op, &prepared.staging) {
            Ok(backup) => backup,
            Err(cause) => {
                // Nothing was migrated, so there is nothing to restore; close the
                // record without a backup reference.
                self.record_step(
                    op,
                    &base,
                    UpgradeStep::RolledBack,
                    None,
                    Some(cause.code.as_str_name()),
                )?;
                return Err(cause);
            }
        };
        self.record_step(op, &base, UpgradeStep::Migrated, Some(&backup), None)?;

        if let Err(cause) = self.migrate_and_check(op, prepared) {
            return self.converge(op, &base, &backup, cause);
        }
        self.record_step(op, &base, UpgradeStep::Checked, None, None)?;

        if let Err(cause) = self
            .host
            .commit(op, &prepared.staging, &prepared.to_manifest)
        {
            return self.converge(op, &base, &backup, cause);
        }
        self.record_step(op, &base, UpgradeStep::Committed, None, None)?;
        Ok(Outcome::Committed)
    }

    /// Read the journal and converge to a terminal state. Idempotent: a terminal
    /// record is reported as such without repeating any effect.
    pub fn recover(&mut self, op: &Operation, transaction_id: &str) -> Result<RecoveryAction> {
        op.check()?;
        bounded_token(transaction_id, "invalid transaction id")?;
        let Some(record) = self.journal.current(op, transaction_id)? else {
            return Ok(RecoveryAction::NotStarted);
        };
        match record.step {
            // Prepared but never activated: the running version was never
            // touched, so discarding the staging is the whole recovery.
            UpgradeStep::Prepared => {
                self.host.discard_staging(op, transaction_id)?;
                self.record_step(op, &record, UpgradeStep::RolledBack, None, Some(ABANDONED))?;
                Ok(RecoveryAction::Abandoned)
            }
            // Migration may have partially run: restore from the recorded backup.
            UpgradeStep::Migrated => {
                let backup = required_backup(&record)?;
                let exported = self.host.rollback(op, &backup, RECOVERED_AFTER_MIGRATION)?;
                self.record_step(
                    op,
                    &record,
                    UpgradeStep::RolledBack,
                    None,
                    Some(RECOVERED_AFTER_MIGRATION),
                )?;
                Ok(RecoveryAction::RolledBack(RollbackReport {
                    cause_code: ErrorCode::Conflict,
                    cause_reason: RECOVERED_AFTER_MIGRATION,
                    exported,
                }))
            }
            // The swap is the commit point. If it landed, the upgrade happened
            // and only the journal record was lost; otherwise roll back.
            UpgradeStep::Checked => {
                if self.host.current_manifest(op)? == record.to_manifest {
                    self.record_step(op, &record, UpgradeStep::Committed, None, None)?;
                    Ok(RecoveryAction::AlreadyCommitted)
                } else {
                    let backup = required_backup(&record)?;
                    let exported = self.host.rollback(op, &backup, RECOVERED_BEFORE_SWAP)?;
                    self.record_step(
                        op,
                        &record,
                        UpgradeStep::RolledBack,
                        None,
                        Some(RECOVERED_BEFORE_SWAP),
                    )?;
                    Ok(RecoveryAction::RolledBack(RollbackReport {
                        cause_code: ErrorCode::Conflict,
                        cause_reason: RECOVERED_BEFORE_SWAP,
                        exported,
                    }))
                }
            }
            UpgradeStep::Committed => Ok(RecoveryAction::AlreadyCommitted),
            UpgradeStep::RolledBack => Ok(RecoveryAction::AlreadyRolledBack),
        }
    }

    fn migrate_and_check(&mut self, op: &Operation, prepared: &Prepared) -> Result<()> {
        op.check()?;
        self.host.migrate(op, &prepared.staging)?;
        op.check()?;
        self.host.self_check(op)
    }

    /// Restore, record the terminal failure, then either report the rollback as
    /// the outcome or propagate a stop signal.
    fn converge(
        &mut self,
        op: &Operation,
        base: &UpgradeRecord,
        backup: &BackupRef,
        cause: UpgradeError,
    ) -> Result<Outcome> {
        let exported = self.host.rollback(op, backup, cause.reason)?;
        self.record_step(
            op,
            base,
            UpgradeStep::RolledBack,
            None,
            Some(cause.code.as_str_name()),
        )?;
        if matches!(
            cause.code,
            ErrorCode::Cancelled | ErrorCode::DeadlineExceeded
        ) {
            return Err(cause);
        }
        Ok(Outcome::RolledBack(RollbackReport {
            cause_code: cause.code,
            cause_reason: cause.reason,
            exported,
        }))
    }

    fn record_step(
        &mut self,
        op: &Operation,
        base: &UpgradeRecord,
        step: UpgradeStep,
        backup: Option<&BackupRef>,
        error_code: Option<&str>,
    ) -> Result<()> {
        let mut next = base.clone();
        next.step = step;
        if let Some(backup) = backup {
            next.backup_ref = Some(backup.as_str().to_owned());
        }
        if let Some(error_code) = error_code {
            next.error_code = Some(error_code.to_owned());
        }
        self.journal.record(op, &next)
    }
}

fn required_backup(record: &UpgradeRecord) -> Result<BackupRef> {
    match &record.backup_ref {
        Some(reference) => BackupRef::new(reference.clone()),
        None => Err(error(
            ErrorCode::Conflict,
            "journal lacks a backup reference",
        )),
    }
}

/// Adapts a probe value back into the `SpaceProbe` the preflight consumes, so
/// the host seam speaks a plain value while the preflight keeps its trait.
struct MeasuredSpace(Probe);
impl SpaceProbe for MeasuredSpace {
    fn free_bytes(&self) -> Probe {
        self.0.clone()
    }
}
