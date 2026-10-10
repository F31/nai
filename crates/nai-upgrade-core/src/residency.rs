//! Capability residency: what a capability costs while it is *not* running, and
//! when an idle one gives its resources back.
//!
//! The acceptance criterion is asymmetric, and the asymmetry is the whole point:
//! a **disabled or uninstalled** capability must hold *nothing* — no process,
//! port, video memory or cloud request — while an enabled one may hold its
//! declared footprint. A footprint that only materialises on `enable` turns
//! "disabled holds nothing" into a structural property of the ledger instead of
//! a promise in a document. [`ResidencyLedger::audit`] then exists to make the
//! property observable: a forgotten preview poller is exactly the kind of leak
//! that no behavioural test notices and only an audit can name.
//!
//! Idle release is deliberately **not** immediate. A model unloaded the moment a
//! session ends costs a full reload on the next sentence, so the specification
//! asks for TTL-based release with per-family budgets: a temporary worker is
//! cheap to restart and leaves first, a model is the expensive reload and stays
//! longest. [`ResidencyLedger::reclaim`] therefore releases only what a TTL
//! allows and reports what it kept back, rather than emptying everything and
//! calling it clean.
use crate::{Result, error};
use nai_contracts::v1::ErrorCode;
use nai_security::Operation;
use std::collections::{BTreeMap, BTreeSet};

/// Upper bound on tracked capabilities, so a ledger cannot grow without bound.
pub const MAX_CAPABILITIES: usize = 1024;
const MAX_NAME: usize = 128;

/// The resource families the acceptance criteria enumerate. Closed on purpose:
/// a family nobody audits is exactly how an "idle" module keeps a socket open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResourceKind {
    /// A background process, e.g. a temporary worker or a render host.
    Process,
    /// A bound local port.
    Port,
    /// Video memory held by a loaded model or avatar.
    Vram,
    /// An outbound cloud request stream, e.g. an unused preview poller.
    CloudRequest,
}

impl ResourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Process => "process",
            Self::Port => "port",
            Self::Vram => "vram",
            Self::CloudRequest => "cloud_request",
        }
    }

    /// Whether two capabilities may hold the same `name` of this kind.
    ///
    /// A process name and a bound port are exclusive at the operating-system
    /// level, so a second claim is a real conflict to refuse. Two capabilities
    /// may legitimately load the same model or poll the same API, so those
    /// families are deliberately *not* exclusive — inventing exclusivity there
    /// would forbid combinations that are perfectly valid.
    fn exclusive(self) -> bool {
        matches!(self, Self::Process | Self::Port)
    }

    /// Default TTL, ordered by reload cost: a worker is cheap to start again and
    /// leaves first; a model is the expensive reload and stays longest.
    fn default_ttl_ms(self) -> i64 {
        match self {
            Self::Process | Self::Port => 30_000,
            Self::CloudRequest => 60_000,
            Self::Vram => 300_000,
        }
    }
}

/// How long each family may stay resident once nothing is being served.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdlePolicy {
    ttl_ms: BTreeMap<ResourceKind, i64>,
}

impl Default for IdlePolicy {
    fn default() -> Self {
        let ttls = [
            ResourceKind::Process,
            ResourceKind::Port,
            ResourceKind::Vram,
            ResourceKind::CloudRequest,
        ];
        Self {
            ttl_ms: ttls
                .into_iter()
                .map(|kind| (kind, kind.default_ttl_ms()))
                .collect(),
        }
    }
}

impl IdlePolicy {
    /// The TTL for one family. An absent entry means "release on the next
    /// reclaim", which is the conservative reading of a missing budget.
    pub fn ttl_ms(&self, kind: ResourceKind) -> i64 {
        self.ttl_ms.get(&kind).copied().unwrap_or(0)
    }

    /// Override one family's TTL. Negative budgets are refused: a TTL that runs
    /// backwards would either never fire or fire before the session ended.
    pub fn set(&mut self, kind: ResourceKind, ttl_ms: i64) -> Result<()> {
        if ttl_ms < 0 {
            return Err(error(ErrorCode::InvalidArgument, "idle ttl is negative"));
        }
        self.ttl_ms.insert(kind, ttl_ms);
        Ok(())
    }
}

