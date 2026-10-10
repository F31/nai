//! Manual-event rehearsal pipeline: the Slice-0 vertical slice.
//!
//! One explicit `manual` event flows through `normalize/dedupe -> knowledge
//! retrieval -> agent decision -> four-stage guard -> text generation ->
//! decision log (hash-chained)`. Every step rechecks the session, the
//! generation epoch, the duty lease and the approval object before the next
//! fail-closed check.
//!
//! When the rehearsal script declares a `render` closure, the chain continues
//! past text: the approval-bound cue text is synthesized into bounded PCM, the
//! PCM is cut into bounded `AudioBlock`s, real RGBA frames are produced by the
//! software 2D renderer, the lane commits on its audio and video water levels,
//! and a rehearsal playback sink emits the acknowledgements. Only an
//! acknowledgement that reports an actual, sample-exact local completion is
//! logged as `PlaybackAck`.
//!
//! Honest boundary: the generator, the synthesizer and the playback sink are all
//! rehearsal-only deterministic stand-ins; the retrieval corpus and the operator
//! approvals come from a trusted-owner rehearsal script; the store's
//! `record_candidate`/`approve` workflow is NOT wired. Frames are produced in
//! memory: no frame slot is written, no keeper consumes them and no device,
//! window or hardware playback exists. Nothing here certifies a real provider,
//! real TTS, a device or Windows.
use nai_avatar_2d::{AssetBytes, Avatar, LicenseGrants};
use nai_contracts::candidate_hash;
use nai_contracts::v1::{
    BillingState, BudgetCharge, Decision, DecisionAction, EventKind, Evidence, GuardRequest,
    GuardResult, GuardStage, GuardVerdict, Observation, PerformanceCue, PermissionSnapshot,
    PlatformEvent, RequestContext, Risk, SessionLifecycle, SessionMode, UsageLedgerEntry,
};
use nai_kernel::evaluate::{
    agent::{AgentPolicy, evaluate_decision},
    guard::{GuardPolicy, PreplayChecks, guard},
};
use nai_kernel::session::Session;
use nai_knowledge_core::{VectorIdentity, VectorSpace, tokenize};
use nai_media_core::playback::{
    LaneSpec, MAX_FRAMES, PlaybackLane, RehearsalSink, frame_boundary, frame_count, rehearse,
};
use nai_media_core::timeline::ack::PlaybackState;
use nai_media_core::timeline::audio::{AudioBlock, SampleFormat};
use nai_plugin_sdk::plugin::PluginContext;
use nai_provider_core::{
    InMemoryGateway, ProviderError, RehearsalSpeechSynthesizer, RehearsalTextGenerator,
    SpeechRequest, SpeechResponse, SpeechSynthesizer, TextGenerator, TextRequest, TextResponse,
    UsageGateway, generate_with_budget, synthesize_with_budget,
};
use nai_runtime::{Composition, ControlAction};
use nai_storage::{DecisionKind, Operation};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::AtomicBool},
    time::{Duration, Instant},
};

/// The only accepted event source. A platform source is never claimed unless it
/// was actually observed through a platform binding.
pub const SOURCE_MANUAL: &str = "manual";
/// Bounded normalize/dedupe queue.
pub const QUEUE_CAPACITY: usize = 64;
/// Bounded normalized event text.
pub const MAX_EVENT_TEXT_CHARS: usize = 1024;
/// Bounds on the script-declared retrieval depth. They match the
/// `KnowledgeQuery.top_k` contract window (1..=100).
pub const MIN_TOP_K: usize = 1;
pub const MAX_TOP_K: usize = 100;
/// Cue policy revision recorded on every candidate.
pub const POLICY_VERSION: &str = "template-v1";
/// Trace prefix that marks a decision entry as pipeline-owned.
const PIPELINE_TRACE_PREFIX: &str = "engine-pipeline-";
/// Trace of the single session bring-up breadcrumb.
const BRINGUP_TRACE_PREFIX: &str = "engine-bringup-";
/// Bounds on the script-declared duty window: the same window the control
/// contract accepts for a command TTL.
pub const MIN_DUTY_TTL_MS: u64 = 1;
pub const MAX_DUTY_TTL_MS: u64 = 3_600_000;
/// Task budget advertised to the generation seam, in milliseconds.
const TASK_BUDGET_MS: u32 = 30_000;
/// The only accepted candidate hash algorithm; the renderer and the media gate
/// both require it verbatim.
const CANDIDATE_HASH_ALGORITHM: &str = "sha256-canonical-v1";
/// The renderer binding the 2D core declares. It is reported verbatim so the
/// summary names the software renderer rather than implying a device renderer.
const RENDER_BINDING: &str = "software-rgba8-v1";
/// One-time avatar admission budget. The schema and closure checks are paid
/// before the first event, never inside a slot or frame deadline.
const ADMIT_BUDGET: Duration = Duration::from_secs(10);
/// First-audio water level of the rehearsal lane, in milliseconds. It is well
/// below one frame span, so the audio side is never the blocking side.
const AUDIO_WATER_MS: u32 = 40;
/// Upper bound on the rehearsal retrieval vocabulary. A corpus beyond it needs
/// the real embedding profile, not this drill index.
const MAX_VOCAB: usize = 4096;
/// Payload size that the store rejects; used only by the storage fault injection.
const OVERSIZED_PAYLOAD: usize = 70_000;
/// Decision entries read back when comparing the log against the replay.
const LOG_READ_LIMIT: u32 = 500;

/// A trusted-owner rehearsal script. It carries the retrieval corpus, the manual
/// events and the operator approvals; every field is explicit, and a missing
/// field is an error rather than a default.
#[derive(Debug, Clone, Deserialize)]
pub struct Script {
    pub schema_version: u32,
    /// The session and workspace the operator drafted this script for. Both are
    /// checked against the live composition; a mismatch is refused rather than
    /// rebound, because a candidate hash is only meaningful inside its scope.
    pub session_id: String,
    pub workspace_id: String,
    /// Revision of the knowledge corpus the operator indexed. Evidence whose
    /// permission revision differs is a facts mismatch.
    pub knowledge_revision: String,
    /// Retrieval depth. It belongs to the script so the candidate preview an
    /// operator approves and the run that follows retrieve identically.
    pub top_k: usize,
    /// The operator's session bring-up, executed before any event is evaluated.
    pub session: SessionSetup,
    pub knowledge: Vec<KnowledgeDoc>,
    pub events: Vec<ManualEvent>,
    #[serde(default)]
    pub approvals: Vec<ApprovalRecord>,
    /// Optional render closure. Absent means the chain honestly stops at text:
    /// `delivery=none`, no audio, no frames and no playback acknowledgement.
    #[serde(default)]
    pub render: Option<RenderSetup>,
    /// Directory the render closure's relative paths resolve against. Filled by
    /// the loader from the script's own location; never taken from the file.
    #[serde(skip)]
    pub base_dir: PathBuf,
}

