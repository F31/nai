//! Explicit loopback engineering control surface. Not production IPC/TLS,
//! device authentication, preflight or hardware playback/mute acknowledgement.
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Request, Response, body::Incoming, service::service_fn};
use hyper_util::rt::TokioIo;
use nai_contracts::{control_http::SCHEMAS_JSON, v1::*};
use nai_kernel::evaluate::memory::{
    ForgetTrigger, MemoryKind, MemoryQuery, MemoryReadOutcome, MemoryRejection, MemoryScope,
    PurgeState,
};
use nai_kernel::session::{CommandLedger, Session, SessionError};
use nai_media_core::EmergencyHandle;
use nai_storage::{Operation, PendingApproval};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingApprovalView {
    pub candidate_hash: String,
    pub approval_id: Option<String>,
    pub expires_at_utc_ms: String,
    pub status: i32,
    pub evidence_count: u64,
    /// Display-only candidate text so the operator approves content, not a hash.
    /// The approval identity stays `candidate_hash`.
    pub text: String,
}
impl From<PendingApproval> for PendingApprovalView {
    fn from(item: PendingApproval) -> Self {
        Self {
            candidate_hash: item.candidate_hash,
            approval_id: item.approval_id,
            expires_at_utc_ms: item.expires_at_utc_ms.to_string(),
            status: item.status,
            evidence_count: item.evidence_count,
            text: item.display_text,
        }
    }
}
fn to_contract(error: nai_storage::StorageError) -> ContractError {
    ContractError {
        code: error.code as i32,
        category: "approval".into(),
        message: error.reason.to_string(),
        ..Default::default()
    }
}
use std::{
    collections::BTreeSet,
    convert::Infallible,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    net::TcpListener,
    sync::{Semaphore, watch},
    task::JoinSet,
};

type HttpResponse = Response<Full<Bytes>>;
const ORIGIN: &str = "http://127.0.0.1:5173";
/// §42.3 读取的**默认**预算与上限。⚠️ 缺省给 16 而不是「不设限」：配额裁剪是**可见**的
/// （被裁条目逐条具名），故默认不会把「被裁掉」伪装成「本来就没有」。
const MEMORY_BUDGET_DEFAULT: usize = 16;
const MEMORY_BUDGET_MAX: usize = 256;

/// Local approval workflow backend backed by the real single-owner store.
/// Absent backend, approval routes return CAPABILITY_NOT_READY instead of
/// fabricating success.
pub struct ApprovalBackend {
    store: Arc<Mutex<nai_storage::Store>>,
    workspace: String,
    actor: String,
}
impl ApprovalBackend {
    pub fn new(
        store: Arc<Mutex<nai_storage::Store>>,
        workspace: &str,
        actor: &str,
    ) -> Result<Self, String> {
        if workspace.is_empty() || actor.is_empty() {
            return Err("approval backend requires workspace and actor".into());
        }
        Ok(Self {
            store,
            workspace: workspace.into(),
            actor: actor.into(),
        })
    }
    fn list(
        &self,
        session: &str,
        epoch: u64,
        limit: u32,
    ) -> Result<Vec<PendingApprovalView>, ContractError> {
        let operation =
            Operation::new(Duration::from_secs(5)).map_err(|error| to_contract(error.into()))?;
        let store = self.store.try_lock().map_err(|_| ContractError {
            code: ErrorCode::Conflict as i32,
            category: "approval".into(),
            message: "approval store locked".into(),
            ..Default::default()
        })?;
        store
            .list_pending_approvals(
                &operation,
                &self.workspace,
                &self.actor,
                session,
                epoch,
                limit,
            )
            .map_err(to_contract)
            .map(|items| items.into_iter().map(Into::into).collect())
    }
    fn reject(
        &self,
        operation: &Operation,
        session: &str,
        epoch: u64,
        candidate: &str,
    ) -> Result<(), ContractError> {
        let mut store = self.store.try_lock().map_err(|_| ContractError {
            code: ErrorCode::Conflict as i32,
            category: "approval".into(),
            message: "approval store locked".into(),
            ..Default::default()
        })?;
        store
            .reject_candidate(
                operation,
                &self.workspace,
                &self.actor,
                session,
                epoch,
                candidate,
            )
            .map_err(to_contract)
    }
}

/// §42.3/§42.4 记忆面结果的三态：**谁的失败**必须分开。
///
/// `Rejected` 是**判定的结论**（§42.3 请求级拒绝、§42.4 陈述不实）；`Fault` 是
/// 路径自身的故障。把后者折进前者，就是把一个存储故障记在判定名下——
/// `write_memory` 明确拒绝这么做，读取面与遗忘面同理。
///
/// 故障载体复用契约的 [`ErrorCode`]，**不新造**一套故障分类。
#[derive(Debug)]
pub enum MemoryControlResult<T> {
    /// 判定放行：这次读取成立／这次遗忘的陈述如实。
    Ok(T),
    /// §42.3／§42.4 的拒绝，带判定自己的稳定名字。
    Rejected(MemoryRejection),
    /// 路径自身的故障（后端未注入、装配根忙、存储拒绝）。
    Fault(ErrorCode),
}