/// One declared or held resource. `name` is the identity that matters for
/// exclusivity (a process name, `tcp/8765`, a model id, an endpoint reference);
/// `amount` is the quantity (MiB for `Vram`, requests per minute for
/// `CloudRequest`, and 1 for a single process or port).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResourceClaim {
    pub kind: ResourceKind,
    pub name: String,
    pub amount: u64,
}

/// What one capability is doing right now, as the ledger sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Residency {
    /// Installed but not enabled. Holds nothing, and must hold nothing.
    Disabled,
    /// Enabled and serving at least one session.
    Serving { sessions: usize },
    /// Enabled, not serving. Holds its footprint until a TTL releases it.
    Resident { since_ms: i64 },
}

/// What one reclaim pass released, and what it deliberately kept.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReclaimReport {
    pub released: Vec<ReleasedResources>,
    /// Claims kept because their TTL has not elapsed. Reported, not hidden: an
    /// operator asking "why is the model still resident" deserves the number.
    pub held_back: usize,
}

impl ReclaimReport {
    pub fn is_empty(&self) -> bool {
        self.released.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleasedResources {
    pub capability: String,
    pub claims: Vec<ResourceClaim>,
}

/// One residency rule that is currently broken, as a stable token plus the
/// subject it is about. Tokens never change with locale, so a report can
/// classify violations without a debug build.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Violation {
    pub token: &'static str,
    pub subject: String,
}

/// One capability's residency as a plain value.
///
/// Deliberately public and independent of the ledger: it lets a host audit *its
/// own* view of what is resident (not just the ledger's), and it is what makes
/// [`audit_residency`] testable as a pure function — every rule can be exercised
/// against a hand-built state instead of waiting for a misuse to occur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityResidency {
    pub capability: String,
    pub declared: Vec<ResourceClaim>,
    pub held: Vec<ResourceClaim>,
    pub enabled: bool,
    pub sessions: usize,
    pub idle_since_ms: Option<i64>,
}

/// The ledger's own opinion of itself.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResidencyAudit {
    /// Currently held claims, per capability (enabled capabilities only).
    pub held: Vec<(String, Vec<ResourceClaim>)>,
    pub violations: Vec<Violation>,
}

impl ResidencyAudit {
    pub fn is_clean(&self) -> bool {
        self.violations.is_empty()
    }
}

#[derive(Debug, Clone)]
struct Entry {
    declared: Vec<ResourceClaim>,
    held: Vec<ResourceClaim>,
    enabled: bool,
    sessions: usize,
    idle_since_ms: Option<i64>,
}

/// The residency ledger. Every mutating entry point checks the `Operation`
/// first, and time enters as a parameter — the ledger never reads a wall clock,
/// which is what makes the TTL rules testable off the target machine.
#[derive(Debug, Clone)]
pub struct ResidencyLedger {
    policy: IdlePolicy,
    entries: BTreeMap<String, Entry>,
}

fn valid_capability(capability: &str) -> bool {
    !capability.is_empty()
        && capability.len() <= MAX_NAME
        && !capability.contains(char::is_whitespace)
}

fn valid_claim(claim: &ResourceClaim) -> bool {
    claim.amount > 0 && !claim.name.is_empty() && claim.name.len() <= MAX_NAME
}

/// Claims within one capability must be free of exact duplicates: a capability
/// declaring the same port twice has an ambiguous footprint. Across capabilities
/// duplicates are allowed except for the exclusive families, which is checked at
/// `enable` time instead.
fn canonical(claims: &[ResourceClaim]) -> bool {
    let mut seen = BTreeSet::new();
    claims
        .iter()
        .all(|claim| valid_claim(claim) && seen.insert((claim.kind, claim.name.clone())))
}

impl ResidencyLedger {
    pub fn new(policy: IdlePolicy) -> Self {
        Self {
            policy,
            entries: BTreeMap::new(),
        }
    }

