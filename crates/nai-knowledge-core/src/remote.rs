//! Remote knowledge API client and local policy gateway (V3.5 §25.3-25.4).
//! Capabilities/health/search are required; answer/sync/delete are optional.
//! Local policy is injected by the trusted composition root and gates every
//! call: unknown operations are not granted, restricted documents never leave
//! the local machine, and answer_only stays a human reference. An optional
//! rerank only reorders already-returned evidence within a sub-budget.
use nai_contracts::remote_knowledge::{
    RemoteCapabilities, RemoteCapability, RemoteDelete, RemoteDocument, RemoteHealth,
    RemoteOperation, RemotePolicy, RemoteQuery, RemoteReceipt, RemoteReply, RemoteStatus,
    RemoteSync,
};
use nai_contracts::v1::{Evidence, RetrievalStatus};
use nai_security::{Operation, Secret};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteError {
    Unauthorized,
    Unavailable,
    Timeout,
    Cancelled,
    Invalid,
}
impl fmt::Display for RemoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "remote: {self:?}")
    }
}
impl Error for RemoteError {}
impl RemoteError {
    pub fn status(&self) -> RetrievalStatus {
        match self {
            Self::Unauthorized => RetrievalStatus::RetrievalUnauthorized,
            Self::Unavailable => RetrievalStatus::RetrievalUnavailable,
            Self::Timeout | Self::Cancelled => RetrievalStatus::RetrievalTimeout,
            Self::Invalid => RetrievalStatus::RetrievalUnavailable,
        }
    }
}

enum WireResult {
    Body(Vec<u8>),
    Http { status: u16, retry_after: u64 },
}

