//! Live manual-question lane. Only the app composes knowledge, API, guards and approvals.
use crate::pipeline::{RenderLane, RenderSetup, render_lane, resolve_render_setup};
use nai_budget_core::{Ceiling, Change, Limits};
use nai_contracts::v1::*;
use nai_control_api::{
    KnowledgePublishRequest, ManualEventRequest, ManualEventSurface, ManualEventView,
    PreviewRequest, PreviewSurface, PreviewView,
};
use nai_kernel::assemble::{AssemblyInput, CandidateFragment, SegmentInput, plan_assembly};
use nai_kernel::evaluate::assembly::*;
use nai_kernel::evaluate::guard::{GuardPolicy, guard};
use nai_kernel::evaluate::harness::{ObservationLayer, ObservationPolicy, ObservationRecord};
use nai_provider_core::{
    DispatchDecision, Protocol, ProviderClient, ProviderError, ProviderProfile, UsageGateway,
};
use nai_runtime::{AssemblyOutcome, Composition, ObservationOutcome};
use nai_security::{Operation, Secret};
use nai_storage::{ImportChunk, ImportedDocument, Store};
use prost::Message;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Semaphore;

#[path = "program.rs"]
mod program;

type Result<T> = std::result::Result<T, ErrorCode>;
struct GenerationFailure {
    code: ErrorCode,
    detail: &'static str,
}
impl GenerationFailure {
    fn at(code: ErrorCode, detail: &'static str) -> Self {
        Self { code, detail }
    }
}
impl From<ErrorCode> for GenerationFailure {
    fn from(code: ErrorCode) -> Self {
        Self::at(code, "生成任务未完成；请检查运行条件，未自动重试。")
    }
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
fn number(v: &Value, key: &str) -> u64 {
    v[key].as_u64().unwrap_or(0)
}
fn op() -> Result<Operation> {
    Operation::new(Duration::from_secs(5)).map_err(|e| e.code)
}

/// Constructs the profile and client exactly once from a validated operator
/// scene price/`provider` section. A missing or blank credential environment
/// variable refuses construction(§638: absence is an explicit CapabilityNotReady,
/// never a fallback) — both [`OperatorLane::new`] and the preflight path share
/// this single source so a demo scene cannot pass preflight with a provider it
/// could not actually construct.
pub(crate) fn build_provider(
    config: &Value,
) -> std::result::Result<(ProviderProfile, ProviderClient), String> {
    if config.get("provider_routes").is_some() {
        let routes = crate::provider_routes::Routes::build(config, "llm")?;
        for e in routes.endpoints {
            if let Some(client) = e.client {
                return Ok((e.profile, client));
            }
        }
        return Err("CAPABILITY_NOT_READY: no credentialed LLM route".into());
    }
    let p = &config["provider"];
    let profile = profile_from(p)?;
    let client = client_from(p, &profile)?;
    Ok((profile, client))
}

pub(crate) fn client_from(
    p: &Value,
    profile: &ProviderProfile,
) -> std::result::Result<ProviderClient, String> {
    let name = text(p, "credential_env");
    let secret = std::env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| format!("CAPABILITY_NOT_READY: missing environment variable {name}"))?;
    profile
        .client(Secret::new(secret.into_bytes()).map_err(|_| "credential invalid")?)
        .map_err(|_| "provider configuration rejected".into())
}
pub(crate) fn profile_from(p: &Value) -> std::result::Result<ProviderProfile, String> {
    Ok(ProviderProfile {
        provider_ref: text(p, "id").into(),
        endpoint: text(p, "endpoint").into(),
        model: text(p, "model").into(),
        price_revision: text(p, "price_revision").into(),
        currency: text(p, "currency").into(),
        timeout: Duration::from_millis(number(p, "timeout_ms")),
        max_retries: 0,
        protocol: Protocol::from_config(text(p, "protocol"))
            .map_err(|_| "provider protocol invalid")?,
    })
}

pub struct Loaded {
    pub config: Value,
    pub base_dir: PathBuf,
}

pub fn load(path: &Path) -> std::result::Result<Loaded, String> {
    if fs::metadata(path)
        .map_err(|_| "operator config unreadable")?
        .len()
        > 1024 * 1024
    {
        return Err("operator config exceeds 1MiB".into());
    }
    let bytes = fs::read(path).map_err(|_| "operator config unreadable")?;
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| "operator config invalid JSON")?;
    let schema: Value = serde_json::from_str(include_str!(
        "../../../schemas/operator/manual-session.schema.json"
    ))
    .map_err(|_| "operator schema invalid")?;
    let validator = jsonschema::validator_for(&schema).map_err(|_| "operator schema invalid")?;
    if !validator.is_valid(&value) {
        return Err("operator config schema rejected".into());
    }
    crate::provider_routes::validate(&value)?;
    let docs = value["knowledge"]["documents"].as_array().unwrap();
    validate_fact_windows(docs)?;
    let ids: std::collections::BTreeSet<_> = docs.iter().map(|d| text(d, "id")).collect();
    if ids.len() != docs.len() {
        return Err("operator document IDs must be unique".into());
    }
    let base_dir = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    Ok(Loaded {
        config: value,
        base_dir,
    })
}