    /// Register a capability's declared footprint. The footprint is **not**
    /// materialised here: installing something must leave the machine exactly as
    /// quiet as it was before.
    pub fn install(
        &mut self,
        op: &Operation,
        capability: &str,
        declared: Vec<ResourceClaim>,
    ) -> Result<()> {
        op.check()?;
        if !valid_capability(capability) {
            return Err(error(
                ErrorCode::InvalidArgument,
                "invalid capability reference",
            ));
        }
        if !canonical(&declared) {
            return Err(error(
                ErrorCode::InvalidArgument,
                "non-canonical declared footprint",
            ));
        }
        if self.entries.contains_key(capability) {
            return Err(error(ErrorCode::Conflict, "capability already installed"));
        }
        if self.entries.len() >= MAX_CAPABILITIES {
            return Err(error(ErrorCode::ResourceExhausted, "capability limit"));
        }
        self.entries.insert(
            capability.to_owned(),
            Entry {
                declared,
                held: Vec::new(),
                enabled: false,
                sessions: 0,
                idle_since_ms: None,
            },
        );
        Ok(())
    }

    /// Materialise the declared footprint. The idle clock starts here: an
    /// enabled-but-never-used capability is still idle, and a never-used preview
    /// has to be reclaimable or "关闭未用预览" would only apply to previews that
    /// were once used.
    pub fn enable(&mut self, op: &Operation, capability: &str, now_ms: i64) -> Result<()> {
        op.check()?;
        let declared = self.entry(capability)?.declared.clone();
        if self.entry(capability)?.enabled {
            return Err(error(ErrorCode::Conflict, "capability already enabled"));
        }
        for claim in declared.iter().filter(|claim| claim.kind.exclusive()) {
            if self.holds_exclusively(capability, claim) {
                return Err(error(
                    ErrorCode::Conflict,
                    "exclusive resource already held by another capability",
                ));
            }
        }
        let entry = self.entry(capability)?;
        entry.held = declared;
        entry.enabled = true;
        entry.sessions = 0;
        entry.idle_since_ms = Some(now_ms);
        Ok(())
    }

    pub fn session_started(&mut self, op: &Operation, capability: &str) -> Result<()> {
        op.check()?;
        let entry = self.entry(capability)?;
        if !entry.enabled {
            return Err(error(
                ErrorCode::CapabilityNotReady,
                "capability is not enabled",
            ));
        }
        entry.sessions = entry.sessions.saturating_add(1);
        entry.idle_since_ms = None;
        Ok(())
    }

    /// End one session. The capability keeps its footprint: the idle clock only
    /// starts, it does not release anything. That is the difference between a
    /// TTL and an unload.
    pub fn session_ended(&mut self, op: &Operation, capability: &str, now_ms: i64) -> Result<()> {
        op.check()?;
        let entry = self.entry(capability)?;
        if !entry.enabled {
            return Err(error(
                ErrorCode::CapabilityNotReady,
                "capability is not enabled",
            ));
        }
        if entry.sessions == 0 {
            return Err(error(ErrorCode::Conflict, "no session to end"));
        }
        entry.sessions -= 1;
        if entry.sessions == 0 {
            entry.idle_since_ms = Some(now_ms);
        }
        Ok(())
    }

    /// Disable a capability and release everything it holds. Refused while it
    /// still serves a session, mirroring the upgrade gate's "drain, then retire"
    /// order: pulling a capability out from under a live session is how a
    /// downgrade becomes an outage. Data is untouched.
    pub fn disable(&mut self, op: &Operation, capability: &str) -> Result<()> {
        op.check()?;
        let entry = self.entry(capability)?;
        if entry.sessions > 0 {
            return Err(error(
                ErrorCode::Conflict,
                "capability still serving a session",
            ));
        }
        entry.held.clear();
        entry.enabled = false;
        entry.idle_since_ms = None;
        Ok(())
    }

    /// Remove a capability entirely.
    pub fn uninstall(&mut self, op: &Operation, capability: &str) -> Result<()> {
        op.check()?;
        if self.entry(capability)?.sessions > 0 {
            return Err(error(
                ErrorCode::Conflict,
                "capability still serving a session",
            ));
        }
        self.entries.remove(capability);
        Ok(())
    }