pub struct RemoteKnowledgeClient {
    http: reqwest::Client,
    endpoint: String,
    api_key: Option<Secret>,
    timeout: Duration,
    max_retries: u32,
}
impl RemoteKnowledgeClient {
    pub fn new(
        endpoint: String,
        api_key: Option<Secret>,
        timeout: Duration,
        max_retries: u32,
    ) -> Result<Self, RemoteError> {
        if endpoint.is_empty()
            || endpoint.len() > 2048
            || timeout.is_zero()
            || timeout > Duration::from_secs(600)
            || max_retries > 10
        {
            return Err(RemoteError::Invalid);
        }
        let http = reqwest::Client::builder()
            .connect_timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| RemoteError::Unavailable)?;
        Ok(Self {
            http,
            endpoint,
            api_key,
            timeout,
            max_retries,
        })
    }

    fn check(op: &Operation) -> Result<(), RemoteError> {
        match op.check() {
            Ok(()) => Ok(()),
            Err(error) => Err(match error.code {
                nai_contracts::v1::ErrorCode::Cancelled => RemoteError::Cancelled,
                nai_contracts::v1::ErrorCode::DeadlineExceeded => RemoteError::Timeout,
                _ => RemoteError::Invalid,
            }),
        }
    }

    fn request(&self, path: &str, body: &Value) -> reqwest::RequestBuilder {
        let mut request = self
            .http
            .post(format!("{}{}", self.endpoint.trim_end_matches('/'), path))
            .json(body);
        if let Some(secret) = &self.api_key {
            request = request.header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", String::from_utf8_lossy(secret.as_bytes())),
            );
        }
        request
    }

    async fn read_body(
        &self,
        op: &Operation,
        mut response: reqwest::Response,
    ) -> Result<Vec<u8>, RemoteError> {
        let mut bytes = Vec::new();
        loop {
            let reading = response.chunk();
            tokio::pin!(reading);
            let chunk = loop {
                Self::check(op)?;
                tokio::select! {
                    chunk = &mut reading => break chunk.map_err(|_| RemoteError::Unavailable)?,
                    _ = sleep(Duration::from_millis(10).min(op.remaining())) => {}
                }
            };
            let Some(chunk) = chunk else {
                break;
            };
            if bytes.len() + chunk.len() > 8 * 1024 * 1024 {
                return Err(RemoteError::Invalid);
            }
            bytes.extend_from_slice(&chunk);
        }
        Self::check(op)?;
        Ok(bytes)
    }

    async fn wire(
        &self,
        op: &Operation,
        request: reqwest::RequestBuilder,
    ) -> Result<WireResult, RemoteError> {
        let sending = request.send();
        tokio::pin!(sending);
        let response = loop {
            Self::check(op)?;
            tokio::select! {
                response = &mut sending => break response,
                _ = sleep(Duration::from_millis(10).min(op.remaining())) => {}
            }
        }
        .map_err(|error| {
            if error.is_timeout() {
                if op.stopped() {
                    RemoteError::Cancelled
                } else {
                    RemoteError::Timeout
                }
            } else {
                RemoteError::Unavailable
            }
        })?;
        let status = response.status().as_u16();
        if (200..300).contains(&status) {
            return Ok(WireResult::Body(self.read_body(op, response).await?));
        }
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        Ok(WireResult::Http {
            status,
            retry_after,
        })
    }

    async fn sleep_until(&self, op: &Operation, target: Instant) -> Result<(), RemoteError> {
        loop {
            Self::check(op)?;
            let now = Instant::now();
            if now >= target {
                return Ok(());
            }
            sleep(Duration::from_millis(50).min(target - now)).await;
        }
    }

    /// Bounded retry for idempotent read operations. Returns Err on non-2xx.
    async fn post_json(
        &self,
        op: &Operation,
        path: &str,
        body: &Value,
    ) -> Result<Value, RemoteError> {
        let mut attempt = 0u32;
        loop {
            Self::check(op)?;
            let request = self
                .request(path, body)
                .timeout(op.remaining().min(self.timeout));
            match self.wire(op, request).await {
                Ok(WireResult::Body(bytes)) => {
                    return serde_json::from_slice(&bytes).map_err(|_| RemoteError::Invalid);
                }
                Ok(WireResult::Http {
                    status: 401 | 403, ..
                }) => {
                    return Err(RemoteError::Unauthorized);
                }
                Ok(WireResult::Http {
                    status,
                    retry_after,
                }) => {
                    if attempt >= self.max_retries
                        || !(status == 429 || (500..600).contains(&status))
                    {
                        return Err(RemoteError::Unavailable);
                    }
                    let backoff = if status == 429 {
                        Duration::from_secs(retry_after)
                    } else {
                        Duration::from_millis(200)
                            .checked_mul(1u32 << attempt.min(8))
                            .unwrap_or(Duration::from_secs(1))
                    };
                    if backoff >= op.remaining() {
                        return Err(RemoteError::Unavailable);
                    }
                    self.sleep_until(op, Instant::now() + backoff).await?;
                    attempt += 1;
                }
                Err(error) => return Err(error),
            }
        }
    }

    /// Single attempt for non-idempotent sync/delete. Unknown submissions must
    /// never be silently repeated.
    async fn post_json_once(
        &self,
        op: &Operation,
        path: &str,
        body: &Value,
    ) -> Result<Value, RemoteError> {
        Self::check(op)?;
        let request = self
            .request(path, body)
            .timeout(op.remaining().min(self.timeout));
        match self.wire(op, request).await {
            Ok(WireResult::Body(bytes)) => {
                serde_json::from_slice(&bytes).map_err(|_| RemoteError::Invalid)
            }
            Ok(WireResult::Http {
                status: 401 | 403, ..
            }) => Err(RemoteError::Unauthorized),
            Ok(WireResult::Http { .. }) => Err(RemoteError::Unavailable),
            Err(error) => Err(error),
        }
    }

    fn parse<T: DeserializeOwned>(&self, op: &Operation, value: Value) -> Result<T, RemoteError> {
        Self::check(op)?;
        serde_json::from_value(value).map_err(|_| RemoteError::Invalid)
    }

    pub async fn capabilities(&self, op: &Operation) -> Result<RemoteCapabilities, RemoteError> {
        let body = self
            .post_json(op, "/capabilities", &Value::Object(Default::default()))
            .await?;
        self.parse(op, body)
    }

    pub async fn health(&self, op: &Operation) -> Result<bool, RemoteError> {
        let body = self
            .post_json(op, "/health", &Value::Object(Default::default()))
            .await?;
        let health: RemoteHealth = self.parse(op, body)?;
        Ok(health.status == "ok")
    }

    /// Required search endpoint; status is independent of HTTP success.
    #[allow(clippy::too_many_arguments)]
    pub async fn search(
        &self,
        op: &Operation,
        request_id: &str,
        session_id: &str,
        kb_ids: &[String],
        query: &str,
        filters: &BTreeMap<String, String>,
        top_k: u32,
    ) -> Result<(RetrievalStatus, Vec<Evidence>), RemoteError> {
        let query = RemoteQuery {
            request_id: request_id.into(),
            session_id: session_id.into(),
            kb_ids: kb_ids.to_vec(),
            query: query.into(),
            filters: filters.clone(),
            top_k: top_k as u64,
            remaining_budget_ms: op.remaining().as_millis() as u64,
        };
        let value = match self
            .post_json(
                op,
                "/search",
                &serde_json::to_value(query).map_err(|_| RemoteError::Invalid)?,
            )
            .await
        {
            Ok(value) => value,
            Err(RemoteError::Unauthorized) => {
                return Ok((RetrievalStatus::RetrievalUnauthorized, Vec::new()));
            }
            Err(RemoteError::Unavailable) => {
                return Ok((RetrievalStatus::RetrievalUnavailable, Vec::new()));
            }
            Err(RemoteError::Timeout | RemoteError::Cancelled) => {
                return Ok((RetrievalStatus::RetrievalTimeout, Vec::new()));
            }
            Err(RemoteError::Invalid) => return Err(RemoteError::Invalid),
        };
        let reply: RemoteReply = self.parse(op, value)?;
        Ok(parse_reply(reply, top_k as usize))
    }

    /// Optional answer endpoint returning the full reply so answer_only remains
    /// a human reference instead of auto-issued output.
    #[allow(clippy::too_many_arguments)]
    pub async fn answer(
        &self,
        op: &Operation,
        request_id: &str,
        session_id: &str,
        kb_ids: &[String],
        query: &str,
        filters: &BTreeMap<String, String>,
        top_k: u32,
    ) -> Result<RemoteReply, RemoteError> {
        let query = RemoteQuery {
            request_id: request_id.into(),
            session_id: session_id.into(),
            kb_ids: kb_ids.to_vec(),
            query: query.into(),
            filters: filters.clone(),
            top_k: top_k as u64,
            remaining_budget_ms: op.remaining().as_millis() as u64,
        };
        let value = self
            .post_json(
                op,
                "/answer",
                &serde_json::to_value(query).map_err(|_| RemoteError::Invalid)?,
            )
            .await?;
        self.parse(op, value)
    }

    pub async fn sync(
        &self,
        op: &Operation,
        request: &RemoteSync,
    ) -> Result<RemoteReceipt, RemoteError> {
        let value = self
            .post_json_once(
                op,
                "/sync",
                &serde_json::to_value(request).map_err(|_| RemoteError::Invalid)?,
            )
            .await?;
        self.parse(op, value)
    }

    pub async fn delete(
        &self,
        op: &Operation,
        request: &RemoteDelete,
    ) -> Result<RemoteReceipt, RemoteError> {
        let value = self
            .post_json_once(
                op,
                "/delete",
                &serde_json::to_value(request).map_err(|_| RemoteError::Invalid)?,
            )
            .await?;
        self.parse(op, value)
    }
}