/// §42.4 一次遗忘的请求。四个字段是**调用方自己的陈述**，不是判定结论：
/// 调用方只陈述它**自己**知道的部分（清理进展、第三方留存、是否宣称已完全擦除）；
/// 「读取侧是否已阻断」不是调用方可断言的字段——它由阻断真的发生这一事实得出。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryForgetRequest {
    pub record_id: String,
    pub trigger: ForgetTrigger,
    pub purge_state: PurgeState,
    pub third_party_retention: Option<String>,
    pub claims_fully_erased: bool,
}

/// 记忆面的**接缝**。
///
/// ⚠️ 它定义在 shared 层而不是 runtime 层，是**分层的强制结果**：
/// `xtask/src/deps.rs` 禁止 shared 依赖 runtime（`nai-runtime` 反过来依赖本 crate），
/// 故「控制面能调到装配根」只能靠「接缝在此定义、实现在 runtime、由 app 注入」。
///
/// 实现 MUST **只转发**：判定只在内核一处（§42.3 `read_records`／§42.4 `forget`），
/// 控制面与实现都 MUST NOT 重跑它，否则一条门槛答两次就会分家（W89）。
///
/// 签名**不带** `now_ms`：时间源只在实现端一处（铁律：MUST NOT 另立第二处时间源）。
/// 签名**不带** `MemoryPolicy`：策略由实现端持有，路由挑策略会让 §42.3 的
/// `viewer_memory_enabled` 被答两次。
pub trait MemorySurface: Send + Sync {
    /// §42.3 读取：取数 → 诚实派生 → 判定。
    fn read(&self, query: &MemoryQuery) -> MemoryControlResult<MemoryReadOutcome>;
    /// §42.4 遗忘：**先**阻断读取，**再**判这次遗忘的陈述是否如实。
    /// 返回**生效中的**撤销时刻（已撤销则返回原有时刻，MUST NOT 覆盖）。
    fn forget(&self, request: &MemoryForgetRequest) -> MemoryControlResult<i64>;
}

/// Root-created, single device credential. Only its hash is retained here.
pub struct Access {
    hash: Vec<u8>,
    device: String,
    permissions: BTreeSet<String>,
    deadline: Instant,
    revoked: AtomicBool,
}
impl Access {
    pub fn new(
        token: &str,
        device: &str,
        permissions: &[&str],
        ttl: Duration,
    ) -> Result<Self, &'static str> {
        if token.len() < 32
            || token.len() > 256
            || !token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            || device.is_empty()
            || device.len() > 128
            || permissions.len() > 8
            || ttl.is_zero()
            || ttl > Duration::from_secs(3600)
        {
            return Err("invalid control access configuration");
        }
        Ok(Self {
            hash: Sha256::digest(token.as_bytes()).to_vec(),
            device: device.into(),
            permissions: permissions.iter().map(|p| (*p).into()).collect(),
            deadline: Instant::now() + ttl,
            revoked: AtomicBool::new(false),
        })
    }
    pub fn revoke(&self) {
        self.revoked.store(true, Ordering::Release);
    }
    fn authorized(&self, token: &str) -> bool {
        if self.revoked.load(Ordering::Acquire)
            || Instant::now() >= self.deadline
            || token.len() > 256
        {
            return false;
        }
        let hash = Sha256::digest(token.as_bytes());
        hash.iter()
            .zip(&self.hash)
            .fold(0u8, |diff, (a, b)| diff | (a ^ b))
            == 0
    }
}

struct State {
    seq: u64,
    commands: CommandLedger<Value>,
}

