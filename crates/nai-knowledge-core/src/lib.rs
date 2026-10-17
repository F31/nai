//! Portable knowledge algorithms: Chinese lexical tokens, document chunking,
//! vector-space identity/top-k, fused ranking and the remote knowledge API.
//! No Python sidecar and no provider/device code here.
pub mod chinese;
pub mod chunk;
pub mod egress;
pub mod rank;
pub mod remote;
pub mod vector;

use nai_contracts::v1::ErrorCode;
use nai_security::Operation;

pub use chinese::{MAX_TOKENS_PER_TEXT, tokenize};
pub use chunk::{Chunk, ChunkError, split_document};
pub use egress::{EgressCandidate, EgressError, EgressPlan, EgressWithholding, plan_egress};
pub use rank::fuse;
pub use remote::{
    RemoteError, RemoteGateway, RemoteKnowledgeClient, RerankOutcome, rerank_evidence,
};
pub use vector::{VectorIdentity, VectorSpace};

pub(crate) fn utc_ms(op: &Operation) -> Result<u64, RemoteError> {
    op.check().map_err(|error| match error.code {
        ErrorCode::Cancelled => RemoteError::Cancelled,
        ErrorCode::DeadlineExceeded => RemoteError::Timeout,
        _ => RemoteError::Invalid,
    })?;
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .map_err(|_| RemoteError::Invalid)
}
