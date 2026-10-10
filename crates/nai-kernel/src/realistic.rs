//! §8.2 realistic-model admission Gate G0–G3: the verdicts, and what counts as
//! a measurement.
//!
//! §8.2 puts four gates in a table — G0 source and rights, G1 correctness,
//! G2 real-time, G3 stability — and B02 asks for the evidence of all four to be
//! complete (§655). §26.5 adds two rules that shape G1 and G2, and §8.4 / §8.2
//! add one that shapes the whole thing: *automatic fallback to 2D is not
//! realistic acceptance*.
//!
//! This module is the **verdict**, not the evidence. It never loads a model,
//! runs inference or touches a GPU; it is handed a report and decides, per gate
//! and overall, whether the report *measures up*. The real truth — a legal
//! complete model and a Windows/CUDA target machine — stays external, and when
//! it is absent the whole admission is [`Acceptance::Blocked`] (§1311).
//!
//! Four rules shape the types.
//!
//! First, **the four gates are a conjunction of four independent verdicts**.
//! Each is [`GateVerdict::Passed`], [`GateVerdict::Failed`] or
//! [`GateVerdict::Unmeasured`], and no method folds them into a score — a score
//! could not say *which* gate is holding the admission back.
//!
//! Second, **"not measured" is neither a pass nor a zero**. A frame rate with
//! no windows, a skew with fewer than [`MIN_PERCENTILE_SAMPLES`] samples, a
//! temperature that was never read: each is `Unmeasured`, and any `Unmeasured`
//! gate keeps the admission out of [`Acceptance::Accepted`].
//!
//! Third, **full GPU is a property of the placement records, not of a runtime
//! receipt**. [`PlacementReport::is_full_gpu`] looks only at the recorded node
//! placements; a successful CUDA EP registration is an audit clue, never a
//! placement (§26.5 forbids claiming full GPU from it).
//!
//! Fourth, **a 2D fallback is an operational record, orthogonal to the gate**.
//! It never enters [`GateReport::verdict`] or [`GateReport::acceptance`]; only
//! *claiming* it as acceptance is a violation.
use crate::golden::Finding;
use crate::metrics::{LatencySamples, MetricsError, Percentile};
use nai_contracts::v1::ErrorCode;
use std::collections::BTreeMap;

fn error(code: ErrorCode, reason: &'static str) -> MetricsError {
    MetricsError { code, reason }
}

type Result<T> = std::result::Result<T, MetricsError>;

/// §12.5: realistic output must hold a stable 25fps.
pub const MIN_STABLE_FPS: u64 = 25;

/// §12.5: the frame/audio skew P95 must stay ≤ 80ms at the designated tier.
pub const FRAME_SKEW_P95_LIMIT_MS: u64 = 80;

/// §8.2's four gates. They are independent: each has its own evidence and its
/// own verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Gate {
    /// G0 source and rights.
    SourceAndRights,
    /// G1 correctness.
    Correctness,
    /// G2 real-time.
    Realtime,
    /// G3 stability.
    Stability,
}

impl Gate {
    pub const ALL: [Gate; 4] = [
        Gate::SourceAndRights,
        Gate::Correctness,
        Gate::Realtime,
        Gate::Stability,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Gate::SourceAndRights => "g0_source_and_rights",
            Gate::Correctness => "g1_correctness",
            Gate::Realtime => "g2_realtime",
            Gate::Stability => "g3_stability",
        }
    }
}

/// The verdict of one gate. There is no fourth state: "not applicable" is a
/// property of the whole admission for a profile that does not enable realistic
/// (§8.1), decided in [`GateReport::acceptance`], never of a single gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateVerdict {
    Passed,
    Failed,
    /// Not enough evidence to decide. It is **not** a pass.
    Unmeasured,
}

impl GateVerdict {
    pub fn label(self) -> &'static str {
        match self {
            GateVerdict::Passed => "passed",
            GateVerdict::Failed => "failed",
            GateVerdict::Unmeasured => "unmeasured",
        }
    }
}

/// §8.1: 2D is the Lite default; realistic (Standard) is the Standard/Pro
/// addition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Lite,
    Standard,
    Pro,
}

