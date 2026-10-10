//! A rehearsal host: an in-memory stand-in for the platform installer.
//!
//! The transaction core needs *some* host to run against, and the real one — a
//! Windows installer that stages a package, snapshots a data directory and swaps
//! a manifest with a rename — is a later injection point. This host stands in
//! for it so the orchestration, the journal transitions and recovery can be
//! exercised on any machine.
//!
//! It is **not** an installer. It holds no bytes and touches no disk. What it
//! does hold is the *interfaces* the core depends on — staging handles, backup
//! references, a current-manifest identity, effect counters — so a test can
//! assert the properties that matter: a failure at any step leaves no half
//! state, recovery is idempotent, and a rollback happens exactly once.
use crate::{
    Result, error,
    plan::{BackupRef, ExportedWrites, StagingId, UpgradeHost, UpgradeJournal},
    preflight::{HostFacts, Probe},
    verify::VerifiedPackage,
};
use nai_contracts::v1::ErrorCode;
use nai_security::Operation;
use nai_storage::{UpgradeRecord, UpgradeStep};
use std::collections::BTreeMap;

/// The step at which a rehearsal should fail, so each transition can be driven
/// without a real failure substrate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultPoint {
    Stage,
    Discard,
    Backup,
    Migrate,
    SelfCheck,
    Commit,
    Rollback,
}
impl FaultPoint {
    fn default_code(self) -> ErrorCode {
        match self {
            Self::Stage | Self::Backup => ErrorCode::ResourceExhausted,
            Self::Migrate | Self::SelfCheck | Self::Commit => ErrorCode::Conflict,
            Self::Discard | Self::Rollback => ErrorCode::Internal,
        }
    }
}

/// Counts of effects actually performed. A test compares these across runs to
/// prove recovery does not repeat side effects.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EffectCounts {
    pub stage: usize,
    pub discard_staging: usize,
    pub backup: usize,
    pub migrate: usize,
    pub self_check: usize,
    pub commit: usize,
    pub rollback: usize,
}

#[derive(Debug)]
pub struct RehearsalHost {
    facts: HostFacts,
    free_bytes: Probe,
    current_manifest: String,
    staging: BTreeMap<String, StagingId>,
    backups: BTreeMap<String, BackupRef>,
    staged_bytes: u64,
    swaps: Vec<String>,
    exports: ExportedWrites,
    fault: Option<FaultPoint>,
    fault_code: Option<ErrorCode>,
    counts: EffectCounts,
}

impl RehearsalHost {
    pub fn new(facts: HostFacts, free_bytes: Probe, current_manifest: impl Into<String>) -> Self {
        Self {
            facts,
            free_bytes,
            current_manifest: current_manifest.into(),
            staging: BTreeMap::new(),
            backups: BTreeMap::new(),
            staged_bytes: 0,
            swaps: Vec::new(),
            exports: ExportedWrites::default(),
            fault: None,
            fault_code: None,
            counts: EffectCounts::default(),
        }
    }

    /// Make one step fail. Only the first matching call fails, so a recovery
    /// path can clear the fault and proceed.
    pub fn fail_at(mut self, fault: FaultPoint) -> Self {
        self.fault = Some(fault);
        self
    }

    /// Fail one step with a chosen error code, so a test can drive the core's
    /// stop-signal paths (a cancellation or deadline during an effect).
    pub fn fail_with(mut self, fault: FaultPoint, code: ErrorCode) -> Self {
        self.fault = Some(fault);
        self.fault_code = Some(code);
        self
    }

    /// Declare how many facts and audit records the host would export on
    /// rollback.
    pub fn with_post_upgrade_writes(mut self, facts: usize, audit: usize) -> Self {
        self.exports = ExportedWrites { facts, audit };
        self
    }

    pub fn counts(&self) -> EffectCounts {
        self.counts
    }
    pub fn current_manifest(&self) -> &str {
        &self.current_manifest
    }
    /// The ordered log of manifest identities written by `commit`.
    pub fn swaps(&self) -> &[String] {
        &self.swaps
    }
    pub fn staged_bytes(&self) -> u64 {
        self.staged_bytes
    }
    pub fn is_staged(&self, transaction_id: &str) -> bool {
        self.staging.contains_key(transaction_id)
    }

    fn clear_fault(&mut self, step: FaultPoint) -> Result<()> {
        if self.fault == Some(step) {
            self.fault = None;
            let code = self
                .fault_code
                .take()
                .unwrap_or_else(|| step.default_code());
            return Err(error(code, "rehearsal: injected step failure"));
        }
        Ok(())
    }
}

impl UpgradeHost for RehearsalHost {
    fn facts(&self) -> HostFacts {
        self.facts.clone()
    }

    fn free_bytes(&self) -> Probe {
        self.free_bytes.clone()
    }

    fn current_manifest(&self, op: &Operation) -> Result<String> {
        op.check()?;
        Ok(self.current_manifest.clone())
    }

    fn stage(
        &mut self,
        op: &Operation,
        transaction_id: &str,
        package: &VerifiedPackage,
    ) -> Result<StagingId> {
        op.check()?;
        self.clear_fault(FaultPoint::Stage)?;
        // Idempotent: re-staging the same transaction returns the same handle
        // and adds no bytes.
        if let Some(existing) = self.staging.get(transaction_id) {
            return Ok(existing.clone());
        }
        let handle = StagingId::new(format!("staging-{transaction_id}"))?;
        self.staging
            .insert(transaction_id.to_owned(), handle.clone());
        self.staged_bytes += package.content_bytes;
        self.counts.stage += 1;
        Ok(handle)
    }