/// Validate declared time windows before importing or bootstrapping a session.
fn validate_fact_windows(docs: &[Value]) -> std::result::Result<(), String> {
    for doc in docs {
        match (doc.get("valid_from_utc_ms"), doc.get("valid_until_utc_ms")) {
            (None, None) => {}
            (Some(from), Some(until))
                if from
                    .as_i64()
                    .zip(until.as_i64())
                    .is_some_and(|(from, until)| {
                        from >= 0 && from < until && until <= 253402300799999
                    }) => {}
            _ => {
                return Err(
                    "operator fact window invalid: both bounds required, from < until".into(),
                );
            }
        }
    }
    Ok(())
}
fn deadline_for_expiry(deadline: Instant, expiry: u64) -> Result<Instant> {
    let remaining = expiry
        .checked_sub(now())
        .filter(|v| *v > 0)
        .ok_or(ErrorCode::EvidenceExpired)?;
    Ok(Instant::now()
        .checked_add(Duration::from_millis(remaining))
        .unwrap_or(deadline)
        .min(deadline))
}
/// Explicit trusted-owner provisioning; never supplied by the model or event route.
pub fn provision(
    store: &mut Store,
    workspace: &str,
    actor: &str,
    config: &Value,
) -> std::result::Result<(), String> {
    let operation =
        Operation::new(nai_storage::BRINGUP_OPERATION).map_err(|_| "operator operation invalid")?;
    for permission in [
        "knowledge.read",
        "knowledge.import",
        "knowledge.publish",
        "budget.manage",
    ] {
        store
            .set_permission(&operation, workspace, actor, permission, "*", true)
            .map_err(|e| e.to_string())?;
    }
    publish_knowledge(store, workspace, actor, config)?;
    store
        .create_session(&operation, workspace, actor, workspace)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Publish into an existing workspace without granting new permissions or starting a session.
pub fn publish_knowledge(
    store: &mut Store,
    workspace: &str,
    actor: &str,
    config: &Value,
) -> std::result::Result<(), String> {
    publish_knowledge_until(
        store,
        workspace,
        actor,
        config,
        Instant::now() + nai_storage::BRINGUP_OPERATION,
    )
}
fn publish_knowledge_until(
    store: &mut Store,
    workspace: &str,
    actor: &str,
    config: &Value,
    deadline: Instant,
) -> std::result::Result<(), String> {
    validate_fact_windows(
        config["knowledge"]["documents"]
            .as_array()
            .ok_or("operator documents invalid")?,
    )?;
    let operation = Operation::new(
        deadline
            .saturating_duration_since(Instant::now())
            .min(nai_storage::BRINGUP_OPERATION),
    )
    .map_err(|e| e.to_string())?;
    let kb = &config["knowledge"];
    let mut ids = Vec::new();
    for doc in kb["documents"].as_array().unwrap() {
        let id = text(doc, "id");
        let content = text(doc, "text");
        store
            .import_document(
                &operation,
                actor,
                &ImportedDocument {
                    workspace_id: workspace.into(),
                    document_id: id.into(),
                    source: text(doc, "source").into(),
                    parser_version: "operator-text-v1".into(),
                    product_scope: workspace.into(),
                    acl_ref: text(kb, "scope").into(),
                    original_artifact_ref: None,
                    valid_from_ms: doc["valid_from_utc_ms"].as_i64(),
                    valid_until_ms: doc["valid_until_utc_ms"].as_i64(),
                    local_only: doc["local_only"].as_bool().unwrap(),
                    content: content.into(),
                    chunks: vec![ImportChunk {
                        chunk_id: format!("{id}-text"),
                        content: content.into(),
                        source_span: "full-text".into(),
                        product_scope: workspace.into(),
                        acl_ref: text(kb, "scope").into(),
                        tokens: nai_knowledge_core::tokenize(content),
                    }],
                },
            )
            .map_err(|e| e.to_string())?;
        ids.push(id.to_owned());
    }
    store
        .stage_snapshot(
            &operation,
            actor,
            workspace,
            text(kb, "snapshot"),
            text(kb, "scope"),
            &ids,
            None,
            None,
        )
        .map_err(|e| e.to_string())?;
    let sealed = store
        .index_snapshot(&operation, actor, workspace, text(kb, "snapshot"))
        .map_err(|e| e.to_string())?;
    store
        .publish_snapshot(&operation, actor, &sealed)
        .map_err(|e| e.to_string())?;
    Ok(())
}

struct Inner {
    program: Mutex<Option<program::Program>>,
    knowledge: Mutex<(String, String)>,
    knowledge_paused: AtomicBool,
    knowledge_capacity: Arc<Semaphore>,
    prepared: Mutex<Option<PreparedKnowledge>>,
    emergency: nai_media_core::EmergencyHandle,
    shared: Arc<Mutex<Composition>>,
    store: Arc<Mutex<Store>>,
    workspace: String,
    actor: String,
    session: String,
    boot: String,
    config: Value,
    llm_routes: crate::provider_routes::Routes,
    tts_routes: crate::provider_routes::Routes,
    capacity: Arc<Semaphore>,
    history: Mutex<VecDeque<ManualEventView>>,
    render: Option<RenderLane>,
    artifacts: PathBuf,
    previews: Mutex<VecDeque<PreviewRecord>>,
    preview_capacity: Arc<Semaphore>,
    resident_output: Mutex<Option<crate::local_playback::ResidentOutput>>,
    stop: Arc<AtomicBool>,
}
#[derive(Clone)]
struct PreparedKnowledge {
    reference: String,
    epoch: u64,
    knowledge: Value,
}

#[derive(Clone)]
struct PreviewRecord {
    local_playback: bool,
    candidate_hash: String,
    status: String,
    message: String,
    sample_count: u64,
    frame_count: u32,
    artifact_ref: String,
    audio_sha256: String,
    report_sha256: String,
}
pub struct OperatorLane {
    inner: Arc<Inner>,
}
impl OperatorLane {
    pub fn new(
        shared: Arc<Mutex<Composition>>,
        loaded: Loaded,
        stop: Arc<AtomicBool>,
        artifacts: PathBuf,
    ) -> std::result::Result<Self, String> {
        let Loaded { config, base_dir } = loaded;
        let c = shared.lock().map_err(|_| "composition poisoned")?;
        let store = c.store().clone();
        let workspace = c.workspace().to_owned();
        let session = c.session_id().to_owned();
        let actor = c.actor().to_owned();
        let boot = c.boot_id().to_owned();
        // The persistent session must exist before candidates and budget reservations can reference it.
        let operation =
            Operation::new(nai_storage::BRINGUP_OPERATION).map_err(|_| "operation invalid")?;
        {
            let mut s = store.lock().map_err(|_| "store poisoned")?;
            match s.session(&operation, &workspace, &actor, &session) {
                Ok(saved) => {
                    c.emergency().mute();
                    let mut recovered = nai_kernel::session::Session::recover(saved.clone())
                        .map_err(|_| "session recovery exhausted")?;
                    recovered
                        .revoke(true)
                        .map_err(|_| "session recovery exhausted")?;
                    s.checkpoint_revocation(&operation, &actor, &saved, recovered.snapshot())
                        .map_err(|e| e.to_string())?;
                    if !saved.knowledge_snapshot_ref.is_empty()
                        && saved.knowledge_snapshot_ref != text(&config["knowledge"], "snapshot")
                    {
                        s.rebind_revoked_session_knowledge(
                            &operation,
                            &workspace,
                            &actor,
                            &session,
                            recovered.snapshot().generation_epoch,
                            (
                                text(&config["knowledge"], "scope"),
                                text(&config["knowledge"], "snapshot"),
                            ),
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    *c.session().lock().map_err(|_| "session poisoned")? = recovered;
                }
                Err(e) if e.code == ErrorCode::NotFound => {
                    s.create_session(&operation, &workspace, &actor, &session)
                        .map_err(|e| e.to_string())?;
                }
                Err(e) => return Err(e.to_string()),
            }
            s.bind_session_knowledge(
                &operation,
                &workspace,
                &actor,
                &session,
                text(&config["knowledge"], "scope"),
                text(&config["knowledge"], "snapshot"),
            )
            .map_err(|e| e.to_string())?;
        }
        if config.get("provider_routes").is_none() {
            build_provider(&config)?;
        }
        let llm_routes = crate::provider_routes::Routes::build(&config, "llm")?;
        let tts_routes = crate::provider_routes::Routes::build(&config, "tts")?;
        let mut setup = config
            .get("render")
            .filter(|value| !value.is_null())
            .cloned()
            .map(serde_json::from_value::<RenderSetup>)
            .transpose()
            .map_err(|_| "operator render closure invalid")?;
        let render = match setup.as_mut() {
            Some(render) => {
                resolve_render_setup(render, &base_dir)?;
                render_lane(Some(render))?
            }
            None => None,
        };
        fs::create_dir_all(&artifacts).map_err(|_| "preview artifact directory unavailable")?;
        let emergency = c.emergency();
        let knowledge = (
            text(&config["knowledge"], "scope").to_owned(),
            text(&config["knowledge"], "snapshot").to_owned(),
        );
        drop(c);
        Ok(Self {
            inner: Arc::new(Inner {
                program: Mutex::new(None),
                knowledge: Mutex::new(knowledge),
                knowledge_paused: AtomicBool::new(false),
                knowledge_capacity: Arc::new(Semaphore::new(1)),
                prepared: Mutex::new(None),
                emergency,
                shared,
                store,
                workspace,
                actor,
                session,
                boot,
                config,
                llm_routes,
                tts_routes,
                capacity: Arc::new(Semaphore::new(1)),
                history: Mutex::new(VecDeque::new()),
                render,
                artifacts,
                previews: Mutex::new(VecDeque::new()),
                preview_capacity: Arc::new(Semaphore::new(1)),
                resident_output: Mutex::new(None),
                stop,
            }),
        })
    }
}
impl ManualEventSurface for OperatorLane {
    fn program(&self, request: nai_control_api::ProgramRequest) -> Result<()> {
        self.program_command(request)
    }
    fn program_view(&self) -> Result<Value> {
        self.read_program()
    }
    fn prewarm(&self, request: nai_control_api::OutputPrewarmRequest) -> Result<()> {
        self.inner.check_epoch(request.epoch, request.deadline)?;
        if self.inner.render.is_none() {
            return Err(ErrorCode::CapabilityNotReady);
        }
        let binary = crate::local_playback::keeper()?;
        let permit = self
            .inner
            .preview_capacity
            .clone()
            .try_acquire_owned()
            .map_err(|_| ErrorCode::ResourceExhausted)?;
        {
            let mut history = self
                .inner
                .history
                .try_lock()
                .map_err(|_| ErrorCode::Conflict)?;
            if history.len() == 200 {
                history.pop_front();
            }
            history.push_back(ManualEventView {
                id: request.command_id.clone(),
                status: "output_preparing".into(),
                message: "正在准备合法中性画面与静音设备；未调用LLM/TTS。".into(),
                candidate_hash: String::new(),
            });
        }
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let result = (|| {
                let deadline = request
                    .deadline
                    .min(Instant::now() + Duration::from_secs(8));
                inner.check_epoch(request.epoch, deadline)?;
                let neutral = inner
                    .render
                    .as_ref()
                    .ok_or(ErrorCode::CapabilityNotReady)?
                    .standby(&inner.session, request.epoch)
                    .map_err(|_| ErrorCode::CapabilityNotReady)?;
                if neutral.rgba.len() > 16 * 1024 * 1024 {
                    return Err(ErrorCode::ResourceExhausted);
                }
                let hash = hex_digest(&neutral.rgba);
                let dir = tempfile::Builder::new()
                    .prefix("output-prewarm-")
                    .tempdir_in(&inner.artifacts)
                    .map_err(|_| ErrorCode::Internal)?;
                fs::write(dir.path().join("standby.rgba"), &neutral.rgba)
                    .map_err(|_| ErrorCode::Internal)?;
                let descriptor = nai_contracts::v1::LocalStandbyMedia {
                    version: 1,
                    session_id: inner.session.clone(),
                    generation_epoch: request.epoch,
                    width: neutral.width,
                    height: neutral.height,
                    standby_sha256: hash.clone(),
                }
                .encode_to_vec();
                let path = dir.path().join("standby.pb");
                fs::write(&path, &descriptor).map_err(|_| ErrorCode::Internal)?;
                crate::local_playback::prewarm(
                    &inner.resident_output,
                    &binary,
                    nai_contracts::v1::LocalOutputWarmup {
                        descriptor_path: path.to_str().ok_or(ErrorCode::InvalidArgument)?.into(),
                        descriptor_sha256: hex_digest(&descriptor),
                        session_id: inner.session.clone(),
                        generation_epoch: request.epoch,
                        remaining_budget_ms: 4000,
                    },
                    &hash,
                    deadline,
                    text(&inner.config["provider"], "credential_env"),
                    || inner.check_epoch(request.epoch, deadline),
                )?;
                inner.check_epoch(request.epoch, deadline)?;
                Ok::<_, ErrorCode>(hash)
            })();
            if result.is_err()
                && let Ok(mut output) = inner.resident_output.try_lock()
            {
                output.take();
            }
            if let Ok(mut history) = inner.history.lock()
                && let Some(item) = history.iter_mut().find(|v| v.id == request.command_id)
            {
                match result {
                    Ok(hash) => {
                        item.status = "output_ready".into();
                        item.candidate_hash = hash;
                        item.message="合法闭嘴画面已待机，单条音频stream已打开且提交语音样本为0；未调用LLM/TTS，未授予语音或平台Ready，playback_ack=false。".into();
                    }
                    Err(code) => {
                        item.status = "failed".into();
                        item.message = format!(
                            "{}：输出预热未确认；未调用LLM/TTS，未自动重试。",
                            code.as_str_name()
                        );
                    }
                }
            }
            drop(permit);
        });
        Ok(())
    }
    fn knowledge(&self, request: KnowledgePublishRequest) -> Result<()> {
        if self.inner.stop.load(Ordering::Acquire) {
            return Err(ErrorCode::Cancelled);
        }
        let permit = self
            .inner
            .knowledge_capacity
            .clone()
            .try_acquire_owned()
            .map_err(|_| ErrorCode::ResourceExhausted)?;
        {
            let mut h = self
                .inner
                .history
                .try_lock()
                .map_err(|_| ErrorCode::Conflict)?;
            if h.len() == 200 {
                h.pop_front();
            }
            h.push_back(ManualEventView {
                id: request.command_id.clone(),
                status: "knowledge_preparing".into(),
                message: "知识更新已接收；等待完成确认。".into(),
                candidate_hash: String::new(),
            });
        }
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let result = inner.change_knowledge(&request);
            drop(permit);
            if let Ok(mut h) = inner.history.lock()
                && let Some(item) = h.iter_mut().find(|v| v.id == request.command_id)
            {
                match result {
                    Ok(snapshot) => {
                        item.status = if request.prepare {
                            "knowledge_prepared"
                        } else {
                            "knowledge_activated"
                        }
                        .into();
                        item.message = if request.prepare {
                            "知识已准备，数字人暂停；等待配置保存与显式激活。"
                        } else {
                            "新知识已生效；旧审批已撤销，需重新生成和审批。"
                        }
                        .into();
                        item.candidate_hash = snapshot;
                    }
                    Err(code) => {
                        item.status = "failed".into();
                        item.message = format!(
                            "{}：知识切换未完成；若已暂停，请重新发布或重启引擎，不会自动恢复旧播报。",
                            code.as_str_name()
                        );
                    }
                }
            }
        });
        Ok(())
    }
    fn submit(&self, request: ManualEventRequest) -> Result<()> {
        self.ensure_manual_allowed()?;
        self.enqueue_generation(request, false)
    }
    fn events(&self) -> Result<Vec<ManualEventView>> {
        let h = self
            .inner
            .history
            .try_lock()
            .map_err(|_| ErrorCode::Conflict)?;
        Ok(h.iter()
            .rev()
            .take(100)
            .map(|v| ManualEventView {
                id: v.id.clone(),
                status: v.status.clone(),
                message: v.message.clone(),
                candidate_hash: v.candidate_hash.clone(),
            })
            .collect())
    }
}
impl OperatorLane {
    fn enqueue_generation(&self, request: ManualEventRequest, timed_evidence: bool) -> Result<()> {
        if self.inner.knowledge_paused.load(Ordering::Acquire) {
            return Err(ErrorCode::CapabilityNotReady);
        }
        if self.inner.stop.load(Ordering::Acquire) {
            return Err(ErrorCode::Cancelled);
        }
        let permit = self
            .inner
            .capacity
            .clone()
            .try_acquire_owned()
            .map_err(|_| ErrorCode::ResourceExhausted)?;
        {
            let mut h = self
                .inner
                .history
                .try_lock()
                .map_err(|_| ErrorCode::Conflict)?;
            if h.len() == 200 {
                h.pop_front();
            }
            h.push_back(ManualEventView {
                id: request.command_id.clone(),
                status: "generating".into(),
                message: "已接收，正在检索与生成；尚未批准或播放。".into(),
                candidate_hash: String::new(),
            });
        }
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let result = inner.generate(&request, timed_evidence).await;
            if timed_evidence {
                program::generation_finished(&inner, &request, &result);
            }
            if let Ok(mut h) = inner.history.lock()
                && let Some(item) = h.iter_mut().find(|v| v.id == request.command_id)
            {
                match result {
                    Ok(hash) => {
                        item.status = "awaiting_approval".into();
                        item.message = "最终文本已进入待审批列表；尚未播放。".into();
                        item.candidate_hash = hash;
                    }
                    Err(code) => {
                        item.status = "failed".into();
                        item.message = format!("{}：{}", code.code.as_str_name(), code.detail);
                    }
                }
            }
            drop(permit);
        });
        Ok(())
    }
}
impl PreviewSurface for OperatorLane {
    fn submit(&self, request: PreviewRequest) -> Result<()> {
        if request.local_playback {
            crate::local_playback::keeper()?;
        }
        if request.candidate_hash.len() != 64
            || !request
                .candidate_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(ErrorCode::InvalidArgument);
        }
        if self.inner.render.is_none() {
            return Err(ErrorCode::CapabilityNotReady);
        }
        {
            let store = self
                .inner
                .store
                .try_lock()
                .map_err(|_| ErrorCode::Conflict)?;
            let row = store
                .read_approval(
                    &op()?,
                    &self.inner.workspace,
                    &self.inner.actor,
                    &self.inner.session,
                    request.epoch,
                    &request.candidate_hash,
                )
                .map_err(|error| error.code)?;
            let approved = store
                .approval_valid(
                    &op()?,
                    &self.inner.workspace,
                    &self.inner.session,
                    request.epoch,
                    &request.candidate_hash,
                )
                .map_err(|error| error.code)?;
            if row.approval_id.is_none() || !approved {
                return Err(ErrorCode::PermissionDenied);
            }
        }
        let permit = self
            .inner
            .preview_capacity
            .clone()
            .try_acquire_owned()
            .map_err(|_| ErrorCode::ResourceExhausted)?;
        {
            let mut previews = self
                .inner
                .previews
                .try_lock()
                .map_err(|_| ErrorCode::Conflict)?;
            if let Some(existing) = previews.iter().find(|record| {
                record.candidate_hash == request.candidate_hash
                    && record.local_playback == request.local_playback
            }) {
                // A retried command is not a second paid synthesis. A failed
                // preview is not silently retried either: the operator learns
                // the failure and decides, instead of the engine spending again.
                return if existing.status == "failed" {
                    Err(ErrorCode::Conflict)
                } else {
                    Ok(())
                };
            }
            if previews.len() == 100 {
                previews.pop_front();
            }
            previews.push_back(PreviewRecord {
                local_playback: request.local_playback,
                candidate_hash: request.candidate_hash.clone(),
                status: "rendering".into(),
                message: "正在合成语音并渲染；尚未播放。".into(),
                sample_count: 0,
                frame_count: 0,
                artifact_ref: String::new(),
                audio_sha256: String::new(),
                report_sha256: String::new(),
            });
        }
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let result = inner.render_preview(&request).await;
            if let Ok(mut previews) = inner.previews.lock()
                && let Some(item) = previews.iter_mut().find(|record| {
                    record.candidate_hash == request.candidate_hash
                        && record.local_playback == request.local_playback
                })
            {
                match result {
                    Ok((samples, frames, artifact, audio_sha256, report_sha256)) => {
                        item.status = if request.local_playback {
                            "submitted"
                        } else {
                            "rendered"
                        }
                        .into();
                        item.message = if request.local_playback {
                            "本机音频已到达设备播放尾部；音画完成仍需人工确认。"
                        } else {
                            "本机预览已写出；未播放，playback_ack=false。"
                        }
                        .into();
                        item.sample_count = samples;
                        item.frame_count = frames;
                        item.artifact_ref = artifact;
                        item.audio_sha256 = audio_sha256;
                        item.report_sha256 = report_sha256;
                    }
                    Err(code) => {
                        item.status = "failed".into();
                        item.message = code.as_str_name().into();
                    }
                }
            }
            drop(permit);
        });
        Ok(())
    }

    fn previews(&self) -> Result<Vec<PreviewView>> {
        let previews = self
            .inner
            .previews
            .try_lock()
            .map_err(|_| ErrorCode::Conflict)?;
        Ok(previews
            .iter()
            .rev()
            .take(100)
            .map(|record| PreviewView {
                local_playback: record.local_playback,
                candidate_hash: record.candidate_hash.clone(),
                status: record.status.clone(),
                message: record.message.clone(),
                sample_count: record.sample_count,
                frame_count: record.frame_count,
                artifact_ref: record.artifact_ref.clone(),
                audio_sha256: record.audio_sha256.clone(),
            })
            .collect())
    }
}