impl Profile {
    /// Whether this profile makes realistic output a target. §8.1: realistic is
    /// a Standard/Pro capability, so only there is "not applicable" a false
    /// claim when it stands for an unfinished target (§1311).
    pub fn requires_realistic(self) -> bool {
        matches!(self, Profile::Standard | Profile::Pro)
    }

    pub fn label(self) -> &'static str {
        match self {
            Profile::Lite => "lite",
            Profile::Standard => "standard",
            Profile::Pro => "pro",
        }
    }
}

/// The overall admission, derived from the four gates. There is no weighted
/// score and no "partial pass".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acceptance {
    /// All four gates passed and a model is present.
    Accepted,
    /// At least one gate failed.
    NotAccepted,
    /// No gate failed, but at least one was not measured.
    Unmeasured,
    /// No legal complete model. §1311: a missing model is `Blocked`, never
    /// "not applicable", and no gate may be waived into acceptance.
    Blocked,
    /// The profile does not enable realistic at all (§8.1).
    NotApplicable,
}

impl Acceptance {
    pub fn label(self) -> &'static str {
        match self {
            Acceptance::Accepted => "accepted",
            Acceptance::NotAccepted => "not_accepted",
            Acceptance::Unmeasured => "unmeasured",
            Acceptance::Blocked => "blocked",
            Acceptance::NotApplicable => "not_applicable",
        }
    }
}

// ---------------------------------------------------------------- G0: rights

/// §8.2 G0: the five rights that must each hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RightKind {
    Weights,
    Code,
    TrainingData,
    Portrait,
    Voice,
}

impl RightKind {
    pub const ALL: [RightKind; 5] = [
        RightKind::Weights,
        RightKind::Code,
        RightKind::TrainingData,
        RightKind::Portrait,
        RightKind::Voice,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RightKind::Weights => "weights",
            RightKind::Code => "code",
            RightKind::TrainingData => "training_data",
            RightKind::Portrait => "portrait",
            RightKind::Voice => "voice",
        }
    }
}

/// The state of one right. `Established` carries the *basis* it rests on, so a
/// right with a blank basis is not established — a licence reference merely
/// existing is not the evidence (the D12 report says as much).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RightStatus {
    Established {
        basis: String,
    },
    /// A reference exists but nothing backs it, or it was not checked.
    Unverified,
    /// No right claimed at all.
    Absent,
}

/// §8.2 G0 evidence.
#[derive(Debug, Clone, Default)]
pub struct RightsEvidence {
    statuses: BTreeMap<RightKind, RightStatus>,
    /// §8.2: the full pipeline must be obtainable.
    pub pipeline_obtainable: bool,
    /// §8.2: a missing component must not be left in an undeclared Python
    /// service.
    pub undeclared_components: Vec<String>,
}

impl RightsEvidence {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, kind: RightKind, status: RightStatus) {
        self.statuses.insert(kind, status);
    }

    pub fn status(&self, kind: RightKind) -> Option<&RightStatus> {
        self.statuses.get(&kind)
    }

    /// A right is established only when its basis is non-blank.
    pub fn established(&self, kind: RightKind) -> bool {
        matches!(
            self.status(kind),
            Some(RightStatus::Established { basis }) if !basis.trim().is_empty()
        )
    }

    /// Rights that are absent (or not recorded at all). A definite gap, as
    /// opposed to a right that is merely unverified.
    pub fn missing(&self) -> Vec<RightKind> {
        RightKind::ALL
            .into_iter()
            .filter(|kind| matches!(self.status(*kind), None | Some(RightStatus::Absent)))
            .collect()
    }

    pub fn verdict(&self) -> GateVerdict {
        if !self.undeclared_components.is_empty() {
            return GateVerdict::Failed;
        }
        if !self.missing().is_empty() {
            return GateVerdict::Failed;
        }
        if !self.pipeline_obtainable {
            return GateVerdict::Failed;
        }
        if RightKind::ALL.iter().all(|kind| self.established(*kind)) {
            return GateVerdict::Passed;
        }
        GateVerdict::Unmeasured
    }
}

// ----------------------------------------------------------- G1: correctness

/// §8.2 G1: the four stages that must align segment by segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    Preprocess,
    AudioFeatures,
    Generation,
    Postprocess,
}

