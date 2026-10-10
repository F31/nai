//! Preflight: refuse on insufficiency *or* uncertainty.
//!
//! Every resource check answers `Sufficient`, `Insufficient(reason)` or
//! `Unknown(reason)`. `Unknown` is refused exactly like `Insufficient`: "we
//! cannot tell whether there is room" is not "there is room", and rendering it
//! as room is the "do not report available" violation R09 names. Nothing here
//! touches the current version manifest, so a refused preflight leaves the
//! running version untouched.
use crate::{
    Result, UpgradeError, error,
    manifest::{RuntimeEntry, UpgradePackage},
};
use nai_contracts::v1::ErrorCode;
use semver::{Version, VersionReq};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceState {
    Sufficient,
    Insufficient(&'static str),
    Unknown(&'static str),
}
impl ResourceState {
    pub fn sufficient(&self) -> bool {
        matches!(self, Self::Sufficient)
    }
    pub fn reason(&self) -> Option<&'static str> {
        match self {
            Self::Sufficient => None,
            Self::Insufficient(reason) | Self::Unknown(reason) => Some(reason),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub name: String,
    pub state: ResourceState,
}

/// What a probe can say about a resource. `Unknown` carries why it could not
/// decide, so the refusal is diagnosable rather than a bare "no".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe {
    Measured(u64),
    Unknown(&'static str),
}

/// Free-space probe. The platform call lives behind this trait so the core stays
/// portable and a build with no real probe returns `Unknown`, which refuses.
pub trait SpaceProbe {
    fn free_bytes(&self) -> Probe;
}

/// A probe that never has a real answer, for environments with no install
/// target. It is the safe default: an unknown resource refuses.
pub struct NoSpaceProbe;
impl SpaceProbe for NoSpaceProbe {
    fn free_bytes(&self) -> Probe {
        Probe::Unknown("no disk probe available")
    }
}

/// Facts about the target that the upgrade core cannot discover by itself.
#[derive(Debug, Clone)]
pub struct HostFacts {
    pub host: String,
    pub os: String,
    pub architecture: String,
    pub kernel_version: String,
    pub capabilities: Vec<String>,
    pub runtime_ids: Vec<String>,
    pub credentials_available: ResourceState,
}

#[derive(Debug, Clone)]
pub struct PreflightReport {
    checks: Vec<CheckResult>,
}
impl PreflightReport {
    pub fn checks(&self) -> &[CheckResult] {
        &self.checks
    }
    /// True only when every check is `Sufficient`.
    pub fn admitted(&self) -> bool {
        self.checks.iter().all(|check| check.state.sufficient())
    }
    pub fn refusals(&self) -> impl Iterator<Item = &CheckResult> {
        self.checks.iter().filter(|check| !check.state.sufficient())
    }
    /// The first refusal as an error, for callers that must stop. An
    /// unmeasured resource maps to `CAPABILITY_NOT_READY` and an insufficient
    /// one to `RESOURCE_EXHAUSTED`, so the two are distinguishable downstream.
    pub fn refusal_error(&self) -> Option<UpgradeError> {
        self.refusals().next().map(|check| match check.state {
            ResourceState::Insufficient(reason) => error(ErrorCode::ResourceExhausted, reason),
            ResourceState::Unknown(reason) => error(ErrorCode::CapabilityNotReady, reason),
            ResourceState::Sufficient => error(ErrorCode::Internal, "unreachable refusal"),
        })
    }
}

fn version_matches(range: &str, version: &Version) -> Result<bool> {
    let requirement = VersionReq::parse(range)
        .map_err(|_| error(ErrorCode::InvalidArgument, "unparsable version range"))?;
    Ok(requirement.matches(version))
}

/// Run every preflight check and collect the per-check verdicts. The caller
/// decides what to do with a non-admitted report; the report itself never
/// modifies anything.
pub fn preflight(
    package: &UpgradePackage,
    facts: &HostFacts,
    space: &dyn SpaceProbe,
) -> Result<PreflightReport> {
    let mut checks = Vec::new();

    // Platform: host, OS and architecture must each be declared.
    let platform_ok = package.hosts.iter().any(|host| host == &facts.host)
        && package.os.iter().any(|os| os == &facts.os)
        && package
            .architectures
            .iter()
            .any(|architecture| architecture == &facts.architecture);
    checks.push(CheckResult {
        name: "platform".into(),
        state: if platform_ok {
            ResourceState::Sufficient
        } else {
            ResourceState::Insufficient("unsupported host, os or architecture")
        },
    });

    // Kernel range.
    let kernel = Version::parse(&facts.kernel_version)
        .map_err(|_| error(ErrorCode::InvalidArgument, "unparsable host kernel version"))?;
    checks.push(CheckResult {
        name: "kernel_range".into(),
        state: if version_matches(&package.kernel_range, &kernel)? {
            ResourceState::Sufficient
        } else {
            ResourceState::Insufficient("kernel version outside declared range")
        },
    });

    // Dependency closure: each requirement must be provided and its version
    // range satisfied by the target kernel.
    for requirement in &package.requires {
        let provided = facts
            .capabilities
            .iter()
            .any(|capability| capability == &requirement.reference);
        let state = if !provided {
            ResourceState::Insufficient("dependency capability absent")
        } else if version_matches(&requirement.range, &kernel)? {
            ResourceState::Sufficient
        } else {
            ResourceState::Insufficient("dependency version outside range")
        };
        checks.push(CheckResult {
            name: format!("requires:{}", requirement.reference),
            state,
        });
    }

    // Runtime matrix: every declared entry must be available on the target, with
    // CPU treated as always available.
    let runtime_state = package
        .runtime
        .iter()
        .find_map(|RuntimeEntry { id, accel, .. }| {
            (!(accel == "cpu" || facts.runtime_ids.iter().any(|entry| entry == id)))
                .then_some(ResourceState::Insufficient("runtime entry unavailable"))
        })
        .unwrap_or(ResourceState::Sufficient);
    checks.push(CheckResult {
        name: "runtime".into(),
        state: runtime_state,
    });

    // Disk: staging plus the migrated data directory coexist, so both counts.
    let required = package
        .install_bytes
        .checked_add(package.migrated_bytes)
        .ok_or_else(|| error(ErrorCode::ResourceExhausted, "disk requirement overflow"))?;
    let space_state = match space.free_bytes() {
        Probe::Measured(free) if free >= required => ResourceState::Sufficient,
        Probe::Measured(_) => ResourceState::Insufficient("insufficient free space"),
        Probe::Unknown(reason) => ResourceState::Unknown(reason),
    };
    checks.push(CheckResult {
        name: "space".into(),
        state: space_state,
    });

    // Credentials.
    checks.push(CheckResult {
        name: "credentials".into(),
        state: facts.credentials_available.clone(),
    });

    Ok(PreflightReport { checks })
}
