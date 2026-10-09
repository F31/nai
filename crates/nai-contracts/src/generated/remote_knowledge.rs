// Generated from schemas/openapi/remote-knowledge.yaml. Do not edit.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyRequest {}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteCapabilities {
    pub capabilities: Vec<RemoteCapability>,
    pub operations: Vec<RemoteOperation>,
    pub retention: RemoteRetention,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RemoteCapability {
    #[serde(rename = "retrieval_with_evidence")]
    RetrievalWithEvidence,
    #[serde(rename = "answer_with_evidence")]
    AnswerWithEvidence,
    #[serde(rename = "answer_only")]
    AnswerOnly,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteDelete {
    pub request_id: String,
    pub idempotency_key: String,
    pub kb_id: String,
    pub document_ids: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteDocument {
    pub document_id: String,
    pub revision: String,
    pub content: String,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteError {
    pub message: String,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteEvidence {
    pub evidence_id: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_span: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from_utc_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_until_utc_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fact_versions: Option<Vec<RemoteRevision>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_verifiable: Option<bool>,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteHealth {
    pub status: String,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RemoteOperation {
    #[serde(rename = "search")]
    Search,
    #[serde(rename = "answer")]
    Answer,
    #[serde(rename = "sync")]
    Sync,
    #[serde(rename = "delete")]
    Delete,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemotePolicy {
    pub source_id: String,
    pub policy_ref: String,
    pub allowed_kb_ids: Vec<String>,
    pub allowed_operations: Vec<RemoteOperation>,
    pub allow_query_upload: bool,
    pub allow_document_upload: bool,
    pub allow_third_party_retention: bool,
    pub max_retention_seconds: u64,
    pub require_remote_delete: bool,
    pub expires_at_utc_ms: String,
    pub permission_revision: String,
    pub unknown_revision_ttl_ms: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteQuery {
    pub request_id: String,
    pub session_id: String,
    pub kb_ids: Vec<String>,
    pub query: String,
    pub filters: std::collections::BTreeMap<String, String>,
    pub top_k: u64,
    pub remaining_budget_ms: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteReceipt {
    pub request_id: String,
    pub status: RemoteReceiptStatus,
    pub policy_ref: String,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RemoteReceiptStatus {
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "deleted")]
    Deleted,
    #[serde(rename = "retained")]
    Retained,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteReply {
    pub status: RemoteStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<RemoteCapability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    pub evidence: Vec<RemoteEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer_draft: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<RemoteUsage>,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteRetention {
    pub policy_ref: String,
    pub retention_seconds: u64,
    pub supports_delete: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteRevision {
    pub object_id: String,
    pub revision: String,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RemoteStatus {
    #[serde(rename = "complete")]
    Complete,
    #[serde(rename = "partial")]
    Partial,
    #[serde(rename = "empty")]
    Empty,
    #[serde(rename = "timeout")]
    Timeout,
    #[serde(rename = "unavailable")]
    Unavailable,
    #[serde(rename = "unauthorized")]
    Unauthorized,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteSync {
    pub request_id: String,
    pub idempotency_key: String,
    pub kb_id: String,
    pub documents: Vec<RemoteDocument>,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteUsage {
    pub unit: String,
    pub quantity: u64,
}
pub const SCHEMAS_JSON: &str = r###"{"$schema":"https://json-schema.org/draft/2020-12/schema","$defs":{"EmptyRequest":{"type":"object","additionalProperties":false,"properties":{}},"RemoteOperation":{"type":"string","enum":["search","answer","sync","delete"]},"RemoteCapability":{"type":"string","enum":["retrieval_with_evidence","answer_with_evidence","answer_only"]},"RemoteStatus":{"type":"string","enum":["complete","partial","empty","timeout","unavailable","unauthorized"]},"RemoteReceiptStatus":{"type":"string","enum":["accepted","pending","deleted","retained"]},"RemoteError":{"type":"object","additionalProperties":false,"required":["message"],"properties":{"message":{"type":"string","maxLength":2048}}},"RemoteRetention":{"type":"object","additionalProperties":false,"required":["policy_ref","retention_seconds","supports_delete"],"properties":{"policy_ref":{"type":"string","minLength":1,"maxLength":256},"retention_seconds":{"type":"integer","minimum":0,"maximum":31536000},"supports_delete":{"type":"boolean"}}},"RemoteCapabilities":{"type":"object","additionalProperties":false,"required":["capabilities","operations","retention"],"properties":{"capabilities":{"type":"array","minItems":1,"maxItems":3,"uniqueItems":true,"items":{"$ref":"#/$defs/RemoteCapability"}},"operations":{"type":"array","minItems":1,"maxItems":4,"uniqueItems":true,"items":{"$ref":"#/$defs/RemoteOperation"}},"retention":{"$ref":"#/$defs/RemoteRetention"}}},"RemoteHealth":{"type":"object","additionalProperties":false,"required":["status"],"properties":{"status":{"type":"string","enum":["ok","unavailable"]}}},"RemoteQuery":{"type":"object","additionalProperties":false,"required":["request_id","session_id","kb_ids","query","filters","top_k","remaining_budget_ms"],"properties":{"request_id":{"type":"string","minLength":1,"maxLength":256},"session_id":{"type":"string","minLength":1,"maxLength":256},"kb_ids":{"type":"array","minItems":1,"maxItems":64,"uniqueItems":true,"items":{"type":"string","minLength":1,"maxLength":256}},"query":{"type":"string","minLength":1,"maxLength":16384},"filters":{"type":"object","maxProperties":64,"additionalProperties":{"type":"string","maxLength":2048}},"top_k":{"type":"integer","minimum":1,"maximum":64},"remaining_budget_ms":{"type":"integer","minimum":1,"maximum":30000}}},"RemoteRevision":{"type":"object","additionalProperties":false,"required":["object_id","revision"],"properties":{"object_id":{"type":"string","minLength":1,"maxLength":256},"revision":{"type":"string","minLength":1,"maxLength":256}}},"RemoteEvidence":{"type":"object","additionalProperties":false,"required":["evidence_id","text"],"properties":{"evidence_id":{"type":"string","minLength":1,"maxLength":256},"text":{"type":"string","minLength":1,"maxLength":65536},"document_id":{"type":"string","maxLength":256},"revision":{"type":"string","maxLength":256},"chunk_id":{"type":"string","maxLength":256},"source_ref":{"type":"string","maxLength":2048},"source_span":{"type":"string","maxLength":256},"product_id":{"type":"string","maxLength":256},"rank":{"type":"integer","minimum":0,"maximum":4294967295},"valid_from_utc_ms":{"anyOf":[{"type":"integer","minimum":0,"maximum":9007199254740991},{"type":"null"}]},"valid_until_utc_ms":{"anyOf":[{"type":"integer","minimum":0,"maximum":9007199254740991},{"type":"null"}]},"permission_revision":{"type":"string","maxLength":256},"authority":{"type":"string","maxLength":256},"fact_versions":{"type":"array","maxItems":32,"items":{"$ref":"#/$defs/RemoteRevision"}},"original_verifiable":{"type":"boolean"}}},"RemoteUsage":{"type":"object","additionalProperties":false,"required":["unit","quantity"],"properties":{"unit":{"type":"string","minLength":1,"maxLength":64},"quantity":{"type":"integer","minimum":0,"maximum":9007199254740991}}},"RemoteReply":{"type":"object","additionalProperties":false,"required":["status","evidence"],"properties":{"status":{"$ref":"#/$defs/RemoteStatus"},"capability":{"$ref":"#/$defs/RemoteCapability"},"provider_id":{"type":"string","maxLength":256},"evidence":{"type":"array","maxItems":64,"items":{"$ref":"#/$defs/RemoteEvidence"}},"answer_draft":{"anyOf":[{"type":"string","maxLength":65536},{"type":"null"}]},"usage":{"$ref":"#/$defs/RemoteUsage"}}},"RemoteDocument":{"type":"object","additionalProperties":false,"required":["document_id","revision","content"],"properties":{"document_id":{"type":"string","minLength":1,"maxLength":256},"revision":{"type":"string","minLength":1,"maxLength":256},"content":{"type":"string","minLength":1,"maxLength":65536}}},"RemoteSync":{"type":"object","additionalProperties":false,"required":["request_id","idempotency_key","kb_id","documents"],"properties":{"request_id":{"type":"string","minLength":1,"maxLength":256},"idempotency_key":{"type":"string","minLength":1,"maxLength":256},"kb_id":{"type":"string","minLength":1,"maxLength":256},"documents":{"type":"array","minItems":1,"maxItems":16,"items":{"$ref":"#/$defs/RemoteDocument"}}}},"RemoteDelete":{"type":"object","additionalProperties":false,"required":["request_id","idempotency_key","kb_id","document_ids"],"properties":{"request_id":{"type":"string","minLength":1,"maxLength":256},"idempotency_key":{"type":"string","minLength":1,"maxLength":256},"kb_id":{"type":"string","minLength":1,"maxLength":256},"document_ids":{"type":"array","minItems":1,"maxItems":16,"uniqueItems":true,"items":{"type":"string","minLength":1,"maxLength":256}}}},"RemoteReceipt":{"type":"object","additionalProperties":false,"required":["request_id","status","policy_ref"],"properties":{"request_id":{"type":"string","minLength":1,"maxLength":256},"status":{"$ref":"#/$defs/RemoteReceiptStatus"},"policy_ref":{"type":"string","minLength":1,"maxLength":256}}},"RemotePolicy":{"type":"object","additionalProperties":false,"description":"Injected by trusted composition root, never inferred from server capabilities or model claims.","required":["source_id","policy_ref","allowed_kb_ids","allowed_operations","allow_query_upload","allow_document_upload","allow_third_party_retention","max_retention_seconds","require_remote_delete","expires_at_utc_ms","permission_revision","unknown_revision_ttl_ms"],"properties":{"source_id":{"type":"string","minLength":1,"maxLength":64,"pattern":"^[A-Za-z0-9_.-]+$"},"policy_ref":{"type":"string","minLength":1,"maxLength":256},"allowed_kb_ids":{"type":"array","minItems":1,"maxItems":64,"uniqueItems":true,"items":{"type":"string","minLength":1,"maxLength":256}},"allowed_operations":{"type":"array","maxItems":4,"uniqueItems":true,"items":{"$ref":"#/$defs/RemoteOperation"}},"allow_query_upload":{"type":"boolean"},"allow_document_upload":{"type":"boolean"},"allow_third_party_retention":{"type":"boolean"},"max_retention_seconds":{"type":"integer","minimum":0,"maximum":31536000},"require_remote_delete":{"type":"boolean"},"expires_at_utc_ms":{"type":"string","pattern":"^[0-9]{1,19}$"},"permission_revision":{"type":"string","minLength":1,"maxLength":256},"unknown_revision_ttl_ms":{"type":"integer","minimum":1,"maximum":5000}}}}}"###;
pub const CAPABILITIES_PATH: &str = "/capabilities";
pub const CAPABILITIES_METHOD: &str = "POST";
pub const HEALTH_PATH: &str = "/health";
pub const HEALTH_METHOD: &str = "POST";
pub const SEARCH_PATH: &str = "/search";
pub const SEARCH_METHOD: &str = "POST";
pub const ANSWER_PATH: &str = "/answer";
pub const ANSWER_METHOD: &str = "POST";
pub const SYNC_PATH: &str = "/sync";
pub const SYNC_METHOD: &str = "POST";
pub const DELETE_PATH: &str = "/delete";
pub const DELETE_METHOD: &str = "POST";