impl Stage {
    pub const ALL: [Stage; 4] = [
        Stage::Preprocess,
        Stage::AudioFeatures,
        Stage::Generation,
        Stage::Postprocess,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Stage::Preprocess => "preprocess",
            Stage::AudioFeatures => "audio_features",
            Stage::Generation => "generation",
            Stage::Postprocess => "postprocess",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageAlignment {
    Aligned,
    Diverged { max_abs_delta: u64 },
    NotRun,
}

/// §26.5: FP32 first, then FP16; INT8 needs quantization and calibration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Precision {
    Fp32,
    Fp16,
    Int8,
}

impl Precision {
    pub fn label(self) -> &'static str {
        match self {
            Precision::Fp32 => "fp32",
            Precision::Fp16 => "fp16",
            Precision::Int8 => "int8",
        }
    }
}

/// §8.2 G1 evidence.
#[derive(Debug, Clone, Default)]
pub struct CorrectnessEvidence {
    alignments: BTreeMap<Stage, StageAlignment>,
    /// §8.2: alignment happens on a fixed input set, which must be named.
    pub fixed_inputs: String,
    certified_precisions: Vec<Precision>,
    /// §26.5: INT8 certification requires a quantization/calibration report.
    pub int8_calibration_report: Option<String>,
    pub undeclared_components: Vec<String>,
}

impl CorrectnessEvidence {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, stage: Stage, alignment: StageAlignment) {
        self.alignments.insert(stage, alignment);
    }

    pub fn alignment(&self, stage: Stage) -> Option<StageAlignment> {
        self.alignments.get(&stage).copied()
    }

    pub fn certify(&mut self, precision: Precision) {
        if !self.certified_precisions.contains(&precision) {
            self.certified_precisions.push(precision);
        }
    }

    pub fn certifies(&self, precision: Precision) -> bool {
        self.certified_precisions.contains(&precision)
    }

    pub fn verdict(&self) -> GateVerdict {
        if !self.undeclared_components.is_empty() {
            return GateVerdict::Failed;
        }
        if Stage::ALL.iter().any(|stage| {
            matches!(
                self.alignment(*stage),
                Some(StageAlignment::Diverged { .. })
            )
        }) {
            return GateVerdict::Failed;
        }
        // §26.5: no precision beyond FP32 is certified before FP32 aligns.
        if (self.certifies(Precision::Fp16) || self.certifies(Precision::Int8))
            && !self.certifies(Precision::Fp32)
        {
            return GateVerdict::Failed;
        }
        let int8_unbacked = self.certifies(Precision::Int8)
            && self
                .int8_calibration_report
                .as_deref()
                .is_none_or(|report| report.trim().is_empty());
        if int8_unbacked {
            return GateVerdict::Failed;
        }
        if self.fixed_inputs.trim().is_empty() {
            return GateVerdict::Unmeasured;
        }
        if Stage::ALL
            .iter()
            .any(|stage| !matches!(self.alignment(*stage), Some(StageAlignment::Aligned)))
        {
            return GateVerdict::Unmeasured;
        }
        GateVerdict::Passed
    }
}

// -------------------------------------------------------------- G2: realtime

/// One measurement window: how many frames were produced in how long.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameWindow {
    pub frames: u64,
    pub elapsed_ms: u64,
}

impl FrameWindow {
    /// Frames per second in thousandths (`frames * 1_000_000 / elapsed_ms`), so
    /// 25 fps is `25_000`. `None` when no time elapsed.
    pub fn fps_permille(&self) -> Option<u64> {
        if self.elapsed_ms == 0 {
            return None;
        }
        Some(self.frames.saturating_mul(1_000_000) / self.elapsed_ms)
    }
}

/// A series of frame windows. §12.5 asks for a *stable* 25fps, so the criterion
/// is the floor across windows, not an average that a burst could carry.
#[derive(Debug, Clone, Default)]
pub struct FrameRate {
    windows: Vec<FrameWindow>,
}

