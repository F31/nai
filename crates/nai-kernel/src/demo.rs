//! Demo package readiness (V3.7 §15, §615, §612; registry O01).
//!
//! O01's acceptance is an *availability goal*: "about ten minutes to the first
//! rehearsal utterance, about thirty minutes to complete one real output
//! verification" (§612). The artefact that goal is demonstrated with is the
//! **demo package** (§615): a licensed demo avatar, sample knowledge, a sample
//! scenario, plus provider presets, a one-click connectivity test and a cost
//! estimate.
//!
//! The truth of O01 — a real ten-minute drill against a real endpoint — needs
//! external conditions. What can be judged here is whether a *declared* demo
//! package is complete and honest, and whether a *declared* drill record is
//! self-consistent and within the targets. [`audit_demo_package`] is a pure
//! function of the descriptor: it opens no endpoint, reads no file and measures
//! no clock.
//!
//! Three rules are load-bearing, and each has an analogue elsewhere in the
//! kernel:
//!
//! - **Not declared is conditional.** An empty descriptor audits empty and
//!   reads [`DemoReadiness::Blocked`], the honest "not started" state. A
//!   half-declared package is a *defect*: once you ship part of it, the rest is
//!   owed.
//! - **"Licensed" is a requirement, not an adjective.** §615 says *licensed*
//!   demo avatar; an avatar with no licence reference is exactly what that
//!   word is there to catch.
//! - **A reachable endpoint is not an acceptance.** A one-click connectivity
//!   test must be a live probe, and its success must never be recorded as an
//!   acceptance result (the same discipline W36/W37 fixed for provider smokes).
use crate::golden::Finding;

/// §612: about ten minutes to the first rehearsal utterance.
pub const FIRST_UTTERANCE_TARGET_MS: u64 = 10 * 60 * 1000;

/// §612: about thirty minutes to complete one real output verification.
pub const REAL_OUTPUT_TARGET_MS: u64 = 30 * 60 * 1000;

/// §615: the licensed demo avatar. The licence reference is the auditable half
/// of the word "licensed".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoAvatar {
    pub profile_ref: String,
    /// Absent means the avatar is shipped without a licence reference, which
    /// §615 does not allow for a demo package.
    pub license_ref: Option<String>,
}

impl DemoAvatar {
    /// An avatar with its licence reference — the only shape §615 accepts.
    pub fn licensed(profile_ref: impl Into<String>, license_ref: impl Into<String>) -> Self {
        Self {
            profile_ref: profile_ref.into(),
            license_ref: Some(license_ref.into()),
        }
    }

    /// An avatar with no licence reference, for exercising the audit.
    pub fn unlicensed(profile_ref: impl Into<String>) -> Self {
        Self {
            profile_ref: profile_ref.into(),
            license_ref: None,
        }
    }
}

/// §615: one sample knowledge item shipped with the demo package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeSeed {
    pub id: String,
}

impl KnowledgeSeed {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

/// §615: the sample scenario the demo package opens with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoScenario {
    pub id: String,
}

impl DemoScenario {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

/// §615: a provider preset shipped with the demo package. It points at a
/// credential by reference (`schemas/config/provider.schema.json` requires
/// `credential_ref`) and never carries the secret itself (S07).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPreset {
    pub id: String,
    /// The credential reference. A preset with neither a reference nor an
    /// embedded secret cannot obtain a credential at all.
    pub credential_ref: Option<String>,
    /// Present only so the audit can catch a preset that embeds a secret.
    pub embeds_secret: bool,
}

impl ProviderPreset {
    /// A preset pointing at its credential by reference.
    pub fn referencing(id: impl Into<String>, credential_ref: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            credential_ref: Some(credential_ref.into()),
            embeds_secret: false,
        }
    }

    /// A preset carrying its secret inline, for exercising the audit.
    pub fn embedding_secret(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            credential_ref: None,
            embeds_secret: true,
        }
    }

    /// A preset with no credential at all, for exercising the audit.
    pub fn without_credential(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            credential_ref: None,
            embeds_secret: false,
        }
    }
}

/// How a one-click connectivity test obtains its result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeKind {
    /// A real call against the configured endpoint.
    Live,
    /// A constant result that exercises nothing.
    Constant,
}

/// §615: the one-click connectivity test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectivityTest {
    pub probe: ProbeKind,
    /// Set when a connectivity success is recorded as an acceptance result.
    pub claims_acceptance: bool,
}