/// The operator's bring-up of the rehearsal session. A manual event can only be
/// evaluated against a live session, so this is an explicit, validated input:
/// the pipeline never moves a session's lifecycle as a hidden side effect.
#[derive(Debug, Clone, Deserialize)]
pub struct SessionSetup {
    /// The operator that takes duty. An empty operator is refused, exactly as
    /// the kernel's lease grant refuses it.
    pub operator: String,
    pub duty_ttl_ms: u64,
    /// `assisted` or `auto_supervised`. `rehearsal` never permits output, and
    /// `auto_supervised` is refused when the runtime profile does not allow
    /// automatic replies without a platform grant.
    pub mode: String,
    /// Lifecycle targets the operator drives, in order, before the first event.
    /// An empty list means the session must already be live.
    #[serde(default)]
    pub lifecycle: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KnowledgeDoc {
    pub document_id: String,
    pub chunk_id: String,
    pub text: String,
    pub revision: String,
    pub permission_revision: String,
    #[serde(default)]
    pub valid_until_utc_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManualEvent {
    pub event_id: String,
    pub utterance_id: String,
    /// Must be `manual`.
    pub source: String,
    /// `comment` or `gift`.
    pub kind: String,
    pub text: String,
    pub generation_epoch: u64,
    /// The candidate expiry is an explicit input so the candidate hash is
    /// reproducible across runs and can be approved out of band.
    pub expires_at_utc_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApprovalRecord {
    pub utterance_id: String,
    pub approval_id: String,
    pub candidate_hash: String,
    pub expires_at_utc_ms: u64,
}

/// The operator's render closure for the rehearsal lane: the exact 2D asset
/// closure, the voice binding, the output identity the acknowledgements are
/// bound to, and the declared playback outcome. Paths are relative to the
/// script's own directory; the loader resolves them, so the engine opens only
/// paths the trusted owner wrote down.
#[derive(Debug, Clone, Deserialize)]
pub struct RenderSetup {
    pub manifest: String,
    pub config: String,
    pub assets: Vec<AssetRef>,
    pub licenses: Vec<String>,
    /// Voice binding handed to the synthesis seam. Never inferred from a cue:
    /// the trusted owner declares it.
    pub voice_ref: String,
    /// Output identity every acknowledgement is bound to.
    pub output_id: String,
    /// `complete`, `partial`, `invalid` or `cancelled`.
    #[serde(default = "default_playback")]
    pub playback: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetRef {
    pub id: String,
    pub path: String,
}

fn default_playback() -> String {
    "complete".into()
}

/// Which stage the rehearsal fault injection breaks. Only a stage failure
/// triggers the ordered halt; a normal block or an incomplete playback does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Injection {
    None,
    Knowledge,
    Guard,
    Generate,
    Speech,
    Storage,
}

impl Injection {
    pub fn parse(text: &str) -> Result<Self, String> {
        match text {
            "none" => Ok(Self::None),
            "knowledge" => Ok(Self::Knowledge),
            "guard" => Ok(Self::Guard),
            "generate" => Ok(Self::Generate),
            "speech" => Ok(Self::Speech),
            "storage" => Ok(Self::Storage),
            other => Err(format!("unknown injection stage {other}")),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Knowledge => "knowledge",
            Self::Guard => "guard",
            Self::Generate => "generate",
            Self::Speech => "speech",
            Self::Storage => "storage",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub script: std::path::PathBuf,
    pub provider_ref: String,
    pub price_revision: String,
    pub max_output_chars: usize,
    pub injection: Injection,
}

/// Where one event stopped and what it decided.
#[derive(Debug, Clone)]
struct EventOutcome {
    event_id: String,
    utterance_id: String,
    stage: &'static str,
    route: String,
    action: i32,
    /// input/evidence/output/preplay verdicts; `None` when a stage never ran.
    guard: [Option<i32>; 4],
    candidate_hash: String,
    approval_id: String,
    generated: String,
    model_ref: String,
    usage: BTreeMap<String, u64>,
    reason: String,
    /// Number of retrieved evidence items.
    evidence: usize,
    /// Samples the speech seam produced; zero when the chain stopped at text.
    audio_samples: u64,
    /// Frames the 2D renderer produced; zero when the chain stopped at text.
    frames: u32,
    speech_model_ref: String,
    /// The playback state after the declared scenario.
    playback: &'static str,
    /// `none` when the chain stopped at text, `memory` when frames were produced
    /// without a slot, keeper or device.
    delivery: &'static str,
    /// Acknowledgements the lane refused.
    playback_rejected: usize,
}

impl EventOutcome {
    /// The volatile-free summary used for the replay comparison: it excludes
    /// timestamps, hashes, sequence numbers, pixel bytes, PCM and any other
    /// run-dependent field.
    fn canonical(&self) -> Value {
        json!({
            "event_id": self.event_id,
            "utterance_id": self.utterance_id,
            "stage": self.stage,
            "route": self.route,
            "action": self.action,
            "guard": self.guard,
            "generated": self.generated,
            "model_ref": self.model_ref,
            "usage": self.usage,
            "reason": self.reason,
            "evidence": self.evidence,
            "audio_samples": self.audio_samples,
            "frames": self.frames,
            "speech_model_ref": self.speech_model_ref,
            "playback": self.playback,
            "delivery": self.delivery,
            "playback_rejected": self.playback_rejected,
        })
    }
    fn canonical_text(&self) -> String {
        serde_json::to_string(&self.canonical()).unwrap_or_default()
    }
}

/// Reads and validates the script. An unknown source, an unknown event kind or a
/// duplicated event is refused rather than coerced.
pub fn load_script(path: &Path) -> Result<Script, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("rehearsal script unreadable: {error}"))?;
    let mut script: Script = serde_json::from_str(&raw)
        .map_err(|error| format!("rehearsal script is not valid JSON: {error}"))?;
    script.base_dir = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    if script.schema_version != 1 {
        return Err("rehearsal script schema_version must be 1".into());
    }
    if script.knowledge_revision.is_empty() {
        return Err("rehearsal script needs knowledge_revision".into());
    }
    if script.session_id.is_empty() || script.workspace_id.is_empty() {
        return Err("rehearsal script needs session_id and workspace_id".into());
    }
    if !(MIN_TOP_K..=MAX_TOP_K).contains(&script.top_k) {
        return Err(format!(
            "rehearsal script top_k must be within {MIN_TOP_K}..={MAX_TOP_K}"
        ));
    }
    if script.session.operator.trim().is_empty() {
        return Err("rehearsal script needs a duty operator".into());
    }
    if !(MIN_DUTY_TTL_MS..=MAX_DUTY_TTL_MS).contains(&script.session.duty_ttl_ms) {
        return Err(format!(
            "rehearsal script duty_ttl_ms must be within {MIN_DUTY_TTL_MS}..={MAX_DUTY_TTL_MS}"
        ));
    }
    session_mode(&script.session.mode)?;
    for target in &script.session.lifecycle {
        lifecycle_target(target)?;
    }
    if script.knowledge.len() > 512 || script.events.len() > QUEUE_CAPACITY {
        return Err("rehearsal script exceeds the bounded corpus or queue".into());
    }
    for doc in &script.knowledge {
        if doc.document_id.is_empty()
            || doc.chunk_id.is_empty()
            || doc.text.trim().is_empty()
            || doc.revision.is_empty()
            || doc.permission_revision.is_empty()
        {
            return Err(format!("knowledge chunk {} is incomplete", doc.chunk_id));
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    for event in &script.events {
        if event.source != SOURCE_MANUAL {
            return Err(format!(
                "event {} has source {}; only {SOURCE_MANUAL} is accepted",
                event.event_id, event.source
            ));
        }
        if event.event_id.is_empty()
            || event.utterance_id.is_empty()
            || event.expires_at_utc_ms == 0
        {
            return Err("manual event is missing required fields".into());
        }
        event_kind(&event.kind)?;
        if !seen.insert(event.event_id.clone()) {
            return Err(format!("duplicate event id {}", event.event_id));
        }
    }
    resolve_render(&mut script)?;
    Ok(script)
}

/// Validates the optional render closure and resolves its paths against the
/// script's own directory. Nothing is defaulted: an incomplete closure is
/// refused before any session state is driven.
fn resolve_render(script: &mut Script) -> Result<(), String> {
    let Some(render) = script.render.as_mut() else {
        return Ok(());
    };
    if render.manifest.is_empty() || render.config.is_empty() {
        return Err("render closure needs a manifest and a config path".into());
    }
    if render.assets.len() != 3 {
        return Err("render closure needs exactly three asset entries".into());
    }
    let mut ids = BTreeSet::new();
    for asset in &render.assets {
        if asset.id.is_empty() || asset.path.is_empty() {
            return Err("render asset entries need an id and a path".into());
        }
        if !ids.insert(asset.id.clone()) {
            return Err(format!("duplicate render asset id {}", asset.id));
        }
    }
    if render.licenses.is_empty() || render.licenses.iter().any(String::is_empty) {
        return Err("render closure needs at least one non-empty license reference".into());
    }
    if render.voice_ref.is_empty() {
        return Err("render closure needs a voice binding".into());
    }
    if render.output_id.is_empty() {
        return Err("render closure needs an output identity".into());
    }
    RehearsalSink::parse(&render.playback)?;
    let base = script.base_dir.clone();
    let resolve = |value: &str| -> String {
        let path = Path::new(value);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            base.join(path)
        }
        .to_string_lossy()
        .into_owned()
    };
    render.manifest = resolve(&render.manifest);
    render.config = resolve(&render.config);
    for asset in &mut render.assets {
        asset.path = resolve(&asset.path);
    }
    Ok(())
}

/// Builds the candidate for one manual event. The construction is fully
/// deterministic: the same event and the same retrieval result always produce
/// the same candidate, so its hash can be approved out of band and replayed.
fn candidate_for(
    session: &str,
    workspace: &str,
    event: &ManualEvent,
    text: &str,
    evidence: &[Evidence],
) -> PerformanceCue {
    PerformanceCue {
        cue_id: format!("{}-cue", event.event_id),
        session_id: session.into(),
        utterance_id: event.utterance_id.clone(),
        generation_epoch: event.generation_epoch,
        display_text: text.into(),
        speech_text: text.into(),
        risk: Risk::Risk0 as i32,
        evidence_refs: evidence
            .iter()
            .map(|item| item.evidence_id.clone())
            .collect(),
        fact_versions: Default::default(),
        voice: None,
        performance: None,
        policy_version: POLICY_VERSION.into(),
        expires_at_utc_ms: event.expires_at_utc_ms,
        approval_ref: String::new(),
        knowledge_snapshot_ref: format!("{workspace}-kb"),
        candidate_hash: String::new(),
        hash_algorithm: String::new(),
    }
}

/// The approval object hash of a candidate: the storage's canonical candidate
/// hash, so an operator approval binds to exactly the object the engine would
/// send. Exposed so a trusted owner can compute the value it approves.
pub fn approval_object_hash(cue: &PerformanceCue) -> String {
    candidate_hash(cue)
}

/// One candidate an operator reviews before approving it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidatePreview {
    pub event_id: String,
    pub utterance_id: String,
    pub candidate_hash: String,
    pub evidence_ids: Vec<String>,
}

/// Computes the exact candidate object hashes a trusted owner would approve.
/// This is the approval draft an operator reviews; it performs no I/O and is
/// the same construction the pipeline uses, so the hashes match by
/// construction rather than by convention.
pub fn preview_candidates(script: &Script) -> Result<Vec<CandidatePreview>, String> {
    let index = index_build(&script.knowledge)?;
    let mode = session_mode(&script.session.mode)? as i32;
    let now_ms = utc_ms();
    let mut previews = Vec::new();
    for event in &script.events {
        let text = normalize(&event.text);
        if text.is_empty() {
            continue;
        }
        let evidence = retrieve(&index, &script.knowledge, &text, script.top_k)?;
        if evidence.is_empty() {
            continue;
        }
        let decision = route_decision(
            &script.session_id,
            &script.workspace_id,
            mode,
            event,
            &text,
            &evidence,
            now_ms,
        )?;
        let Some(cue) = spoken_cue(&decision) else {
            continue;
        };
        previews.push(CandidatePreview {
            event_id: event.event_id.clone(),
            utterance_id: event.utterance_id.clone(),
            candidate_hash: approval_object_hash(&cue),
            evidence_ids: cue.evidence_refs.clone(),
        });
    }
    Ok(previews)
}

/// The request context one manual event is evaluated under.
fn observation_for(
    session: &str,
    workspace: &str,
    mode: i32,
    event: &ManualEvent,
    text: &str,
) -> Result<Observation, String> {
    Ok(Observation {
        context: Some(RequestContext {
            session_id: session.into(),
            workspace_id: workspace.into(),
            generation_epoch: Some(event.generation_epoch),
            remaining_budget_ms: TASK_BUDGET_MS,
            ..Default::default()
        }),
        mode,
        capabilities: None,
        trigger: Some(PlatformEvent {
            kind: event_kind(&event.kind)? as i32,
            text: text.into(),
            ..Default::default()
        }),
        ..Default::default()
    })
}

/// Builds and validates the candidate for one event through the kernel's agent
/// rules. This is the single construction the operator preview and the run both
/// use, so the hash an operator approves is the object the guard then sees.
///
/// The kernel recomputes the cue risk from controlled rules, and that
/// recomputed risk is part of the object hash: hashing the pre-validation cue
/// would bind the approval to a different object than the one that is guarded.
fn route_decision(
    session: &str,
    workspace: &str,
    mode: i32,
    event: &ManualEvent,
    text: &str,
    evidence: &[Evidence],
    now_ms: u64,
) -> Result<Decision, String> {
    let observation = observation_for(session, workspace, mode, event, text)?;
    let proposed = Decision {
        action: DecisionAction::Speak as i32,
        route: "template".into(),
        reason_code: "manual_event".into(),
        risk: Risk::Risk0 as i32,
        candidate: Some(candidate_for(session, workspace, event, text, evidence)),
        ..Default::default()
    };
    evaluate_decision(
        &observation,
        Some(&proposed),
        &AgentPolicy::default(),
        now_ms,
    )
    .map_err(|error| format!("agent decision rejected: {error:?}"))
}

/// The candidate a validated `Speak` decision carries. Any other action is a
/// silent or human-request outcome and never reaches generation.
fn spoken_cue(decision: &Decision) -> Option<PerformanceCue> {
    if DecisionAction::try_from(decision.action).ok() == Some(DecisionAction::Speak) {
        decision.candidate.clone()
    } else {
        None
    }
}

/// Normalizes event text: trims, collapses whitespace and bounds the length.
/// Nothing is invented; an empty result is an empty result.
fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_EVENT_TEXT_CHARS)
        .collect()
}

fn event_kind(kind: &str) -> Result<EventKind, String> {
    match kind {
        "comment" => Ok(EventKind::Comment),
        "gift" => Ok(EventKind::Gift),
        other => Err(format!("unknown manual event kind {other}")),
    }
}

/// The declared session mode. `rehearsal` is refused here: rehearsal mode never
/// emits output, so a script that asks for it could not produce a candidate.
fn session_mode(mode: &str) -> Result<SessionMode, String> {
    match mode {
        "assisted" => Ok(SessionMode::Assisted),
        "auto_supervised" => Ok(SessionMode::AutoSupervised),
        "rehearsal" => Err("rehearsal mode never permits output".into()),
        other => Err(format!("unknown session mode {other}")),
    }
}

/// One lifecycle target the operator drives. The kernel validates the order;
/// this only rejects a target the protocol cannot name.
fn lifecycle_target(target: &str) -> Result<SessionLifecycle, String> {
    match target {
        "preparing" => Ok(SessionLifecycle::Preparing),
        "ready" => Ok(SessionLifecycle::Ready),
        "running" => Ok(SessionLifecycle::Running),
        other => Err(format!("unknown session lifecycle target {other}")),
    }
}

fn lifecycle_name(lifecycle: i32) -> &'static str {
    match SessionLifecycle::try_from(lifecycle) {
        Ok(SessionLifecycle::Created) => "created",
        Ok(SessionLifecycle::Preparing) => "preparing",
        Ok(SessionLifecycle::Ready) => "ready",
        Ok(SessionLifecycle::ReadyDegraded) => "ready_degraded",
        Ok(SessionLifecycle::Running) => "running",
        Ok(SessionLifecycle::Draining) => "draining",
        Ok(SessionLifecycle::Stopped) => "stopped",
        Ok(SessionLifecycle::Faulted) => "faulted",
        _ => "unspecified",
    }
}