impl FrameRate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, frames: u64, elapsed_ms: u64) -> Result<()> {
        if elapsed_ms == 0 {
            return Err(error(ErrorCode::InvalidArgument, "zero frame window"));
        }
        self.windows.push(FrameWindow { frames, elapsed_ms });
        Ok(())
    }

    pub fn windows(&self) -> &[FrameWindow] {
        &self.windows
    }

    /// The lowest window rate, in thousandths. `None` when nothing was measured.
    pub fn min_fps_permille(&self) -> Option<u64> {
        self.windows
            .iter()
            .filter_map(FrameWindow::fps_permille)
            .min()
    }

    /// Whether every window holds [`MIN_STABLE_FPS`]. `None` when unmeasured.
    pub fn meets_min_fps(&self) -> Option<bool> {
        self.min_fps_permille()
            .map(|permille| permille >= MIN_STABLE_FPS * 1_000)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    Gpu,
    Cpu,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodePlacement {
    pub node: String,
    pub device: Device,
}

/// §26.5: record the actual node placement, CPU fallbacks and GPU copies;
/// never claim full GPU from a successful CUDA EP registration.
#[derive(Debug, Clone, Default)]
pub struct PlacementReport {
    /// A runtime receipt. It is an audit clue, **not** placement evidence.
    pub cuda_ep_registered: bool,
    pub node_placements: Vec<NodePlacement>,
    pub gpu_copies: u64,
    /// The caller's claim, checked against the records — not trusted.
    pub claims_full_gpu: bool,
}

impl PlacementReport {
    pub fn cpu_fallbacks(&self) -> usize {
        self.node_placements
            .iter()
            .filter(|placement| placement.device == Device::Cpu)
            .count()
    }

    /// Full GPU is a property of the placement records: there must be records,
    /// and every one of them must be on the GPU.
    pub fn is_full_gpu(&self) -> bool {
        !self.node_placements.is_empty()
            && self
                .node_placements
                .iter()
                .all(|placement| placement.device == Device::Gpu)
    }

    /// A claim of full GPU that the records do not support.
    pub fn falsely_claims_full_gpu(&self) -> bool {
        self.claims_full_gpu && !self.is_full_gpu()
    }
}

/// §8.2 G2 evidence.
#[derive(Debug, Default)]
pub struct RealtimeEvidence {
    /// §8.2: measured on the target machine.
    pub target_machine: String,
    /// §8.2: measured while the live tool runs alongside.
    pub concurrent_with_live_tool: bool,
    pub frame_rate: Option<FrameRate>,
    /// §12.5 frame/audio skew, as an end-to-end sample set.
    pub frame_skew: LatencySamples,
    pub vram_mb: Option<u64>,
    pub cpu_pct: Option<u64>,
    pub temperature_c: Option<u64>,
    pub placement: PlacementReport,
}

impl RealtimeEvidence {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn verdict(&self) -> GateVerdict {
        if self.placement.falsely_claims_full_gpu() {
            return GateVerdict::Failed;
        }
        if !self.concurrent_with_live_tool {
            return GateVerdict::Failed;
        }
        if self.target_machine.trim().is_empty() {
            return GateVerdict::Unmeasured;
        }
        match self.frame_rate.as_ref().and_then(FrameRate::meets_min_fps) {
            None => return GateVerdict::Unmeasured,
            Some(false) => return GateVerdict::Failed,
            Some(true) => {}
        }
        match self.frame_skew.percentile(Percentile::P95) {
            Err(_) => return GateVerdict::Unmeasured,
            Ok(p95) if p95 > FRAME_SKEW_P95_LIMIT_MS => return GateVerdict::Failed,
            Ok(_) => {}
        }
        if self.vram_mb.is_none() || self.cpu_pct.is_none() || self.temperature_c.is_none() {
            return GateVerdict::Unmeasured;
        }
        GateVerdict::Passed
    }
}

// ------------------------------------------------------------- G3: stability

/// §8.2 G3: the seven situations that must be survived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StabilityScenario {
    Mute,
    Cancel,
    SentenceBoundary,
    LongBroadcast,
    Oom,
    ProcessRestart,
    AvatarSwitchAndOutputRecovery,
}

impl StabilityScenario {
    pub const ALL: [StabilityScenario; 7] = [
        StabilityScenario::Mute,
        StabilityScenario::Cancel,
        StabilityScenario::SentenceBoundary,
        StabilityScenario::LongBroadcast,
        StabilityScenario::Oom,
        StabilityScenario::ProcessRestart,
        StabilityScenario::AvatarSwitchAndOutputRecovery,
    ];

