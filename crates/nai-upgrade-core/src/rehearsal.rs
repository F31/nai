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
    gates::{ActiveSessionGate, SessionDrain},
    plan::{BackupRef, ExportedWrites, StagingId, UpgradeHost, UpgradeJournal, UpgradeRequest},
    preflight::{HostFacts, Probe, ResourceState},
    revocation::RevocationList,
    signature::DeterministicVerifier,
    verify::{PackageFile, VerifiedPackage, VerifyLimits},
};
use nai_contracts::v1::ErrorCode;
use nai_security::Operation;
use nai_storage::{UpgradeRecord, UpgradeStep};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::{Duration, Instant},
};

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

/// An in-memory session gate: a count a test sets and changes between calls.
#[derive(Debug, Default)]
pub struct RehearsalSessions {
    active: AtomicUsize,
}
impl RehearsalSessions {
    pub fn new(active: usize) -> Self {
        Self {
            active: AtomicUsize::new(active),
        }
    }
    /// Change the live count — modelling a session starting or ending.
    pub fn set(&self, active: usize) {
        self.active.store(active, Ordering::SeqCst);
    }
}
impl ActiveSessionGate for RehearsalSessions {
    fn active(&self) -> Result<usize> {
        Ok(self.active.load(Ordering::SeqCst))
    }
}

/// An in-memory drain: sessions that end when asked, capabilities that retire
/// once nothing holds them, and a record count a retire must never touch.
#[derive(Debug, Default)]
pub struct RehearsalDrain {
    dependents: BTreeMap<String, usize>,
    stubborn: BTreeSet<String>,
    retired: Vec<String>,
    records: usize,
}
impl RehearsalDrain {
    pub fn new() -> Self {
        Self::default()
    }
    /// Declare that `sessions` sessions depend on `capability` right now.
    pub fn with_dependents(mut self, capability: &str, sessions: usize) -> Self {
        self.dependents.insert(capability.to_owned(), sessions);
        self
    }
    /// Make a capability's sessions outlive any drain, so the core must refuse
    /// to retire it instead of stranding them.
    pub fn with_stubborn(mut self, capability: &str, sessions: usize) -> Self {
        self.dependents.insert(capability.to_owned(), sessions);
        self.stubborn.insert(capability.to_owned());
        self
    }
    /// Declare how many stored records exist, so a test can prove a downgrade
    /// kept them.
    pub fn with_records(mut self, records: usize) -> Self {
        self.records = records;
        self
    }
    pub fn retired(&self) -> &[String] {
        &self.retired
    }
    pub fn records(&self) -> usize {
        self.records
    }
}
impl SessionDrain for RehearsalDrain {
    fn dependents(&self, capability: &str) -> Result<usize> {
        Ok(self.dependents.get(capability).copied().unwrap_or(0))
    }

    fn drain(&mut self, op: &Operation, capability: &str) -> Result<usize> {
        op.check()?;
        if self.stubborn.contains(capability) {
            return Ok(self.dependents.get(capability).copied().unwrap_or(0));
        }
        self.dependents.insert(capability.to_owned(), 0);
        Ok(0)
    }

    fn retire(&mut self, op: &Operation, capability: &str) -> Result<()> {
        op.check()?;
        // A retire while sessions still hold the capability is refused, not
        // silently honoured: the core is supposed to have drained first.
        if self.dependents.get(capability).copied().unwrap_or(0) > 0 {
            return Err(error(
                ErrorCode::Conflict,
                "rehearsal: retiring a held capability",
            ));
        }
        self.retired.push(capability.to_owned());
        // Retiring touches code, never the stored records.
        Ok(())
    }
}

/// The single fake binary the demo package carries. Seven bytes, so a caller can
/// assert a staged content size without deriving it.
pub const DEMO_CONTENT: &[(&str, &[u8])] = &[("bin/app", b"payload")];