impl Inner {
    fn change_knowledge(&self, request: &KnowledgePublishRequest) -> Result<String> {
        let deadline = Instant::now() + Duration::from_secs(25);
        if request.prepare {
            let name = &request.resource_ref;
            if !name.starts_with("product-publish-")
                || name.len() > 64
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            {
                return Err(ErrorCode::InvalidArgument);
            }
            let root = self.artifacts.parent().ok_or(ErrorCode::Internal)?;
            let dir = root.join(name);
            let path = dir.join("provider-scene.json");
            if !fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_dir())
                || !fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_file())
            {
                return Err(ErrorCode::PermissionDenied);
            }
            let loaded = load(&path).map_err(|_| ErrorCode::InvalidArgument)?;
            let snapshot = text(&loaded.config["knowledge"], "snapshot").to_owned();
            if self
                .knowledge
                .try_lock()
                .map_err(|_| ErrorCode::Conflict)?
                .1
                == snapshot
                || loaded.config["knowledge"]["documents"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| !text(d, "id").starts_with(&snapshot))
            {
                return Err(ErrorCode::InvalidArgument);
            }
            self.emergency.mute();
            self.knowledge_paused.store(true, Ordering::Release);
            let mut c = self.shared.try_lock().map_err(|_| ErrorCode::Conflict)?;
            let session = c.session();
            {
                let s = session.try_lock().map_err(|_| ErrorCode::Conflict)?;
                if s.snapshot().generation_epoch != request.epoch
                    || s.snapshot().state_revision != request.revision
                {
                    return Err(ErrorCode::StaleEpoch);
                }
            }
            let epoch = c
                .control(nai_runtime::ControlAction::Mute)
                .map_err(|e| e.code)?;
            let revoked = session
                .try_lock()
                .map_err(|_| ErrorCode::Conflict)?
                .snapshot()
                .clone();
            let operation = Operation::new(Duration::from_secs(20)).map_err(|e| e.code)?;
            let mut store = self.store.try_lock().map_err(|_| ErrorCode::Conflict)?;
            let previous = store
                .session(&operation, &self.workspace, &self.actor, &self.session)
                .map_err(|e| e.code)?;
            store
                .checkpoint_revocation(&operation, &self.actor, &previous, &revoked)
                .map_err(|e| e.code)?;
            publish_knowledge_until(
                &mut store,
                &self.workspace,
                &self.actor,
                &loaded.config,
                deadline,
            )
            .map_err(|_| ErrorCode::CapabilityNotReady)?;
            if Instant::now() >= deadline || self.stop.load(Ordering::Acquire) {
                return Err(ErrorCode::DeadlineExceeded);
            }
            *self.prepared.lock().map_err(|_| ErrorCode::Internal)? = Some(PreparedKnowledge {
                reference: name.clone(),
                epoch,
                knowledge: loaded.config["knowledge"].clone(),
            });
            Ok(snapshot)
        } else {
            let prepared = self
                .prepared
                .try_lock()
                .map_err(|_| ErrorCode::Conflict)?
                .clone()
                .ok_or(ErrorCode::CapabilityNotReady)?;
            if prepared.reference != request.resource_ref || prepared.epoch != request.epoch {
                return Err(ErrorCode::StaleEpoch);
            }
            let root = self.artifacts.parent().ok_or(ErrorCode::Internal)?;
            let export = root.join("operator-scenes/provider-scene.json");
            if !fs::symlink_metadata(root.join("operator-scenes"))
                .is_ok_and(|m| m.file_type().is_dir())
                || !fs::symlink_metadata(&export).is_ok_and(|m| m.file_type().is_file())
            {
                return Err(ErrorCode::PermissionDenied);
            }
            let persisted = load(&export).map_err(|_| ErrorCode::CapabilityNotReady)?;
            if persisted.config["knowledge"] != prepared.knowledge {
                return Err(ErrorCode::Conflict);
            }
            let scope = text(&prepared.knowledge, "scope").to_owned();
            let snapshot = text(&prepared.knowledge, "snapshot").to_owned();
            let c = self.shared.try_lock().map_err(|_| ErrorCode::Conflict)?;
            let session = c.session();
            let s = session.try_lock().map_err(|_| ErrorCode::Conflict)?;
            if s.snapshot().generation_epoch != prepared.epoch
                || s.snapshot().state_revision != request.revision
            {
                return Err(ErrorCode::StaleEpoch);
            }
            self.store
                .try_lock()
                .map_err(|_| ErrorCode::Conflict)?
                .rebind_revoked_session_knowledge(
                    &op()?,
                    &self.workspace,
                    &self.actor,
                    &self.session,
                    prepared.epoch,
                    (&scope, &snapshot),
                )
                .map_err(|e| e.code)?;
            *self.knowledge.lock().map_err(|_| ErrorCode::Internal)? = (scope, snapshot.clone());
            *self.prepared.lock().map_err(|_| ErrorCode::Internal)? = None;
            self.knowledge_paused.store(false, Ordering::Release);
            Ok(snapshot)
        }
    }
    fn check(&self, request: &ManualEventRequest) -> Result<()> {
        self.check_epoch(request.epoch, request.deadline)
    }

    fn check_epoch(&self, epoch: u64, deadline: Instant) -> Result<()> {
        if self.knowledge_paused.load(Ordering::Acquire) {
            return Err(ErrorCode::CapabilityNotReady);
        }
        if self.stop.load(Ordering::Acquire) {
            return Err(ErrorCode::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(ErrorCode::DeadlineExceeded);
        }
        let c = self.shared.try_lock().map_err(|_| ErrorCode::Conflict)?;
        let session = c.session();
        let s = session.try_lock().map_err(|_| ErrorCode::Conflict)?;
        if s.snapshot().generation_epoch != epoch {
            return Err(ErrorCode::StaleEpoch);
        }
        Ok(())
    }
    fn limits(&self, profile: &ProviderProfile) -> Limits {
        let b = &self.config["budget"];
        let tokens = number(b, "token_limit");
        let ceiling = Ceiling {
            cost_minor: number(b, "cost_limit_minor"),
            units: [
                ("requests".into(), number(b, "request_limit")),
                ("tokens".into(), tokens),
                ("prompt_tokens".into(), tokens),
                ("completion_tokens".into(), tokens),
                ("total_tokens".into(), tokens),
                ("characters".into(), number(b, "character_limit")),
            ]
            .into(),
            concurrent: [("provider_slots".into(), 1)].into(),
        };
        Limits {
            currency: profile.currency.clone(),
            global: ceiling.clone(),
            sessions: [(self.session.clone(), ceiling.clone())].into(),
            providers: self
                .llm_routes
                .endpoints
                .iter()
                .chain(self.tts_routes.endpoints.iter())
                .map(|e| (e.profile.provider_ref.clone(), ceiling.clone()))
                .collect(),
            max_entries: 10000,
        }
    }
    async fn render_preview(
        self: &Arc<Self>,
        request: &PreviewRequest,
    ) -> Result<(u64, u32, String, String, String)> {
        self.check_epoch(request.epoch, request.deadline)?;
        let render = self.render.as_ref().ok_or(ErrorCode::CapabilityNotReady)?;
        let standby = if request.local_playback {
            Some(
                render
                    .standby(&self.session, request.epoch)
                    .map_err(|_| ErrorCode::CapabilityNotReady)?,
            )
        } else {
            None
        };
        let candidate = request.candidate_hash.as_str();
        let (cue, approval_id) = {
            let store = self.store.try_lock().map_err(|_| ErrorCode::Conflict)?;
            let row = store
                .read_approval(
                    &op()?,
                    &self.workspace,
                    &self.actor,
                    &self.session,
                    request.epoch,
                    candidate,
                )
                .map_err(|error| error.code)?;
            let approved = store
                .approval_valid(
                    &op()?,
                    &self.workspace,
                    &self.session,
                    request.epoch,
                    candidate,
                )
                .map_err(|error| error.code)?;
            if row.approval_id.is_none() || !approved {
                return Err(ErrorCode::PermissionDenied);
            }
            let cue = store
                .read_candidate_cue(
                    &op()?,
                    &self.workspace,
                    &self.actor,
                    &self.session,
                    request.epoch,
                    candidate,
                )
                .map_err(|error| error.code)?;
            (cue, row.approval_id.unwrap_or_default())
        };
        let bounded_request = PreviewRequest {
            local_playback: request.local_playback,
            command_id: request.command_id.clone(),
            candidate_hash: request.candidate_hash.clone(),
            epoch: request.epoch,
            deadline: deadline_for_expiry(request.deadline, cue.expires_at_utc_ms)?,
        };
        let request = &bounded_request;
        let approved_text = cue.speech_text.clone();
        if approved_text.trim().is_empty() {
            return Err(ErrorCode::CapabilityNotReady);
        }
        let selected = self.tts_routes.select()?;
        let endpoint = selected.endpoint();
        let profile = &endpoint.profile;
        let sample_rate = render.sample_rate;
        let utterance_id = cue.utterance_id.clone();
        let request_id = format!(
            "{}-{}-{}",
            self.boot,
            if request.local_playback {
                "local-playback"
            } else {
                "preview"
            },
            &candidate[..16]
        );
        let tts_request = TtsRequest {
            context: Some(RequestContext {
                request_id: request_id.clone(),
                workspace_id: self.workspace.clone(),
                session_id: self.session.clone(),
                generation_epoch: Some(request.epoch),
                ..Default::default()
            }),
            utterance_id: utterance_id.clone(),
            speech_text: approved_text.clone(),
            display_text_ref: candidate.into(),
            voice: Some(VoiceRef {
                provider_ref: profile.provider_ref.clone(),
                voice_ref: endpoint.voice.clone(),
                revision: String::new(),
            }),
            format: Some(AudioFormat {
                codec: "pcm".into(),
                sample_rate,
                channels: 1,
                sample_format: "s16".into(),
            }),
            expires_at_utc_ms: cue.expires_at_utc_ms,
            approved_segment_hash: candidate.into(),
            ..Default::default()
        };
        let characters = approved_text.chars().count() as u64;
        let entry = UsageLedgerEntry {
            workspace_id: self.workspace.clone(),
            session_id: self.session.clone(),
            provider_ref: profile.provider_ref.clone(),
            request_id: request_id.clone(),
            generation_epoch: request.epoch,
            state: BillingState::Reserved as i32,
            estimate: Some(BudgetCharge {
                currency: profile.currency.clone(),
                cost_minor: number(&self.config["budget"], "request_estimate_minor"),
                price_revision: profile.price_revision.clone(),
                units: [
                    ("requests".into(), 1),
                    ("characters".into(), characters.max(1)),
                ]
                .into(),
            }),
            ..Default::default()
        };
        let remaining = request
            .deadline
            .saturating_duration_since(Instant::now())
            .min(profile.timeout);
        let operation = Operation::new(remaining).map_err(|error| error.code)?;
        let mut gateway = DurableGateway {
            store: self.store.clone(),
            workspace: self.workspace.clone(),
            actor: self.actor.clone(),
            limits: self.limits(profile),
            dispatched: false,
        };
        let client = profile
            .speech(
                endpoint
                    .client
                    .as_ref()
                    .ok_or(ErrorCode::CapabilityNotReady)?,
            )
            .map_err(|error| error.code)?;
        let result = {
            let call = client.tts_pcm(&operation, &tts_request, &mut gateway, entry);
            tokio::pin!(call);
            loop {
                tokio::select! {
                    result = &mut call => break result.map_err(|error| error.code),
                    _ = tokio::time::sleep(Duration::from_millis(20)) => {
                        if now()>=cue.expires_at_utc_ms {operation.cancel();break Err(ErrorCode::EvidenceExpired);}
                        match self.check_epoch(request.epoch, request.deadline) {
                            Ok(()) | Err(ErrorCode::Conflict) => {}
                            Err(code) => {
                                operation.cancel();
                                break Err(code);
                            }
                        }
                    }
                }
            }
        };
        let result = result.map_err(|code| {
            if matches!(code, ErrorCode::DeadlineExceeded | ErrorCode::Cancelled)
                && now() >= cue.expires_at_utc_ms
            {
                ErrorCode::EvidenceExpired
            } else {
                code
            }
        });
        if gateway.dispatched {
            selected.finish(result.as_ref().map(|_| ()).map_err(|code| *code));
        }
        let (bytes, format) = match result {
            Ok(value) => value,
            Err(code) => {
                gateway.abandon(&profile.provider_ref, &request_id)?;
                return Err(code);
            }
        };
        if format.codec != "pcm"
            || format.sample_format != "s16"
            || format.channels != 1
            || !(8_000..=48_000).contains(&format.sample_rate)
        {
            gateway.abandon(&profile.provider_ref, &request_id)?;
            return Err(ErrorCode::InvalidArgument);
        }
        // Provider metadata describes the received bytes, not our requested rate.
        // Convert in the application layer before rendering, hashing or WAV output.
        decode_s16(&bytes, format.sample_rate)?;
        let source: Vec<i16> = bytes
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        let normalized = nai_media_core::timeline::resample::resample_linear_i16(
            &source,
            format.sample_rate,
            sample_rate,
            nai_provider_core::MAX_SPEECH_SAMPLES,
        )
        .map_err(|_| ErrorCode::ResourceExhausted)?;
        let bytes: Vec<u8> = normalized
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        let pcm = decode_s16(&bytes, sample_rate)?;
        if request.local_playback && pcm.len() > sample_rate as usize * 20 {
            return Err(ErrorCode::ResourceExhausted);
        }
        let playback_dir = if request.local_playback {
            Some(
                tempfile::Builder::new()
                    .prefix("local-playback-")
                    .tempdir_in(&self.artifacts)
                    .map_err(|_| ErrorCode::Internal)?,
            )
        } else {
            None
        };
        let mut frame_file = playback_dir
            .as_ref()
            .map(|dir| fs::File::create(dir.path().join("frames.rgba")))
            .transpose()
            .map_err(|_| ErrorCode::Internal)?;
        use sha2::Digest;
        let mut frame_bytes = 0usize;
        let mut frame_digest = sha2::Sha256::new();
        let rendered = render
            .render_audio_to(
                &self.session,
                &cue,
                &pcm,
                request.epoch,
                now(),
                &format!("operator-preview:{approval_id}"),
                &mut |rgba| {
                    match self.check_epoch(request.epoch, request.deadline) {
                        Ok(()) | Err(ErrorCode::Conflict) => {}
                        Err(_) => return Err("render cancelled or epoch revoked".into()),
                    }
                    if let Some(file) = frame_file.as_mut() {
                        use std::io::Write;
                        frame_bytes = frame_bytes
                            .checked_add(rgba.len())
                            .ok_or("frame output overflow")?;
                        if frame_bytes > 512 * 1024 * 1024 || rgba.len() > 16 * 1024 * 1024 {
                            return Err("local frame output exceeds disk/frame quota".into());
                        }
                        frame_digest.update(rgba);
                        file.write_all(rgba)
                            .map_err(|_| "local frame write failed".to_string())?;
                    }
                    Ok(())
                },
            )
            .map_err(|error| {
                eprintln!("nai-engine preview render failed: {error}");
                ErrorCode::Internal
            })?;
        if !rendered.lane.committed() {
            return Err(ErrorCode::Conflict);
        }
        self.check_epoch(request.epoch, request.deadline)?;
        let audio_sha256 = hex_digest(&bytes);
        if let Some(dir) = playback_dir {
            drop(frame_file);
            fs::write(dir.path().join("audio.pcm"), &bytes).map_err(|_| ErrorCode::Internal)?;
            let standby = standby.ok_or(ErrorCode::CapabilityNotReady)?;
            if standby.width != rendered.width || standby.height != rendered.height {
                return Err(ErrorCode::InvalidArgument);
            }
            fs::write(dir.path().join("standby.rgba"), &standby.rgba)
                .map_err(|_| ErrorCode::Internal)?;
            let frames_sha256 = frame_digest
                .finalize()
                .iter()
                .map(|v| format!("{v:02x}"))
                .collect();
            let media = LocalPlaybackMedia {
                version: 2,
                session_id: self.session.clone(),
                generation_epoch: request.epoch,
                candidate_hash: candidate.into(),
                sample_rate,
                sample_count: pcm.len() as u64,
                width: rendered.width,
                height: rendered.height,
                frame_count: rendered.frames,
                fps_num: render.fps_num,
                fps_den: render.fps_den,
                audio_sha256: audio_sha256.clone(),
                frames_sha256,
                frame_sha256: rendered.frame_hashes.clone(),
                standby_sha256: hex_digest(&standby.rgba),
            };
            use prost::Message;
            let encoded = media.encode_to_vec();
            fs::write(dir.path().join("media.pb"), &encoded).map_err(|_| ErrorCode::Internal)?;
            let expected_standby = media.standby_sha256.clone();
            let media_hash = hex_digest(&encoded);
            let inner = self.clone();
            let candidate = candidate.to_owned();
            let epoch = request.epoch;
            let deadline = request
                .deadline
                .min(Instant::now() + Duration::from_secs(26));
            let expected_samples = (pcm.len() as u64 * 48000).div_ceil(u64::from(sample_rate));
            let status = tokio::task::spawn_blocking(move || {
                let binary = crate::local_playback::keeper()?;
                crate::local_playback::run(
                    &inner.resident_output,
                    &binary,
                    crate::local_playback::CueTarget {
                        descriptor: &dir.path().join("media.pb"),
                        media_sha256: &media_hash,
                        expected_samples,
                        expected_frames: rendered.frames,
                    },
                    deadline,
                    text(&inner.config["provider"], "credential_env"),
                    |seq| {
                        inner.check_epoch(epoch, deadline)?;
                        if seq == 0 {
                            return Ok(LocalPlaybackGrant::default());
                        }
                        inner.playback_grant(&candidate, epoch, seq, &media_hash)
                    },
                    |status| {
                        if let Ok(mut previews) = inner.previews.try_lock()
                            && let Some(item) = previews
                                .iter_mut()
                                .find(|p| p.candidate_hash == candidate && p.local_playback)
                        {
                            item.status = "playing".into();
                            item.message = "正在提交本机音画；可立即静音。".into();
                            item.sample_count = status.copied_samples.unwrap_or(0);
                        }
                    },
                )
            })
            .await
            .map_err(|_| ErrorCode::Internal)??;
            if status.copied_samples != Some(expected_samples)
                || status.device_played_samples != Some(expected_samples)
                || !status.device_position_observed
                || status.audio_backend != "wasapi-iaudioclock"
                || status.audio_stream_open_count != 1
                || status.standby_sha256 != expected_standby
                || !status.video_tail_submitted
                || status.video_frame_index != rendered.frames.checked_sub(1)
                || status.video_device_samples != Some(expected_samples)
            {
                return Err(ErrorCode::InvalidArgument);
            }
            self.check_epoch(request.epoch, request.deadline)?;
            let report_name = format!("{}-{}.playback.pb", request.candidate_hash, request.epoch);
            let report = status.encode_to_vec();
            let report_sha256 = hex_digest(&report);
            fs::write(self.artifacts.join(&report_name), report)
                .map_err(|_| ErrorCode::Internal)?;
            return Ok((
                expected_samples,
                rendered.frames,
                report_name,
                audio_sha256,
                report_sha256,
            ));
        }
        let wav_name = format!("{candidate}-{}.wav", request.epoch);
        write_wav(&self.artifacts, &wav_name, sample_rate, &bytes)?;
        let frames_name = format!("{candidate}-{}.frames.json", request.epoch);
        write_json(
            &self.artifacts,
            &frames_name,
            &json!({
                "candidate_hash": candidate,
                "generation_epoch": request.epoch.to_string(),
                "sample_rate": sample_rate,
                "sample_count": pcm.len(),
                "frame_count": rendered.frames,
                "width": rendered.width,
                "height": rendered.height,
                "audio_sha256": audio_sha256,
                "artifact": wav_name,
                "frame_sha256": rendered.frame_hashes,
                "scope": "local-preview",
                "playback_ack": false,
            }),
        )?;
        Ok((
            pcm.len() as u64,
            rendered.frames,
            wav_name,
            audio_sha256,
            String::new(),
        ))
    }

    fn playback_grant(
        &self,
        candidate: &str,
        epoch: u64,
        seq: u64,
        media_hash: &str,
    ) -> Result<LocalPlaybackGrant> {
        {
            let c = self.shared.try_lock().map_err(|_| ErrorCode::Conflict)?;
            let session = c.session();
            let session = session.try_lock().map_err(|_| ErrorCode::Conflict)?;
            if session.snapshot().mode != SessionMode::Assisted as i32 {
                return Err(ErrorCode::PermissionDenied);
            }
        }
        let store = self.store.try_lock().map_err(|_| ErrorCode::Conflict)?;
        let approval = store
            .read_approval_proof(
                &op()?,
                &self.workspace,
                &self.actor,
                &self.session,
                epoch,
                candidate,
            )
            .map_err(|e| e.code)?;
        let mut cue = store
            .read_candidate_cue(
                &op()?,
                &self.workspace,
                &self.actor,
                &self.session,
                epoch,
                candidate,
            )
            .map_err(|e| e.code)?;
        cue.approval_ref = approval.approval_id.clone();
        let policy = GuardPolicy::default();
        let facts: BTreeMap<_, _> = approval
            .fact_versions
            .iter()
            .map(|f| (f.object_id.clone(), f.revision.clone()))
            .collect();
        let verdict = guard(
            &GuardRequest {
                context: Some(RequestContext {
                    session_id: self.session.clone(),
                    generation_epoch: Some(epoch),
                    ..Default::default()
                }),
                stage: GuardStage::Preplay as i32,
                candidate: Some(cue.clone()),
                ..Default::default()
            },
            &policy,
            0,
            now(),
            Some(nai_kernel::evaluate::guard::PreplayChecks {
                approval_valid: true,
                facts_match: facts == cue.fact_versions
                    && facts.len() == approval.fact_versions.len(),
                epoch_match: cue.generation_epoch == epoch,
                permission_current: approval.action_scope == "speak"
                    && !approval.actor_id.is_empty(),
            }),
        );
        if cue.policy_version != policy.rule_version || verdict.verdict != GuardVerdict::Pass as i32
        {
            return Err(ErrorCode::PermissionDenied);
        }
        let lease = PlaybackLease {
            lease_id: format!("{}-local-{candidate}", self.boot),
            session_id: self.session.clone(),
            generation_epoch: epoch,
            allowed_utterance_ids: vec![cue.utterance_id.clone()],
            mode: SessionMode::Assisted as i32,
            approval_refs: vec![approval.approval_id.clone()],
            expires_at_utc_ms: (now() + 600)
                .min(approval.expires_at_utc_ms)
                .min(cue.expires_at_utc_ms),
            fact_versions: approval.fact_versions.clone(),
            candidate_hashes: [(cue.utterance_id.clone(), candidate.into())].into(),
            policy_revision: cue.policy_version.clone(),
        };
        Ok(LocalPlaybackGrant {
            version: 1,
            seq,
            cue: Some(cue),
            approval: Some(approval),
            lease: Some(lease),
            media_sha256: media_hash.into(),
        })
    }

    async fn generate(
        &self,
        request: &ManualEventRequest,
        timed_evidence: bool,
    ) -> std::result::Result<String, GenerationFailure> {
        self.check(request)?;
        let (knowledge_scope, knowledge_snapshot) = self
            .knowledge
            .try_lock()
            .map_err(|_| ErrorCode::Conflict)?
            .clone();
        let operation = op()?;
        let evidence = {
            let mut store = self.store.try_lock().map_err(|_| ErrorCode::Conflict)?;
            let (status, evidence) = store
                .retrieve_session_knowledge(
                    &operation,
                    &self.actor,
                    &self.workspace,
                    &self.session,
                    &[],
                    &nai_knowledge_core::tokenize(&request.text),
                    None,
                    5,
                )
                .map_err(|e| e.code)?;
            if status == RetrievalStatus::RetrievalEmpty
                || (status == RetrievalStatus::RetrievalComplete && evidence.is_empty())
            {
                return Err(GenerationFailure::at(
                    ErrorCode::CapabilityNotReady,
                    "未找到与问题匹配的已发布知识，未调用LLM。请补充相关知识或提出有知识依据的问题。",
                ));
            }
            if status != RetrievalStatus::RetrievalComplete {
                return Err(GenerationFailure::at(
                    ErrorCode::CapabilityNotReady,
                    "知识检索未就绪；请检查会话绑定的已发布知识快照。未调用LLM。",
                ));
            }
            if timed_evidence
                && evidence.iter().any(|e| {
                    e.valid_from_utc_ms.is_none_or(|from| from > now())
                        || e.valid_until_utc_ms.is_none_or(|until| until <= now())
                })
            {
                return Err(GenerationFailure::at(
                    ErrorCode::EvidenceExpired,
                    "讲品节目要求检索事实声明有效期；未知有效期不作为持续讲品依据。未调用LLM。",
                ));
            }
            let egress = store
                .remote_egress_targets(&operation, &self.workspace, &knowledge_scope, &self.actor)
                .map_err(|e| e.code)?;
            if evidence.iter().any(|e| {
                !egress
                    .releasable()
                    .iter()
                    .any(|d| d.document_id == e.document_id && d.revision == e.revision)
            }) {
                return Err(GenerationFailure::at(
                    ErrorCode::PermissionDenied,
                    "命中知识未获远端外发许可（如local_only）；未调用LLM。请按业务授权流程准备允许外发的知识。",
                ));
            }
            evidence
        };
        let fact_expiry = evidence.iter().filter_map(|v| v.valid_until_utc_ms).min();
        let expiry = (now()
            + request
                .deadline
                .saturating_duration_since(Instant::now())
                .as_millis() as u64)
            .min(fact_expiry.unwrap_or(u64::MAX));
        let bounded_request = ManualEventRequest {
            command_id: request.command_id.clone(),
            text: request.text.clone(),
            epoch: request.epoch,
            deadline: deadline_for_expiry(request.deadline, expiry)?,
        };
        let request = &bounded_request;
        let mut cue = PerformanceCue {
            session_id: self.session.clone(),
            utterance_id: format!("{}-{}", self.boot, request.command_id),
            generation_epoch: request.epoch,
            policy_version: GuardPolicy::default().rule_version.into(),
            display_text: request.text.clone(),
            speech_text: request.text.clone(),
            knowledge_snapshot_ref: knowledge_snapshot,
            evidence_refs: evidence.iter().map(|e| e.evidence_id.clone()).collect(),
            expires_at_utc_ms: expiry,
            ..Default::default()
        };
        let context = RequestContext {
            session_id: self.session.clone(),
            workspace_id: self.workspace.clone(),
            generation_epoch: Some(request.epoch),
            ..Default::default()
        };
        for stage in [GuardStage::Input, GuardStage::Evidence] {
            check_guard(stage, &context, &cue, &evidence)?;
        }
        let selected = self.llm_routes.select().map_err(|code| {
            GenerationFailure::at(code, "LLM路由未就绪；请检查保存的凭据与供应商健康状态。")
        })?;
        let endpoint = selected.endpoint();
        let profile = &endpoint.profile;
        let prompt = self.assemble(request, &evidence, profile)?;
        self.check(request)?;
        let max_tokens = endpoint.max_tokens;
        let llm_request = LlmRequest {
            context: Some(context.clone()),
            provider_profile: profile.provider_ref.clone(),
            prompt_version: text(&self.config["prompt"], "revision").into(),
            permitted_context: prompt,
            max_output_tokens: max_tokens,
            ..Default::default()
        };
        let request_id = format!("{}-{}", self.boot, request.command_id);
        let entry = UsageLedgerEntry {
            workspace_id: self.workspace.clone(),
            session_id: self.session.clone(),
            provider_ref: profile.provider_ref.clone(),
            request_id: request_id.clone(),
            generation_epoch: request.epoch,
            state: BillingState::Reserved as i32,
            estimate: Some(BudgetCharge {
                currency: profile.currency.clone(),
                cost_minor: number(&self.config["budget"], "request_estimate_minor"),
                price_revision: profile.price_revision.clone(),
                units: [
                    ("requests".into(), 1),
                    (
                        "tokens".into(),
                        llm_request.permitted_context.len() as u64 + u64::from(max_tokens),
                    ),
                ]
                .into(),
            }),
            ..Default::default()
        };
        let operation = Operation::new(
            request
                .deadline
                .saturating_duration_since(Instant::now())
                .min(profile.timeout),
        )
        .map_err(|e| e.code)?;
        let mut gateway = DurableGateway {
            store: self.store.clone(),
            workspace: self.workspace.clone(),
            actor: self.actor.clone(),
            limits: self.limits(profile),
            dispatched: false,
        };
        let client = profile
            .llm(
                endpoint
                    .client
                    .as_ref()
                    .ok_or(ErrorCode::CapabilityNotReady)?,
            )
            .map_err(|e| e.code)?;
        let result = {
            let call = client.generate(&operation, &llm_request, &mut gateway, entry);
            tokio::pin!(call);
            loop {
                tokio::select! {
                    result=&mut call=>break result.map_err(|e|e.code),
                    _=tokio::time::sleep(Duration::from_millis(20))=> {
                        if fact_expiry.is_some_and(|until|now()>=until){operation.cancel();break Err(ErrorCode::EvidenceExpired);}
                        match self.check(request) {
                            Ok(())|Err(ErrorCode::Conflict)=>{},
                            Err(code)=> {operation.cancel();break Err(code);},
                        }
                    }
                }
            }
        };
        let result = result.map_err(|code| {
            if matches!(code, ErrorCode::DeadlineExceeded | ErrorCode::Cancelled)
                && fact_expiry.is_some_and(|until| now() >= until)
            {
                ErrorCode::EvidenceExpired
            } else {
                code
            }
        });
        if gateway.dispatched {
            selected.finish(result.as_ref().map(|_| ()).map_err(|code| *code));
        }
        let chunks = match result {
            Ok(chunks) => chunks,
            Err(code) => {
                gateway.abandon(&profile.provider_ref, &request_id)?;
                if fact_expiry.is_some_and(|until| now() >= until) {
                    return Err(GenerationFailure::at(
                        ErrorCode::EvidenceExpired,
                        "知识事实有效期已结束，生成已取消且未产生候选；已派发请求可能计费，未自动重试。",
                    ));
                }
                return Err(GenerationFailure::at(
                    code,
                    "LLM调用未完成；请检查服务、额度和超时，未自动重试。",
                ));
            }
        };
        if fact_expiry.is_some_and(|until| now() >= until) {
            return Err(GenerationFailure::at(
                ErrorCode::EvidenceExpired,
                "知识事实在生成期间已过期，未产生候选；未自动重试。",
            ));
        }
        self.check(request)?;
        let output: String = chunks
            .iter()
            .filter(|c| c.end == StreamEnd::StreamData as i32)
            .map(|c| c.text_delta.as_str())
            .collect();
        if output.trim().is_empty() {
            return Err(GenerationFailure::at(
                ErrorCode::CapabilityNotReady,
                "LLM返回了空正文，未产生候选；请检查模型响应或输出预算，未自动重试。",
            ));
        }
        cue.display_text = output.clone();
        cue.speech_text = output;
        check_guard(GuardStage::Output, &context, &cue, &evidence)?;
        // Short commit section: an old epoch cannot win a race with control revocation.
        let c = self.shared.try_lock().map_err(|_| ErrorCode::Conflict)?;
        let shared_session = c.session();
        let session = shared_session.try_lock().map_err(|_| ErrorCode::Conflict)?;
        if session.snapshot().generation_epoch != request.epoch {
            return Err(ErrorCode::StaleEpoch.into());
        }
        if self.stop.load(Ordering::Acquire) || Instant::now() >= request.deadline {
            return Err(ErrorCode::Cancelled.into());
        }
        let mut store = self.store.try_lock().map_err(|_| ErrorCode::Conflict)?;
        let saved = store
            .session(&op()?, &self.workspace, &self.actor, &self.session)
            .map_err(|e| e.code)?;
        if saved.generation_epoch != session.snapshot().generation_epoch {
            store
                .checkpoint_revocation(&op()?, &self.actor, &saved, session.snapshot())
                .map_err(|e| e.code)?;
        }
        cue.candidate_hash = nai_contracts::candidate_hash(&cue);
        cue.hash_algorithm = "sha256-canonical-v1".into();
        store
            .record_candidate(&op()?, &self.workspace, &self.actor, &cue, &[])
            .map_err(|e| GenerationFailure::from(e.code))
    }

    fn assemble(
        &self,
        request: &ManualEventRequest,
        evidence: &[Evidence],
        profile: &ProviderProfile,
    ) -> Result<String> {
        let p = &self.config["prompt"];
        let mut bodies = vec![
            text(p, "policy").to_owned(),
            text(p, "persona").to_owned(),
            text(p, "output_contract").to_owned(),
            request.text.clone(),
        ];
        bodies.extend(evidence.iter().map(|e| e.text.clone()));
        let ids: Vec<String> = (0..bodies.len())
            .map(|i| format!("{}-fragment-{i}", request.command_id))
            .collect();
        let candidates: Vec<CandidateFragment<'_>> = bodies
            .iter()
            .enumerate()
            .map(|(i, body)| CandidateFragment {
                fragment: Fragment {
                    id: &ids[i],
                    source: match i {
                        0 => SourceKind::SystemPolicy,
                        1 | 2 => SourceKind::Scenario,
                        3 => SourceKind::Event,
                        _ => SourceKind::KnowledgeEvidence,
                    },
                    source_ref: if i >= 4 {
                        &evidence[i - 4].evidence_id
                    } else {
                        &ids[i]
                    },
                    scope: Some(&self.session),
                    age_ms: Some(0),
                    trust: if i < 3 {
                        Trust::Instruction
                    } else {
                        Trust::Data
                    },
                },
                tokens: body.len() as u64,
                chars: body.chars().count() as u64,
            })
            .collect();
        let cap = number(p, "segment_chars");
        let names = [
            Segment::SystemPolicy,
            Segment::Persona,
            Segment::OutputContract,
            Segment::EventSummary,
            Segment::Evidence,
        ];
        let segments: Vec<SegmentInput<'_>> = names
            .iter()
            .enumerate()
            .map(|(i, s)| SegmentInput {
                segment: s.label(),
                candidates: if i < 4 {
                    &candidates[i..i + 1]
                } else {
                    &candidates[4..]
                },
                budget: Some(SegmentBudget {
                    tokens: Some(cap * 4),
                    chars: Some(cap),
                    items: Some(5),
                    floor: s.uncroppable().then_some(1),
                }),
            })
            .collect();
        let input = AssemblyInput {
            model_version: &profile.model,
            prompt_version: text(p, "revision"),
            segments: &segments,
        };
        let plan = plan_assembly(&input).map_err(|_| ErrorCode::InvalidArgument)?;
        let kept: Vec<&Fragment<'_>> = plan
            .segments()
            .iter()
            .flat_map(|s| s.fragments.iter())
            .collect();
        // An event or evidence cropped to nothing cannot be turned into an ungrounded query.
        if !kept.iter().any(|f| f.source == SourceKind::Event)
            || !kept
                .iter()
                .any(|f| f.source == SourceKind::KnowledgeEvidence)
        {
            return Err(ErrorCode::ResourceExhausted);
        }
        let kept_ids: Vec<&str> = kept.iter().map(|f| f.id).collect();
        let logged: Vec<LoggedFragment<'_>> = kept
            .iter()
            .map(|f| LoggedFragment {
                id: f.id,
                source_ref: f.source_ref,
                content: Some(&bodies[ids.iter().position(|id| id == f.id).unwrap()]),
            })
            .collect();
        let record = AssemblyRecord {
            fragment_ids: &kept_ids,
            model_version: &profile.model,
            prompt_version: text(p, "revision"),
        };
        let mut c = self.shared.try_lock().map_err(|_| ErrorCode::Conflict)?;
        match plan
            .with_plan(|p| c.record_assembly(p, &record, &logged))
            .map_err(|e| e.code)?
        {
            AssemblyOutcome::Logged(ack) if ack.persisted => {}
            _ => return Err(ErrorCode::PermissionDenied),
        }
        let fields = BTreeMap::from([
            ("fragment_ref".into(), json!(kept_ids).to_string()),
            (
                "fragment_source".into(),
                json!(kept.iter().map(|f| f.source_ref).collect::<Vec<_>>()).to_string(),
            ),
            ("segment_budget".into(), cap.to_string()),
            (
                "cropped_items".into(),
                json!(
                    plan.segments()
                        .iter()
                        .flat_map(|s| s.dropped.iter())
                        .collect::<Vec<_>>()
                )
                .to_string(),
            ),
            (
                "crop_reason".into(),
                json!(
                    plan.segments()
                        .iter()
                        .filter_map(|s| s.crop_reason)
                        .collect::<Vec<_>>()
                )
                .to_string(),
            ),
            ("model_version".into(), profile.model.clone()),
            ("prompt_version".into(), text(p, "revision").into()),
        ]);
        match c
            .record_observation(
                &ObservationRecord {
                    layer: ObservationLayer::Assembly,
                    subject: request.command_id.clone(),
                    fields,
                    viewer_text: None,
                },
                &ObservationPolicy::default(),
            )
            .map_err(|e| e.code)?
        {
            ObservationOutcome::Logged(ack) if ack.persisted => {}
            _ => return Err(ErrorCode::PermissionDenied),
        }
        let prompt=json!(logged.iter().zip(kept.iter()).map(|(l,f)| json!({"source":f.source.label(),"trust":f.trust.label(),"text":l.content})).collect::<Vec<_>>()).to_string();
        if prompt.len() > 32 * 1024 {
            return Err(ErrorCode::ResourceExhausted);
        }
        Ok(prompt)
    }
}
fn check_guard(
    stage: GuardStage,
    context: &RequestContext,
    cue: &PerformanceCue,
    evidence: &[Evidence],
) -> Result<()> {
    let verdict = guard(
        &GuardRequest {
            context: Some(context.clone()),
            stage: stage as i32,
            candidate: Some(cue.clone()),
            evidence: evidence.to_vec(),
            permissions: None,
        },
        &GuardPolicy::default(),
        0,
        now(),
        None,
    );
    if verdict.verdict != GuardVerdict::Pass as i32 {
        return Err(ErrorCode::PermissionDenied);
    }
    Ok(())
}