pub struct ControlApi {
    session_id: String,
    workspace: String,
    access: Arc<Access>,
    emergency: EmergencyHandle,
    /// The single session shared with the composition root. Revocation here is
    /// immediately visible to lease issuance, duty and fault handling.
    session: Arc<Mutex<Session>>,
    state: Mutex<State>,
    command_schema: jsonschema::Validator,
    approval_schema: jsonschema::Validator,
    approvals: Option<Arc<ApprovalBackend>>,
    /// §42.3/§42.4 记忆面后端。**缺失 ⇒ `CAPABILITY_NOT_READY`**，MUST NOT 假成功
    /// （与 `approvals` 同形）。
    memory: Option<Arc<dyn MemorySurface>>,
}
impl ControlApi {
    /// 挂上记忆面后端。链式是为了不破坏既有构造器的调用点。
    pub fn with_memory(mut self, memory: Arc<dyn MemorySurface>) -> Self {
        self.memory = Some(memory);
        self
    }
    /// Owns its session. The composition root should use
    /// [`Self::with_shared_session`] so revocation is not forked.
    pub fn new(
        session: Session,
        emergency: EmergencyHandle,
        access: Arc<Access>,
    ) -> Result<Self, String> {
        Self::with_approvals(session, emergency, access, None)
    }
    pub fn with_approvals(
        session: Session,
        emergency: EmergencyHandle,
        access: Arc<Access>,
        approvals: Option<Arc<ApprovalBackend>>,
    ) -> Result<Self, String> {
        Self::with_shared_session(Arc::new(Mutex::new(session)), emergency, access, approvals)
    }
    /// Shares one session with the composition root. Both sides then observe the
    /// same epoch; the control surface never holds a private copy.
    pub fn with_shared_session(
        session: Arc<Mutex<Session>>,
        emergency: EmergencyHandle,
        access: Arc<Access>,
        approvals: Option<Arc<ApprovalBackend>>,
    ) -> Result<Self, String> {
        let mut schema: Value = serde_json::from_str(SCHEMAS_JSON).map_err(|e| e.to_string())?;
        schema["$ref"] = json!("#/$defs/SessionCommand");
        let command_schema = jsonschema::validator_for(&schema).map_err(|e| e.to_string())?;
        schema["$ref"] = json!("#/$defs/ApprovalCommand");
        let approval_schema = jsonschema::validator_for(&schema).map_err(|e| e.to_string())?;
        let session_snapshot = session
            .lock()
            .map_err(|_| "session lock poisoned".to_string())?
            .snapshot()
            .clone();
        if approvals
            .as_ref()
            .is_some_and(|backend| backend.workspace != session_snapshot.workspace_id)
        {
            return Err("approval backend workspace mismatch".into());
        }
        let session_id = session_snapshot.session_id.clone();
        if !session_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            return Err("session ID must be path-safe".into());
        }
        Ok(Self {
            session_id,
            workspace: session_snapshot.workspace_id,
            access,
            emergency,
            session,
            state: Mutex::new(State {
                seq: 0,
                commands: CommandLedger::new(256),
            }),
            command_schema,
            approval_schema,
            approvals,
            memory: None,
        })
    }

    async fn handle(&self, mut request: Request<Incoming>) -> Result<HttpResponse, Infallible> {
        let origin = request
            .headers()
            .get("origin")
            .and_then(|h| h.to_str().ok());
        if origin.is_some_and(|origin| origin != ORIGIN) {
            return Ok(error(403, ErrorCode::PermissionDenied, "origin denied"));
        }
        if request.uri().query().is_some() {
            return Ok(error(
                400,
                ErrorCode::InvalidArgument,
                "query parameters forbidden",
            ));
        }
        let cors = origin.is_some();
        if request.method() == hyper::Method::OPTIONS {
            let mut response = reply(204, json!(null));
            if cors {
                cors_headers(&mut response);
            }
            return Ok(response);
        }
        let token = request
            .headers()
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .unwrap_or_default()
            .to_owned();
        let mut response = if !self.access.authorized(&token) {
            error(401, ErrorCode::Unauthenticated, "control identity invalid")
        } else {
            let method = request.method().clone();
            let path = request.uri().path().to_owned();
            let session_route = format!("/api/v1/sessions/{}", self.session_id);
            let permission = if method == hyper::Method::GET
                && matches!(path.as_str(), "/api/v1/health" | "/api/v1/runtime/status")
            {
                Some("runtime.read")
            } else if method == hyper::Method::GET && path == session_route {
                Some("session.read")
            } else if method == hyper::Method::POST && path == format!("{session_route}/mute") {
                Some("media.mute")
            } else if method == hyper::Method::POST && path == format!("{session_route}/takeover") {
                Some("session.takeover")
            } else if method == hyper::Method::GET && path == "/api/v1/approvals" {
                Some("approval.read")
            } else if method == hyper::Method::POST
                && (path.ends_with("/approve") || path.ends_with("/reject"))
                && path.starts_with("/api/v1/approvals/")
            {
                Some("approval.write")
            } else if method == hyper::Method::POST && path == "/api/v1/memory/read" {
                Some("memory.read")
            } else if method == hyper::Method::POST && path == "/api/v1/memory/forget" {
                Some("memory.forget")
            } else {
                None
            };
            if let Some(permission) = permission {
                if !self.access.permissions.contains(permission) {
                    error(
                        403,
                        ErrorCode::PermissionDenied,
                        "control permission denied",
                    )
                } else if method == hyper::Method::GET && path == "/api/v1/approvals" {
                    self.approvals_list()
                } else if method == hyper::Method::GET {
                    self.status()
                } else if request
                    .headers()
                    .get("content-type")
                    .and_then(|h| h.to_str().ok())
                    != Some("application/json")
                {
                    error(
                        400,
                        ErrorCode::InvalidArgument,
                        "JSON content type required",
                    )
                } else {
                    let received = Instant::now();
                    let mut bytes = Vec::new();
                    let mut failed = None;
                    while let Some(frame) = request.body_mut().frame().await {
                        match frame {
                            Ok(frame) => {
                                if let Ok(data) = frame.into_data() {
                                    if bytes.len() + data.len() > 4096 {
                                        failed = Some(error(
                                            413,
                                            ErrorCode::ResourceExhausted,
                                            "command body limit",
                                        ));
                                        break;
                                    }
                                    bytes.extend_from_slice(&data);
                                }
                            }
                            Err(_) => {
                                failed = Some(error(
                                    400,
                                    ErrorCode::InvalidArgument,
                                    "invalid request body",
                                ));
                                break;
                            }
                        }
                    }
                    if let Some(response) = failed {
                        response
                    } else if !self.access.authorized(&token) {
                        error(401, ErrorCode::Unauthenticated, "control identity invalid")
                    } else if path == "/api/v1/memory/read" {
                        self.memory_read(&bytes)
                    } else if path == "/api/v1/memory/forget" {
                        self.memory_forget(&bytes)
                    } else if let Some(action) = path.strip_prefix("/api/v1/approvals/") {
                        match action.strip_suffix("/approve") {
                            Some(id) => self.approval_action(id, true, &bytes, received),
                            None => match action.strip_suffix("/reject") {
                                Some(id) => self.approval_action(id, false, &bytes, received),
                                None => error(
                                    403,
                                    ErrorCode::PermissionDenied,
                                    "approval action unavailable",
                                ),
                            },
                        }
                    } else {
                        self.command(
                            &bytes,
                            if permission == "media.mute" {
                                CommandAction::Mute
                            } else {
                                CommandAction::Takeover
                            },
                            received,
                        )
                    }
                }
            } else if path.starts_with("/api/v1/sessions/") {
                error(
                    403,
                    ErrorCode::PermissionDenied,
                    "session route unavailable or out of scope",
                )
            } else {
                error(
                    404,
                    ErrorCode::CapabilityAbsent,
                    "control operation not implemented",
                )
            }
        };
        if cors {
            cors_headers(&mut response);
        }
        Ok(response)
    }

    fn status(&self) -> HttpResponse {
        let Ok(state) = self.state.try_lock() else {
            return error(409, ErrorCode::Conflict, "control state busy");
        };
        let Ok(session) = self.session.try_lock() else {
            return error(409, ErrorCode::Conflict, "session state busy");
        };
        let snapshot = session.snapshot();
        let lifecycle = SessionLifecycle::try_from(snapshot.lifecycle)
            .ok()
            .map(|v| v.as_str_name().to_ascii_lowercase())
            .unwrap_or_else(|| "faulted".into());
        let mode = SessionMode::try_from(snapshot.mode)
            .ok()
            .map(|v| v.as_str_name().to_ascii_lowercase())
            .unwrap_or_else(|| "assisted".into());
        let media = match MediaState::try_from(snapshot.media_state) {
            Ok(MediaState::MediaFailed) => "failed".into(),
            Ok(v) => v.as_str_name().to_ascii_lowercase(),
            Err(_) => "failed".into(),
        };
        reply(
            200,
            json!({"resource_id":self.session_id,"revision":snapshot.state_revision.to_string(),"status":"control_connected",
            "safe_message":"控制状态已连接；真实媒体设备未连接，命令 ACK 不代表声卡静音或播放完成。",
            "device_id":self.access.device,"last_command_seq":state.seq.to_string(),
            "session":{"session_id":snapshot.session_id,"generation_epoch":snapshot.generation_epoch.to_string(),"state_revision":snapshot.state_revision.to_string(),"lifecycle":lifecycle,"mode":mode,"media_state":media}}),
        )
    }

    fn command(&self, bytes: &[u8], action: CommandAction, received: Instant) -> HttpResponse {
        let body: Value = match serde_json::from_slice(bytes) {
            Ok(value) => value,
            Err(_) => return error(400, ErrorCode::InvalidArgument, "invalid command JSON"),
        };
        if !self.command_schema.is_valid(&body) {
            return error(400, ErrorCode::InvalidArgument, "command schema rejected");
        }
        let number = |name: &str| body[name].as_str().and_then(|v| v.parse::<u64>().ok());
        let (Some(epoch), Some(revision), Some(seq)) = (
            number("generation_epoch"),
            number("expected_state_revision"),
            number("seq"),
        ) else {
            return error(400, ErrorCode::InvalidArgument, "uint64 out of range");
        };
        if body["session_id"] != self.session_id || body["device_id"] != self.access.device {
            return error(403, ErrorCode::PermissionDenied, "command scope mismatch");
        }
        let ttl = body["ttl_ms"].as_u64().unwrap();
        if ttl > 30000 || body["command_id"].as_str().unwrap().len() > 256 {
            return error(400, ErrorCode::InvalidArgument, "command limit");
        }
        if Instant::now() >= received + Duration::from_millis(ttl) {
            return error(409, ErrorCode::DeadlineExceeded, "command TTL expired");
        }
        // Intentional before state locking, dedup capacity, epoch/revision checks.
        self.emergency.mute();
        let Ok(mut state) = self.state.try_lock() else {
            return error(
                409,
                ErrorCode::Conflict,
                "media revoked; control state busy",
            );
        };
        let Ok(mut session) = self.session.try_lock() else {
            return error(
                409,
                ErrorCode::Conflict,
                "media revoked; session state busy",
            );
        };
        let mut ledger = std::mem::replace(&mut state.commands, CommandLedger::new(256));
        let command = ControlCommand {
            command_id: body["command_id"].as_str().unwrap().into(),
            device_id: self.access.device.clone(),
            seq,
            expected_state_revision: revision,
            ttl_ms: ttl as u32,
            action: action as i32,
            context: Some(RequestContext {
                workspace_id: self.workspace.clone(),
                session_id: self.session_id.clone(),
                generation_epoch: Some(epoch),
                remaining_budget_ms: ttl as u32,
                ..Default::default()
            }),
            ..Default::default()
        };
        // Canonical JSON fingerprint binds action plus all validated fields.
        let fingerprint = serde_json::to_vec(&json!({"action":action as i32,"body":body})).unwrap();
        let now = Instant::now();
        let result=ledger.execute(&command.command_id,&fingerprint,now,now+Duration::from_secs(300),|| {
            if seq<=state.seq {return Err(SessionError::Stale);}
            let ack=session.command(&command,true,false,received,now,now+Duration::from_secs(300))?;
            state.seq=seq;
            Ok(json!({"command_id":ack.command_id,"executed":ack.executed,"state_revision":ack.state_revision.to_string(),"generation_epoch":ack.generation_epoch.to_string()}))
        });
        state.commands = ledger;
        match result {
            Ok(ack) => reply(200, ack),
            Err(reason) => error(
                409,
                match reason {
                    SessionError::Capacity => ErrorCode::ResourceExhausted,
                    SessionError::Expired => ErrorCode::DeadlineExceeded,
                    SessionError::Stale => ErrorCode::StaleEpoch,
                    _ => ErrorCode::Conflict,
                },
                "media revoked; command state update rejected",
            ),
        }
    }

    /// §42.3 记忆读取。
    ///
    /// `workspace` 与 `session` 取自**控制面自身的绑定**：作用域是事实，不是请求参数，
    /// 请求里出现同名字段一律忽略（有测试钉它）。观众维度是**查询**维度（取数边界只按
    /// workspace），故 `viewer_scoped_id` 可由请求给出，缺失即为空串。
    ///
    /// 判定**不在**这里：本方法只做鉴权、入参构造与结果传输。请求级拒绝 MUST 以具名
    /// 错误返回，MUST NOT 折成 `200` 加空 `records`——否则「本次读取不成立」与
    /// 「读出来零条」不可区分。
    fn memory_read(&self, bytes: &[u8]) -> HttpResponse {
        let body: Value = match serde_json::from_slice(bytes) {
            Ok(Value::Object(body)) => Value::Object(body),
            Ok(_) => {
                return error(
                    400,
                    ErrorCode::InvalidArgument,
                    "memory read body must be a JSON object",
                );
            }
            Err(_) => return error(400, ErrorCode::InvalidArgument, "invalid memory read JSON"),
        };
        // ⚠️ 「没给输入」≠「空集」：缺失/Null ⇒ 全部三类；显式 `[]` ⇒ 空集（零类）。
        let kinds = match body.get("kinds") {
            None | Some(Value::Null) => MemoryKind::ALL.to_vec(),
            Some(Value::Array(items)) => {
                let mut parsed = Vec::with_capacity(items.len());
                for item in items {
                    let Some(name) = item.as_str() else {
                        return error(
                            400,
                            ErrorCode::InvalidArgument,
                            "memory kind must be a string",
                        );
                    };
                    let Some(kind) = MemoryKind::parse_label(name) else {
                        return error(400, ErrorCode::InvalidArgument, "memory kind unknown");
                    };
                    parsed.push(kind);
                }
                parsed
            }
            Some(_) => {
                return error(
                    400,
                    ErrorCode::InvalidArgument,
                    "memory kinds must be an array",
                );
            }
        };
        let budget = match body.get("budget") {
            None | Some(Value::Null) => MEMORY_BUDGET_DEFAULT,
            Some(value) => match value.as_u64() {
                Some(value) if value <= MEMORY_BUDGET_MAX as u64 => value as usize,
                _ => {
                    return error(
                        400,
                        ErrorCode::InvalidArgument,
                        "memory budget out of range",
                    );
                }
            },
        };
        let text = |name: &str| -> Option<String> {
            body.get(name)
                .and_then(Value::as_str)
                .map(|value| value.to_owned())
        };
        let query = MemoryQuery {
            scope: MemoryScope::new(
                self.workspace.clone(),
                self.session_id.clone(),
                text("viewer_scoped_id").unwrap_or_default(),
            ),
            kinds,
            budget,
            authorization_ref: text("authorization_ref"),
            purpose: text("purpose"),
            allow_approximate: body
                .get("allow_approximate")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        };
        let Some(surface) = self.memory.as_ref() else {
            return error(
                503,
                ErrorCode::CapabilityNotReady,
                "memory backend not configured; no fabricated results",
            );
        };
        match surface.read(&query) {
            MemoryControlResult::Ok(outcome) => reply(
                200,
                json!({
                    "records": outcome.records.iter().map(memory_record_json).collect::<Vec<_>>(),
                    "refused": outcome.refused.iter().map(|refusal| json!({
                        "record_ref": refusal.record_ref, "reason": refusal.reason.label(),
                    })).collect::<Vec<_>>(),
                }),
            ),
            // 判定拒绝是**结论**，不是输入错误 ⇒ 与 400 分开；名字取自判定自己。
            MemoryControlResult::Rejected(rejection) => {
                error(422, ErrorCode::InvalidArgument, rejection.label())
            }
            MemoryControlResult::Fault(code) => fault(code),
        }
    }

    /// §42.4 遗忘：**先**阻断读取，**再**判这次遗忘的陈述是否如实。
    ///
    /// 调用方只陈述**它自己**知道的部分；「读取侧是否已阻断」不是它的字段。
    /// 缺省值一律取**安全方向**：`trigger` 缺 ⇒ 用户请求删除（本控制面正是它的载体），
    /// `purge_state` 缺 ⇒ 未开始（不宣称已清除），`claims_fully_erased` 缺 ⇒ 不宣称。
    fn memory_forget(&self, bytes: &[u8]) -> HttpResponse {
        let body: Value = match serde_json::from_slice(bytes) {
            Ok(Value::Object(body)) => Value::Object(body),
            Ok(_) => {
                return error(
                    400,
                    ErrorCode::InvalidArgument,
                    "memory forget body must be a JSON object",
                );
            }
            Err(_) => {
                return error(
                    400,
                    ErrorCode::InvalidArgument,
                    "invalid memory forget JSON",
                );
            }
        };
        let Some(record_id) = body.get("record_id").and_then(Value::as_str) else {
            return error(
                400,
                ErrorCode::InvalidArgument,
                "memory forget needs record_id",
            );
        };
        if record_id.trim().is_empty() {
            return error(
                400,
                ErrorCode::InvalidArgument,
                "memory forget record_id empty",
            );
        }
        let trigger = match body.get("trigger") {
            None | Some(Value::Null) => ForgetTrigger::UserRequest,
            Some(value) => {
                let Some(name) = value.as_str() else {
                    return error(
                        400,
                        ErrorCode::InvalidArgument,
                        "memory forget trigger must be a string",
                    );
                };
                let Some(trigger) = ForgetTrigger::ALL
                    .iter()
                    .copied()
                    .find(|trigger| trigger.label() == name)
                else {
                    return error(
                        400,
                        ErrorCode::InvalidArgument,
                        "memory forget trigger unknown",
                    );
                };
                trigger
            }
        };
        let purge_state = match body.get("purge_state") {
            None | Some(Value::Null) => PurgeState::NotStarted,
            Some(value) => {
                let Some(name) = value.as_str() else {
                    return error(
                        400,
                        ErrorCode::InvalidArgument,
                        "memory purge state must be a string",
                    );
                };
                let Some(state) = PurgeState::ALL
                    .iter()
                    .copied()
                    .find(|state| state.label() == name)
                else {
                    return error(
                        400,
                        ErrorCode::InvalidArgument,
                        "memory purge state unknown",
                    );
                };
                state
            }
        };
        let request = MemoryForgetRequest {
            record_id: record_id.to_owned(),
            trigger,
            purge_state,
            third_party_retention: body
                .get("third_party_retention")
                .and_then(Value::as_str)
                .map(|value| value.to_owned()),
            claims_fully_erased: body
                .get("claims_fully_erased")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        };
        let Some(surface) = self.memory.as_ref() else {
            return error(
                503,
                ErrorCode::CapabilityNotReady,
                "memory backend not configured; no fabricated results",
            );
        };
        match surface.forget(&request) {
            MemoryControlResult::Ok(revoked_at_ms) => reply(
                200,
                json!({
                    "record_ref": request.record_id,
                    "revoked_at_utc_ms": revoked_at_ms.to_string(),
                    // 如实：阻断已生效，**清除副本**不在本入口（被遗忘的记录仍在库里）。
                    "safe_message": "读取侧已阻断；副本清除未执行，该记录仍存在于库中。",
                }),
            ),
            MemoryControlResult::Rejected(rejection) => {
                error(422, ErrorCode::InvalidArgument, rejection.label())
            }
            MemoryControlResult::Fault(code) => fault(code),
        }
    }

    fn approvals_list(&self) -> HttpResponse {
        let Some(backend) = self.approvals.as_ref() else {
            return error(
                503,
                ErrorCode::CapabilityNotReady,
                "approval backend not configured; no fabricated results",
            );
        };
        let (session_id, epoch) = {
            let Ok(session) = self.session.try_lock() else {
                return error(409, ErrorCode::Conflict, "session state busy");
            };
            let snapshot = session.snapshot();
            (snapshot.session_id.clone(), snapshot.generation_epoch)
        };
        match backend.list(&session_id, epoch, 200) {
            Ok(items) => {
                let at_limit = items.len() == 200;
                let items: Vec<Value> = items
                    .iter()
                    .map(|item| {
                        json!({
                            "id": item.candidate_hash,
                            "revision": item.approval_id.clone().unwrap_or_default(),
                            "status": if item.approval_id.is_some() { "approved" } else { "awaiting_approval" },
                            "encoded_contract_ref": format!("approval:candidate_hash:{}", item.candidate_hash),
                            "expires_at_utc_ms": item.expires_at_utc_ms,
                            "evidence_count": item.evidence_count.to_string(),
                            "display_text": item.text,
                        })
                    })
                    .collect();
                reply(
                    200,
                    json!({
                        "resource_id": self.session_id,
                        "revision": "approvals",
                        "status": if at_limit { "approvals_limit_reached" } else { "approvals_ok" },
                        "safe_message": if at_limit { "列表已达200条上限；处理候选后刷新，不代表完整队列。" } else { "当前有效的待审批候选元信息；不授予播放权限。" },
                        "items": items,
                    }),
                )
            }
            Err(error) => from_contract(error),
        }
    }

    fn approval_action(
        &self,
        candidate: &str,
        approve: bool,
        bytes: &[u8],
        received: Instant,
    ) -> HttpResponse {
        let Some(backend) = self.approvals.as_ref() else {
            return error(
                503,
                ErrorCode::CapabilityNotReady,
                "approval backend not configured; no fabricated results",
            );
        };
        let body: Value = match serde_json::from_slice(bytes) {
            Ok(value) => value,
            Err(_) => return error(400, ErrorCode::InvalidArgument, "invalid approval JSON"),
        };
        if !self.approval_schema.is_valid(&body) {
            return error(400, ErrorCode::InvalidArgument, "approval schema rejected");
        }
        let number = |name: &str| body[name].as_str().and_then(|v| v.parse::<u64>().ok());
        let (Some(epoch), Some(revision), Some(seq)) = (
            number("generation_epoch"),
            number("expected_state_revision"),
            number("seq"),
        ) else {
            return error(400, ErrorCode::InvalidArgument, "uint64 out of range");
        };
        if body["session_id"] != self.session_id || body["device_id"] != self.access.device {
            return error(403, ErrorCode::PermissionDenied, "approval scope mismatch");
        }
        if candidate.len() != 64
            || !candidate
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            || body["candidate_hash"] != candidate
        {
            return error(400, ErrorCode::InvalidArgument, "candidate hash mismatch");
        }
        if body["hash_algorithm"] != "sha256-canonical-v1" {
            return error(
                400,
                ErrorCode::InvalidArgument,
                "hash algorithm unsupported",
            );
        }
        let ttl = body["ttl_ms"].as_u64().unwrap_or(0);
        if ttl == 0 || ttl > 30000 || body["command_id"].as_str().unwrap_or("").len() > 256 {
            return error(400, ErrorCode::InvalidArgument, "approval limit");
        }
        if Instant::now() >= received + Duration::from_millis(ttl) {
            return error(409, ErrorCode::DeadlineExceeded, "approval TTL expired");
        }
        // ApprovalCommand has no expiry field; the operator's local decision is
        // valid for the command TTL, matching the trusted root's budget window.
        let expires_at_utc_ms = match utc_ms().checked_add(ttl) {
            Some(value) => value,
            None => {
                return error(400, ErrorCode::InvalidArgument, "ttl overflow");
            }
        };
        let Ok(mut state) = self.state.try_lock() else {
            return error(409, ErrorCode::Conflict, "control state busy");
        };
        let Ok(session) = self.session.try_lock() else {
            return error(409, ErrorCode::Conflict, "session state busy");
        };
        let mut ledger = std::mem::replace(&mut state.commands, CommandLedger::new(256));
        let fingerprint = serde_json::to_vec(&json!({"approval":approve,"body":body})).unwrap();
        let command_id = body["command_id"].as_str().unwrap().to_owned();
        let now = Instant::now();
        let mut backend_error = None;
        let result = ledger.execute(&command_id, &fingerprint, now, now + Duration::from_secs(300), || {
            if seq <= state.seq {
                return Err(SessionError::Stale);
            }
            let (session_id, snapshot_epoch, snapshot_revision) = (
                session.snapshot().session_id.clone(),
                session.snapshot().generation_epoch,
                session.snapshot().state_revision,
            );
            if epoch != snapshot_epoch || revision != snapshot_revision {
                return Err(SessionError::Stale);
            }
            let deadline = received + Duration::from_millis(ttl);
            let budget = deadline.saturating_duration_since(Instant::now()).min(Duration::from_secs(5));
            let operation = Operation::new(budget).map_err(|_| SessionError::Expired)?;
            if approve {
                // Synchronize this control surface to the store's approval in a
                // single local decision; the store validates facts/evidence.
                backend
                    .store
                    .try_lock()
                    .map_err(|_| SessionError::Conflict)?
                    .approve(
                        &operation,
                        &self.workspace,
                        &backend.actor,
                        &Approval {
                            approval_id: command_id.clone(),
                            session_id: session_id.clone(),
                            candidate_hash: candidate.into(),
                            hash_algorithm: "sha256-canonical-v1".into(),
                            action_scope: "speak".into(),
                            expires_at_utc_ms,
                            generation_epoch: epoch,
                            ..Default::default()
                        },
                    )
                    .map_err(|error| {
                        backend_error = Some(to_contract(error));
                        SessionError::Conflict
                    })?;
            } else {
                backend
                    .reject(&operation, &session_id, epoch, candidate)
                    .map_err(|error| {
                        backend_error = Some(error);
                        SessionError::Conflict
                    })?;
            }
            state.seq = seq;
            Ok(json!({"command_id":command_id,"executed":true,"state_revision":snapshot_revision.to_string(),"generation_epoch":snapshot_epoch.to_string()}))
        });
        state.commands = ledger;
        if let Some(error) = backend_error {
            return from_contract(error);
        }
        match result {
            Ok(ack) => reply(200, ack),
            Err(reason) => error(
                409,
                match reason {
                    SessionError::Capacity => ErrorCode::ResourceExhausted,
                    SessionError::Expired => ErrorCode::DeadlineExceeded,
                    SessionError::Stale => ErrorCode::StaleEpoch,
                    _ => ErrorCode::Conflict,
                },
                "approval state update rejected",
            ),
        }
    }
}