fn mode_name(mode: i32) -> &'static str {
    match SessionMode::try_from(mode) {
        Ok(SessionMode::Rehearsal) => "rehearsal",
        Ok(SessionMode::Assisted) => "assisted",
        Ok(SessionMode::AutoSupervised) => "auto_supervised",
        _ => "unspecified",
    }
}

/// The rehearsal retrieval index. It exercises the real `VectorSpace` cosine
/// ranking, but its embedding is an exact token index over the corpus rather
/// than a hashing trick: a collision must never turn a real miss into a hit, and
/// "no knowledge hit" has to be a checkable fact. Tokens the corpus does not
/// contain map to one sink slot that every document leaves at zero, so an
/// unrelated query scores exactly zero. A drill index for the topic search, not
/// a semantic model claim.
struct Index {
    space: VectorSpace,
    slots: BTreeMap<String, usize>,
    sink: usize,
}

impl Index {
    fn embed(&self, text: &str) -> Vec<f32> {
        let mut vector = vec![0f32; self.sink + 1];
        for token in tokenize(text) {
            match self.slots.get(&token) {
                Some(slot) => vector[*slot] += 1.0,
                None => vector[self.sink] += 1.0,
            }
        }
        vector
    }
}

fn index_build(docs: &[KnowledgeDoc]) -> Result<Index, String> {
    let mut slots: BTreeMap<String, usize> = BTreeMap::new();
    for doc in docs {
        for token in tokenize(&doc.text) {
            if slots.contains_key(&token) {
                continue;
            }
            if slots.len() >= MAX_VOCAB {
                return Err(format!(
                    "rehearsal corpus exceeds the {MAX_VOCAB}-token index bound"
                ));
            }
            let next = slots.len();
            slots.insert(token, next);
        }
    }
    if slots.is_empty() {
        return Err("rehearsal corpus has no indexable tokens".into());
    }
    let sink = slots.len();
    let mut index = Index {
        space: VectorSpace::new(VectorIdentity {
            embedding_provider: "rehearsal-vocabulary-v1".into(),
            model_version: "1".into(),
            dimension: sink + 1,
            normalization: "cosine".into(),
            chunker_version: "token-v1".into(),
        }),
        slots,
        sink,
    };
    for doc in docs {
        let vector = index.embed(&doc.text);
        index
            .space
            .insert(&doc.chunk_id, vector)
            .map_err(|error| format!("retrieval index rejected {}: {error}", doc.chunk_id))?;
    }
    Ok(index)
}

/// Top-k retrieval with a positive-score gate. The score is an exact cosine over
/// the token sets, so a positive value means a shared token and an empty result
/// is a real "no hit" - never padded with a default document.
fn retrieve(
    index: &Index,
    docs: &[KnowledgeDoc],
    query: &str,
    top_k: usize,
) -> Result<Vec<Evidence>, String> {
    let ranked = index
        .space
        .top_k(&index.embed(query), top_k)
        .map_err(|error| format!("retrieval failed: {error}"))?;
    let by_chunk: BTreeMap<&str, &KnowledgeDoc> = docs
        .iter()
        .map(|doc| (doc.chunk_id.as_str(), doc))
        .collect();
    let mut evidence = Vec::new();
    for (chunk_id, score) in ranked {
        if score <= 0.0 {
            continue;
        }
        let Some(doc) = by_chunk.get(chunk_id.as_str()) else {
            return Err(format!("retrieval returned an unknown chunk {chunk_id}"));
        };
        evidence.push(Evidence {
            evidence_id: doc.chunk_id.clone(),
            document_id: doc.document_id.clone(),
            revision: doc.revision.clone(),
            chunk_id: doc.chunk_id.clone(),
            text: doc.text.clone(),
            source_ref: doc.document_id.clone(),
            product_id: String::new(),
            rank: evidence.len() as u32 + 1,
            valid_until_utc_ms: (doc.valid_until_utc_ms != 0).then_some(doc.valid_until_utc_ms),
            authority: "rehearsal-knowledge-file".into(),
            permission_revision: doc.permission_revision.clone(),
            original_verifiable: true,
            ..Default::default()
        });
    }
    Ok(evidence)
}

/// Runtime gate snapshot, taken once after bring-up and rechecked for every
/// event. Every field is a snapshot of the live session, so both passes over the
/// same script observe exactly the same gates.
#[derive(Debug, Clone)]
struct Gate {
    workspace: String,
    session: String,
    live_epoch: u64,
    state_revision: u64,
    lifecycle: i32,
    mode: i32,
    duty_valid: bool,
    /// Captured at snapshot time: a live session, a current duty lease and a
    /// mode that permits output. Rechecked per event from this snapshot, never
    /// re-read, so the replay cannot disagree with the original pass.
    permission_current: bool,
    knowledge_revision: String,
}

impl Gate {
    fn lifecycle(&self) -> &'static str {
        lifecycle_name(self.lifecycle)
    }
    fn mode(&self) -> &'static str {
        mode_name(self.mode)
    }
    /// Preplay checks. The approval check binds the operator record to the exact
    /// candidate object: a mismatched hash never passes.
    fn preplay(
        &self,
        event: &ManualEvent,
        approval: Option<&ApprovalRecord>,
        candidate_hash: &str,
        now_ms: u64,
    ) -> PreplayChecks {
        let approval_valid = approval.is_some_and(|approval| {
            approval.utterance_id == event.utterance_id
                && approval.candidate_hash == candidate_hash
                && approval.expires_at_utc_ms > now_ms
        });
        PreplayChecks {
            approval_valid,
            facts_match: true,
            epoch_match: event.generation_epoch == self.live_epoch,
            permission_current: self.permission_current,
        }
    }
}

/// Output authorization for an event: the session is live, the operator's duty
/// lease still holds and the mode permits output at all. `rehearsal` mode never
/// does, and `assisted` still needs a current lease. This is the value the
/// preplay stage consumes as `permission_current`.
fn output_permitted(session: &Session, now: Instant) -> bool {
    let snapshot = session.snapshot();
    matches!(
        SessionLifecycle::try_from(snapshot.lifecycle),
        Ok(SessionLifecycle::Running)
    ) && SessionMode::try_from(snapshot.mode).ok() != Some(SessionMode::Rehearsal)
        && session.duty_valid(now)
}

/// What the trusted owner left the session in, observed after bring-up.
#[derive(Debug, Clone)]
struct BringUp {
    lifecycle: &'static str,
    mode: &'static str,
    epoch: u64,
    state_revision: u64,
    duty_valid: bool,
}