/// The demo cost table: two components known and two `null`, which is what makes
/// "an unknown cost survives as unknown rather than as zero" observable.
pub fn demo_cost() -> Value {
    json!({
        "currency": "CNY",
        "cloud": "10",
        "download": null,
        "storage": "5",
        "runtime_delta": null
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// A schema-valid demo manifest whose `files` array matches `files` exactly.
///
/// Public because a caller that must break one field — the preflight and
/// signature tests do exactly that — needs a valid base to mutate, and
/// restating every other field per test file is how copies drift.
pub fn demo_manifest(files: &[(&str, &[u8])], cost: Value) -> Value {
    let declarations: Vec<Value> = files
        .iter()
        .map(|(path, content)| {
            json!({
                "path": path,
                "sha256": sha256_hex(content),
                "bytes": content.len().to_string()
            })
        })
        .collect();
    json!({
        "manifest_schema": 1,
        "package_id": "plugin.demo",
        "package_version": "1.2.3",
        "kind": "plugin",
        "kernel_range": ">=1.0.0, <2.0.0",
        "requires": [{ "ref": "cap.one", "range": ">=1.0.0, <2.0.0" }],
        "hosts": ["inproc"],
        "os": ["linux"],
        "architectures": ["x86_64"],
        "runtime": [{ "id": "rt-cpu", "accel": "cpu" }],
        "files": declarations,
        "install_bytes": "1024",
        "migrated_bytes": "2048",
        "signer_ref": "signer.demo",
        "license_ref": "license.demo",
        "migration_ref": "migration.forward",
        "rollback_range": ">=1.0.0, <1.2.0",
        "cost": cost
    })
}

/// The host facts the demo manifest needs to pass its own preflight: they mirror
/// the `hosts`/`runtime`/`requires` declarations in [`demo_manifest`]. The
/// preflight is what keeps the two honest — a mismatch makes every `prepare`
/// refuse rather than quietly pass — and a caller that must break one of them
/// starts from here rather than restating the rest.
pub fn demo_facts() -> HostFacts {
    HostFacts {
        host: "inproc".into(),
        os: "linux".into(),
        architecture: "x86_64".into(),
        kernel_version: "1.96.0".into(),
        capabilities: vec!["cap.one".into()],
        runtime_ids: vec!["rt-cpu".into()],
        credentials_available: ResourceState::Sufficient,
    }
}

/// A signed demo package plus the verification environment a transaction needs.
///
/// The real package bytes come from a distributor, so every caller that must
/// drive a **real** `prepare` needs the same demo manifest, the same
/// deterministic signature and the same empty revocation list. Deriving that per
/// test file is how copies drift — and a caller in *another* crate has no way to
/// reach a fixture that lives in this crate's `tests/` — so it lives here beside
/// the other rehearsal doubles, one definition usable from anywhere.
///
/// It is **not** a distributor: it holds one fake binary, signs it
/// deterministically, and touches no disk, network or clock.
pub struct RehearsalPackage {
    manifest: Vec<u8>,
    signature: Vec<u8>,
    revocations: RevocationList,
    verifier: DeterministicVerifier,
    cancelled: AtomicBool,
}

impl RehearsalPackage {
    /// The demo package with the default cost table.
    pub fn new() -> Self {
        Self::with_cost(demo_cost())
    }

    /// The demo package with a chosen `cost` block. Cost is the one manifest
    /// field that is policy rather than identity, so it is the one a caller
    /// varies.
    pub fn with_cost(cost: Value) -> Self {
        let manifest = serde_json::to_vec(&demo_manifest(DEMO_CONTENT, cost)).unwrap();
        let signature = DeterministicVerifier::sign(&manifest);
        Self {
            manifest,
            signature,
            revocations: RevocationList::new(1_000, 60_000, []).unwrap(),
            verifier: DeterministicVerifier,
            cancelled: AtomicBool::new(false),
        }
    }

    /// The manifest bytes, for a caller that must parse the package itself.
    pub fn manifest(&self) -> &[u8] {
        &self.manifest
    }

    /// A rehearsal host carrying [`demo_facts`], `space` free bytes and the
    /// identity that is claimed to be installed right now.
    pub fn host(&self, space: Probe, current_manifest: impl Into<String>) -> RehearsalHost {
        RehearsalHost::new(demo_facts(), space, current_manifest)
    }

    /// A request for this package. `from_manifest` is the identity the caller
    /// claims is installed; the target identity is derived from the bytes, so a
    /// caller that needs the two to differ passes anything but the derived hash.
    pub fn request(&self, transaction_id: &str, from_manifest: &str) -> UpgradeRequest<'_> {
        UpgradeRequest {
            transaction_id: transaction_id.to_owned(),
            from_manifest: from_manifest.to_owned(),
            manifest: &self.manifest,
            files: DEMO_CONTENT
                .iter()
                .map(|&(path, content)| PackageFile { path, content })
                .collect(),
            signature: &self.signature,
            verifier: &self.verifier,
            revocations: &self.revocations,
            now_ms: 1_000,
            limits: VerifyLimits::default(),
            deadline: Instant::now() + Duration::from_secs(30),
            cancelled: &self.cancelled,
        }
    }
}

impl Default for RehearsalPackage {
    fn default() -> Self {
        Self::new()
    }
}