    pub fn label(self) -> &'static str {
        match self {
            StabilityScenario::Mute => "mute",
            StabilityScenario::Cancel => "cancel",
            StabilityScenario::SentenceBoundary => "sentence_boundary",
            StabilityScenario::LongBroadcast => "long_broadcast",
            StabilityScenario::Oom => "oom",
            StabilityScenario::ProcessRestart => "process_restart",
            StabilityScenario::AvatarSwitchAndOutputRecovery => "avatar_switch_and_output_recovery",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScenarioResult {
    /// Passed, with the evidence it rests on. A blank evidence reference is not
    /// a pass — it is a claim without support.
    Passed {
        evidence: String,
    },
    Failed {
        reason: String,
    },
    NotRun,
}

impl ScenarioResult {
    /// A `Passed` result whose evidence reference is blank.
    pub fn is_unbacked_pass(&self) -> bool {
        matches!(self, ScenarioResult::Passed { evidence } if evidence.trim().is_empty())
    }
}

/// §8.2 G3 evidence.
#[derive(Debug, Clone, Default)]
pub struct StabilityEvidence {
    results: BTreeMap<StabilityScenario, ScenarioResult>,
}

impl StabilityEvidence {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, scenario: StabilityScenario, result: ScenarioResult) {
        self.results.insert(scenario, result);
    }

    pub fn result(&self, scenario: StabilityScenario) -> Option<&ScenarioResult> {
        self.results.get(&scenario)
    }

    /// Scenarios reported as passed while carrying no evidence.
    pub fn unbacked_passes(&self) -> Vec<StabilityScenario> {
        StabilityScenario::ALL
            .into_iter()
            .filter(|scenario| {
                self.result(*scenario)
                    .is_some_and(ScenarioResult::is_unbacked_pass)
            })
            .collect()
    }

    pub fn verdict(&self) -> GateVerdict {
        let mut any_failed = false;
        let mut any_unmeasured = false;
        for scenario in StabilityScenario::ALL {
            match self.result(scenario) {
                Some(ScenarioResult::Failed { .. }) => any_failed = true,
                Some(result) if result.is_unbacked_pass() => any_failed = true,
                Some(ScenarioResult::Passed { .. }) => {}
                Some(ScenarioResult::NotRun) | None => any_unmeasured = true,
            }
        }
        if any_failed {
            GateVerdict::Failed
        } else if any_unmeasured {
            GateVerdict::Unmeasured
        } else {
            GateVerdict::Passed
        }
    }
}

// ---------------------------------------------------------------- fallback

/// §8.4: realistic may switch to 2D mid-sentence only with prior operator
/// authorization, at a sentence boundary, and it must be logged. This is an
/// operational record — it is orthogonal to the admission (§8.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackRecord {
    pub reason: String,
    pub at_sentence_boundary: bool,
    pub operator_authorized: bool,
    pub logged: bool,
}

// ------------------------------------------------------------- the report

/// Everything the four gates decide on, plus the two report-level claims that
/// the audit checks. The fields are public so a report can be assembled in a
/// test and the audit exercised without going through a builder.
#[derive(Debug)]
pub struct GateReport {
    pub profile: Profile,
    /// §1311: no legal complete model → `Blocked`.
    pub model_present: bool,
    /// A claim that this profile does not enable realistic.
    pub realistic_not_enabled: bool,
    pub rights: RightsEvidence,
    pub correctness: CorrectnessEvidence,
    pub realtime: RealtimeEvidence,
    pub stability: StabilityEvidence,
    /// §8.4 operational 2D fallback, if one happened.
    pub fallback: Option<FallbackRecord>,
    /// A claim that the 2D fallback stands in for realistic acceptance.
    /// §8.2 forbids it.
    pub acceptance_claimed_via_fallback: bool,
}

impl GateReport {
    pub fn verdict(&self, gate: Gate) -> GateVerdict {
        match gate {
            Gate::SourceAndRights => self.rights.verdict(),
            Gate::Correctness => self.correctness.verdict(),
            Gate::Realtime => self.realtime.verdict(),
            Gate::Stability => self.stability.verdict(),
        }
    }

