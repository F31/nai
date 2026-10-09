//! Explicit loopback engineering control surface. Not production IPC/TLS,
//! device authentication, preflight or hardware playback/mute acknowledgement.
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Request, Response, body::Incoming, service::service_fn};
use hyper_util::rt::TokioIo;
use nai_contracts::{control_http::SCHEMAS_JSON, v1::*};
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
}
impl From<PendingApproval> for PendingApprovalView {
    fn from(item: PendingApproval) -> Self {
        Self {
            candidate_hash: item.candidate_hash,
            approval_id: item.approval_id,
            expires_at_utc_ms: item.expires_at_utc_ms.to_string(),
            status: item.status,
            evidence_count: item.evidence_count,
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
}
impl ControlApi {
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