fn parse_reply(reply: RemoteReply, top_k: usize) -> (RetrievalStatus, Vec<Evidence>) {
    let status = match reply.status {
        RemoteStatus::Complete => RetrievalStatus::RetrievalComplete,
        RemoteStatus::Partial => RetrievalStatus::RetrievalPartial,
        RemoteStatus::Empty => RetrievalStatus::RetrievalEmpty,
        RemoteStatus::Timeout => RetrievalStatus::RetrievalTimeout,
        RemoteStatus::Unavailable => RetrievalStatus::RetrievalUnavailable,
        RemoteStatus::Unauthorized => RetrievalStatus::RetrievalUnauthorized,
    };
    let evidence = reply
        .evidence
        .into_iter()
        .filter_map(to_evidence)
        .take(top_k)
        .collect();
    (status, evidence)
}

fn to_evidence(value: nai_contracts::remote_knowledge::RemoteEvidence) -> Option<Evidence> {
    if value.evidence_id.is_empty() {
        return None;
    }
    Some(Evidence {
        evidence_id: value.evidence_id,
        text: value.text,
        document_id: value.document_id.unwrap_or_default(),
        revision: value.revision.unwrap_or_default(),
        chunk_id: value.chunk_id.unwrap_or_default(),
        source_ref: value.source_ref.unwrap_or_default(),
        source_span: value.source_span.unwrap_or_default(),
        product_id: value.product_id.unwrap_or_default(),
        rank: value.rank.unwrap_or_default() as u32,
        valid_from_utc_ms: value.valid_from_utc_ms,
        valid_until_utc_ms: value.valid_until_utc_ms,
        permission_revision: value.permission_revision.unwrap_or_default(),
        authority: value.authority.unwrap_or_default(),
        fact_versions: value
            .fact_versions
            .unwrap_or_default()
            .into_iter()
            .map(|item| nai_contracts::v1::RevisionRef {
                object_id: item.object_id,
                revision: item.revision,
            })
            .collect(),
        original_verifiable: value.original_verifiable.unwrap_or(false),
    })
}

