//! Memory evaluation (V3.5 §23.5) plus the memory read/write surface of
//! V3.8 §42.2–§42.6 (W57, acceptance H03/H04).
//!
//! The evaluation half is the older one: working memory is bounded and only
//! holds actually-played content, session summaries are deterministically
//! extracted, and viewer long-term memory is off by default with
//! scope/purpose/authorization/retention discipline.
//!
//! The surface half fills the gaps §23.5 left open. §42.2 adds a **source**
//! dimension: a write carries where it came from, and a tool return, a peer
//! return, a raw comment or a model draft is never a fact or a promise. §42.3
//! adds the **read chain** (scope before time, then revocation, permission and
//! the segment budget) and forbids substituting an approximate entry. §42.4
//! adds the **delete chain** the four forget triggers share. §42.6 pins the
//! four operations of an already-`frozen` `memory.access@1`.
//!
//! Every refusal is named. Silently dropping a record off the context and
//! silently cropping one out of the budget are the two failure modes this
//! surface exists to prevent, so [`MemoryRejection`] carries a stable label
//! for each of them and [`MemoryReadOutcome`] reports the refusals rather than
//! swallowing them (V3.8 §40, INV-32: every layer failure is named).
//!
//! V3.11 §5.5 / PD-31 (W89): the "viewer long-term memory is off by default"
//! switch is answered in **exactly one** place, [`viewer_memory_gate`], and the
//! read face, the write face and the request-admission face all call it. The
//! switch used to be answered on the read face only, so a viewer candidate was
//! accepted by [`admit_write`] while the capability was closed.
use super::EvaluateError;
use nai_contracts::v1::{MemoryRecord, MemoryRequest};
use std::collections::BTreeSet;

pub const WORKING_KIND: &str = "working";
pub const SESSION_KIND: &str = "session";
pub const VIEWER_KIND: &str = "viewer";
/// §42.1: 语义记忆（已发布知识）经 §6 检索，不参与记忆段的配额裁剪。
pub const SEMANTIC_KIND: &str = "semantic";

pub struct MemoryPolicy {
    pub viewer_memory_enabled: bool,
    pub allowed_viewer_scopes: &'static [&'static str],
    pub allowed_purposes: &'static [&'static str],
    pub allowed_viewer_fields: &'static [&'static str],
    pub max_content_chars: usize,
    pub max_retention_ms: u64,
    pub forbidden_sensitive: &'static [&'static str],
}
impl Default for MemoryPolicy {
    fn default() -> Self {
        Self {
            viewer_memory_enabled: false,
            allowed_viewer_scopes: &["douyin.app_scope"],
            allowed_purposes: &["order_status", "answered_once"],
            allowed_viewer_fields: &["preference", "answered_topic"],
            max_content_chars: 2048,
            max_retention_ms: 30 * 24 * 3600 * 1000,
            forbidden_sensitive: &["手机号", "身份证", "收货地址", "银行卡", "情绪"],
        }
    }
}