    fn discard_staging(&mut self, op: &Operation, transaction_id: &str) -> Result<()> {
        op.check()?;
        self.clear_fault(FaultPoint::Discard)?;
        // Idempotent: removing an absent staging is not an error.
        if self.staging.remove(transaction_id).is_some() {
            self.counts.discard_staging += 1;
        }
        Ok(())
    }

    fn backup(&mut self, op: &Operation, staging: &StagingId) -> Result<BackupRef> {
        op.check()?;
        self.clear_fault(FaultPoint::Backup)?;
        if !self.staging.values().any(|known| known == staging) {
            return Err(error(ErrorCode::Conflict, "rehearsal: unknown staging"));
        }
        let reference = BackupRef::new(format!("backup-{}", staging.as_str()))?;
        self.backups
            .insert(staging.as_str().to_owned(), reference.clone());
        self.counts.backup += 1;
        Ok(reference)
    }

    fn migrate(&mut self, op: &Operation, staging: &StagingId) -> Result<()> {
        op.check()?;
        self.clear_fault(FaultPoint::Migrate)?;
        if !self.staging.values().any(|known| known == staging) {
            return Err(error(ErrorCode::Conflict, "rehearsal: unknown staging"));
        }
        self.counts.migrate += 1;
        Ok(())
    }

    fn self_check(&mut self, op: &Operation) -> Result<()> {
        op.check()?;
        self.clear_fault(FaultPoint::SelfCheck)?;
        self.counts.self_check += 1;
        Ok(())
    }

    fn commit(&mut self, op: &Operation, staging: &StagingId, to_manifest: &str) -> Result<()> {
        op.check()?;
        self.clear_fault(FaultPoint::Commit)?;
        if !self.staging.values().any(|known| known == staging) {
            return Err(error(ErrorCode::Conflict, "rehearsal: unknown staging"));
        }
        // The swap is a single assignment: a reader never observes a partial
        // manifest, which is the atomicity the core relies on.
        self.swaps.push(to_manifest.to_owned());
        self.current_manifest = to_manifest.to_owned();
        self.counts.commit += 1;
        Ok(())
    }

    fn rollback(
        &mut self,
        op: &Operation,
        backup: &BackupRef,
        _cause: &'static str,
    ) -> Result<ExportedWrites> {
        op.check()?;
        self.clear_fault(FaultPoint::Rollback)?;
        // Refuse a reference this host never issued rather than fabricate a
        // restore from nothing.
        if !self.backups.values().any(|known| known == backup) {
            return Err(error(
                ErrorCode::Conflict,
                "rehearsal: unknown backup reference",
            ));
        }
        self.counts.rollback += 1;
        Ok(self.exports)
    }
}

/// An in-memory journal for tests. It enforces the same monotonic step rule the
/// storage journal enforces — a step may only advance one rank or converge to
/// `rolled_back`, and a terminal step never moves — so a core test still catches
/// a regression, and it coalesces `backup_ref`/`error_code` exactly as the
/// storage `UPSERT` does.
#[derive(Debug, Default)]
pub struct RehearsalJournal {
    records: BTreeMap<String, UpgradeRecord>,
}

impl RehearsalJournal {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn record_for(&self, transaction_id: &str) -> Option<&UpgradeRecord> {
        self.records.get(transaction_id)
    }
}

impl UpgradeJournal for RehearsalJournal {
    fn record(&mut self, op: &Operation, record: &UpgradeRecord) -> Result<()> {
        op.check()?;
        match self.records.get(&record.transaction_id) {
            Some(current) => {
                if current.from_manifest != record.from_manifest
                    || current.to_manifest != record.to_manifest
                {
                    return Err(error(
                        ErrorCode::InvalidArgument,
                        "rehearsal: transaction identity changed",
                    ));
                }
                if !permits(current.step, record.step) {
                    return Err(error(
                        ErrorCode::Conflict,
                        "rehearsal: upgrade step regression or skip",
                    ));
                }
                let mut next = record.clone();
                next.backup_ref = record
                    .backup_ref
                    .clone()
                    .or_else(|| current.backup_ref.clone());
                next.error_code = record
                    .error_code
                    .clone()
                    .or_else(|| current.error_code.clone());
                self.records.insert(record.transaction_id.clone(), next);
            }
            None => {
                self.records
                    .insert(record.transaction_id.clone(), record.clone());
            }
        }
        Ok(())
    }

    fn current(&self, op: &Operation, transaction_id: &str) -> Result<Option<UpgradeRecord>> {
        op.check()?;
        Ok(self.records.get(transaction_id).cloned())
    }
}

fn rank(step: UpgradeStep) -> u8 {
    match step {
        UpgradeStep::Prepared => 0,
        UpgradeStep::Migrated => 1,
        UpgradeStep::Checked => 2,
        UpgradeStep::Committed => 3,
        UpgradeStep::RolledBack => 4,
    }
}

fn permits(from: UpgradeStep, to: UpgradeStep) -> bool {
    if from == to {
        return true;
    }
    if matches!(from, UpgradeStep::Committed | UpgradeStep::RolledBack) {
        return false;
    }
    to == UpgradeStep::RolledBack || rank(to) == rank(from) + 1
}