fn from_contract(error: ContractError) -> HttpResponse {
    let code = ErrorCode::try_from(error.code).unwrap_or(ErrorCode::Internal);
    let status = match code {
        ErrorCode::Unauthenticated => 401,
        ErrorCode::PermissionDenied => 403,
        ErrorCode::ResourceExhausted => 413,
        ErrorCode::DeadlineExceeded | ErrorCode::ApprovalExpired | ErrorCode::EvidenceExpired => {
            409
        }
        ErrorCode::StaleEpoch | ErrorCode::Conflict => 409,
        _ => 400,
    };
    reply(
        status,
        json!({"request_id":"","error":{"code":code.as_str_name(),"category":"control","retryable":error.retryable,"message":error.message}}),
    )
}
fn utc_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|time| time.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}
fn reply(status: u16, body: Value) -> HttpResponse {
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .header("cache-control", "no-store")
        .body(Full::new(Bytes::from(serde_json::to_vec(&body).unwrap())))
        .unwrap()
}
/// 记忆记录的 JSON 形状。大整数按契约惯例走字符串，避免 TS 侧精度丢失。
fn memory_record_json(record: &MemoryRecord) -> Value {
    json!({
        "record_id": record.record_id,
        "workspace_id": record.workspace_id,
        "session_id": record.session_id,
        "viewer_scoped_id": record.viewer_scoped_id,
        "kind": record.kind,
        "purpose": record.purpose,
        "authorization_ref": record.authorization_ref,
        "content": record.content,
        "expires_at_utc_ms": record.expires_at_utc_ms.to_string(),
    })
}