/// Working memory only keeps records for actually-played content; unplayed
/// candidates are never written as facts. Bounded by `max`.
pub fn working_memory(
    records: &[MemoryRecord],
    played_ids: &[&str],
    max: usize,
) -> Vec<MemoryRecord> {
    let played: BTreeSet<&str> = played_ids.iter().copied().collect();
    let mut kept = records
        .iter()
        .filter(|record| played.contains(record.record_id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    kept.truncate(max.max(1));
    kept
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionSummary {
    pub topics: Vec<String>,
    pub answered_questions: Vec<String>,
    pub approved_commitments: Vec<String>,
    pub recovery_point: String,
}

/// Deterministic summary extraction; optional API summarization is a later
/// concern and never fabricates topics from unplayed candidates.
pub fn session_summary(records: &[MemoryRecord], kind: &str, max_fields: usize) -> SessionSummary {
    let mut summary = SessionSummary::default();
    let mut topics = Vec::new();
    let mut answered = Vec::new();
    let mut commitments = Vec::new();
    let mut seen_topics = BTreeSet::new();
    let mut seen_answered = BTreeSet::new();
    let mut seen_commitments = BTreeSet::new();
    for record in records {
        if record.kind != kind {
            continue;
        }
        match record.purpose.as_str() {
            "topic" if seen_topics.insert(record.content.clone()) => {
                topics.push(record.content.clone())
            }
            "answered" if seen_answered.insert(record.content.clone()) => {
                answered.push(record.content.clone());
            }
            "commitment" if seen_commitments.insert(record.content.clone()) => {
                commitments.push(record.content.clone());
            }
            "recovery_point" => summary.recovery_point = record.content.clone(),
            _ => {}
        }
        if topics.len() >= max_fields
            && answered.len() >= max_fields
            && commitments.len() >= max_fields
        {
            break;
        }
    }
    topics.truncate(max_fields);
    answered.truncate(max_fields);
    commitments.truncate(max_fields);
    summary.topics = topics;
    summary.answered_questions = answered;
    summary.approved_commitments = commitments;
    summary
}

fn scope_ok(id: &str, allowed: &[&str]) -> bool {
    allowed
        .iter()
        .any(|scope| id.starts_with(&format!("{scope}:")))
}

/// §5.5 / PD-31: viewer long-term memory is **off by default**.
///
/// This is the **single answering point** for that switch. The read face
/// ([`read_records`]), the write face ([`admit_write`]) and the request
/// admission face ([`viewer_record_allowed`]) all go through it; none of them
/// restates `if !policy.viewer_memory_enabled`, because a switch answered twice
/// drifts (W88: one thing MUST NOT be defined in two places).
pub fn viewer_memory_gate(policy: &MemoryPolicy) -> Result<(), MemoryRejection> {
    if policy.viewer_memory_enabled {
        Ok(())
    } else {
        Err(MemoryRejection::ViewerMemoryDisabled)
    }
}

/// Viewer long-term memory is off by default. Enabling/reading requires a
/// scoped ID (never a nickname), an allowed purpose, authorization and retention.
pub fn viewer_record_allowed(
    request: &MemoryRequest,
    policy: &MemoryPolicy,
) -> Result<(), EvaluateError> {
    // The answer lives in [`viewer_memory_gate`]; this legacy face keeps its
    // `EvaluateError` contract and only *maps* the gate's refusal onto it.
    // Mapping is not a second definition — the decision itself is not restated.
    if viewer_memory_gate(policy).is_err() {
        return Err(EvaluateError::Unauthorized);
    }
    if request.kind != VIEWER_KIND || request.purpose.is_empty() {
        return Err(EvaluateError::Invalid);
    }
    if !policy.allowed_purposes.contains(&request.purpose.as_str()) {
        return Err(EvaluateError::Unauthorized);
    }
    if request.viewer_scoped_id.is_empty()
        || request.viewer_scoped_id.len() > 256
        || !scope_ok(&request.viewer_scoped_id, policy.allowed_viewer_scopes)
        || request.viewer_scoped_id.contains('@')
        || request.viewer_scoped_id.contains(char::is_whitespace)
    {
        // A nickname-like or cross-scope id is not a stable unique identifier.
        return Err(EvaluateError::Unauthorized);
    }
    Ok(())
}

fn validate_record(
    record: &MemoryRecord,
    policy: &MemoryPolicy,
    now_ms: u64,
) -> Result<(), EvaluateError> {
    if record.record_id.is_empty() || record.workspace_id.is_empty() || record.session_id.is_empty()
    {
        return Err(EvaluateError::Invalid);
    }
    if record.content.is_empty() || record.content.len() > policy.max_content_chars {
        return Err(EvaluateError::Capacity);
    }
    if policy
        .forbidden_sensitive
        .iter()
        .any(|marker| record.content.contains(marker))
    {
        return Err(EvaluateError::Unauthorized);
    }
    if record.expires_at_utc_ms <= now_ms
        || record.expires_at_utc_ms > now_ms.saturating_add(policy.max_retention_ms)
    {
        return Err(EvaluateError::Expired);
    }
    Ok(())
}

/// Validate a viewer record's scope, fields and retention before any persistence.
pub fn viewer_record(
    record: &MemoryRecord,
    policy: &MemoryPolicy,
    now_ms: u64,
) -> Result<(), EvaluateError> {
    if record.kind != VIEWER_KIND {
        return Err(EvaluateError::Invalid);
    }
    if record.authorization_ref.is_empty()
        || !scope_ok(&record.viewer_scoped_id, policy.allowed_viewer_scopes)
    {
        return Err(EvaluateError::Unauthorized);
    }
    if !policy
        .allowed_viewer_fields
        .iter()
        .any(|field| record.purpose == *field)
    {
        return Err(EvaluateError::Unauthorized);
    }
    validate_record(record, policy, now_ms)
}

// ---------------------------------------------------------------------------
// §42.6 四项操作
// ---------------------------------------------------------------------------

/// §42.6: `memory.access@1` 已由能力目录标为 `frozen`，其访问面恰好是四项操作。
/// 四项之外的操作名是**拒因**，不是「未知但先收下」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MemoryOperation {
    Write,
    Read,
    Forget,
    Scopes,
}

impl MemoryOperation {
    pub const ALL: [MemoryOperation; 4] = [
        MemoryOperation::Write,
        MemoryOperation::Read,
        MemoryOperation::Forget,
        MemoryOperation::Scopes,
    ];

    /// §42.6: 只接受 `MemoryRequest.operation` 的四个冻结取值。
    pub fn parse(name: &str) -> Result<Self, MemoryRejection> {
        match name {
            "write" => Ok(MemoryOperation::Write),
            "read" => Ok(MemoryOperation::Read),
            "forget" => Ok(MemoryOperation::Forget),
            "scopes" => Ok(MemoryOperation::Scopes),
            _ => Err(MemoryRejection::UnknownOperation),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            MemoryOperation::Write => "write",
            MemoryOperation::Read => "read",
            MemoryOperation::Forget => "forget",
            MemoryOperation::Scopes => "scopes",
        }
    }
}

// ---------------------------------------------------------------------------
// §42.2 写入面（H03）
// ---------------------------------------------------------------------------

/// §42.2: 记忆写入的来源。**只有已批准且已播完的内容**可写为场次记忆；
/// 其余来源即使「看起来已经播过」也不得成为事实或承诺（INV-09、INV-24、INV-31）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WriteSource {
    /// 已批准且已播完——唯一可写为事实的来源。
    ApprovedPlayed,
    /// 已批准但尚未播出（§5.7 的检查点纪律）。
    ApprovedUnplayed,
    /// 工具返回。
    ToolReturn,
    /// 外部智能体（A2A 对端）返回。
    PeerReturn,
    /// 评论原文。
    CommentText,
    /// 模型草稿。
    ModelDraft,
}

impl WriteSource {
    pub const ALL: [WriteSource; 6] = [
        WriteSource::ApprovedPlayed,
        WriteSource::ApprovedUnplayed,
        WriteSource::ToolReturn,
        WriteSource::PeerReturn,
        WriteSource::CommentText,
        WriteSource::ModelDraft,
    ];

    /// §42.2 / INV-31: 工具返回、外部智能体返回、评论原文与模型草稿一律是**数据**，
    /// 其自称「已批准」「只读」不产生效力，也不得直接写成事实或承诺。
    pub fn is_untrusted(self) -> bool {
        matches!(
            self,
            WriteSource::ToolReturn
                | WriteSource::PeerReturn
                | WriteSource::CommentText
                | WriteSource::ModelDraft
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            WriteSource::ApprovedPlayed => "approved_played",
            WriteSource::ApprovedUnplayed => "approved_unplayed",
            WriteSource::ToolReturn => "tool_return",
            WriteSource::PeerReturn => "peer_return",
            WriteSource::CommentText => "comment_text",
            WriteSource::ModelDraft => "model_draft",
        }
    }
}

/// §42.2「叙述还是权威事实」的标注；两者不可互相顶替。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimKind {
    /// 叙述：已讲内容的记录，不构成权威事实。
    Narrative,
    /// 权威事实：只能经 §6 的导入与发布得到。
    AuthoritativeFact,
}

