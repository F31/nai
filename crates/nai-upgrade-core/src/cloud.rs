//! Cloud disconnect and low-cloud capacity: which way the dependencies point,
//! and how much capacity may be written down.
//!
//! A10 is asymmetric in the same way residency is. A cloud outage must leave the
//! *local* real-time path and the local mute untouched, while remote capabilities
//! fail closed through their own gate. The only way to keep that honest is to make
//! the dependency direction a **value**: a link graph whose local critical nodes
//! are checked transitively for cloud reachability. A comment saying "the mute
//! does not need the cloud" cannot regress; a graph assertion can, and does.
//!
//! The second half is capacity. A low-cloud profile that writes down a number
//! nobody measured is exactly how a fleet gets sized by optimism, so capacity is
//! only constructible from a [`Measurement`] taken at declared instances and load,
//! and a budget breach is reported as throttling rather than silently absorbed.
//!
//! Nothing here reaches a network. The cloud link is a value the host supplies,
//! which is what makes "本地静音不受影响" testable off the target machine.
use crate::residency::Violation;
use crate::{Result, error};
use nai_contracts::v1::ErrorCode;
use std::collections::{BTreeMap, BTreeSet};

/// Upper bound on graph nodes, so a graph cannot grow without bound.
pub const MAX_NODES: usize = 4096;
const MAX_ID: usize = 128;

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_ID && !id.contains(char::is_whitespace)
}

/// Where a node lives. The distinction is the whole point of A10: a `Cloud` node
/// may vanish with the link, a `Local` one must not care.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeClass {
    Local,
    Cloud,
}

/// A directed dependency graph: `from` depends on `to`.
///
/// Direction matters. Writing an edge `local_mute -> cloud.gateway` is a bug that
/// only a transitively-checked graph can catch, and it is the exact bug A10 names.
#[derive(Debug, Clone, Default)]
pub struct LinkGraph {
    nodes: BTreeMap<String, NodeClass>,
    edges: BTreeMap<String, BTreeSet<String>>,
}

impl LinkGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_local(&mut self, node: &str) -> Result<()> {
        self.add(node, NodeClass::Local)
    }

    pub fn add_cloud(&mut self, node: &str) -> Result<()> {
        self.add(node, NodeClass::Cloud)
    }

    fn add(&mut self, node: &str, class: NodeClass) -> Result<()> {
        if !valid_id(node) {
            return Err(error(ErrorCode::InvalidArgument, "invalid node reference"));
        }
        if self.nodes.contains_key(node) {
            return Err(error(ErrorCode::Conflict, "node already declared"));
        }
        if self.nodes.len() >= MAX_NODES {
            return Err(error(ErrorCode::ResourceExhausted, "node limit"));
        }
        self.nodes.insert(node.to_owned(), class);
        Ok(())
    }

    /// Declare that `from` depends on `to`. Both must exist, and a node may not
    /// depend on itself: a self-loop would make the transistorily-checked graph
    /// report a dependency whose origin is unclear.
    pub fn depends_on(&mut self, from: &str, to: &str) -> Result<()> {
        if !self.nodes.contains_key(from) || !self.nodes.contains_key(to) {
            return Err(error(ErrorCode::CapabilityAbsent, "node is not declared"));
        }
        if from == to {
            return Err(error(ErrorCode::InvalidArgument, "self dependency"));
        }
        self.edges
            .entry(from.to_owned())
            .or_default()
            .insert(to.to_owned());
        Ok(())
    }

    pub fn class_of(&self, node: &str) -> Option<NodeClass> {
        self.nodes.get(node).copied()
    }

    pub fn contains(&self, node: &str) -> bool {
        self.nodes.contains_key(node)
    }

    /// Every `Cloud` node reachable from `node`, following dependencies. `None`
    /// means the node is not declared, which the audit reports separately from a
    /// genuine cloud dependency.
    pub fn cloud_reach(&self, node: &str) -> Option<BTreeSet<String>> {
        if !self.nodes.contains_key(node) {
            return None;
        }
        let mut pending = vec![node.to_owned()];
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut clouds = BTreeSet::new();
        while let Some(current) = pending.pop() {
            let Some(next) = self.edges.get(&current) else {
                continue;
            };
            for dependency in next {
                if !seen.insert(dependency.clone()) {
                    continue;
                }
                if self.nodes.get(dependency) == Some(&NodeClass::Cloud) {
                    clouds.insert(dependency.clone());
                }
                pending.push(dependency.clone());
            }
        }
        Some(clouds)
    }
}

/// What the autonomy audit found, as plain values.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AutonomyAudit {
    /// The local critical nodes that are declared as `Local`, with the cloud
    /// nodes each of them can reach (empty when the path is truly local).
    pub critical: Vec<(String, Vec<String>)>,
    pub violations: Vec<Violation>,
}