/// 路径自身的故障。状态码由契约错误码映射，MUST NOT 与判定拒绝共用同一个码——
/// 否则「存储故障」会被记在「判定说不可以」的名下。
fn fault(code: ErrorCode) -> HttpResponse {
    let status = match code {
        ErrorCode::Conflict => 409,
        ErrorCode::PermissionDenied => 403,
        ErrorCode::Unauthenticated => 401,
        ErrorCode::CapabilityNotReady => 503,
        ErrorCode::ResourceExhausted => 429,
        ErrorCode::DeadlineExceeded => 504,
        _ => 500,
    };
    error(status, code, "memory surface fault")
}

fn error(status: u16, code: ErrorCode, message: &str) -> HttpResponse {
    reply(
        status,
        json!({"request_id":"","error":{"code":code.as_str_name(),"category":"control","retryable":false,"message":message}}),
    )
}
fn cors_headers(response: &mut HttpResponse) {
    let headers = response.headers_mut();
    headers.insert("access-control-allow-origin", ORIGIN.parse().unwrap());
    headers.insert(
        "access-control-allow-methods",
        "GET, POST, OPTIONS".parse().unwrap(),
    );
    headers.insert(
        "access-control-allow-headers",
        "authorization, content-type".parse().unwrap(),
    );
    headers.insert("vary", "Origin".parse().unwrap());
}

