//! Package verification: the gate that runs *before* anything is staged.
//!
//! Ordered exactly as the specification orders it — schema, canonical
//! declaration, exact content set, per-file hash, signature, revocation — with a
//! deadline/cancellation check at the entry, inside every per-file loop, and at
//! the exit. A failure at any point returns before any staging exists, so there
//! is no half-materialised package to clean up.
use crate::{
    Result, checkpoint, error,
    manifest::{FileDecl, UpgradePackage},
    revocation::RevocationList,
    signature::SignatureVerifier,
};
use nai_contracts::v1::ErrorCode;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::atomic::AtomicBool, time::Instant};

/// Caller-owned immutable content for one declared path.
pub struct PackageFile<'a> {
    pub path: &'a str,
    pub content: &'a [u8],
}

#[derive(Debug, Clone, Copy)]
pub struct VerifyLimits {
    pub max_files: usize,
    pub max_bytes: u64,
}
impl Default for VerifyLimits {
    fn default() -> Self {
        Self {
            max_files: 4096,
            max_bytes: 8 << 30,
        }
    }
}

#[derive(Debug)]
pub struct VerifiedPackage {
    pub package: UpgradePackage,
    pub manifest_hash: String,
    pub content_bytes: u64,
}

/// Lowercase hex SHA-256 of a whole buffer. The streaming hash inside
/// [`verify_package`] is the same digest in a different shape (it hashes in
/// chunks so a deadline can interrupt mid-file); the download path, which has
/// the assembled artifact in hand, reuses *this* one rather than repeating the
/// digest by hand.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Verify a package against its manifest, detached signature and the current
/// revocation list. On success the manifest is known to be structurally valid,
/// every declared file is present with the declared bytes and SHA256, the
/// signature is valid for the declared signer, and the package is not revoked.
#[allow(clippy::too_many_arguments)]
pub fn verify_package(
    manifest: &[u8],
    files: &[PackageFile<'_>],
    signature: &[u8],
    verifier: &dyn SignatureVerifier,
    revocations: &RevocationList,
    now_ms: i64,
    limits: VerifyLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<VerifiedPackage> {
    checkpoint(deadline, cancelled)?;
    if files.len() > limits.max_files {
        return Err(error(
            ErrorCode::ResourceExhausted,
            "package file count limit",
        ));
    }
    let package = UpgradePackage::parse(manifest)?;
    checkpoint(deadline, cancelled)?;

    let declared: BTreeMap<String, (String, u64)> = package
        .files
        .iter()
        .map(
            |FileDecl {
                 path,
                 sha256,
                 bytes,
             }| (path.clone(), (sha256.clone(), *bytes)),
        )
        .collect();

    // The provided content must equal the declared set: a missing, extra or
    // duplicate entry is refused rather than reconciled.
    let mut provided: BTreeMap<&str, &[u8]> = BTreeMap::new();
    let mut content_bytes: u64 = 0;
    for file in files {
        if !declared.contains_key(file.path) || provided.insert(file.path, file.content).is_some() {
            return Err(error(
                ErrorCode::InvalidArgument,
                "unexpected or duplicate package content",
            ));
        }
        content_bytes = content_bytes
            .checked_add(file.content.len() as u64)
            .filter(|total| *total <= limits.max_bytes)
            .ok_or_else(|| error(ErrorCode::ResourceExhausted, "package content byte limit"))?;
    }
    if provided.len() != declared.len() {
        return Err(error(ErrorCode::InvalidArgument, "missing package content"));
    }

    for (path, (expected_hash, expected_bytes)) in &declared {
        checkpoint(deadline, cancelled)?;
        let content = provided[path.as_str()];
        if content.len() as u64 != *expected_bytes {
            return Err(error(
                ErrorCode::InvalidArgument,
                "package file size mismatch",
            ));
        }
        let mut hasher = Sha256::new();
        for chunk in content.chunks(65536) {
            checkpoint(deadline, cancelled)?;
            hasher.update(chunk);
        }
        let actual: String = hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        if actual != *expected_hash {
            return Err(error(
                ErrorCode::InvalidArgument,
                "package content hash mismatch",
            ));
        }
    }
    checkpoint(deadline, cancelled)?;

    verifier.verify(manifest, signature, &package.signer_ref)?;
    checkpoint(deadline, cancelled)?;

    revocations.check_install(&package.package_id, now_ms)?;
    checkpoint(deadline, cancelled)?;

    Ok(VerifiedPackage {
        manifest_hash: sha256_hex(manifest),
        content_bytes,
        package,
    })
}
