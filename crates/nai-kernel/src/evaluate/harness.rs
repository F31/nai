//! Harness observation and named degradation (V3.9 §47.1, §47.4; W66, acceptance H15/H17).
//!
//! §47.1 lists five observation kinds and §47.4 lists six failure layers, but until this
//! module both existed only as prose: whether a call had degraded silently was something a
//! reviewer had to read code to answer. This surface makes it decidable.
//!
//! Two halves, both instances of INV-32 ("every layer failure is named"):
//!
//! * **Observation (H15)**: a record is judged *before* it reaches the append-only hashed
//!   decision log — required fields present, no plaintext credential in any value (INV-11),
//!   viewer text masked and inside its retention (§5.8). Every failure is named.
//! * **Degradation (H17)**: each failure layer lands on one level of the §12.2 order
//!   (template → approved cache → human → silent standby), carries a distinct code, and is
//!   always visible. A degraded call that has lost its policy or persona constraint is a
//!   violation, not a licence to generate freely.
//!
//! **Scope boundary.** This module decides *whether* a record may be logged and in what
//! shape. Appending it to the actual hash chain in `nai-storage` is runtime wiring and is
//! **not** claimed here. Likewise [`DegradeVerdict`] is the kernel's judgement of where a
//! failure lands; rendering it into `CapabilityView` is a presentation concern.
//!
//! **Not to be confused with [`crate::bridge::Degradation`]**, which serves §119 platform-link
//! interruption (`bridge_lost`, `auto_reply_revoked`). An MCP/A2A peer going away is not a
//! platform interruption and vice versa; folding both into one structure would make "what kind
//! of degradation is this" undecidable again.

use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// H15: observation (§47.1)
// ---------------------------------------------------------------------------

/// §47.1: the five observation kinds. Exactly five — a missing kind MUST NOT be read as
/// "the other four cover it".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObservationLayer {
    /// L1 assembly.
    Assembly,
    /// L3 intent.
    Intent,
    /// L4 memory.
    Memory,
    /// L2 tool loop.
    Tool,
    /// Semantic / TTS-exact cache.
    Cache,
}

/// Every observation kind. Tests assert this is complete and that labels are distinct.
pub const OBSERVATION_LAYERS: [ObservationLayer; 5] = [
    ObservationLayer::Assembly,
    ObservationLayer::Intent,
    ObservationLayer::Memory,
    ObservationLayer::Tool,
    ObservationLayer::Cache,
];

impl ObservationLayer {
    /// Stable decision-log subject for the kind.
    pub fn label(self) -> &'static str {
        match self {
            ObservationLayer::Assembly => "harness.assembly",
            ObservationLayer::Intent => "harness.intent",
            ObservationLayer::Memory => "harness.memory",
            ObservationLayer::Tool => "harness.tool",
            ObservationLayer::Cache => "harness.cache",
        }
    }

    /// §47.1: the fields this kind MUST carry, transcribed from the table — neither added to
    /// nor trimmed.
    pub fn required_fields(self) -> &'static [&'static str] {
        match self {
            ObservationLayer::Assembly => &[
                "fragment_ref",
                "fragment_source",
                "segment_budget",
                "cropped_items",
                "crop_reason",
                "model_version",
                "prompt_version",
            ],
            ObservationLayer::Intent => &[
                "enum_value",
                "confidence",
                "evidence_ref",
                "threshold_hit",
                "guard_conflict",
            ],
            ObservationLayer::Memory => &["key", "scope", "source", "hit", "delete_status"],
            ObservationLayer::Tool => &[
                "skill_id",
                "skill_version",
                "params_hash",
                "result_summary",
                "result_size",
                "termination_reason",
                "cost",
            ],
            ObservationLayer::Cache => &["hit", "invalidation_reason", "reused_entry_version"],
        }
    }
}

/// INV-11: literal markers that MUST NOT appear in a logged **value**. Matching is on the
/// value, never on the field name — a field called `key` whose value is clean MUST pass.
pub const CREDENTIAL_MARKERS: &[&str] = &[
    "sk-",
    "api_key=",
    "authorization:",
    "bearer ",
    "password=",
    "secret=",
    "token=",
];

/// §5.8: viewer text and nicknames are retained under a bound and masked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewerText {
    /// The text as captured.
    pub text: String,
    /// Already masked (nickname replaced, raw comment redacted).
    pub masked: bool,
    /// Age of the capture, against [`ObservationPolicy::max_viewer_retention_ms`].
    pub age_ms: u64,
}

/// One observation record offered for the decision log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationRecord {
    pub layer: ObservationLayer,
    /// What was observed (fragment id, skill id, memory key, cache entry version…).
    pub subject: String,
    /// The recorded fields.
    pub fields: BTreeMap<String, String>,
    /// Viewer raw text / nickname, if this record carries one.
    pub viewer_text: Option<ViewerText>,
}