impl ConnectivityTest {
    /// A live probe that does not over-claim its result.
    pub fn live() -> Self {
        Self {
            probe: ProbeKind::Live,
            claims_acceptance: false,
        }
    }

    /// A constant stub, for exercising the audit.
    pub fn constant() -> Self {
        Self {
            probe: ProbeKind::Constant,
            claims_acceptance: false,
        }
    }

    /// Mark this test as presenting its result as an acceptance.
    pub fn claiming_acceptance(mut self) -> Self {
        self.claims_acceptance = true;
        self
    }
}

/// One line of the demo package's cost estimate. A component that cannot be
/// estimated is [`CostLine::Unknown`] with its reason, never `Known(0)` — the
/// same rule O05 fixes for the upgrade cost display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostLine {
    Known(u64),
    Unknown(&'static str),
}

/// §615: the demo package's cost estimate, per component.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CostDeclaration {
    pub lines: Vec<(&'static str, CostLine)>,
}

impl CostDeclaration {
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

    /// Add a component in report order.
    pub fn with(mut self, name: &'static str, line: CostLine) -> Self {
        self.lines.push((name, line));
        self
    }
}

/// §612: a record of one rehearsal drill. Every field is a *claim* by whoever
/// ran the drill; this module checks the claim's self-consistency and its
/// distance from the targets, and never measures the clock itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrillRecord {
    /// When installation was reported ready.
    pub install_ready_at_ms: u64,
    /// When the first rehearsal utterance was heard.
    pub first_utterance_at_ms: Option<u64>,
    /// When one real output verification finished.
    pub real_output_verified_at_ms: Option<u64>,
}

impl DrillRecord {
    /// A complete drill record.
    pub fn complete(
        install_ready_at_ms: u64,
        first_utterance_at_ms: u64,
        real_output_verified_at_ms: u64,
    ) -> Self {
        Self {
            install_ready_at_ms,
            first_utterance_at_ms: Some(first_utterance_at_ms),
            real_output_verified_at_ms: Some(real_output_verified_at_ms),
        }
    }
}

/// §615 + §612: a declared demo package and the drill record that demonstrates
/// its availability goal. The audit judges this descriptor; it demonstrates
/// nothing by itself.
#[derive(Debug, Clone, Default)]
pub struct DemoPackage {
    pub avatar: Option<DemoAvatar>,
    pub knowledge: Vec<KnowledgeSeed>,
    pub scenario: Option<DemoScenario>,
    pub provider_presets: Vec<ProviderPreset>,
    pub connectivity: Option<ConnectivityTest>,
    pub cost: Option<CostDeclaration>,
    pub drill: Option<DrillRecord>,
}

impl DemoPackage {
    /// A package with nothing declared at all. It audits empty and reads as
    /// [`DemoReadiness::Blocked`], the honest O01 state.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Whether *anything* about the package has been stated. An undeclared
    /// package owes nothing yet; a declared one owes every remaining part.
    pub fn is_declared(&self) -> bool {
        self.avatar.is_some()
            || !self.knowledge.is_empty()
            || self.scenario.is_some()
            || !self.provider_presets.is_empty()
            || self.connectivity.is_some()
            || self.cost.is_some()
            || self.drill.is_some()
    }

    /// `Defective` takes precedence: a package that violates a boundary must
    /// not be read as merely "not rehearsed yet".
    pub fn readiness(&self) -> DemoReadiness {
        if !audit_demo_package(self).is_empty() {
            return DemoReadiness::Defective;
        }
        if !self.is_declared() || self.drill.is_none() {
            return DemoReadiness::Blocked;
        }
        DemoReadiness::Ready
    }
}

/// Readiness of the declared demo package. `Blocked` is the honest state for
/// O01 while no package is declared or no drill has been recorded (§1261: an
/// unrun acceptance is marked Blocked, never rendered as a pass).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoReadiness {
    /// Nothing declared, or declared but never rehearsed.
    Blocked,
    /// Declared, and a boundary is violated.
    Defective,
    /// Declared, complete, honest, and rehearsed within the targets.
    Ready,
}