/// Drives the session to the state the script declares, the same way the
/// trusted owner drives the real one: grant the duty lease, set the mode, then
/// walk the lifecycle. A step that already holds is skipped, so a re-run against
/// a live session does not move it. The kernel validates the transition order:
/// an impossible chain is refused rather than coerced.
fn bring_up(shared: &Arc<Mutex<Composition>>, setup: &SessionSetup) -> Result<BringUp, String> {
    let mode = session_mode(&setup.mode)?;
    let ttl = Duration::from_millis(setup.duty_ttl_ms);
    let now = Instant::now();
    let mut composition = shared
        .lock()
        .map_err(|_| "composition lock poisoned".to_string())?;
    if mode == SessionMode::AutoSupervised {
        let auto_allowed =
            composition.config()["session_defaults"]["allow_auto_without_platform_grant"]
                .as_bool()
                .unwrap_or(false);
        if !auto_allowed {
            return Err(
                "the runtime profile forbids automatic replies without a platform grant".into(),
            );
        }
    }
    composition
        .confirm_duty(&setup.operator, ttl, now)
        .map_err(|error| format!("rehearsal duty lease refused: {error}"))?;
    let observed = {
        let mut session = composition.session_lock().map_err(|e| e.to_string())?;
        if session.snapshot().mode != mode as i32 {
            session
                .change_mode(true, mode, true, now)
                .map_err(|error| format!("rehearsal mode change refused: {error:?}"))?;
        }
        for target in &setup.lifecycle {
            let target = lifecycle_target(target)?;
            if session.snapshot().lifecycle == target as i32 {
                continue;
            }
            session.transition(true, target, true).map_err(|error| {
                format!(
                    "rehearsal lifecycle {} refused: {error:?}",
                    lifecycle_name(target as i32)
                )
            })?;
        }
        let snapshot = session.snapshot();
        BringUp {
            lifecycle: lifecycle_name(snapshot.lifecycle),
            mode: mode_name(snapshot.mode),
            epoch: snapshot.generation_epoch,
            state_revision: snapshot.state_revision,
            duty_valid: session.duty_valid(now),
        }
    };
    // One telemetry breadcrumb. The authoritative authorization record is the
    // critical duty-lease entry `confirm_duty` just wrote; this records the
    // state the operator drove so the change is auditable, and claims no
    // device, output or playback effect.
    let session_id = composition.session_id().to_string();
    composition
        .record(
            DecisionKind::Observation,
            Some(&session_id),
            &json!({
                "event": "session_bringup",
                "operator": setup.operator,
                "lifecycle": observed.lifecycle,
                "mode": observed.mode,
                "generation_epoch": observed.epoch,
                "state_revision": observed.state_revision,
                "duty_valid": observed.duty_valid,
                "candidate_output": false,
                "note": "control-state change; not a device or playback acknowledgement",
            }),
            &format!("{BRINGUP_TRACE_PREFIX}{session_id}"),
        )
        .map_err(|error| format!("bring-up record failed: {error}"))?;
    Ok(observed)
}

/// The live workspace and session ids. A script scoped to another pair is
/// refused rather than rebound.
fn composition_scope(shared: &Arc<Mutex<Composition>>) -> Result<(String, String), String> {
    let composition = shared
        .lock()
        .map_err(|_| "composition lock poisoned".to_string())?;
    Ok((
        composition.workspace().to_string(),
        composition.session_id().to_string(),
    ))
}

/// The admitted rehearsal render lane: the software 2D renderer plus everything
/// the per-event loop needs. It is admitted once per run so the one-time schema
/// and closure cost is never paid inside an event, exactly as the render-worker
/// pays it before its slot window opens.
struct RenderLane {
    avatar: Arc<Avatar>,
    grants: LicenseGrants,
    voice_ref: String,
    output_id: String,
    playback: RehearsalSink,
    sample_rate: u32,
    fps_num: u32,
    fps_den: u32,
}

fn read_render_json(path: &str, what: &str) -> Result<Value, String> {
    let raw = fs::read_to_string(path).map_err(|error| format!("{what} unreadable: {error}"))?;
    serde_json::from_str(&raw).map_err(|error| format!("{what} is not JSON: {error}"))
}

/// Admits the render closure the script declared. A missing, unreadable or
/// unlicensed input is a hard failure: the engine never substitutes a default
/// asset, voice or output identity.
fn render_lane(script: &Script) -> Result<Option<RenderLane>, String> {
    let Some(render) = script.render.as_ref() else {
        return Ok(None);
    };
    let manifest = read_render_json(&render.manifest, "avatar manifest")?;
    let config = read_render_json(&render.config, "avatar config")?;
    let mut asset_bytes = Vec::new();
    for asset in &render.assets {
        asset_bytes.push(
            fs::read(&asset.path)
                .map_err(|error| format!("render asset {} unreadable: {error}", asset.id))?,
        );
    }
    let assets = render
        .assets
        .iter()
        .zip(&asset_bytes)
        .map(|(asset, bytes)| AssetBytes {
            id: &asset.id,
            bytes,
        })
        .collect::<Vec<_>>();
    let grants = LicenseGrants::from(
        render
            .licenses
            .iter()
            .cloned()
            .collect::<BTreeSet<String>>(),
    );
    let ctx = PluginContext::new(
        "nai-avatar-2d",
        "",
        1,
        ADMIT_BUDGET.as_millis() as u32,
        json!({}),
        Vec::new(),
        Instant::now() + ADMIT_BUDGET,
        Arc::new(AtomicBool::new(false)),
        utc_ms(),
    );
    let avatar = Avatar::admit(&ctx, &manifest, &config, &assets, &grants).map_err(|error| {
        format!(
            "avatar admission rejected: code {}: {}",
            error.code, error.message
        )
    })?;
    let frame_geometry = |value: &Value, key: &str| -> Result<u32, String> {
        value[key]
            .as_u64()
            .filter(|number| (1..=u32::MAX as u64).contains(number))
            .map(|number| number as u32)
            .ok_or_else(|| format!("render input has no valid {key}"))
    };
    let lane = RenderLane {
        avatar: Arc::new(avatar),
        grants,
        voice_ref: render.voice_ref.clone(),
        output_id: render.output_id.clone(),
        playback: RehearsalSink::parse(&render.playback)?,
        sample_rate: frame_geometry(&config, "sample_rate")?,
        fps_num: frame_geometry(&manifest, "fps_num")?,
        fps_den: frame_geometry(&manifest, "fps_den")?,
    };
    Ok(Some(lane))
}

/// Everything the per-event evaluation needs. Pure inputs only, so two passes
/// over the same script produce identical outcomes.
struct Rehearsal<'a> {
    gate: &'a Gate,
    index: &'a Index,
    docs: &'a [KnowledgeDoc],
    approvals: &'a [ApprovalRecord],
    /// The admitted render lane, when the script declared one.
    render: Option<&'a RenderLane>,
    provider_ref: &'a str,
    price_revision: &'a str,
    max_output_chars: usize,
    top_k: usize,
    now_ms: u64,
}