impl ClaimKind {
    pub fn label(self) -> &'static str {
        match self {
            ClaimKind::Narrative => "narrative",
            ClaimKind::AuthoritativeFact => "authoritative_fact",
        }
    }
}

/// §42.2: 一次候选写入。作用域、来源标识、时间与标注都是**必填项**，
/// 缺失时是缺项，不是「取默认值」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryCandidate {
    pub record: MemoryRecord,
    pub source: WriteSource,
    pub claim: ClaimKind,
    /// §42.2: 来源标识（谁写的、写的是哪一次事件的产物）。
    pub source_ref: String,
    /// §42.2: 写入时间。
    pub written_at_utc_ms: u64,
    /// §6: 权威事实的发布引用。
    pub publish_ref: Option<String>,
}

impl MemoryCandidate {
    /// 一个「尚未填写」的候选：来源标识为空、时间为 0、标注为叙述。
    /// 这样构造出来的候选**必然**被 [`admit_write`] 拒绝——缺项不会被默认值掩盖。
    pub fn new(record: MemoryRecord, source: WriteSource) -> Self {
        Self {
            record,
            source,
            claim: ClaimKind::Narrative,
            source_ref: String::new(),
            written_at_utc_ms: 0,
            publish_ref: None,
        }
    }

    pub fn with_claim(mut self, claim: ClaimKind) -> Self {
        self.claim = claim;
        self
    }

    pub fn with_source_ref(mut self, source_ref: impl Into<String>) -> Self {
        self.source_ref = source_ref.into();
        self
    }

    pub fn written_at(mut self, written_at_utc_ms: u64) -> Self {
        self.written_at_utc_ms = written_at_utc_ms;
        self
    }

    pub fn with_publish_ref(mut self, publish_ref: impl Into<String>) -> Self {
        self.publish_ref = Some(publish_ref.into());
        self
    }
}

