//! Signature verification seam. The upgrade core never owns a key or a CA: it
//! asks a verifier whether the detached signature over the manifest is valid.
use crate::{Result, error};
use nai_contracts::v1::ErrorCode;
use sha2::{Digest, Sha256};

pub trait SignatureVerifier {
    /// `subject` is the exact byte range that was signed (the manifest bytes),
    /// `signature` the detached signature, and `signer_ref` the declared signer
    /// identity from the manifest. Implementations MUST return an error rather
    /// than a default-allow when they cannot reach a decision.
    fn verify(&self, subject: &[u8], signature: &[u8], signer_ref: &str) -> Result<()>;
}

/// Rehearsal-only verifier: the "signature" is `SHA256(subject)`.
///
/// This proves the verification *seam* is wired — a wrong signature stops
/// staging — and nothing else. It is **not** a real signature chain and MUST
/// NOT be used to certify provenance. A verifier that chains to a CA or an
/// Ed25519 key is a deliberate later injection point, and the upgrade report
/// records that no real chain was exercised.
pub struct DeterministicVerifier;

impl DeterministicVerifier {
    /// The rehearsal signature for `subject`, for portable fixtures only.
    pub fn sign(subject: &[u8]) -> Vec<u8> {
        Sha256::digest(subject).to_vec()
    }
}

impl SignatureVerifier for DeterministicVerifier {
    fn verify(&self, subject: &[u8], signature: &[u8], signer_ref: &str) -> Result<()> {
        if signer_ref.trim().is_empty() || signer_ref.len() > 128 {
            return Err(error(
                ErrorCode::InvalidArgument,
                "invalid signer reference",
            ));
        }
        let expected = Self::sign(subject);
        if !constant_time_eq(signature, &expected) {
            return Err(error(ErrorCode::PermissionDenied, "signature invalid"));
        }
        Ok(())
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (a, b) in left.iter().zip(right) {
        difference |= a ^ b;
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_signature_passes_and_wrong_one_is_refused() {
        let verifier = DeterministicVerifier;
        let subject = b"manifest-bytes";
        let signature = DeterministicVerifier::sign(subject);
        verifier.verify(subject, &signature, "signer-1").unwrap();
        // a signature over different bytes is refused
        assert_eq!(
            verifier
                .verify(b"other", &signature, "signer-1")
                .unwrap_err()
                .code,
            ErrorCode::PermissionDenied
        );
        // a truncated signature is refused
        assert!(
            verifier
                .verify(subject, &signature[..31], "signer-1")
                .is_err()
        );
        // an empty signer reference is refused
        assert!(verifier.verify(subject, &signature, "  ").is_err());
    }
}