/// Optional rerank result. `partial` means the rerank sub-budget expired and the
/// already-completed base evidence was preserved.
pub struct RerankOutcome {
    pub status: RetrievalStatus,
    pub evidence: Vec<Evidence>,
}

/// Rerank may only reorder evidence already returned by search/answer. Unknown
/// IDs are dropped (never injected) and the whole operation is bounded: a
/// sub-budget expiry preserves the base as partial, while the caller's
/// Operation cancellation/timeout still stops processing.
pub async fn rerank_evidence(
    op: &Operation,
    base_status: RetrievalStatus,
    base: Vec<Evidence>,
    sub_budget: Duration,
    rerank: &Reranker,
) -> Result<RerankOutcome, RemoteError> {
    RemoteKnowledgeClient::check(op)?;
    if base.is_empty() {
        return Ok(RerankOutcome {
            status: RetrievalStatus::RetrievalEmpty,
            evidence: Vec::new(),
        });
    }
    let known = base
        .iter()
        .map(|item| item.evidence_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let input = base;
    let preserved = input.clone();
    let rerank = Arc::clone(rerank);
    let reranked = tokio::task::spawn_blocking(move || rerank(input));
    let reranked = match tokio::time::timeout(sub_budget, reranked).await {
        Ok(result) => result.map_err(|_| RemoteError::Invalid)?,
        Err(_) => {
            if op.stopped() {
                return Err(RemoteError::Cancelled);
            }
            return Ok(RerankOutcome {
                status: RetrievalStatus::RetrievalPartial,
                evidence: preserved,
            });
        }
    };
    RemoteKnowledgeClient::check(op)?;
    let mut seen = std::collections::BTreeSet::new();
    let evidence = reranked
        .into_iter()
        .filter(|item| known.contains(&item.evidence_id))
        .filter(|item| seen.insert(item.evidence_id.clone()))
        .collect();
    Ok(RerankOutcome {
        status: base_status,
        evidence,
    })
}

/// Optional rerank function: reorder evidence already returned by search/answer.
pub type Reranker = Arc<dyn Fn(Vec<Evidence>) -> Vec<Evidence> + Send + Sync>;

/// Local policy gate (V3.5 §25.3). Policy is injected by the composition root,
/// never derived from server capabilities or model claims.
pub struct RemoteGateway {
    client: RemoteKnowledgeClient,
    policy: RemotePolicy,
    rerank: Option<Reranker>,
}
impl RemoteGateway {
    pub fn new(
        client: RemoteKnowledgeClient,
        policy: RemotePolicy,
        rerank: Option<Reranker>,
    ) -> Result<Self, RemoteError> {
        if policy.source_id.is_empty()
            || policy.policy_ref.is_empty()
            || policy.permission_revision.is_empty()
            || policy.allowed_kb_ids.is_empty()
            || policy.unknown_revision_ttl_ms == 0
            || policy.unknown_revision_ttl_ms > 5000
        {
            return Err(RemoteError::Invalid);
        }
        Ok(Self {
            client,
            policy,
            rerank,
        })
    }

    pub fn policy(&self) -> &RemotePolicy {
        &self.policy
    }

    fn gate(
        &self,
        op: &Operation,
        operation: RemoteOperation,
        kb_ids: &[String],
        query_upload: bool,
    ) -> Result<(), RemoteError> {
        RemoteKnowledgeClient::check(op)?;
        let now = crate::utc_ms(op)?;
        let expires = self
            .policy
            .expires_at_utc_ms
            .parse::<u64>()
            .map_err(|_| RemoteError::Invalid)?;
        if now >= expires {
            return Err(RemoteError::Unauthorized);
        }
        if !self.policy.allowed_operations.contains(&operation) {
            return Err(RemoteError::Unauthorized);
        }
        if query_upload && !self.policy.allow_query_upload {
            return Err(RemoteError::Unauthorized);
        }
        if kb_ids.is_empty()
            || kb_ids
                .iter()
                .any(|kb| !self.policy.allowed_kb_ids.contains(kb))
        {
            return Err(RemoteError::Unauthorized);
        }
        Ok(())
    }

    pub async fn capabilities(&self, op: &Operation) -> Result<RemoteCapabilities, RemoteError> {
        RemoteKnowledgeClient::check(op)?;
        let allowed = matches!(
            self.policy.allowed_operations.first(),
            Some(RemoteOperation::Search) | Some(RemoteOperation::Answer)
        );
        if !allowed {
            return Err(RemoteError::Unauthorized);
        }
        self.client.capabilities(op).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn search(
        &self,
        op: &Operation,
        request_id: &str,
        session_id: &str,
        kb_ids: &[String],
        query: &str,
        filters: &BTreeMap<String, String>,
        top_k: u32,
    ) -> Result<(RetrievalStatus, Vec<Evidence>), RemoteError> {
        self.gate(op, RemoteOperation::Search, kb_ids, !query.is_empty())?;
        let (status, evidence) = self
            .client
            .search(op, request_id, session_id, kb_ids, query, filters, top_k)
            .await?;
        let Some(rerank) = &self.rerank else {
            return Ok((status, evidence));
        };
        let budget = op.remaining().min(Duration::from_millis(500));
        let outcome = rerank_evidence(op, status, evidence, budget, rerank).await?;
        Ok((outcome.status, outcome.evidence))
    }

    /// Answer-only must remain a human reference and never auto-issued output.
    #[allow(clippy::too_many_arguments)]
    pub async fn answer(
        &self,
        op: &Operation,
        request_id: &str,
        session_id: &str,
        kb_ids: &[String],
        query: &str,
        filters: &BTreeMap<String, String>,
        top_k: u32,
    ) -> Result<(RetrievalStatus, Vec<Evidence>, Option<String>, bool), RemoteError> {
        self.gate(op, RemoteOperation::Answer, kb_ids, !query.is_empty())?;
        let reply = self
            .client
            .answer(op, request_id, session_id, kb_ids, query, filters, top_k)
            .await?;
        let (status, evidence) = parse_reply(reply.clone(), top_k as usize);
        let answer_only = reply.capability == Some(RemoteCapability::AnswerOnly);
        Ok((status, evidence, reply.answer_draft, answer_only))
    }

    /// Upload is a write operation and must be explicitly allowed and bounded.
    pub async fn sync(
        &self,
        op: &Operation,
        kb_id: &str,
        request_id: &str,
        idempotency_key: &str,
        documents: Vec<RemoteDocument>,
    ) -> Result<RemoteReceipt, RemoteError> {
        self.gate(op, RemoteOperation::Sync, &[kb_id.to_string()], false)?;
        if !self.policy.allow_document_upload
            || !self.policy.allow_third_party_retention
            || documents.is_empty()
            || documents.len() > 16
            || documents
                .iter()
                .any(|document| document.content.len() > 65536)
        {
            return Err(RemoteError::Unauthorized);
        }
        let request = RemoteSync {
            request_id: request_id.into(),
            idempotency_key: idempotency_key.into(),
            kb_id: kb_id.into(),
            documents,
        };
        self.client.sync(op, &request).await
    }

    /// Delete must follow a local tombstone; the receipt is remote acknowledgment,
    /// never proof that third-party copies were erased.
    pub async fn delete(
        &self,
        op: &Operation,
        kb_id: &str,
        request_id: &str,
        idempotency_key: &str,
        document_ids: Vec<String>,
    ) -> Result<RemoteReceipt, RemoteError> {
        self.gate(op, RemoteOperation::Delete, &[kb_id.to_string()], false)?;
        if !self.policy.require_remote_delete
            || document_ids.is_empty()
            || document_ids.len() > 16
            || document_ids.iter().any(|document| document.is_empty())
        {
            return Err(RemoteError::Unauthorized);
        }
        let request = RemoteDelete {
            request_id: request_id.into(),
            idempotency_key: idempotency_key.into(),
            kb_id: kb_id.into(),
            document_ids,
        };
        self.client.delete(op, &request).await
    }
}