/// Observation policy. Defaults match §47.1 / §5.8 and add no new knobs.
pub struct ObservationPolicy {
    pub credential_markers: &'static [&'static str],
    pub max_viewer_retention_ms: u64,
    pub max_field_chars: usize,
}

impl Default for ObservationPolicy {
    fn default() -> Self {
        Self {
            credential_markers: &[
                "sk-",
                "api_key=",
                "authorization:",
                "bearer ",
                "password=",
                "secret=",
                "token=",
            ],
            max_viewer_retention_ms: 30 * 24 * 3600 * 1000,
            max_field_chars: 4096,
        }
    }
}

/// §47.1 / INV-11 / §5.8: every reason a record must not reach the log as offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationRejection {
    /// A field §47.1 requires for this kind is absent.
    MissingRequiredField,
    /// A required field is present but blank; an empty value is not a recorded value.
    EmptyFieldValue,
    /// A **value** carries a credential literal (INV-11).
    PlaintextCredential,
    /// Viewer text is carried unmasked.
    ViewerTextUnmasked,
    /// Viewer text is older than its retention (§5.8).
    ViewerTextOverRetention,
}

impl ObservationRejection {
    /// All five rejection names. Tests assert the constructed set is exactly this.
    pub const ALL: [ObservationRejection; 5] = [
        ObservationRejection::MissingRequiredField,
        ObservationRejection::EmptyFieldValue,
        ObservationRejection::PlaintextCredential,
        ObservationRejection::ViewerTextUnmasked,
        ObservationRejection::ViewerTextOverRetention,
    ];

    /// Stable decision-log label.
    pub fn label(self) -> &'static str {
        match self {
            ObservationRejection::MissingRequiredField => "observation.missing_required_field",
            ObservationRejection::EmptyFieldValue => "observation.empty_field_value",
            ObservationRejection::PlaintextCredential => "observation.plaintext_credential",
            ObservationRejection::ViewerTextUnmasked => "observation.viewer_text_unmasked",
            ObservationRejection::ViewerTextOverRetention => {
                "observation.viewer_text_over_retention"
            }
        }
    }
}

/// Outcome of judging one observation record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationVerdict {
    pub layer: ObservationLayer,
    /// True only when nothing was rejected. Gating on "no rejections" alone is not enough —
    /// see [`assert_no_free_form`] for the cross-field half.
    pub loggable: bool,
    pub rejections: Vec<ObservationRejection>,
    /// Required fields that were absent.
    pub missing_fields: Vec<&'static str>,
    /// Required fields present but blank.
    pub empty_fields: Vec<&'static str>,
    /// Fields whose value carried a credential literal.
    pub leaked_fields: Vec<String>,
    /// Loggable payload: credential-bearing values replaced, viewer text masked or dropped.
    /// Never the original bytes.
    pub redacted: BTreeMap<String, String>,
    /// Viewer text after masking; absent when it was over retention.
    pub redacted_viewer: Option<String>,
}

/// §47.1 / INV-11 / §5.8: judge a record before it reaches the decision log.
pub fn evaluate_observation(
    record: &ObservationRecord,
    policy: &ObservationPolicy,
) -> ObservationVerdict {
    let mut rejections = Vec::new();
    let mut missing_fields = Vec::new();
    let mut empty_fields = Vec::new();
    let mut leaked_fields = Vec::new();

    for field in record.layer.required_fields() {
        match record.fields.get(*field) {
            None => {
                missing_fields.push(*field);
                if !rejections.contains(&ObservationRejection::MissingRequiredField) {
                    rejections.push(ObservationRejection::MissingRequiredField);
                }
            }
            Some(value) if value.trim().is_empty() => {
                empty_fields.push(*field);
                if !rejections.contains(&ObservationRejection::EmptyFieldValue) {
                    rejections.push(ObservationRejection::EmptyFieldValue);
                }
            }
            Some(_) => {}
        }
    }

    let mut redacted = BTreeMap::new();
    for (name, value) in &record.fields {
        let lowered = value.to_ascii_lowercase();
        let leaked = policy
            .credential_markers
            .iter()
            .any(|marker| lowered.contains(marker));
        if leaked {
            leaked_fields.push(name.clone());
            if !rejections.contains(&ObservationRejection::PlaintextCredential) {
                rejections.push(ObservationRejection::PlaintextCredential);
            }
            redacted.insert(name.clone(), "[redacted]".to_string());
        } else if value.chars().count() > policy.max_field_chars {
            redacted.insert(name.clone(), "[truncated]".to_string());
        } else {
            redacted.insert(name.clone(), value.clone());
        }
    }

    let mut redacted_viewer = None;
    if let Some(viewer) = &record.viewer_text {
        if viewer.age_ms > policy.max_viewer_retention_ms {
            if !rejections.contains(&ObservationRejection::ViewerTextOverRetention) {
                rejections.push(ObservationRejection::ViewerTextOverRetention);
            }
        } else if !viewer.masked {
            if !rejections.contains(&ObservationRejection::ViewerTextUnmasked) {
                rejections.push(ObservationRejection::ViewerTextUnmasked);
            }
            redacted_viewer = Some("[masked]".to_string());
        } else {
            redacted_viewer = Some(viewer.text.clone());
        }
    }

    ObservationVerdict {
        layer: record.layer,
        loggable: rejections.is_empty(),
        rejections,
        missing_fields,
        empty_fields,
        leaked_fields,
        redacted,
        redacted_viewer,
    }
}