/// H03 / §42.2: 写入的来源门槛。
///
/// 判据顺序刻意固定：先补**必填项**（作用域、来源标识、时间），再判**能力开关**，
/// 再判**来源**，最后判**标注**。理由是「缺了必填项」「能力没开」「来源不可信」
/// 是**三件事**，混在一句「不合规」里会让调用方无法区分该补字段、该去开开关，
/// 还是该换来源。能力开关的位置与读取面同序（`read_records` 先报 `invalid_scope`、
/// 再报 `viewer_memory_disabled`）——它是不依赖条目内容就成立的判定，故先于条目级的来源判定。
pub fn admit_write(
    candidate: &MemoryCandidate,
    policy: &MemoryPolicy,
) -> Result<(), MemoryRejection> {
    let record = &candidate.record;

    // §42.2: 每次写入必带作用域（workspace/session）、来源标识与时间。
    if record.workspace_id.trim().is_empty() || record.session_id.trim().is_empty() {
        return Err(MemoryRejection::WriteWithoutScope);
    }
    if candidate.source_ref.trim().is_empty() {
        return Err(MemoryRejection::WriteWithoutSourceRef);
    }
    if candidate.written_at_utc_ms == 0 {
        return Err(MemoryRejection::WriteWithoutTimestamp);
    }

    // §5.5 / PD-31: 观众长期记忆默认关闭——**写入面也要判**，且用的是与读取面
    // **同一处**判定（[`viewer_memory_gate`]）。此前这里没有这一步：能力关闭时
    // 一条 viewer 候选仍会被接受（「读取被拒、写入却被接受」的半覆盖）。
    if record.kind == VIEWER_KIND {
        viewer_memory_gate(policy)?;
    }

    // §42.2 / INV-31: 工具返回、外部智能体返回、评论原文与模型草稿**一律**不是事实来源。
    // 不按 `claim` 区分：这些来源既非「已批准且已播完」，也不得写成事实或承诺。
    if candidate.source.is_untrusted() {
        return Err(MemoryRejection::UntrustedSourceAsFact);
    }

    // §5.7 / §42.2: 未播出的生成文本不得写成事实。
    if candidate.source == WriteSource::ApprovedUnplayed {
        return Err(MemoryRejection::UnplayedCandidateAsFact);
    }

    // §42.2: 权威事实只能经 §6 导入与发布，且只落在语义记忆。
    if candidate.claim == ClaimKind::AuthoritativeFact {
        if record.kind != SEMANTIC_KIND {
            return Err(MemoryRejection::FactOutsideSemanticMemory);
        }
        if candidate
            .publish_ref
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            return Err(MemoryRejection::FactWithoutPublishRef);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// §42.3 读取面（H04）
// ---------------------------------------------------------------------------

/// §42.3: 记忆段覆盖的三类记忆（语义记忆经 §6 检索，不在本段配额内）。
/// 变体的顺序即超配额裁剪的**固定优先级**：场次 > 工作 > 观众长期记忆。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MemoryKind {
    Session,
    Working,
    Viewer,
}

impl MemoryKind {
    pub const ALL: [MemoryKind; 3] = [MemoryKind::Session, MemoryKind::Working, MemoryKind::Viewer];

    /// §42.1 的四类记忆在协议里以 `kind` 字符串表达。
    pub fn parse_label(name: &str) -> Option<Self> {
        match name {
            SESSION_KIND => Some(MemoryKind::Session),
            WORKING_KIND => Some(MemoryKind::Working),
            VIEWER_KIND => Some(MemoryKind::Viewer),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            MemoryKind::Session => SESSION_KIND,
            MemoryKind::Working => WORKING_KIND,
            MemoryKind::Viewer => VIEWER_KIND,
        }
    }
}

/// §42.3: 作用域的三个维度。作用域**不完整**时不得「就近匹配」。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryScope {
    pub workspace_id: String,
    pub session_id: String,
    pub viewer_scoped_id: String,
}

impl MemoryScope {
    pub fn new(
        workspace_id: impl Into<String>,
        session_id: impl Into<String>,
        viewer_scoped_id: impl Into<String>,
    ) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            session_id: session_id.into(),
            viewer_scoped_id: viewer_scoped_id.into(),
        }
    }

    /// §42.3: workspace 与 session 是必填维度；观众维度只在读观众长期记忆时需要。
    pub fn is_complete(&self) -> bool {
        !self.workspace_id.trim().is_empty() && !self.session_id.trim().is_empty()
    }
}

/// §42.3: 一次读取请求。
#[derive(Debug, Clone)]
pub struct MemoryQuery {
    pub scope: MemoryScope,
    pub kinds: Vec<MemoryKind>,
    pub budget: usize,
    /// §42.3: 观众长期记忆读取所需的授权引用。
    pub authorization_ref: Option<String>,
    /// §42.3: 观众长期记忆读取所需的用途记录。
    pub purpose: Option<String>,
    /// §42.3: 请求「允许用近似条目顶替」——这是一个**可被拒绝的请求**，
    /// 而不是一句注释，故显式建模。
    pub allow_approximate: bool,
}