/// Explicitly supplied loopback listener only. No implicit service startup.
pub async fn serve(
    listener: TcpListener,
    api: Arc<ControlApi>,
    mut shutdown: watch::Receiver<bool>,
) -> std::io::Result<()> {
    if !listener.local_addr()?.ip().is_loopback() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "control listener must be loopback",
        ));
    }
    let slots = Arc::new(Semaphore::new(16));
    let mut tasks = JoinSet::new();
    loop {
        if *shutdown.borrow() {
            break;
        }
        tokio::select! {
            changed=shutdown.changed()=>{if changed.is_err() || *shutdown.borrow() {break;}},
            accepted=listener.accept()=>{
                let (socket,_)=accepted?;
                if tasks.len() < 16 && let Ok(permit)=slots.clone().try_acquire_owned() {
                    let api=api.clone();
                    tasks.spawn(async move {
                        let _permit=permit;
                        let service=service_fn(move |request|{let api=api.clone();async move {api.handle(request).await}});
                        let mut builder=hyper::server::conn::http1::Builder::new();
                        builder.keep_alive(false).max_buf_size(8192).max_headers(32);
                        let connection=builder.serve_connection(TokioIo::new(socket),service);
                        let _=tokio::time::timeout(Duration::from_secs(5),connection).await;
                    });
                }
            },
            _=tasks.join_next(), if !tasks.is_empty()=>{},
        }
    }
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    Ok(())
}

#[cfg(test)]
mod tests;
