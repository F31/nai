//! Deployment migration: `Local → Hybrid → Distributed`.
//!
//! Three refusals happen *before* anything moves, because each of them is a way
//! to lose data or availability that no post-hoc verification could undo:
//!
//! 1. a live session — the realtime chain stays on the original deployment and
//!    the switch belongs in a maintenance window;
//! 2. a distributed target with more than one writer on the SQLite store — there
//!    is no multi-node write path, and pretending otherwise corrupts the logs;
//! 3. a permission with no mapping in the target — dropping a grant silently is a
//!    security defect, not a convenience.
//!
//! Then three consistency checks confirm what arrived: the mapped permission set,
//! the evidence version, and the data hash. Evidence version and hash use
//! different error codes on purpose — "the versions disagree" and "the bytes were
//! altered" are different stories and an operator has to be able to tell them
//! apart. Checking only the hash would also pass a migration that quietly rewrote
//! `admin.team` into `viewer`, because a wrong permission table hashes just as
//! consistently as the right one.
use crate::{Result, error};
use nai_contracts::v1::ErrorCode;
use nai_security::Operation;
use std::collections::{BTreeMap, BTreeSet};

/// The deployment shapes of §14.2. They are orthogonal to tiers: a Pro tier can
/// run on `Local` and a Lite tier can run on `Standard` infrastructure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Deployment {
    Local,
    Hybrid,
    Distributed,
}

impl Deployment {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Hybrid => "hybrid",
            Self::Distributed => "distributed",
        }
    }
}

/// One permission held on a deployment.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PermissionGrant {
    pub capability: String,
    pub scope: String,
}

/// The exportable shape of a deployment: what is granted, which evidence version
/// the operator was working against, and the hash of the data being moved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentState {
    pub deployment: Deployment,
    pub permissions: Vec<PermissionGrant>,
    pub evidence_version: String,
    pub data_hash: String,
}

/// Where permissions land in a target deployment. Data, not behaviour: the
/// answer differs per deployment shape, and a mapping table the operator can read
/// is what makes "权限映射" auditable.
#[derive(Debug, Clone, Default)]
pub struct PermissionMap {
    entries: BTreeMap<Deployment, BTreeMap<(String, String), PermissionGrant>>,
}

impl PermissionMap {
    pub fn insert(&mut self, target: Deployment, from: PermissionGrant, to: PermissionGrant) {
        self.entries
            .entry(target)
            .or_default()
            .insert((from.capability, from.scope), to);
    }

    pub fn map(&self, target: Deployment, grant: &PermissionGrant) -> Option<PermissionGrant> {
        self.entries
            .get(&target)?
            .get(&(grant.capability.clone(), grant.scope.clone()))
            .cloned()
    }
}

/// What the caller knows about the world at planning time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationRequest {
    pub target: Deployment,
    /// Sessions live on the source deployment right now.
    pub live_sessions: usize,
    /// Nodes that would write the SQLite store at the target.
    pub sqlite_writers: usize,
}

/// The planned move: which grants become which, and what the target must end up
/// holding for the migration to count as consistent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationPlan {
    pub from: Deployment,
    pub to: Deployment,
    pub mappings: Vec<(PermissionGrant, PermissionGrant)>,
    /// The mapped grants, sorted and deduplicated — the exact set the import has
    /// to produce, so consistency is a comparison rather than a judgement call.
    pub expected: Vec<PermissionGrant>,
    /// The state the plan was computed from. Kept so the post-import checks
    /// compare against what was actually exported, not against whatever happens
    /// to be around later.
    pub source: DeploymentState,
}

/// Plan a migration, refusing the three ways to lose data or availability.
pub fn plan_migration(
    current: &DeploymentState,
    request: &MigrationRequest,
    map: &PermissionMap,
) -> Result<MigrationPlan> {
    if request.target == current.deployment {
        return Err(error(
            ErrorCode::InvalidArgument,
            "migration target equals the current deployment",
        ));
    }
    if request.live_sessions > 0 {
        return Err(error(
            ErrorCode::CapabilityNotReady,
            "migration requires a maintenance window",
        ));
    }
    if request.target == Deployment::Distributed && request.sqlite_writers > 1 {
        return Err(error(
            ErrorCode::Conflict,
            "sqlite store cannot be written by several nodes",
        ));
    }
    let mut mappings = Vec::new();
    let mut expected = BTreeSet::new();
    for grant in &current.permissions {
        let mapped = map.map(request.target, grant).ok_or_else(|| {
            error(
                ErrorCode::InvalidArgument,
                "permission has no mapping in the target deployment",
            )
        })?;
        expected.insert(mapped.clone());
        mappings.push((grant.clone(), mapped));
    }
    Ok(MigrationPlan {
        from: current.deployment,
        to: request.target,
        mappings,
        expected: expected.into_iter().collect(),
        source: current.clone(),
    })
}

