// Generated from schemas/openapi/cosyvoice-stream.yaml. Do not edit.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Completion {
    pub protocol: String,
    pub request_id: String,
    pub instance_id: String,
    pub state: String,
    pub samples: u64,
    pub sha256: String,
}