    /// Release whatever the TTLs allow. Only claims whose family TTL has elapsed
    /// go; the rest are counted in `held_back` rather than quietly cleared.
    /// Serving capabilities are never touched, and this never changes enablement
    /// or stored data — it releases resources, nothing else.
    pub fn reclaim(&mut self, op: &Operation, now_ms: i64) -> Result<ReclaimReport> {
        op.check()?;
        let policy = self.policy.clone();
        let mut report = ReclaimReport::default();
        for (capability, entry) in self.entries.iter_mut() {
            if !entry.enabled || entry.sessions > 0 {
                continue;
            }
            let Some(since_ms) = entry.idle_since_ms else {
                continue;
            };
            let elapsed = now_ms.saturating_sub(since_ms);
            let mut released = Vec::new();
            entry.held.retain(|claim| {
                if elapsed >= policy.ttl_ms(claim.kind) {
                    released.push(claim.clone());
                    false
                } else {
                    true
                }
            });
            report.held_back += entry.held.len();
            if !released.is_empty() {
                report.released.push(ReleasedResources {
                    capability: capability.clone(),
                    claims: released,
                });
            }
        }
        Ok(report)
    }

    pub fn residency_of(&self, capability: &str) -> Option<Residency> {
        let entry = self.entries.get(capability)?;
        Some(if !entry.enabled {
            Residency::Disabled
        } else if entry.sessions > 0 {
            Residency::Serving {
                sessions: entry.sessions,
            }
        } else {
            Residency::Resident {
                since_ms: entry.idle_since_ms.unwrap_or(0),
            }
        })
    }

    /// Everything currently materialised, across all capabilities.
    pub fn held(&self) -> Vec<ResourceClaim> {
        let mut held = BTreeSet::new();
        for entry in self.entries.values() {
            held.extend(entry.held.iter().cloned());
        }
        held.into_iter().collect()
    }

    /// Total quantity held for one family, e.g. MiB of video memory.
    pub fn total(&self, kind: ResourceKind) -> u64 {
        self.entries
            .values()
            .flat_map(|entry| entry.held.iter())
            .filter(|claim| claim.kind == kind)
            .fold(0u64, |sum, claim| sum.saturating_add(claim.amount))
    }

    /// Every capability's residency, as a plain value.
    pub fn snapshot(&self) -> Vec<CapabilityResidency> {
        self.entries
            .iter()
            .map(|(capability, entry)| CapabilityResidency {
                capability: capability.clone(),
                declared: entry.declared.clone(),
                held: entry.held.clone(),
                enabled: entry.enabled,
                sessions: entry.sessions,
                idle_since_ms: entry.idle_since_ms,
            })
            .collect()
    }

    /// Audit the ledger's own view.
    pub fn audit(&self, now_ms: i64) -> ResidencyAudit {
        audit_residency(&self.snapshot(), &self.policy, now_ms)
    }

    fn entry(&mut self, capability: &str) -> Result<&mut Entry> {
        if !valid_capability(capability) {
            return Err(error(
                ErrorCode::InvalidArgument,
                "invalid capability reference",
            ));
        }
        self.entries
            .get_mut(capability)
            .ok_or_else(|| error(ErrorCode::CapabilityAbsent, "capability is not installed"))
    }

    fn holds_exclusively(&self, capability: &str, claim: &ResourceClaim) -> bool {
        self.entries.iter().any(|(other, entry)| {
            other != capability
                && entry.enabled
                && entry
                    .held
                    .iter()
                    .any(|held| held.kind == claim.kind && held.name == claim.name)
        })
    }
}

impl CapabilityResidency {
    pub fn residency(&self) -> Residency {
        if !self.enabled {
            Residency::Disabled
        } else if self.sessions > 0 {
            Residency::Serving {
                sessions: self.sessions,
            }
        } else {
            Residency::Resident {
                since_ms: self.idle_since_ms.unwrap_or(0),
            }
        }
    }
}