    /// The overall admission, derived from the four gates. The fallback is not
    /// consulted: it can neither rescue a gate nor be the acceptance.
    pub fn acceptance(&self) -> Acceptance {
        if self.realistic_not_enabled && !self.profile.requires_realistic() {
            return Acceptance::NotApplicable;
        }
        if !self.model_present {
            return Acceptance::Blocked;
        }
        let mut any_failed = false;
        let mut any_unmeasured = false;
        for gate in Gate::ALL {
            match self.verdict(gate) {
                GateVerdict::Failed => any_failed = true,
                GateVerdict::Unmeasured => any_unmeasured = true,
                GateVerdict::Passed => {}
            }
        }
        if any_failed {
            Acceptance::NotAccepted
        } else if any_unmeasured {
            Acceptance::Unmeasured
        } else {
            Acceptance::Accepted
        }
    }
}

/// Audit a report as a **pure function of its own fields**, so a report that
/// was assembled rather than measured is still caught. It reports every hit.
pub fn audit_gate(report: &GateReport) -> Vec<Finding> {
    let mut findings = Vec::new();

    for kind in RightKind::ALL {
        if let Some(RightStatus::Established { basis }) = report.rights.status(kind)
            && basis.trim().is_empty()
        {
            findings.push(Finding::new(
                "established_right_without_basis",
                kind.label(),
            ));
        }
    }
    for component in report
        .rights
        .undeclared_components
        .iter()
        .chain(report.correctness.undeclared_components.iter())
    {
        findings.push(Finding::new("undeclared_component", component.clone()));
    }

    let correctness = &report.correctness;
    if (correctness.certifies(Precision::Fp16) || correctness.certifies(Precision::Int8))
        && !correctness.certifies(Precision::Fp32)
    {
        findings.push(Finding::new(
            "precision_without_fp32_baseline",
            "fp16/int8 certified before fp32",
        ));
    }
    if correctness.certifies(Precision::Int8)
        && correctness
            .int8_calibration_report
            .as_deref()
            .is_none_or(|report| report.trim().is_empty())
    {
        findings.push(Finding::new("int8_without_calibration", "int8"));
    }

    for scenario in report.stability.unbacked_passes() {
        findings.push(Finding::new(
            "scenario_passed_without_evidence",
            scenario.label(),
        ));
    }

    let placement = &report.realtime.placement;
    if placement.claims_full_gpu && placement.cpu_fallbacks() > 0 {
        findings.push(Finding::new(
            "full_gpu_claimed_with_cpu_fallback",
            format!("{} cpu fallback(s)", placement.cpu_fallbacks()),
        ));
    }
    if placement.claims_full_gpu && placement.node_placements.is_empty() {
        findings.push(Finding::new(
            "full_gpu_claimed_without_placement_records",
            "no placements recorded",
        ));
    }
    if placement.cuda_ep_registered && placement.node_placements.is_empty() {
        findings.push(Finding::new(
            "cuda_ep_registered_without_placement",
            "registration is not placement",
        ));
    }

    if !report.model_present
        && Gate::ALL
            .iter()
            .any(|gate| report.verdict(*gate) == GateVerdict::Passed)
    {
        findings.push(Finding::new(
            "gates_evaluated_without_model",
            "a gate passed with no model present",
        ));
    }

    if report.acceptance_claimed_via_fallback {
        findings.push(Finding::new(
            "fallback_claimed_as_acceptance",
            "2D fallback is not realistic acceptance",
        ));
    }
    if let Some(fallback) = &report.fallback {
        if !fallback.operator_authorized {
            findings.push(Finding::new(
                "fallback_without_authorization",
                fallback.reason.clone(),
            ));
        }
        if !fallback.at_sentence_boundary {
            findings.push(Finding::new(
                "fallback_outside_sentence_boundary",
                fallback.reason.clone(),
            ));
        }
        if !fallback.logged {
            findings.push(Finding::new("fallback_unlogged", fallback.reason.clone()));
        }
    }

    if report.realistic_not_enabled && report.profile.requires_realistic() {
        findings.push(Finding::new(
            "not_applicable_for_target_capability",
            report.profile.label(),
        ));
    }

    findings
}