/// What the target reports after importing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedState {
    pub permissions: Vec<PermissionGrant>,
    pub evidence_version: String,
    pub data_hash: String,
}

/// Confirm that the target holds what the plan promised: the mapped permission
/// set, the source evidence version, and the source data hash.
pub fn verify_migration(plan: &MigrationPlan, imported: &ImportedState) -> Result<()> {
    let reported: BTreeSet<&PermissionGrant> = imported.permissions.iter().collect();
    let expected: BTreeSet<&PermissionGrant> = plan.expected.iter().collect();
    if reported != expected {
        return Err(error(
            ErrorCode::InvalidArgument,
            "migrated permissions differ from the mapped set",
        ));
    }
    if imported.evidence_version != plan.source.evidence_version {
        return Err(error(
            ErrorCode::VersionIncompatible,
            "migrated evidence version differs from the source",
        ));
    }
    if imported.data_hash != plan.source.data_hash {
        return Err(error(
            ErrorCode::InvalidArgument,
            "migrated data hash differs from the source",
        ));
    }
    Ok(())
}

/// Output switching, made idempotent.
///
/// The switch key is chosen by the caller (typically `session/epoch`) because
/// only the caller knows what counts as "the same switch". This type's whole job
/// is that the same key is never announced twice: a retry inside the maintenance
/// window must not make the system speak the same sentence again. The
/// specification asks for exactly this — "输出切换不重复播报".
#[derive(Debug, Clone, Default)]
pub struct OutputSwitch {
    announced: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Announcement {
    Announced,
    Duplicate,
}

impl OutputSwitch {
    pub fn switch(&mut self, op: &Operation, key: &str) -> Result<Announcement> {
        op.check()?;
        if key.is_empty() || key.len() > 128 {
            return Err(error(
                ErrorCode::InvalidArgument,
                "invalid output switch key",
            ));
        }
        Ok(if self.announced.insert(key.to_owned()) {
            Announcement::Announced
        } else {
            Announcement::Duplicate
        })
    }

    pub fn announced_count(&self) -> usize {
        self.announced.len()
    }
}

/// A way for a rehearsal import to be wrong. Each variant is a mistake that a
/// real importer could plausibly make, so the consistency checks can be shown to
/// actually fire rather than assumed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationFault {
    /// The target reports a different evidence version.
    EvidenceVersionDrift,
    /// The target reports a different data hash.
    DataHashDrift,
    /// The target dropped one mapped permission.
    DroppedPermission,
}

/// In-memory migration host, so the move can be driven end to end off a device.
#[derive(Debug, Clone)]
pub struct RehearsalMigration {
    state: DeploymentState,
    fault: Option<MigrationFault>,
}

impl RehearsalMigration {
    pub fn new(state: DeploymentState) -> Self {
        Self { state, fault: None }
    }

    pub fn with_fault(mut self, fault: MigrationFault) -> Self {
        self.fault = Some(fault);
        self
    }

    pub fn state(&self) -> &DeploymentState {
        &self.state
    }

    /// A faithful export: permissions copied, evidence version and hash carried
    /// through untouched. The host never recomputes either value, because an
    /// import that recomputed its own evidence version would make the check that
    /// compares it with the source vacuous.
    pub fn export(&self, op: &Operation) -> Result<DeploymentState> {
        op.check()?;
        Ok(self.state.clone())
    }

    /// Import under a plan. A correct import lands exactly on the plan's expected
    /// permission set and preserves the evidence version and hash; the injected
    /// faults are what an incorrect import looks like.
    pub fn import(&mut self, op: &Operation, plan: &MigrationPlan) -> Result<ImportedState> {
        op.check()?;
        let mut permissions = plan.expected.clone();
        if self.fault == Some(MigrationFault::DroppedPermission) {
            permissions.pop();
        }
        self.state.deployment = plan.to;
        self.state.permissions = permissions.clone();
        let evidence_version = match self.fault {
            Some(MigrationFault::EvidenceVersionDrift) => {
                format!("{}-drift", self.state.evidence_version)
            }
            _ => self.state.evidence_version.clone(),
        };
        let data_hash = match self.fault {
            Some(MigrationFault::DataHashDrift) => "0".repeat(64),
            _ => self.state.data_hash.clone(),
        };
        Ok(ImportedState {
            permissions,
            evidence_version,
            data_hash,
        })
    }
}