// ---------------------------------------------------------------------------
// H17: named degradation (§47.4, §12.2, INV-32)
// ---------------------------------------------------------------------------

/// §47.4: the six failure layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureLayer {
    /// Assembly failed, e.g. an uncroppable segment is missing.
    Assembly,
    /// Memory read failed.
    MemoryRead,
    /// Intent unavailable or below threshold.
    Intent,
    /// A tool step failed.
    Tool,
    /// Cache unavailable or invalidated.
    Cache,
    /// An MCP/A2A peer is unavailable.
    Peer,
}

/// Every failure layer. Tests assert the six codes are distinct and non-empty.
pub const FAILURE_LAYERS: [FailureLayer; 6] = [
    FailureLayer::Assembly,
    FailureLayer::MemoryRead,
    FailureLayer::Intent,
    FailureLayer::Tool,
    FailureLayer::Cache,
    FailureLayer::Peer,
];

impl FailureLayer {
    /// §47.4 + INV-32: the named error code. Distinct per layer.
    pub fn code(self) -> &'static str {
        match self {
            FailureLayer::Assembly => "HARNESS_ASSEMBLY_FAILED",
            FailureLayer::MemoryRead => "HARNESS_MEMORY_READ_FAILED",
            FailureLayer::Intent => "HARNESS_INTENT_UNAVAILABLE",
            FailureLayer::Tool => "HARNESS_TOOL_FAILED",
            FailureLayer::Cache => "HARNESS_CACHE_UNAVAILABLE",
            FailureLayer::Peer => "HARNESS_PEER_UNAVAILABLE",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FailureLayer::Assembly => "harness.assembly",
            FailureLayer::MemoryRead => "harness.memory_read",
            FailureLayer::Intent => "harness.intent",
            FailureLayer::Tool => "harness.tool",
            FailureLayer::Cache => "harness.cache",
            FailureLayer::Peer => "harness.peer",
        }
    }

    /// §47.4: the level this failure starts from. Walking down §12.2 begins here.
    pub fn preferred_level(self) -> DegradeLevel {
        match self {
            FailureLayer::Assembly | FailureLayer::MemoryRead | FailureLayer::Tool => {
                DegradeLevel::Template
            }
            FailureLayer::Intent | FailureLayer::Cache | FailureLayer::Peer => DegradeLevel::Human,
        }
    }

    /// §47.4: only assembly refuses the call outright, and it does so regardless of where the
    /// walk-down lands — "refuse the call" does not lapse because a template was unavailable.
    pub fn refuses_call(self) -> bool {
        matches!(self, FailureLayer::Assembly)
    }

    /// What the call proceeds without, if it proceeds at all.
    pub fn drops(self) -> Option<&'static str> {
        match self {
            FailureLayer::Assembly => None,
            FailureLayer::MemoryRead => Some("memory"),
            FailureLayer::Intent => Some("intent"),
            FailureLayer::Tool => Some("tool_result"),
            FailureLayer::Cache => Some("cache_entry"),
            FailureLayer::Peer => Some("peer_capability"),
        }
    }

    /// §38.4: a peer going away MUST NOT disturb sessions already running.
    pub fn keeps_existing_session(self) -> bool {
        matches!(self, FailureLayer::Peer)
    }

    /// §12.2 / §47.4: the operator-facing next action.
    pub fn actionable(self) -> &'static str {
        match self {
            FailureLayer::Assembly => "补齐不可裁剪段后重试；本次调用已拒绝",
            FailureLayer::MemoryRead => "按无记忆继续并已在界面标注；不得以推断补齐",
            FailureLayer::Intent => "澄清或转人工；不得用分类默认值",
            FailureLayer::Tool => "显式标注缺失后退模板或人工；不得凭印象补",
            FailureLayer::Cache => "重新生成或转人工；不得沿用旧引用",
            FailureLayer::Peer => "关闭该对端能力；已有会话不受影响",
        }
    }
}

/// §12.2: the degradation order. The variant order **is** the fallback chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DegradeLevel {
    /// 1 — template.
    Template,
    /// 2 — approved cache.
    ApprovedCache,
    /// 3 — human.
    Human,
    /// 4 — silent standby. Always available: it is the floor, not a fallback that can fail.
    SilentStandby,
}