/// Check the residency rules against a snapshot.
///
/// A correctly used [`ResidencyLedger`] cannot reach three of these states — that
/// is the point of materialising the footprint only on `enable`. They are kept as
/// an audit anyway because "cannot happen" is a claim about today's code: the
/// audit is what turns it into something a host can re-check after a refactor,
/// and what catches the fourth state (a resource past its TTL that nobody
/// reclaimed) which *is* reachable the moment a reclaim pass is missed.
pub fn audit_residency(
    entries: &[CapabilityResidency],
    policy: &IdlePolicy,
    now_ms: i64,
) -> ResidencyAudit {
    let mut held = Vec::new();
    let mut violations = Vec::new();
    let mut exclusive: BTreeMap<(ResourceKind, String), Vec<String>> = BTreeMap::new();

    for entry in entries {
        let capability = &entry.capability;
        if !entry.enabled {
            if !entry.held.is_empty() {
                violations.push(Violation {
                    token: "disabled capability holds resources",
                    subject: capability.clone(),
                });
            }
            continue;
        }
        if !entry.held.is_empty() {
            held.push((capability.clone(), entry.held.clone()));
        }
        for claim in &entry.held {
            if !entry.declared.contains(claim) {
                violations.push(Violation {
                    token: "enabled capability holds undeclared resource",
                    subject: format!("{capability}:{}:{}", claim.kind.as_str(), claim.name),
                });
            }
            if claim.kind.exclusive() {
                exclusive
                    .entry((claim.kind, claim.name.clone()))
                    .or_default()
                    .push(capability.clone());
            }
        }
        match (entry.sessions, entry.idle_since_ms) {
            (0, Some(since_ms)) => {
                let elapsed = now_ms.saturating_sub(since_ms);
                if entry
                    .held
                    .iter()
                    .any(|claim| elapsed >= policy.ttl_ms(claim.kind))
                {
                    violations.push(Violation {
                        token: "idle resource past its ttl",
                        subject: capability.clone(),
                    });
                }
            }
            (0, None) => violations.push(Violation {
                token: "idle capability has no idle clock",
                subject: capability.clone(),
            }),
            (_, Some(_)) => violations.push(Violation {
                token: "serving capability has an idle clock",
                subject: capability.clone(),
            }),
            (_, None) => {}
        }
    }

    for ((kind, name), holders) in exclusive {
        if holders.len() > 1 {
            violations.push(Violation {
                token: "exclusive resource held twice",
                subject: format!("{}:{}", kind.as_str(), name),
            });
        }
    }
    violations.sort();
    violations.dedup();
    held.sort();
    ResidencyAudit { held, violations }
}

/// A capability tier preset. The tiers are the *same kernel* with different
/// capabilities enabled (§14.1), which is why moving between them is a set
/// operation and not an installation of a different product.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Lite,
    Standard,
    Pro,
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lite => "lite",
            Self::Standard => "standard",
            Self::Pro => "pro",
        }
    }
}

/// Capabilities `Standard` adds over `Lite`.
pub const STANDARD_ADDITIONS: &[&str] = &[
    "connector.douyin",
    "knowledge.hybrid",
    "show.plan",
    "avatar.realistic",
    "agent.shadow",
    "output.adapter.extra",
];

/// Capabilities `Pro` adds over `Standard`.
pub const PRO_ADDITIONS: &[&str] = &[
    "connector.multi-platform",
    "knowledge.multi-space",
    "skill.extended",
    "memory.long-term",
    "output.multi-session",
    "admin.team",
];

/// What a tier change installs and removes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TierPlan {
    pub install: Vec<&'static str>,
    pub remove: Vec<&'static str>,
}

impl TierPlan {
    pub fn is_empty(&self) -> bool {
        self.install.is_empty() && self.remove.is_empty()
    }
}

fn additions(tier: Tier) -> &'static [&'static str] {
    match tier {
        Tier::Lite => &[],
        Tier::Standard => STANDARD_ADDITIONS,
        Tier::Pro => PRO_ADDITIONS,
    }
}

/// The single migration graph between tiers.
///
/// Crossing a boundary means installing the additions between them, so
/// `Lite → Pro` is just the union of both hops — which is why the specification
/// can say "不要求先装中间档" without a special case. The sets are constants of
/// capability names, so a kernel can never appear in a tier plan: "不重装内核"
/// is a property of the data, not a rule someone has to remember.
pub fn tier_plan(from: Tier, to: Tier) -> TierPlan {
    if from == to {
        return TierPlan::default();
    }
    let mut plan = TierPlan::default();
    for tier in [Tier::Standard, Tier::Pro] {
        if to >= tier && from < tier {
            plan.install.extend_from_slice(additions(tier));
        }
        if from >= tier && to < tier {
            plan.remove.extend_from_slice(additions(tier));
        }
    }
    plan
}