impl MemoryQuery {
    pub fn new(scope: MemoryScope, kinds: Vec<MemoryKind>, budget: usize) -> Self {
        Self {
            scope,
            kinds,
            budget,
            authorization_ref: None,
            purpose: None,
            allow_approximate: false,
        }
    }

    pub fn with_viewer_grant(
        mut self,
        authorization_ref: impl Into<String>,
        purpose: impl Into<String>,
    ) -> Self {
        self.authorization_ref = Some(authorization_ref.into());
        self.purpose = Some(purpose.into());
        self
    }

    pub fn allowing_approximate(mut self) -> Self {
        self.allow_approximate = true;
        self
    }
}

/// §42.3: 一条候选记录及其门控状态。`approximate_of` 非空表示这条是**为顶替别条而准备**的条目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedRecord {
    pub record: MemoryRecord,
    pub revoked: bool,
    pub permitted: bool,
    pub approximate_of: Option<String>,
}

impl ScopedRecord {
    /// 一条「干净」的候选：未撤销、有权限、不是顶替条目。
    pub fn admissible(record: MemoryRecord) -> Self {
        Self {
            record,
            revoked: false,
            permitted: true,
            approximate_of: None,
        }
    }
}

/// §42.3: 一次被拒的条目——**哪一条**、**为什么**。两者都必须在场，
/// 否则「静默丢弃」与「具名拒绝」无法区分。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRefusal {
    pub record_ref: String,
    pub reason: MemoryRejection,
}

impl MemoryRefusal {
    pub fn new(record_ref: impl Into<String>, reason: MemoryRejection) -> Self {
        Self {
            record_ref: record_ref.into(),
            reason,
        }
    }
}

/// §42.3: 读取结果。`records` 是**实际进入上下文**的条目，`refused` 是没进去的每一条及其名字。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryReadOutcome {
    pub records: Vec<MemoryRecord>,
    pub refused: Vec<MemoryRefusal>,
}

impl MemoryReadOutcome {
    pub fn refused_for(&self, reason: MemoryRejection) -> Vec<&MemoryRefusal> {
        self.refused
            .iter()
            .filter(|refusal| refusal.reason == reason)
            .collect()
    }
}

/// §42.3: 记录与查询是否同一作用域。观众长期记忆按**观众作用域**隔离，其余按**会话**。
fn scope_matches(record: &MemoryRecord, scope: &MemoryScope) -> bool {
    if record.workspace_id != scope.workspace_id {
        return false;
    }
    if record.kind == VIEWER_KIND {
        !record.viewer_scoped_id.is_empty() && record.viewer_scoped_id == scope.viewer_scoped_id
    } else {
        record.session_id == scope.session_id
    }
}

/// H04 / §42.3: 读取。
///
/// 顺序是刻意的：**先作用域、再时效**（§42.3 的原话），其后是撤销、权限、近似顶替，
/// 最后才轮到配额。请求级违规（作用域不完整、观众记忆未启用/缺授权/缺用途、
/// 要求近似顶替）直接失败；条目级违规进 `refused` 并**具名**。
pub fn read_records(
    records: &[ScopedRecord],
    query: &MemoryQuery,
    policy: &MemoryPolicy,
    now_ms: u64,
) -> Result<MemoryReadOutcome, MemoryRejection> {
    // §42.3: 作用域必须完整——不完整的查询不得靠「就近匹配」拿到别的作用域。
    if !query.scope.is_complete() {
        return Err(MemoryRejection::InvalidScope);
    }
    // §42.3: 不得用近似条目顶替。请求「允许顶替」本身即被拒。
    if query.allow_approximate {
        return Err(MemoryRejection::ApproximateSubstitutionRequested);
    }
    // §42.3: 观众长期记忆默认关闭；读取需授权引用并记录用途。
    if query.kinds.contains(&MemoryKind::Viewer) {
        viewer_memory_gate(policy)?;
        if query
            .authorization_ref
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            return Err(MemoryRejection::ViewerReadWithoutAuthorization);
        }
        if query
            .purpose
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            return Err(MemoryRejection::ViewerReadWithoutPurpose);
        }
    }

    let mut refused = Vec::new();
    let mut eligible: Vec<MemoryRecord> = Vec::new();
    for entry in records {
        let record = &entry.record;
        // 未请求的类别不是「被拒」，只是不在本次读取范围内。
        if !query.kinds.iter().any(|kind| kind.label() == record.kind) {
            continue;
        }
        if !scope_matches(record, &query.scope) {
            refused.push(MemoryRefusal::new(
                record.record_id.clone(),
                MemoryRejection::OutOfScope,
            ));
            continue;
        }
        if record.expires_at_utc_ms <= now_ms {
            refused.push(MemoryRefusal::new(
                record.record_id.clone(),
                MemoryRejection::Expired,
            ));
            continue;
        }
        if entry.revoked {
            refused.push(MemoryRefusal::new(
                record.record_id.clone(),
                MemoryRejection::Revoked,
            ));
            continue;
        }
        if !entry.permitted {
            refused.push(MemoryRefusal::new(
                record.record_id.clone(),
                MemoryRejection::PermissionDenied,
            ));
            continue;
        }
        if entry.approximate_of.is_some() {
            refused.push(MemoryRefusal::new(
                record.record_id.clone(),
                MemoryRejection::ApproximateSubstitution,
            ));
            continue;
        }
        eligible.push(record.clone());
    }

    // §42.3: 超配额按「场次 > 工作 > 观众长期记忆」裁剪，且**裁剪可见**。
    // 保留原序（同优先级先到先得），只把落选的那些按名字记进 `refused`。
    if eligible.len() > query.budget {
        let mut order: Vec<(usize, MemoryKind)> = eligible
            .iter()
            .enumerate()
            .map(|(index, record)| {
                let kind = MemoryKind::parse_label(&record.kind).unwrap_or(MemoryKind::Viewer);
                (index, kind)
            })
            .collect();
        order.sort_by_key(|&(index, kind)| (kind, index));
        let keep: BTreeSet<usize> = order
            .iter()
            .take(query.budget)
            .map(|&(index, _)| index)
            .collect();
        let mut kept = Vec::with_capacity(query.budget);
        for (index, record) in eligible.into_iter().enumerate() {
            if keep.contains(&index) {
                kept.push(record);
            } else {
                refused.push(MemoryRefusal::new(
                    record.record_id.clone(),
                    MemoryRejection::OverBudget,
                ));
            }
        }
        eligible = kept;
    }

    Ok(MemoryReadOutcome {
        records: eligible,
        refused,
    })
}