/// §12.2 order, used both for the walk-down and for the ordering assertion in tests.
pub const DEGRADE_ORDER: [DegradeLevel; 4] = [
    DegradeLevel::Template,
    DegradeLevel::ApprovedCache,
    DegradeLevel::Human,
    DegradeLevel::SilentStandby,
];

impl DegradeLevel {
    pub fn label(self) -> &'static str {
        match self {
            DegradeLevel::Template => "template",
            DegradeLevel::ApprovedCache => "approved_cache",
            DegradeLevel::Human => "human",
            DegradeLevel::SilentStandby => "silent_standby",
        }
    }

    /// Position in [`DEGRADE_ORDER`].
    pub fn index(self) -> usize {
        match self {
            DegradeLevel::Template => 0,
            DegradeLevel::ApprovedCache => 1,
            DegradeLevel::Human => 2,
            DegradeLevel::SilentStandby => 3,
        }
    }
}

/// What is available to fall back to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DegradeContext {
    pub template_available: bool,
    pub approved_cache_available: bool,
    pub human_available: bool,
}

impl DegradeContext {
    /// Everything available.
    pub fn all_available() -> Self {
        Self {
            template_available: true,
            approved_cache_available: true,
            human_available: true,
        }
    }
}

/// §47.4: where one failure lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DegradeVerdict {
    pub layer: FailureLayer,
    pub code: &'static str,
    /// The §12.2 level reached.
    pub level: DegradeLevel,
    /// Always true: there is no invisible degradation.
    pub visible: bool,
    pub refuses_call: bool,
    pub drops: Option<&'static str>,
    pub keeps_existing_session: bool,
    pub actionable: &'static str,
}

fn level_available(level: DegradeLevel, ctx: &DegradeContext) -> bool {
    match level {
        DegradeLevel::Template => ctx.template_available,
        DegradeLevel::ApprovedCache => ctx.approved_cache_available,
        DegradeLevel::Human => ctx.human_available,
        DegradeLevel::SilentStandby => true,
    }
}

/// §47.4 + §12.2: resolve one failure to a named, ordered, visible degradation.
///
/// The walk starts at the layer's preferred level and takes the first available level at or
/// below it. Silent standby is always available, so this never returns `None` and never
/// leaves a failure unassigned.
pub fn evaluate_degradation(layer: FailureLayer, ctx: &DegradeContext) -> DegradeVerdict {
    let start = layer.preferred_level().index();
    let level = DEGRADE_ORDER
        .iter()
        .copied()
        .skip(start)
        .find(|candidate| level_available(*candidate, ctx))
        .unwrap_or(DegradeLevel::SilentStandby);

    DegradeVerdict {
        layer,
        code: layer.code(),
        level,
        visible: true,
        refuses_call: layer.refuses_call(),
        drops: layer.drops(),
        keeps_existing_session: layer.keeps_existing_session(),
        actionable: layer.actionable(),
    }
}

/// INV-32: the way a layer failure is named, not the failure itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreeFormViolation {
    /// Degraded, and the context no longer carries the policy constraint.
    DegradedWithoutPolicyConstraint,
    /// Degraded, and the context no longer carries the persona.
    DegradedWithoutPersona,
}

impl FreeFormViolation {
    pub const ALL: [FreeFormViolation; 2] = [
        FreeFormViolation::DegradedWithoutPolicyConstraint,
        FreeFormViolation::DegradedWithoutPersona,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FreeFormViolation::DegradedWithoutPolicyConstraint => {
                "free_form.degraded_without_policy_constraint"
            }
            FreeFormViolation::DegradedWithoutPersona => "free_form.degraded_without_persona",
        }
    }
}

/// The state of a call that has already degraded, offered for generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DegradedCallContext {
    pub degradations: Vec<DegradeVerdict>,
    /// The assembled context still carries the system-policy segment.
    pub has_policy_constraint: bool,
    /// The assembled context still carries the persona segment.
    pub has_persona: bool,
}

/// INV-32, second half: a degraded call MUST NOT be handed to the model as an empty context
/// and left to improvise.
///
/// Naming the error code is not sufficient on its own — an implementation can emit a code and
/// still generate. This is the cross-field assertion that closes that gap.
pub fn assert_no_free_form(call: &DegradedCallContext) -> Vec<FreeFormViolation> {
    if call.degradations.is_empty() {
        return Vec::new();
    }
    let mut violations = Vec::new();
    if !call.has_policy_constraint {
        violations.push(FreeFormViolation::DegradedWithoutPolicyConstraint);
    }
    if !call.has_persona {
        violations.push(FreeFormViolation::DegradedWithoutPersona);
    }
    violations
}
