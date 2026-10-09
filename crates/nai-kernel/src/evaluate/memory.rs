//! Memory evaluation (V3.5 §23.5): working memory is bounded and only holds
//! actually-played content, session summaries are deterministically extracted,
//! and viewer long-term memory is off by default with scope/purpose/authorization/
//! retention/delete discipline.
use super::EvaluateError;
use nai_contracts::v1::{MemoryRecord, MemoryRequest};
use std::collections::BTreeSet;

pub const WORKING_KIND: &str = "working";
pub const SESSION_KIND: &str = "session";
pub const VIEWER_KIND: &str = "viewer";

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

/// Viewer long-term memory is off by default. Enabling/reading requires a
/// scoped ID (never a nickname), an allowed purpose, authorization and retention.
pub fn viewer_record_allowed(
    request: &MemoryRequest,
    policy: &MemoryPolicy,
) -> Result<(), EvaluateError> {
    if !policy.viewer_memory_enabled {
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