/// Audit a declared demo package. A pure function of the descriptor: every
/// finding is directly constructible, and nothing external is touched.
pub fn audit_demo_package(package: &DemoPackage) -> Vec<Finding> {
    let mut findings = Vec::new();

    // §615: nothing declared owes nothing. This is the conditional rule — an
    // unbuilt demo package is "not started", not "in violation".
    if !package.is_declared() {
        return findings;
    }

    audit_avatar(package.avatar.as_ref(), &mut findings);
    audit_connectivity(package.connectivity.as_ref(), &mut findings);
    audit_cost(package.cost.as_ref(), &mut findings);

    if package.knowledge.is_empty() {
        findings.push(Finding::new("demo_knowledge_missing", "knowledge"));
    }
    if package.scenario.is_none() {
        findings.push(Finding::new("demo_scenario_missing", "scenario"));
    }

    audit_presets(&package.provider_presets, &mut findings);

    if let Some(drill) = &package.drill {
        audit_drill(drill, &mut findings);
    }

    findings
}

/// §615: the avatar exists *and* is licensed.
fn audit_avatar(avatar: Option<&DemoAvatar>, findings: &mut Vec<Finding>) {
    match avatar {
        None => findings.push(Finding::new("demo_avatar_missing", "avatar")),
        Some(avatar) if avatar.license_ref.is_none() => findings.push(Finding::new(
            "demo_avatar_without_license",
            avatar.profile_ref.clone(),
        )),
        Some(_) => {}
    }
}

/// §615: provider presets are present, point at a credential by reference, and
/// never embed a secret.
fn audit_presets(presets: &[ProviderPreset], findings: &mut Vec<Finding>) {
    if presets.is_empty() {
        findings.push(Finding::new(
            "demo_provider_presets_missing",
            "provider_presets",
        ));
    }
    for preset in presets {
        if preset.embeds_secret {
            findings.push(Finding::new(
                "provider_preset_embeds_secret",
                preset.id.clone(),
            ));
        }
        if preset.credential_ref.is_none() && !preset.embeds_secret {
            findings.push(Finding::new(
                "provider_preset_without_credential",
                preset.id.clone(),
            ));
        }
    }
}

/// §615: a one-click connectivity test exists, probes for real, and does not
/// present its result as an acceptance.
fn audit_connectivity(test: Option<&ConnectivityTest>, findings: &mut Vec<Finding>) {
    let Some(test) = test else {
        findings.push(Finding::new("demo_connectivity_missing", "connectivity"));
        return;
    };
    if test.probe == ProbeKind::Constant {
        findings.push(Finding::new("connectivity_probe_is_stub", "connectivity"));
    }
    if test.claims_acceptance {
        findings.push(Finding::new("smoke_claims_acceptance", "connectivity"));
    }
}

/// §615: the cost estimate exists and every unknown component names its reason.
fn audit_cost(cost: Option<&CostDeclaration>, findings: &mut Vec<Finding>) {
    let Some(cost) = cost else {
        findings.push(Finding::new("demo_cost_estimate_missing", "cost"));
        return;
    };
    for (name, line) in &cost.lines {
        if let CostLine::Unknown(reason) = line
            && reason.trim().is_empty()
        {
            findings.push(Finding::new("cost_unknown_without_reason", *name));
        }
    }
}

/// §612: a drill record is complete, self-consistent, and within the targets.
fn audit_drill(drill: &DrillRecord, findings: &mut Vec<Finding>) {
    let (Some(first), Some(real)) = (
        drill.first_utterance_at_ms,
        drill.real_output_verified_at_ms,
    ) else {
        findings.push(Finding::new("drill_record_incomplete", "drill"));
        return;
    };

    // A timestamp earlier than installation-ready cannot have happened; the
    // record is inconsistent rather than merely slow.
    for (milestone, at) in [("first_utterance", first), ("real_output", real)] {
        if at < drill.install_ready_at_ms {
            findings.push(Finding::new("drill_timeline_inconsistent", milestone));
        }
    }

    if first >= drill.install_ready_at_ms {
        let elapsed = first - drill.install_ready_at_ms;
        if elapsed > FIRST_UTTERANCE_TARGET_MS {
            findings.push(Finding::new(
                "first_utterance_beyond_target",
                elapsed.to_string(),
            ));
        }
    }
    if real >= drill.install_ready_at_ms {
        let elapsed = real - drill.install_ready_at_ms;
        if elapsed > REAL_OUTPUT_TARGET_MS {
            findings.push(Finding::new(
                "real_output_beyond_target",
                elapsed.to_string(),
            ));
        }
    }
}