// ---------------------------------------------------------------------------
// §42.4 遗忘与删除
// ---------------------------------------------------------------------------

/// §42.4: 四类触发共用同一条删除链。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ForgetTrigger {
    /// TTL 到期。
    Ttl,
    /// 事实失效。
    FactInvalidated,
    /// 权限撤销。
    PermissionRevoked,
    /// 用户请求删除。
    UserRequest,
}

impl ForgetTrigger {
    pub const ALL: [ForgetTrigger; 4] = [
        ForgetTrigger::Ttl,
        ForgetTrigger::FactInvalidated,
        ForgetTrigger::PermissionRevoked,
        ForgetTrigger::UserRequest,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ForgetTrigger::Ttl => "ttl",
            ForgetTrigger::FactInvalidated => "fact_invalidated",
            ForgetTrigger::PermissionRevoked => "permission_revoked",
            ForgetTrigger::UserRequest => "user_request",
        }
    }
}

/// §42.4: 清理侧的完成状态。`BlockedThirdParty` 是**如实**的受阻态，不是失败。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurgeState {
    NotStarted,
    InProgress,
    Complete,
    BlockedThirdParty,
}

impl PurgeState {
    /// 全部取值。**长度是类型的一部分**：新增一个取值而此处未列 ⇒ 数组长度不再匹配
    /// `[PurgeState; 4]` ⇒ **编译失败**，而不是让解析静默地不认识它。
    pub const ALL: [PurgeState; 4] = [
        PurgeState::NotStarted,
        PurgeState::InProgress,
        PurgeState::Complete,
        PurgeState::BlockedThirdParty,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PurgeState::NotStarted => "not_started",
            PurgeState::InProgress => "in_progress",
            PurgeState::Complete => "complete",
            PurgeState::BlockedThirdParty => "blocked_third_party",
        }
    }
}

/// §42.4: 一次遗忘的结果陈述。判定的是**陈述是否如实**。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgetOutcome {
    pub trigger: ForgetTrigger,
    pub record_ref: String,
    /// §42.4: 读取侧先失效——顺序不能反。
    pub read_side_blocked: bool,
    pub purge_state: PurgeState,
    /// §24.4: 第三方留存说明；存在留存时必须在使用前明示。
    pub third_party_retention: Option<String>,
    /// §42.4: 是否宣称已完全擦除。
    pub claims_fully_erased: bool,
}

impl ForgetOutcome {
    /// 一个「只阻断了读取、还没开始清理」的最保守陈述。
    pub fn blocked(trigger: ForgetTrigger, record_ref: impl Into<String>) -> Self {
        Self {
            trigger,
            record_ref: record_ref.into(),
            read_side_blocked: true,
            purge_state: PurgeState::NotStarted,
            third_party_retention: None,
            claims_fully_erased: false,
        }
    }
}