/// Evaluates one manual event against the controlled rules. This performs no
/// persistence: it returns the outcome the pipeline then records. An `Err` means
/// a stage *failed* (the ordered halt applies); a blocked or silent outcome is a
/// normal `Ok`.
fn evaluate_event(
    rehearsal: &Rehearsal<'_>,
    event: &ManualEvent,
    injection: Injection,
    gateway: &mut dyn UsageGateway,
) -> Result<EventOutcome, String> {
    let gate = rehearsal.gate;
    let mut outcome = EventOutcome {
        event_id: event.event_id.clone(),
        utterance_id: event.utterance_id.clone(),
        stage: "received",
        route: String::new(),
        action: DecisionAction::ActionUnspecified as i32,
        guard: [None; 4],
        candidate_hash: String::new(),
        approval_id: String::new(),
        generated: String::new(),
        model_ref: String::new(),
        usage: BTreeMap::new(),
        reason: String::new(),
        evidence: 0,
        audio_samples: 0,
        frames: 0,
        speech_model_ref: String::new(),
        playback: "none",
        delivery: "none",
        playback_rejected: 0,
    };
    let text = normalize(&event.text);
    if text.is_empty() {
        outcome.stage = "normalized_empty";
        outcome.reason = "empty_event_text".into();
        return Ok(outcome);
    }

    // A gate recheck happens before the first outbound step: a stale epoch or a
    // lapsed duty lease never reaches retrieval or generation.
    let gate_now = gate.preplay(event, None, "", rehearsal.now_ms);
    if !gate_now.epoch_match {
        outcome.stage = "stale_epoch";
        outcome.reason = "stale_generation_epoch".into();
        return Ok(outcome);
    }
    if !gate_now.permission_current {
        outcome.stage = "permission_stale";
        outcome.reason = "duty_or_permission_stale".into();
        return Ok(outcome);
    }

    // Knowledge retrieval. A stage failure here is fatal; a miss is not.
    if injection == Injection::Knowledge {
        return Err("injected knowledge-stage failure".into());
    }
    let evidence = retrieve(rehearsal.index, rehearsal.docs, &text, rehearsal.top_k)?;
    outcome.evidence = evidence.len();
    if evidence.is_empty() {
        outcome.stage = "knowledge_no_hit";
        outcome.reason = "knowledge_no_hit".into();
        return Ok(outcome);
    }

    // Agent decision over a template candidate. The candidate is built and
    // validated by the kernel's rules, and the object hash is taken from the
    // validated object, so the approval binds exactly what the guard guards.
    let approval = rehearsal
        .approvals
        .iter()
        .find(|approval| approval.utterance_id == event.utterance_id);
    outcome.approval_id = approval
        .map(|approval| approval.approval_id.clone())
        .unwrap_or_default();
    let decision = route_decision(
        &gate.session,
        &gate.workspace,
        gate.mode,
        event,
        &text,
        &evidence,
        rehearsal.now_ms,
    )?;
    outcome.route = decision.route.clone();
    outcome.action = decision.action;
    let Some(validated) = spoken_cue(&decision) else {
        outcome.stage = "decision_not_speak";
        outcome.reason = decision.reason_code.clone();
        return Ok(outcome);
    };
    let mut cue = validated;
    outcome.candidate_hash = approval_object_hash(&cue);
    let observation = observation_for(&gate.session, &gate.workspace, gate.mode, event, &text)?;

    // Four-stage guard. Guard unavailability is a stage failure; a non-pass
    // verdict is a normal block.
    if injection == Injection::Guard {
        return Err("injected guard-unavailable failure".into());
    }
    let permissions = PermissionSnapshot {
        revision: gate.knowledge_revision.clone(),
        ..Default::default()
    };
    let preplay = gate.preplay(event, approval, &outcome.candidate_hash, rehearsal.now_ms);
    let stages = [
        (0usize, GuardStage::Input),
        (1, GuardStage::Evidence),
        (2, GuardStage::Output),
        (3, GuardStage::Preplay),
    ];
    let mut rewrites = 0u32;
    let policy = GuardPolicy::default();
    let mut blocked: Option<(GuardStage, &'static str)> = None;
    for (slot, stage) in stages {
        let request = GuardRequest {
            context: observation.context.clone(),
            stage: stage as i32,
            candidate: Some(cue.clone()),
            evidence: evidence.clone(),
            permissions: Some(permissions.clone()),
        };
        let result: GuardResult = guard(
            &request,
            &policy,
            rewrites,
            rehearsal.now_ms,
            (stage == GuardStage::Preplay).then_some(preplay),
        );
        outcome.guard[slot] = Some(result.verdict);
        match GuardVerdict::try_from(result.verdict) {
            Ok(GuardVerdict::Pass) => {}
            Ok(GuardVerdict::Rewrite) => {
                rewrites += 1;
                if let Some(rewritten) = result.rewritten_candidate {
                    cue = rewritten;
                }
            }
            Ok(_) => {
                outcome.reason = result.reason_codes.first().cloned().unwrap_or_default();
                blocked = Some((stage, "blocked"));
                break;
            }
            Err(_) => return Err("guard returned an unknown verdict".into()),
        }
    }
    if let Some((stage, _)) = blocked {
        outcome.stage = match stage {
            GuardStage::Input => "guard_input",
            GuardStage::Evidence => "guard_evidence",
            GuardStage::Output => "guard_output",
            _ => "guard_preplay",
        };
        return Ok(outcome);
    }

    // Text generation through the metered seam.
    let budget_ms = observation
        .context
        .as_ref()
        .map(|context| context.remaining_budget_ms)
        .unwrap_or(0);
    if budget_ms == 0 {
        outcome.stage = "budget_exhausted";
        outcome.reason = "task_budget_exhausted".into();
        return Ok(outcome);
    }
    let (generated, model_ref, usage) = generate(
        rehearsal,
        event,
        &cue,
        u64::from(budget_ms),
        injection,
        gateway,
    )
    .map_err(|error| format!("generation failed: {}", error.message))?;
    outcome.generated = generated;
    outcome.model_ref = model_ref;
    outcome.usage = usage;

    // Without a declared render closure the chain honestly stops at text:
    // nothing was synthesized, rendered, delivered or played.
    let Some(render) = rehearsal.render else {
        outcome.stage = "generated";
        return Ok(outcome);
    };

    // The guard clears the hash of every candidate it rewrote, and the approved
    // hash is the object the operator signed. An object no approval covers must
    // never be voiced, so a superseded cue stops here instead of being rendered.
    let spoken_hash = candidate_hash(&cue);
    if spoken_hash != outcome.candidate_hash {
        outcome.stage = "guard_superseded";
        outcome.reason = "guard_replaced_the_approved_candidate".into();
        return Ok(outcome);
    }
    // Recheck the gate immediately before the next outbound step: a lapsed duty
    // lease or a moved epoch must not reach speech.
    let gate_now = gate.preplay(event, approval, &outcome.candidate_hash, rehearsal.now_ms);
    if !gate_now.epoch_match || !gate_now.permission_current {
        outcome.stage = "permission_stale";
        outcome.reason = "duty_or_permission_stale_before_speech".into();
        return Ok(outcome);
    }
    // The cue that reaches the renderer must carry the approved hash verbatim:
    // the media layer's lease check and the renderer's own cue check both compare
    // it against the approved value.
    cue.candidate_hash = spoken_hash;
    cue.hash_algorithm = CANDIDATE_HASH_ALGORITHM.into();

    // Speech synthesis through the metered seam. The voiced text is the
    // approval-bound cue text, never the raw model rendering: the approval hash
    // has to cover exactly what is said.
    let speech = synthesize(
        rehearsal,
        event,
        &cue,
        u64::from(budget_ms),
        render,
        injection,
        gateway,
    )
    .map_err(|error| format!("synthesis failed: {}", error.message))?;
    outcome.audio_samples = speech.pcm.len() as u64;
    outcome.speech_model_ref = speech.model_ref.clone();
    outcome.delivery = "memory";

    let played = render_and_play(rehearsal, event, &cue, render, &speech)?;
    outcome.frames = played.frames;
    outcome.playback = played.state;
    outcome.playback_rejected = played.rejected;
    outcome.stage = played.stage;
    Ok(outcome)
}

/// What one event's render and playback pass produced.
struct PlaybackOutcome {
    frames: u32,
    /// The lane's playback state after the declared scenario.
    state: &'static str,
    rejected: usize,
    /// The stage the decision log records.
    stage: &'static str,
}

fn playback_state_name(state: PlaybackState) -> &'static str {
    match state {
        PlaybackState::Pending => "pending",
        PlaybackState::Committed => "committed",
        PlaybackState::Playing => "playing",
        PlaybackState::Completed => "completed",
        PlaybackState::Cancelled => "cancelled",
    }
}

/// Produces real pixels for the approved cue, commits the lane on its water
/// levels, and applies the declared playback outcome. Frames live in memory
/// only: no slot is written, no keeper consumes them and no device is involved.
fn render_and_play(
    rehearsal: &Rehearsal<'_>,
    event: &ManualEvent,
    cue: &PerformanceCue,
    render: &RenderLane,
    speech: &SpeechResponse,
) -> Result<PlaybackOutcome, String> {
    let total = speech.pcm.len() as u64;
    let expected_frames = frame_count(total, render.sample_rate, render.fps_num, render.fps_den);
    if expected_frames == 0 || expected_frames > MAX_FRAMES {
        return Err("synthesized audio has no bounded frame span".into());
    }
    let ctx = PluginContext::new(
        "nai-avatar-2d",
        &rehearsal.gate.session,
        event.generation_epoch,
        TASK_BUDGET_MS,
        json!({}),
        Vec::new(),
        Instant::now() + Duration::from_millis(u64::from(TASK_BUDGET_MS)),
        Arc::new(AtomicBool::new(false)),
        rehearsal.now_ms,
    );
    let mut lane = PlaybackLane::new(&LaneSpec {
        output_id: &render.output_id,
        utterance_id: &event.utterance_id,
        sample_rate: render.sample_rate,
        audio_water_ms: AUDIO_WATER_MS,
        expected_samples: total,
        expected_frames,
    })
    .map_err(|error| format!("playback lane rejected the utterance: {error:?}"))?;
    let mut stream = render.avatar.stream(&ctx, cue).map_err(contract_detail)?;
    let frame_ref = format!("engine-rehearsal:{}", event.event_id);
    let mut consumed = 0u64;
    let mut seq = 0u32;
    let mut frames = 0u32;
    while consumed < total {
        let end = frame_boundary(seq + 1, render.sample_rate, render.fps_num, render.fps_den);
        if end <= consumed {
            return Err("frame boundary did not advance".into());
        }
        let take = (end - consumed).min(total - consumed);
        let eos = consumed + take >= total;
        let block = AudioBlock {
            session_id: rehearsal.gate.session.clone(),
            utterance_id: event.utterance_id.clone(),
            generation_epoch: event.generation_epoch,
            seq,
            sample_rate: render.sample_rate,
            channels: 1,
            format: SampleFormat::F32,
            sample_start: consumed,
            sample_count: take as u32,
            end_of_stream: eos,
        };
        let slice = &speech.pcm[consumed as usize..(consumed + take) as usize];
        stream
            .push(&ctx, &render.grants, block.clone(), slice)
            .map_err(contract_detail)?;
        lane.push_audio(&block)
            .map_err(|error| format!("playback lane rejected the audio block: {error:?}"))?;
        if let Some(frame) = stream
            .next_frame(&ctx, &render.grants, &frame_ref)
            .map_err(contract_detail)?
        {
            // The renderer numbers frames from zero; the lane numbers them from
            // one. The conversion is checked rather than assumed: a stream that
            // skipped, repeated or reordered a frame must fail the run instead
            // of silently advancing the video water level.
            if frame.metadata.frame_seq != u64::from(frames) {
                return Err(format!(
                    "renderer frame sequence {} is not contiguous at {frames}",
                    frame.metadata.frame_seq
                ));
            }
            frames += 1;
            lane.push_frame(frames)
                .map_err(|error| format!("playback lane rejected frame {frames}: {error:?}"))?;
        }
        consumed += take;
        seq += 1;
    }
    if frames != expected_frames {
        return Err(format!(
            "declared {expected_frames} frames but the renderer produced {frames}"
        ));
    }
    lane.commit(rehearsal.now_ms, rehearsal.now_ms)
        .map_err(|error| format!("playback lane commit rejected: {error:?}"))?;
    if !lane.committed() {
        return Ok(PlaybackOutcome {
            frames,
            state: playback_state_name(lane.state()),
            rejected: 0,
            stage: "timeline_not_committed",
        });
    }
    let report = rehearse(render.playback, &mut lane);
    let stage = if report.completed {
        "played"
    } else {
        match render.playback {
            RehearsalSink::Partial | RehearsalSink::Complete => "playback_incomplete",
            RehearsalSink::Invalid => "playback_invalid",
            RehearsalSink::Cancelled => "playback_cancelled",
        }
    };
    Ok(PlaybackOutcome {
        frames,
        state: playback_state_name(lane.state()),
        rejected: report.rejected,
        stage,
    })
}

