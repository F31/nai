//! Versioned contract boundary for nai.
//!
//! DTOs are generated from schemas/core and schemas/domain. Validation here is
//! structural only; authorization, factual validity and playback are runtime work.

pub mod v1 {
    include!("generated/nai.v1.rs");
}

pub mod remote_knowledge {
    //! Generated adapter DTOs and endpoint constants (remote-knowledge.yaml).
    include!("generated/remote_knowledge.rs");
}

pub mod control_http {
    //! Generated validation schemas from control.yaml.
    include!("generated/control_http.rs");
}

include!("generated/catalog.rs");
mod validation;
pub use validation::{MAX_CONTROL_BYTES, decode_envelope, negotiate, validate_envelope};

/// sha256-canonical-v1: generated maps are BTreeMap, so encoding is stable.
/// Approval reference and hash itself are excluded from the approved object.
pub fn candidate_hash(cue: &v1::PerformanceCue) -> String {
    use prost::Message;
    use sha2::Digest;
    let mut cue = cue.clone();
    cue.candidate_hash.clear();
    cue.approval_ref.clear();
    cue.hash_algorithm = "sha256-canonical-v1".into();
    sha2::Sha256::digest(cue.encode_to_vec())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