impl AutonomyAudit {
    pub fn is_local(&self) -> bool {
        self.violations.is_empty()
    }
}

/// Check that a set of critical nodes is local **and** has no cloud in its
/// transitive dependency closure.
///
/// Three distinct failures are reported with stable tokens so a report can
/// classify them without a debug build: an undeclared node, a node that is not
/// local at all, and a local node that reaches a cloud node.
pub fn audit_local_autonomy(graph: &LinkGraph, critical: &[&str]) -> AutonomyAudit {
    let mut audit = AutonomyAudit::default();
    let mut reported: BTreeSet<(String, String)> = BTreeSet::new();
    for node in critical {
        if !graph.contains(node) {
            let violation = Violation {
                token: "local critical node is undefined",
                subject: (*node).to_owned(),
            };
            if reported.insert((violation.token.into(), violation.subject.clone())) {
                audit.violations.push(violation);
            }
            continue;
        }
        if graph.class_of(node) != Some(NodeClass::Local) {
            let violation = Violation {
                token: "local critical node is not local",
                subject: (*node).to_owned(),
            };
            if reported.insert((violation.token.into(), violation.subject.clone())) {
                audit.violations.push(violation);
            }
            continue;
        }
        let reach = graph.cloud_reach(node).unwrap_or_default();
        if !reach.is_empty() {
            let violation = Violation {
                token: "local critical node depends on cloud",
                subject: format!(
                    "{node}->{}",
                    reach.iter().cloned().collect::<Vec<_>>().join(",")
                ),
            };
            if reported.insert((violation.token.into(), violation.subject.clone())) {
                audit.violations.push(violation);
            }
        }
        audit
            .critical
            .push(((*node).to_owned(), reach.into_iter().collect()));
    }
    audit.critical.sort();
    audit.violations.sort();
    audit
}

/// Whether the cloud link is up. Supplied by the host, never probed here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudLink {
    Reachable,
    Unreachable,
}

/// Whether a capability may run, given the link and where it lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Available,
    Unavailable { token: &'static str },
}

/// Availability is a function of the link **only for cloud capabilities**. A
/// local capability is available under either link state, which is the A10
/// assertion stated as code rather than prose.
pub fn availability(link: CloudLink, class: NodeClass) -> Availability {
    match (link, class) {
        (_, NodeClass::Local) => Availability::Available,
        (CloudLink::Reachable, NodeClass::Cloud) => Availability::Available,
        (CloudLink::Unreachable, NodeClass::Cloud) => Availability::Unavailable {
            token: "cloud link unreachable",
        },
    }
}

/// The gate state of one capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityMode {
    /// Never activated, or deactivated by a link loss.
    Disabled,
    /// Explicitly activated and currently running.
    Active,
    /// The link came back, but the capability must pass Prepare and permission
    /// review again before it may run. §27.3 forbids resuming the old Auto state.
    NeedsPrepare,
}

/// What one link transition changed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LinkTransition {
    /// Cloud capabilities taken out of `Active`.
    pub deactivated: Vec<String>,
    /// Cloud capabilities parked awaiting Prepare after the link returned.
    pub needs_prepare: Vec<String>,
}

/// Tracks which capabilities are cloud-hosted and what mode they are in.
#[derive(Debug, Clone, Default)]
pub struct CapabilityGate {
    classes: BTreeMap<String, NodeClass>,
    modes: BTreeMap<String, CapabilityMode>,
}

impl CapabilityGate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, capability: &str, class: NodeClass) -> Result<()> {
        if !valid_id(capability) {
            return Err(error(
                ErrorCode::InvalidArgument,
                "invalid capability reference",
            ));
        }
        if self.classes.contains_key(capability) {
            return Err(error(ErrorCode::Conflict, "capability already registered"));
        }
        self.classes.insert(capability.to_owned(), class);
        self.modes
            .insert(capability.to_owned(), CapabilityMode::Disabled);
        Ok(())
    }

    pub fn mode(&self, capability: &str) -> Option<CapabilityMode> {
        self.modes.get(capability).copied()
    }

    /// Bring a capability up. Only a deliberate activation reaches `Active`;
    /// `NeedsPrepare` is not `Active` until this is called, which is how "不自动
    /// 恢复旧的 Auto 状态" is enforced by the type rather than by convention.
    pub fn activate(&mut self, capability: &str) -> Result<()> {
        let mode = self
            .modes
            .get_mut(capability)
            .ok_or_else(|| error(ErrorCode::CapabilityAbsent, "capability is not registered"))?;
        if *mode == CapabilityMode::Active {
            return Err(error(ErrorCode::Conflict, "capability already active"));
        }
        *mode = CapabilityMode::Active;
        Ok(())
    }

    /// The link dropped. Every active **cloud** capability is deactivated; local
    /// capabilities are returned untouched, which is the point.
    pub fn disconnect(&mut self) -> LinkTransition {
        let mut transition = LinkTransition::default();
        for (capability, class) in &self.classes {
            if *class != NodeClass::Cloud {
                continue;
            }
            let mode = self.modes.get_mut(capability).expect("mode registered");
            if *mode == CapabilityMode::Active {
                *mode = CapabilityMode::Disabled;
                transition.deactivated.push(capability.clone());
            }
        }
        transition.deactivated.sort();
        transition
    }

    /// The link returned. Cloud capabilities go to `NeedsPrepare`, never straight
    /// to `Active`: recovery enters Prepare and a permission review.
    pub fn reconnect(&mut self) -> LinkTransition {
        let mut transition = LinkTransition::default();
        for (capability, class) in &self.classes {
            if *class != NodeClass::Cloud {
                continue;
            }
            let mode = self.modes.get_mut(capability).expect("mode registered");
            if *mode == CapabilityMode::Disabled {
                *mode = CapabilityMode::NeedsPrepare;
                transition.needs_prepare.push(capability.clone());
            }
        }
        transition.needs_prepare.sort();
        transition
    }
}