fn decode_s16(bytes: &[u8], sample_rate: u32) -> Result<Vec<f32>> {
    if sample_rate == 0 || !bytes.len().is_multiple_of(2) {
        return Err(ErrorCode::InvalidArgument);
    }
    let samples = bytes.len() / 2;
    if samples == 0 || samples > nai_provider_core::MAX_SPEECH_SAMPLES {
        return Err(ErrorCode::ResourceExhausted);
    }
    Ok(bytes
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]) as f32 / 32768.0)
        .collect())
}

fn hex_digest(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn write_wav(dir: &Path, name: &str, sample_rate: u32, pcm: &[u8]) -> Result<()> {
    let data_len = u32::try_from(pcm.len()).map_err(|_| ErrorCode::ResourceExhausted)?;
    let mut header = Vec::with_capacity(44);
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&(36 + data_len).to_le_bytes());
    header.extend_from_slice(b"WAVEfmt ");
    header.extend_from_slice(&16u32.to_le_bytes());
    header.extend_from_slice(&1u16.to_le_bytes());
    header.extend_from_slice(&1u16.to_le_bytes());
    header.extend_from_slice(&sample_rate.to_le_bytes());
    header.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    header.extend_from_slice(&2u16.to_le_bytes());
    header.extend_from_slice(&16u16.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data_len.to_le_bytes());
    header.extend_from_slice(pcm);
    atomic_write(dir, name, &header)
}