/// Runs the metered synthesis seam over the approval-bound cue text. The stub's
/// declared sample count is the total a complete acknowledgement must match, so
/// a synthesizer that disagrees with it is a defect rather than a variation.
fn synthesize(
    rehearsal: &Rehearsal<'_>,
    event: &ManualEvent,
    cue: &PerformanceCue,
    budget_ms: u64,
    render: &RenderLane,
    injection: Injection,
    gateway: &mut dyn UsageGateway,
) -> Result<SpeechResponse, ProviderError> {
    let request = SpeechRequest {
        request_id: format!("{}-tts", event.event_id),
        provider_ref: rehearsal.provider_ref.into(),
        session_id: rehearsal.gate.session.clone(),
        utterance_id: event.utterance_id.clone(),
        generation_epoch: event.generation_epoch,
        text: cue.speech_text.clone(),
        sample_rate: render.sample_rate,
        voice_ref: render.voice_ref.clone(),
        remaining_budget_ms: budget_ms,
        cancelled: false,
    };
    let expected = RehearsalSpeechSynthesizer::sample_count(&request.text, request.sample_rate);
    let mut stub = RehearsalSpeechSynthesizer::new();
    let mut failing = FailingSpeechSynthesizer;
    let synthesizer: &mut dyn SpeechSynthesizer = if injection == Injection::Speech {
        &mut failing
    } else {
        &mut stub
    };
    let response = synthesize_with_budget(
        synthesizer,
        gateway,
        usage_entry_tts(rehearsal, event),
        &request,
        rehearsal.price_revision,
    )?;
    if response.pcm.len() != expected {
        return Err(ProviderError::invalid_argument(
            "rehearsal synthesis disagreed with its declared sample count",
        ));
    }
    Ok(response)
}

fn contract_detail(error: nai_contracts::v1::ContractError) -> String {
    format!("code {}: {}", error.code, error.message)
}

/// Runs the metered generation seam. The generator is pure; reservation,
/// cancellation and usage accounting happen in `generate_with_budget`.
fn generate(
    rehearsal: &Rehearsal<'_>,
    event: &ManualEvent,
    cue: &PerformanceCue,
    budget_ms: u64,
    injection: Injection,
    gateway: &mut dyn UsageGateway,
) -> Result<(String, String, BTreeMap<String, u64>), ProviderError> {
    let request = TextRequest {
        request_id: format!("{}-gen", event.event_id),
        provider_ref: rehearsal.provider_ref.into(),
        session_id: rehearsal.gate.session.clone(),
        utterance_id: event.utterance_id.clone(),
        generation_epoch: event.generation_epoch,
        prompt: cue.speech_text.clone(),
        max_output_chars: rehearsal.max_output_chars,
        remaining_budget_ms: budget_ms,
        cancelled: false,
    };
    let mut stub = RehearsalTextGenerator::new();
    let mut failing = FailingTextGenerator;
    let generator: &mut dyn TextGenerator = if injection == Injection::Generate {
        &mut failing
    } else {
        &mut stub
    };
    let entry = usage_entry(rehearsal, event);
    let response = generate_with_budget(
        generator,
        gateway,
        entry,
        &request,
        rehearsal.price_revision,
    )?;
    let usage = response
        .usage
        .as_object()
        .map(|object| {
            object
                .iter()
                .filter_map(|(key, value)| value.as_u64().map(|value| (key.clone(), value)))
                .collect()
        })
        .unwrap_or_default();
    Ok((response.text, response.model_ref, usage))
}

/// A rehearsal budget book scoped to the engine workspace. Rehearsal-only: it
/// lives for the duration of one pipeline pass and never persists.
fn budget_gateway(
    workspace: &str,
    session: &str,
    provider: &str,
) -> Result<InMemoryGateway, ProviderError> {
    use nai_budget_core::{Ceiling, Limits};
    let ceiling = Ceiling {
        cost_minor: 0,
        units: [
            // One text call plus one synthesis call per event.
            ("requests".to_string(), QUEUE_CAPACITY as u64 * 4),
            ("tokens".to_string(), 1_000_000u64),
            ("prompt_tokens".to_string(), 1_000_000u64),
            ("completion_tokens".to_string(), 1_000_000u64),
            ("total_tokens".to_string(), 1_000_000u64),
            ("characters".to_string(), 1_000_000u64),
            ("audio_seconds".to_string(), 1_000_000u64),
        ]
        .into(),
        concurrent: [("provider_slots".to_string(), 1u64)].into(),
    };
    InMemoryGateway::new(
        workspace,
        Limits {
            currency: "CNY".into(),
            global: ceiling.clone(),
            sessions: [(session.to_string(), ceiling.clone())].into(),
            providers: [(provider.to_string(), ceiling)].into(),
            max_entries: 256,
        },
    )
    .map_err(|_| ProviderError::invalid_argument("rehearsal budget rejected"))
}

fn usage_entry(rehearsal: &Rehearsal<'_>, event: &ManualEvent) -> UsageLedgerEntry {
    UsageLedgerEntry {
        workspace_id: rehearsal.gate.workspace.clone(),
        session_id: rehearsal.gate.session.clone(),
        provider_ref: rehearsal.provider_ref.into(),
        request_id: format!("{}-gen", event.event_id),
        generation_epoch: event.generation_epoch,
        estimate: Some(BudgetCharge {
            currency: "CNY".into(),
            cost_minor: 0,
            units: BTreeMap::from([
                ("requests".to_string(), 1u64),
                ("total_tokens".to_string(), 4096u64),
            ]),
            price_revision: rehearsal.price_revision.into(),
        }),
        state: BillingState::Reserved as i32,
        ..Default::default()
    }
}

/// A generator that always fails. Used only by the `--inject generate` drill to
/// prove the ordered halt runs when provider generation fails.
struct FailingTextGenerator;
impl TextGenerator for FailingTextGenerator {
    fn generate(&mut self, _request: &TextRequest) -> Result<TextResponse, ProviderError> {
        Err(ProviderError::unavailable("injected generation failure"))
    }
}

/// The synthesis usage entry, reserved before the seam is called. The unit
/// vocabulary is the provider layer's own, so the charge is comparable with a
/// real TTS adapter's.
fn usage_entry_tts(rehearsal: &Rehearsal<'_>, event: &ManualEvent) -> UsageLedgerEntry {
    UsageLedgerEntry {
        workspace_id: rehearsal.gate.workspace.clone(),
        session_id: rehearsal.gate.session.clone(),
        provider_ref: rehearsal.provider_ref.into(),
        request_id: format!("{}-tts", event.event_id),
        generation_epoch: event.generation_epoch,
        estimate: Some(BudgetCharge {
            currency: "CNY".into(),
            cost_minor: 0,
            units: BTreeMap::from([
                ("requests".to_string(), 1u64),
                ("audio_seconds".to_string(), 8u64),
            ]),
            price_revision: rehearsal.price_revision.into(),
        }),
        state: BillingState::Reserved as i32,
        ..Default::default()
    }
}

/// A synthesizer that always fails. Used only by the `--inject speech` drill to
/// prove the ordered halt runs when the speech stage fails.
struct FailingSpeechSynthesizer;
impl SpeechSynthesizer for FailingSpeechSynthesizer {
    fn synthesize(&mut self, _request: &SpeechRequest) -> Result<SpeechResponse, ProviderError> {
        Err(ProviderError::unavailable("injected synthesis failure"))
    }
}