/// The instances and load a measurement was taken at. Writing a capacity number
/// without this is the failure A10 names, so it is not optional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Measurement {
    /// The declared instance count the numbers were measured at.
    pub instances: u32,
    /// The declared load percentage the numbers were measured at (1..=100).
    pub load_pct: u32,
}

/// Whether a capacity figure was measured, and at what.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sizing {
    Measured(Measurement),
    Unmeasured,
}

/// A measured low-cloud capacity figure.
///
/// The struct has no public constructor that skips [`Sizing`]: "不能写未经测量
/// 容量" is a property of the type, not a review step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capacity {
    pub connections: u64,
    pub memory_mib: u64,
    pub latency_ms: u64,
    pub measurement: Measurement,
}

impl Capacity {
    /// Declare a capacity figure. `Sizing::Unmeasured` is **refused**; there is
    /// no zero-default, because a zero written into a plan reads as a measurement.
    pub fn declare(
        connections: u64,
        memory_mib: u64,
        latency_ms: u64,
        sizing: Sizing,
    ) -> Result<Self> {
        let Sizing::Measured(measurement) = sizing else {
            return Err(error(
                ErrorCode::CapabilityNotReady,
                "capacity is unmeasured",
            ));
        };
        if measurement.instances == 0 || measurement.load_pct == 0 || measurement.load_pct > 100 {
            return Err(error(
                ErrorCode::InvalidArgument,
                "measurement must name instances and a load within 1..=100",
            ));
        }
        Ok(Self {
            connections,
            memory_mib,
            latency_ms,
            measurement,
        })
    }
}

/// The ceilings a low-cloud profile promises not to exceed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacityBudget {
    pub max_connections: u64,
    pub max_memory_mib: u64,
    pub max_latency_ms: u64,
}

/// The outcome of comparing a capacity against a budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapacityVerdict {
    Within,
    /// Over one or more ceilings. The breach families are stable tokens.
    Throttled {
        breaches: Vec<&'static str>,
    },
}

/// Compare a measured capacity against a budget. The first ceiling that is
/// exceeded does not hide the others: every breach is reported.
pub fn assess(capacity: &Capacity, budget: &CapacityBudget) -> CapacityVerdict {
    let mut breaches = Vec::new();
    if capacity.connections > budget.max_connections {
        breaches.push("connections over budget");
    }
    if capacity.memory_mib > budget.max_memory_mib {
        breaches.push("memory over budget");
    }
    if capacity.latency_ms > budget.max_latency_ms {
        breaches.push("latency over budget");
    }
    if breaches.is_empty() {
        CapacityVerdict::Within
    } else {
        CapacityVerdict::Throttled { breaches }
    }
}

/// What a profile does when a budget is breached. `Absorb` is the state the
/// audit exists to catch: the specification says a breach **may** throttle, and
/// silently absorbing it is how a measured ceiling becomes an unmeasured one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThrottlePolicy {
    RateLimit,
    Absorb,
}

/// A low-cloud profile: a capacity, a budget, and what happens on breach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LowCloudProfile {
    pub capacity: Capacity,
    pub budget: CapacityBudget,
    pub policy: ThrottlePolicy,
}

/// Audit a profile. A breach under `RateLimit` is fine; a breach under `Absorb`
/// is a violation, because the profile is then claiming a ceiling it will not
/// enforce.
pub fn audit_low_cloud(profile: &LowCloudProfile) -> Vec<Violation> {
    let mut violations = Vec::new();
    let breached = matches!(
        assess(&profile.capacity, &profile.budget),
        CapacityVerdict::Throttled { .. }
    );
    if breached && profile.policy == ThrottlePolicy::Absorb {
        violations.push(Violation {
            token: "budget breach absorbed instead of throttled",
            subject: "low-cloud profile".to_owned(),
        });
    }
    violations
}