fn write_json(dir: &Path, name: &str, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        eprintln!("nai-engine preview sidecar encode failed: {error}");
        ErrorCode::Internal
    })?;
    atomic_write(dir, name, &bytes)
}

fn atomic_write(dir: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let target = dir.join(name);
    let temporary = dir.join(format!(".{name}.tmp"));
    let mut file = fs::File::create(&temporary).map_err(|error| {
        eprintln!("nai-engine preview artifact create failed: {error}");
        ErrorCode::Internal
    })?;
    if file.write_all(bytes).is_err() || file.sync_all().is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(ErrorCode::Internal);
    }
    drop(file);
    if let Err(error) = fs::rename(&temporary, &target) {
        eprintln!("nai-engine preview artifact rename failed: {error}");
        let _ = fs::remove_file(&temporary);
        return Err(ErrorCode::Internal);
    }
    Ok(())
}

/// Production gateway: dispatch claim is durable BEFORE any provider I/O.
struct DurableGateway {
    store: Arc<Mutex<Store>>,
    workspace: String,
    actor: String,
    limits: Limits,
    dispatched: bool,
}
impl DurableGateway {
    fn abandon(&mut self, provider: &str, request: &str) -> Result<()> {
        let mut store = self.store.try_lock().map_err(|_| ErrorCode::Conflict)?;
        let operation = op()?;
        let book = store
            .budget_book(
                &operation,
                &self.workspace,
                &self.actor,
                self.limits.clone(),
            )
            .map_err(|e| e.code)?;
        if let Some(entry) = book.get(provider, request) {
            let change = match BillingState::try_from(entry.state) {
                Ok(BillingState::Reserved) => Some(Change::Cancel),
                Ok(BillingState::Dispatched) => Some(Change::Finish(None)),
                _ => None,
            };
            if let Some(change) = change {
                store
                    .update_usage(
                        &operation,
                        &self.workspace,
                        &self.actor,
                        self.limits.clone(),
                        provider,
                        request,
                        entry.revision,
                        change,
                    )
                    .map_err(|e| e.code)?;
            }
        }
        Ok(())
    }
}
fn provider_error(code: ErrorCode) -> ProviderError {
    ProviderError {
        code,
        retryable: false,
        message: "manual generation budget unavailable".into(),
    }
}
impl UsageGateway for DurableGateway {
    fn reserve(
        &mut self,
        entry: UsageLedgerEntry,
    ) -> std::result::Result<UsageLedgerEntry, ProviderError> {
        self.store
            .try_lock()
            .map_err(|_| provider_error(ErrorCode::Conflict))?
            .reserve_usage(
                &op().map_err(provider_error)?,
                &self.actor,
                self.limits.clone(),
                entry,
            )
            .map_err(|e| provider_error(e.code))
    }
    fn dispatch(
        &mut self,
        provider: &str,
        request: &str,
        revision: u64,
    ) -> std::result::Result<DispatchDecision, ProviderError> {
        let claim = self
            .store
            .try_lock()
            .map_err(|_| provider_error(ErrorCode::Conflict))?
            .dispatch_usage(
                &op().map_err(provider_error)?,
                &self.workspace,
                &self.actor,
                self.limits.clone(),
                provider,
                request,
                revision,
            )
            .map_err(|e| provider_error(e.code))?;
        self.dispatched = claim.acquired;
        Ok(if claim.acquired {
            DispatchDecision::Acquired(claim.entry)
        } else {
            DispatchDecision::Already(claim.entry)
        })
    }
    fn finish(
        &mut self,
        provider: &str,
        request: &str,
        revision: u64,
        actual: Option<BudgetCharge>,
    ) -> std::result::Result<UsageLedgerEntry, ProviderError> {
        self.store
            .try_lock()
            .map_err(|_| provider_error(ErrorCode::Conflict))?
            .update_usage(
                &op().map_err(provider_error)?,
                &self.workspace,
                &self.actor,
                self.limits.clone(),
                provider,
                request,
                revision,
                Change::Finish(actual),
            )
            .map_err(|e| provider_error(e.code))
    }
}