/// §42.4: 删除的可验证性。宣称不得超过实际做到的。
pub fn forget(outcome: &ForgetOutcome) -> Result<(), MemoryRejection> {
    // §42.4: 先阻断检索与读取，再异步清除副本——顺序不可交换。
    if !outcome.read_side_blocked {
        return Err(MemoryRejection::ReadSideNotBlocked);
    }
    let disclosed = outcome
        .third_party_retention
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty();
    // §24.4: 受阻于第三方留存时必须明示政策，否则「说不清」会被读成「已清完」。
    if outcome.purge_state == PurgeState::BlockedThirdParty && disclosed {
        return Err(MemoryRejection::ThirdPartyRetentionUndisclosed);
    }
    if outcome.claims_fully_erased {
        if outcome.purge_state != PurgeState::Complete {
            return Err(MemoryRejection::ErasedClaimWithoutCompletePurge);
        }
        if !disclosed {
            return Err(MemoryRejection::ErasedClaimWithThirdPartyRetention);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// §40 INV-32：具名拒绝
// ---------------------------------------------------------------------------

/// 记忆面拒绝的原因。每一种都有稳定名字：静默丢弃与静默裁剪是记忆面最危险的两个失效模式，
/// 故拒绝必须可被分类、可被记进决策日志，而不是一个笼统的「不合规」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MemoryRejection {
    // 写入（§42.2 / H03）
    UntrustedSourceAsFact,
    UnplayedCandidateAsFact,
    WriteWithoutScope,
    WriteWithoutSourceRef,
    WriteWithoutTimestamp,
    FactWithoutPublishRef,
    FactOutsideSemanticMemory,
    // 请求级（§42.3 / H04）
    InvalidScope,
    ViewerMemoryDisabled,
    ViewerReadWithoutAuthorization,
    ViewerReadWithoutPurpose,
    ApproximateSubstitutionRequested,
    UnknownOperation,
    // 条目级（§42.3 / H04）
    OutOfScope,
    Expired,
    Revoked,
    PermissionDenied,
    ApproximateSubstitution,
    OverBudget,
    // 遗忘（§42.4）
    ReadSideNotBlocked,
    ErasedClaimWithoutCompletePurge,
    ErasedClaimWithThirdPartyRetention,
    ThirdPartyRetentionUndisclosed,
}

impl MemoryRejection {
    /// 全部 23 个拒绝名。测试据此断言「构造出的拒绝集合**恰为**全部」——
    /// 既证每条都能被构造，也防出现未登记的名字。
    pub const ALL: [MemoryRejection; 23] = [
        MemoryRejection::UntrustedSourceAsFact,
        MemoryRejection::UnplayedCandidateAsFact,
        MemoryRejection::WriteWithoutScope,
        MemoryRejection::WriteWithoutSourceRef,
        MemoryRejection::WriteWithoutTimestamp,
        MemoryRejection::FactWithoutPublishRef,
        MemoryRejection::FactOutsideSemanticMemory,
        MemoryRejection::InvalidScope,
        MemoryRejection::ViewerMemoryDisabled,
        MemoryRejection::ViewerReadWithoutAuthorization,
        MemoryRejection::ViewerReadWithoutPurpose,
        MemoryRejection::ApproximateSubstitutionRequested,
        MemoryRejection::UnknownOperation,
        MemoryRejection::OutOfScope,
        MemoryRejection::Expired,
        MemoryRejection::Revoked,
        MemoryRejection::PermissionDenied,
        MemoryRejection::ApproximateSubstitution,
        MemoryRejection::OverBudget,
        MemoryRejection::ReadSideNotBlocked,
        MemoryRejection::ErasedClaimWithoutCompletePurge,
        MemoryRejection::ErasedClaimWithThirdPartyRetention,
        MemoryRejection::ThirdPartyRetentionUndisclosed,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MemoryRejection::UntrustedSourceAsFact => "untrusted_source_as_fact",
            MemoryRejection::UnplayedCandidateAsFact => "unplayed_candidate_as_fact",
            MemoryRejection::WriteWithoutScope => "write_without_scope",
            MemoryRejection::WriteWithoutSourceRef => "write_without_source_ref",
            MemoryRejection::WriteWithoutTimestamp => "write_without_timestamp",
            MemoryRejection::FactWithoutPublishRef => "fact_without_publish_ref",
            MemoryRejection::FactOutsideSemanticMemory => "fact_outside_semantic_memory",
            MemoryRejection::InvalidScope => "invalid_scope",
            MemoryRejection::ViewerMemoryDisabled => "viewer_memory_disabled",
            MemoryRejection::ViewerReadWithoutAuthorization => "viewer_read_without_authorization",
            MemoryRejection::ViewerReadWithoutPurpose => "viewer_read_without_purpose",
            MemoryRejection::ApproximateSubstitutionRequested => {
                "approximate_substitution_requested"
            }
            MemoryRejection::UnknownOperation => "unknown_operation",
            MemoryRejection::OutOfScope => "out_of_scope",
            MemoryRejection::Expired => "expired",
            MemoryRejection::Revoked => "revoked",
            MemoryRejection::PermissionDenied => "permission_denied",
            MemoryRejection::ApproximateSubstitution => "approximate_substitution",
            MemoryRejection::OverBudget => "over_budget",
            MemoryRejection::ReadSideNotBlocked => "read_side_not_blocked",
            MemoryRejection::ErasedClaimWithoutCompletePurge => {
                "erased_claim_without_complete_purge"
            }
            MemoryRejection::ErasedClaimWithThirdPartyRetention => {
                "erased_claim_with_third_party_retention"
            }
            MemoryRejection::ThirdPartyRetentionUndisclosed => "third_party_retention_undisclosed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unplayed_content_is_never_working_memory() {
        let records = vec![
            MemoryRecord {
                record_id: "utt-1".into(),
                content: "已播完内容".into(),
                kind: WORKING_KIND.into(),
                purpose: "topic".into(),
                expires_at_utc_ms: 1_000_000_000_000,
                ..Default::default()
            },
            MemoryRecord {
                record_id: "utt-2".into(),
                content: "未播候选".into(),
                kind: WORKING_KIND.into(),
                purpose: "topic".into(),
                expires_at_utc_ms: 1_000_000_000_000,
                ..Default::default()
            },
        ];
        let kept = working_memory(&records, &["utt-1"], 8);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].record_id, "utt-1");
        let bounded = working_memory(&records, &["utt-1"], 1);
        assert_eq!(bounded.len(), 1);
    }

    #[test]
    fn session_summary_is_deterministic_and_deduped() {
        let records = vec![
            MemoryRecord {
                record_id: "a".into(),
                kind: SESSION_KIND.into(),
                purpose: "topic".into(),
                content: "价格".into(),
                ..Default::default()
            },
            MemoryRecord {
                record_id: "b".into(),
                kind: SESSION_KIND.into(),
                purpose: "answered".into(),
                content: "已回答保修期".into(),
                ..Default::default()
            },
            MemoryRecord {
                record_id: "c".into(),
                kind: SESSION_KIND.into(),
                purpose: "recovery_point".into(),
                content: "segment-3".into(),
                ..Default::default()
            },
            MemoryRecord {
                record_id: "d".into(),
                kind: SESSION_KIND.into(),
                purpose: "topic".into(),
                content: "价格".into(),
                ..Default::default()
            },
        ];
        let summary = session_summary(&records, SESSION_KIND, 8);
        assert_eq!(summary.topics, vec!["价格".to_string()]);
        assert_eq!(summary.answered_questions.len(), 1);
        assert_eq!(summary.recovery_point, "segment-3");
    }

    #[test]
    fn viewer_memory_is_off_by_default() {
        let policy = MemoryPolicy::default();
        let request = MemoryRequest {
            kind: VIEWER_KIND.into(),
            purpose: "answered_once".into(),
            viewer_scoped_id: "douyin.app_scope:viewer-1".into(),
            ..Default::default()
        };
        assert_eq!(
            viewer_record_allowed(&request, &policy),
            Err(EvaluateError::Unauthorized)
        );
        let enabled = MemoryPolicy {
            viewer_memory_enabled: true,
            ..Default::default()
        };
        assert_eq!(viewer_record_allowed(&request, &enabled), Ok(()));
        let nickname = MemoryRequest {
            kind: VIEWER_KIND.into(),
            purpose: "answered_once".into(),
            viewer_scoped_id: "小明".into(),
            ..Default::default()
        };
        assert_eq!(
            viewer_record_allowed(&nickname, &enabled),
            Err(EvaluateError::Unauthorized)
        );
        let wrong_scope = MemoryRequest {
            kind: VIEWER_KIND.into(),
            purpose: "answered_once".into(),
            viewer_scoped_id: "other.platform:viewer-1".into(),
            ..Default::default()
        };
        assert_eq!(
            viewer_record_allowed(&wrong_scope, &enabled),
            Err(EvaluateError::Unauthorized)
        );
        let mut cross = request.clone();
        cross.viewer_scoped_id = "douyin.app_scope:viewer@joe".into();
        assert_eq!(
            viewer_record_allowed(&cross, &enabled),
            Err(EvaluateError::Unauthorized)
        );
        let sensitive = MemoryRecord {
            record_id: "r".into(),
            workspace_id: "w".into(),
            session_id: "s".into(),
            kind: VIEWER_KIND.into(),
            purpose: "answered_topic".into(),
            content: "用户手机号被记录".into(),
            viewer_scoped_id: "douyin.app_scope:v1".into(),
            expires_at_utc_ms: 1_000_000_000_000,
            authorization_ref: "auth".into(),
        };
        assert_eq!(
            viewer_record(&sensitive, &enabled, 0),
            Err(EvaluateError::Unauthorized)
        );
    }
}