/// Runs the full rehearsal pipeline against the live composition. On a stage
/// failure it applies the ordered halt (P0 mute -> revoke with epoch increment
/// -> critical log) and returns an error, so the engine exits without a success
/// line.
pub fn run(shared: &Arc<Mutex<Composition>>, config: &PipelineConfig) -> Result<Value, String> {
    let script = load_script(&config.script)?;
    let index = index_build(&script.knowledge)?;
    // The render lane is admitted once, before any session state is driven: an
    // unlicensed or corrupt avatar closure must fail the run, not an event.
    let render = render_lane(&script)?;
    // An injection that could never fire is refused rather than silently
    // ignored: the drill must always be able to prove what it claims.
    if config.injection == Injection::Speech && script.render.is_none() {
        return Err("--inject speech requires a script with a render closure".into());
    }

    // The script is scoped to one session/workspace pair, checked before any
    // state is driven: a candidate hash is only meaningful inside its scope.
    let (workspace, session) = composition_scope(shared)?;
    if session != script.session_id || workspace != script.workspace_id {
        return Err(format!(
            "rehearsal script is scoped to {}/{} but the live session is {workspace}/{session}",
            script.workspace_id, script.session_id
        ));
    }

    // The operator's bring-up: duty lease, mode, lifecycle. It is explicit input
    // rather than a hidden side effect, and it is what makes a manual event
    // admissible at all.
    bring_up(shared, &script.session)?;
    let gate = snapshot_gate(shared, &script.knowledge_revision)?;
    if !gate.duty_valid {
        return Err("rehearsal duty lease is not valid".into());
    }
    if !gate.permission_current {
        return Err(format!(
            "rehearsal session {}/{} is {} in {} mode: no event can be admitted",
            gate.workspace,
            gate.session,
            gate.lifecycle(),
            gate.mode()
        ));
    }
    let now_ms = utc_ms();

    let rehearsal = Rehearsal {
        gate: &gate,
        index: &index,
        docs: &script.knowledge,
        approvals: &script.approvals,
        render: render.as_ref(),
        provider_ref: &config.provider_ref,
        price_revision: &config.price_revision,
        max_output_chars: config.max_output_chars,
        top_k: script.top_k,
        now_ms,
    };

    // Pass 1: evaluate and persist. One budget book spans the whole pass.
    let mut gateway = budget_gateway(&gate.workspace, &gate.session, &config.provider_ref)
        .map_err(|error| format!("rehearsal budget unavailable: {error}"))?;
    let mut outcomes = Vec::new();
    for event in &script.events {
        let outcome = match evaluate_event(&rehearsal, event, config.injection, &mut gateway) {
            Ok(outcome) => outcome,
            Err(error) => {
                let _ = ordered_halt(shared);
                return Err(error);
            }
        };
        if config.injection == Injection::Storage {
            // Mute and revoke first; the oversized write that follows must fail
            // and must not roll the mute back.
            ordered_halt(shared)?;
        }
        if let Err(error) = persist(shared, &outcome, config.injection == Injection::Storage) {
            let _ = ordered_halt(shared);
            return Err(error);
        }
        outcomes.push(outcome);
    }

    // Pass 2: replay the same inputs with the same deterministic generator, then
    // compare the *persisted* log against the replay. A divergence fails the
    // R07 criterion honestly instead of being smoothed over.
    let mut replayed = Vec::new();
    let mut replay_gateway = budget_gateway(&gate.workspace, &gate.session, &config.provider_ref)
        .map_err(|error| format!("rehearsal budget unavailable: {error}"))?;
    for event in &script.events {
        let outcome = evaluate_event(&rehearsal, event, Injection::None, &mut replay_gateway)?;
        replayed.push(outcome);
    }
    let logged = read_log_canonical(shared)?;
    let replay_matched = logged.len() == replayed.len()
        && logged
            .iter()
            .zip(replayed.iter())
            .all(|(left, right)| *left == right.canonical_text());
    let chain_verified = verify_chain(shared)?;
    let generated = outcomes
        .iter()
        .filter(|outcome| !outcome.generated.is_empty())
        .count();
    let played = outcomes
        .iter()
        .filter(|outcome| outcome.stage == "played")
        .count();
    let blocked = outcomes
        .iter()
        .filter(|outcome| outcome.stage.starts_with("guard_"))
        .count();
    let silent = outcomes.len() - generated - blocked;

    let summary = json!({
        "pipeline": "engine-manual-event-pipeline",
        "generator": RehearsalTextGenerator::MODEL_REF,
        "speech": render.as_ref().map(|_| RehearsalSpeechSynthesizer::MODEL_REF),
        "renderer": render.as_ref().map(|_| RENDER_BINDING),
        "playback_sink": render.as_ref().map(|_| RehearsalSink::OUTPUT_KIND),
        "playback_scenario": render.as_ref().map(|lane| lane.playback.as_str()),
        "injection": config.injection.as_str(),
        "rehearsal": true,
        "real_provider": false,
        "real_tts": false,
        "devices": false,
        "frame_slot": false,
        "keeper_consume": false,
        // True only when every event that produced text also reached a verified
        // actual-playback completion.
        "playback_ack": generated > 0 && played == generated,
        "delivery": if render.is_some() { "memory" } else { "none" },
        // The live gate every event was evaluated under.
        "session_lifecycle": gate.lifecycle(),
        "session_mode": gate.mode(),
        "session_epoch": gate.live_epoch,
        "session_state_revision": gate.state_revision,
        "duty_operator": script.session.operator,
        "retrieval_top_k": script.top_k,
        // Frames exist in memory only: no slot, keeper, window or device output.
        "output_available": false,
        "events": outcomes.len(),
        "generated": generated,
        "played": played,
        "blocked": blocked,
        "silent_or_unrouted": silent,
        "chain_verified": chain_verified,
        "replay_matched": replay_matched,
        "logged_entries": logged.len(),
        "digest": log_digest(&logged),
        "outcomes": outcomes.iter().map(outcome_summary).collect::<Vec<_>>(),
    });
    if !chain_verified || !replay_matched {
        return Err(format!(
            "pipeline criteria not met: chain_verified={chain_verified} replay_matched={replay_matched}"
        ));
    }
    println!("nai-engine pipeline: {summary}");
    Ok(summary)
}

/// One event's line in the printed summary.
fn outcome_summary(outcome: &EventOutcome) -> Value {
    json!({
        "event_id": outcome.event_id,
        "utterance_id": outcome.utterance_id,
        "stage": outcome.stage,
        "route": outcome.route,
        "guard": outcome.guard,
        "candidate_hash": outcome.candidate_hash,
        "approval_id": outcome.approval_id,
        "reason": outcome.reason,
        "audio_samples": outcome.audio_samples,
        "frames": outcome.frames,
        "speech_model_ref": outcome.speech_model_ref,
        "playback": outcome.playback,
        "delivery": outcome.delivery,
        "playback_rejected": outcome.playback_rejected,
    })
}

/// A stable digest over the canonical log summaries, used for the cross-process
/// replay comparison. Rehearsal-only, not a cryptographic commitment.
fn log_digest(logged: &[String]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for line in logged {
        for byte in line.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash ^= 0xffff_ffff;
    }
    format!("{hash:016x}")
}

/// Snapshots the live gate: workspace, session, epoch, state revision, mode,
/// lifecycle, duty validity and the derived output authorization.
fn snapshot_gate(
    shared: &Arc<Mutex<Composition>>,
    knowledge_revision: &str,
) -> Result<Gate, String> {
    let composition = shared
        .lock()
        .map_err(|_| "composition lock poisoned".to_string())?;
    let session = composition.session_lock().map_err(|e| e.to_string())?;
    let now = Instant::now();
    let snapshot = session.snapshot();
    Ok(Gate {
        workspace: composition.workspace().to_string(),
        session: composition.session_id().to_string(),
        live_epoch: snapshot.generation_epoch,
        state_revision: snapshot.state_revision,
        lifecycle: snapshot.lifecycle,
        mode: snapshot.mode,
        duty_valid: session.duty_valid(now),
        permission_current: output_permitted(&session, now),
        knowledge_revision: knowledge_revision.to_string(),
    })
}

/// Appends one event outcome to the decision log. The canonical summary is
/// stored alongside the payload, so the read-back comparison compares exactly
/// the same object the replay produced.
fn persist(
    shared: &Arc<Mutex<Composition>>,
    outcome: &EventOutcome,
    inject_storage_failure: bool,
) -> Result<(), String> {
    let mut composition = shared
        .lock()
        .map_err(|_| "composition lock poisoned".to_string())?;
    let session = composition.session_id().to_string();
    let mut payload = json!({
        "event_id": outcome.event_id,
        "utterance_id": outcome.utterance_id,
        "stage": outcome.stage,
        "route": outcome.route,
        "action": outcome.action,
        "guard": outcome.guard,
        "candidate_hash": outcome.candidate_hash,
        "approval_id": outcome.approval_id,
        "generated": outcome.generated,
        "model_ref": outcome.model_ref,
        "usage": outcome.usage,
        "reason": outcome.reason,
        "evidence": outcome.evidence,
        "canonical": outcome.canonical(),
        "rehearsal": true,
        "delivery": outcome.delivery,
        "audio_samples": outcome.audio_samples,
        "frames": outcome.frames,
        "speech_model_ref": outcome.speech_model_ref,
        "playback": outcome.playback,
        "playback_rejected": outcome.playback_rejected,
    });
    if inject_storage_failure {
        // A payload larger than the store bound: the store rejects the write.
        payload["injected_padding"] = Value::String("x".repeat(OVERSIZED_PAYLOAD));
    }
    // Only a verified actual-playback completion is a `PlaybackAck`. An
    // incomplete, invalid or cancelled playback produced text but did not play,
    // so it stays a synthesis record rather than claiming the audience heard it.
    let kind = if outcome.stage == "played" {
        DecisionKind::PlaybackAck
    } else if outcome.stage.starts_with("guard_") {
        DecisionKind::Guard
    } else if !outcome.generated.is_empty() {
        DecisionKind::Synthesis
    } else {
        DecisionKind::Observation
    };
    composition
        .record(
            kind,
            Some(&session),
            &payload,
            &format!("{PIPELINE_TRACE_PREFIX}{}", outcome.event_id),
        )
        .map(|_| ())
        .map_err(|error| format!("decision log write failed: {error}"))
}

/// Reads back the pipeline's own decision entries and returns their canonical
/// summaries in chain order.
fn read_log_canonical(shared: &Arc<Mutex<Composition>>) -> Result<Vec<String>, String> {
    let (workspace, store) = {
        let composition = shared
            .lock()
            .map_err(|_| "composition lock poisoned".to_string())?;
        (
            composition.workspace().to_string(),
            composition.store().clone(),
        )
    };
    let operation = Operation::new(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    let guard = store
        .try_lock()
        .map_err(|_| "decision store busy while reading the log".to_string())?;
    let entries = guard
        .decision_entries(&operation, &workspace, LOG_READ_LIMIT)
        .map_err(|error| format!("decision log read failed: {error}"))?;
    let mut canonical = Vec::new();
    for entry in entries {
        if !entry.trace_id.starts_with(PIPELINE_TRACE_PREFIX) {
            continue;
        }
        let payload: Value = serde_json::from_str(&entry.payload_json)
            .map_err(|error| format!("decision payload unreadable: {error}"))?;
        let summary = payload
            .get("canonical")
            .ok_or_else(|| "decision payload has no canonical summary".to_string())?;
        canonical.push(serde_json::to_string(summary).unwrap_or_default());
    }
    Ok(canonical)
}

fn verify_chain(shared: &Arc<Mutex<Composition>>) -> Result<bool, String> {
    let (workspace, store) = {
        let composition = shared
            .lock()
            .map_err(|_| "composition lock poisoned".to_string())?;
        (
            composition.workspace().to_string(),
            composition.store().clone(),
        )
    };
    let operation = Operation::new(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    let guard = store
        .try_lock()
        .map_err(|_| "decision store busy during chain verification".to_string())?;
    guard
        .verify_decision_chain(&operation, &workspace)
        .map(|_| true)
        .map_err(|error| format!("decision log chain broken: {error}"))
}

/// The ordered halt: P0 mute first, then revocation with an epoch increment,
/// then the critical cancellation entry. A log failure is reported and never
/// rolls the mute back.
pub fn ordered_halt(shared: &Arc<Mutex<Composition>>) -> Result<(), String> {
    let mut composition = shared
        .lock()
        .map_err(|_| "composition lock poisoned".to_string())?;
    composition.emergency().mute();
    composition
        .control(ControlAction::Mute)
        .map(|_| ())
        .map_err(|error| format!("ordered halt log failed: {error}"))
}

fn utc_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(chunk: &str, text: &str) -> KnowledgeDoc {
        KnowledgeDoc {
            document_id: "kb-doc-1".into(),
            chunk_id: chunk.into(),
            text: text.into(),
            revision: "kb-doc-1@1".into(),
            permission_revision: "kb-rev-1".into(),
            valid_until_utc_ms: 0,
        }
    }

    #[test]
    fn normalize_is_deterministic_and_bounded() {
        assert_eq!(normalize("  你好   世界  "), "你好 世界");
        let long = "字".repeat(MAX_EVENT_TEXT_CHARS * 2);
        assert_eq!(normalize(&long).chars().count(), MAX_EVENT_TEXT_CHARS);
    }

    #[test]
    fn retrieval_is_deterministic_and_reports_a_real_miss() {
        let docs = vec![doc("c1", "商品价格是 199 元"), doc("c2", "发货时间是三天")];
        let index = index_build(&docs).unwrap();
        let hits = retrieve(&index, &docs, "商品价格", 3).unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].evidence_id, "c1");
        let again = retrieve(&index, &docs, "商品价格", 3).unwrap();
        assert_eq!(
            hits.iter()
                .map(|e| e.evidence_id.clone())
                .collect::<Vec<_>>(),
            again
                .iter()
                .map(|e| e.evidence_id.clone())
                .collect::<Vec<_>>()
        );
        // The scores are exact token overlaps, so an unrelated query is a real
        // zero rather than a low-scoring hit that a threshold has to hide.
        assert!(
            retrieve(&index, &docs, "完全无关的查询词", 3)
                .unwrap()
                .is_empty()
        );
        assert!(
            retrieve(&index, &docs, "你好啊朋友们", 3)
                .unwrap()
                .is_empty()
        );
        assert_eq!(index.sink, index.slots.len());
        // Precision boundary of a token index: one shared unigram ("天" with
        // "发货时间是三天") is a real, if weak, overlap. It is reported as a
        // hit rather than hidden, and no hash collision is involved.
        assert!(
            !retrieve(&index, &docs, "今天天气真不错", 3)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn candidate_hash_is_reproducible_and_binds_the_expiry() {
        let event = ManualEvent {
            event_id: "ev-1".into(),
            utterance_id: "utt-1".into(),
            source: SOURCE_MANUAL.into(),
            kind: "comment".into(),
            text: "这个多少钱".into(),
            generation_epoch: 1,
            expires_at_utc_ms: 4_000_000_000_000,
        };
        let evidence = vec![Evidence {
            evidence_id: "c1".into(),
            text: "价格 199 元".into(),
            revision: "kb@1".into(),
            permission_revision: "kb-rev-1".into(),
            original_verifiable: true,
            ..Default::default()
        }];
        let first = candidate_for("s", "w", &event, "这个多少钱", &evidence);
        let second = candidate_for("s", "w", &event, "这个多少钱", &evidence);
        assert_eq!(approval_object_hash(&first), approval_object_hash(&second));
        let mut moved = event.clone();
        moved.expires_at_utc_ms += 1;
        let third = candidate_for("s", "w", &moved, "这个多少钱", &evidence);
        assert_ne!(
            approval_object_hash(&first),
            approval_object_hash(&third),
            "a different candidate must not reuse an approval"
        );
    }

    #[test]
    fn preplay_binds_the_approval_to_the_exact_object() {
        let gate = Gate {
            workspace: "w".into(),
            session: "s".into(),
            live_epoch: 4,
            state_revision: 3,
            lifecycle: SessionLifecycle::Running as i32,
            mode: SessionMode::Assisted as i32,
            duty_valid: true,
            permission_current: true,
            knowledge_revision: "kb-rev-1".into(),
        };
        let event = ManualEvent {
            event_id: "e".into(),
            utterance_id: "u".into(),
            source: SOURCE_MANUAL.into(),
            kind: "comment".into(),
            text: "t".into(),
            generation_epoch: 4,
            expires_at_utc_ms: 2_000,
        };
        let matching = ApprovalRecord {
            utterance_id: "u".into(),
            approval_id: "ap".into(),
            candidate_hash: "abc".into(),
            expires_at_utc_ms: 10_000,
        };
        assert!(
            gate.preplay(&event, Some(&matching), "abc", 1_000)
                .approval_valid
        );
        assert!(
            !gate
                .preplay(&event, Some(&matching), "def", 1_000)
                .approval_valid,
            "a mismatched object hash must not pass"
        );
        let expired = ApprovalRecord {
            expires_at_utc_ms: 500,
            ..matching.clone()
        };
        assert!(
            !gate
                .preplay(&event, Some(&expired), "abc", 1_000)
                .approval_valid
        );
        assert!(!gate.preplay(&event, None, "abc", 1_000).approval_valid);
        let mut stale = event.clone();
        stale.generation_epoch = 3;
        assert!(
            !gate
                .preplay(&stale, Some(&matching), "abc", 1_000)
                .epoch_match
        );
    }

    fn script_json(source: &str, mode: &str, lifecycle: &[&str]) -> Value {
        json!({
            "schema_version": 1,
            "session_id": "s",
            "workspace_id": "w",
            "knowledge_revision": "kb-rev-1",
            "top_k": 3,
            "session": {
                "operator": "local-operator",
                "duty_ttl_ms": 300_000,
                "mode": mode,
                "lifecycle": lifecycle,
            },
            "knowledge": [{"document_id":"d","chunk_id":"c","text":"t","revision":"r","permission_revision":"p"}],
            "events": [{"event_id":"e","utterance_id":"u","source":source,"kind":"comment","text":"t","generation_epoch":1,"expires_at_utc_ms":1}]
        })
    }

    fn write_script(name: &str, value: &Value) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("nai-script-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("script.json");
        std::fs::write(&path, value.to_string()).unwrap();
        path
    }

    #[test]
    fn script_refuses_a_non_manual_source() {
        let path = write_script("source", &script_json("platform", "assisted", &["running"]));
        let error = load_script(&path).unwrap_err();
        assert!(error.contains("only manual"), "got {error}");
    }

    #[test]
    fn script_refuses_rehearsal_mode_and_an_unnamed_lifecycle_target() {
        let path = write_script("mode", &script_json("manual", "rehearsal", &["running"]));
        let error = load_script(&path).unwrap_err();
        assert!(error.contains("never permits output"), "got {error}");

        let path = write_script("lifecycle", &script_json("manual", "assisted", &["live"]));
        let error = load_script(&path).unwrap_err();
        assert!(error.contains("unknown session lifecycle"), "got {error}");
    }

    #[test]
    fn script_bounds_the_retrieval_depth_and_the_duty_window() {
        let mut value = script_json("manual", "assisted", &["running"]);
        value["top_k"] = json!(0);
        let error = load_script(&write_script("topk", &value)).unwrap_err();
        assert!(error.contains("top_k"), "got {error}");

        let mut value = script_json("manual", "assisted", &["running"]);
        value["session"]["duty_ttl_ms"] = json!(0);
        let error = load_script(&write_script("duty", &value)).unwrap_err();
        assert!(error.contains("duty_ttl_ms"), "got {error}");
    }

    fn render_block() -> Value {
        json!({
            "manifest": "manifest.json",
            "config": "config.json",
            "assets": [
                {"id": "base", "path": "base.rgba"},
                {"id": "closed", "path": "closed.rgba"},
                {"id": "open", "path": "open.rgba"},
            ],
            "licenses": ["test-license"],
            "voice_ref": "voice-rehearsal",
            "output_id": "engine-rehearsal-out",
            "playback": "complete",
        })
    }

    #[test]
    fn the_render_closure_is_optional_but_never_defaulted() {
        // Absent: the chain honestly stops at text with no audio and no frames.
        let value = script_json("manual", "assisted", &["running"]);
        assert!(
            load_script(&write_script("no-render", &value))
                .unwrap()
                .render
                .is_none()
        );

        // Present and complete: relative paths resolve against the script.
        let mut value = script_json("manual", "assisted", &["running"]);
        value["render"] = render_block();
        let script = load_script(&write_script("render", &value)).unwrap();
        let render = script.render.as_ref().unwrap();
        assert!(
            std::path::Path::new(&render.manifest).is_absolute(),
            "the loader must resolve the closure against the script directory"
        );
        assert_eq!(render.playback, "complete");
        assert_eq!(render.assets.len(), 3);

        let mut two_assets = value.clone();
        two_assets["render"]["assets"] = json!([
            {"id": "base", "path": "base.rgba"},
            {"id": "open", "path": "open.rgba"}
        ]);
        assert!(
            load_script(&write_script("render-assets", &two_assets))
                .unwrap_err()
                .contains("exactly three")
        );

        let mut duplicate = value.clone();
        duplicate["render"]["assets"] = json!([
            {"id": "base", "path": "base.rgba"},
            {"id": "base", "path": "closed.rgba"},
            {"id": "open", "path": "open.rgba"}
        ]);
        assert!(
            load_script(&write_script("render-duplicate", &duplicate))
                .unwrap_err()
                .contains("duplicate render asset id")
        );

        let mut no_voice = value.clone();
        no_voice["render"]["voice_ref"] = json!("");
        assert!(
            load_script(&write_script("render-voice", &no_voice))
                .unwrap_err()
                .contains("voice binding")
        );

        let mut no_output = value.clone();
        no_output["render"]["output_id"] = json!("");
        assert!(
            load_script(&write_script("render-output", &no_output))
                .unwrap_err()
                .contains("output identity")
        );

        let mut no_license = value.clone();
        no_license["render"]["licenses"] = json!([]);
        assert!(
            load_script(&write_script("render-license", &no_license))
                .unwrap_err()
                .contains("license reference")
        );

        let mut bad_scenario = value.clone();
        bad_scenario["render"]["playback"] = json!("maybe");
        assert!(
            load_script(&write_script("render-scenario", &bad_scenario))
                .unwrap_err()
                .contains("unknown playback scenario")
        );
    }
}
