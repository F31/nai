//! Offline install: the same verification, with one precondition in front of it.
//!
//! The precondition is "every declared artifact is already on local media", and
//! it exists because it produces a *different refusal* than a network install
//! would. `verify_package` reports missing content as "missing package content",
//! which is exactly right when the bytes simply have not arrived yet. For an
//! offline install the operator needs the other sentence: "this package cannot
//! be installed offline because this file is not here".
//!
//! The real guarantee is structural: this crate has no HTTP client, so "offline
//! install never quietly fetches" is not maintained by an `offline: bool` flag —
//! fetching would require adding a dependency, which is a loud, reviewable act.
//! A `false` flag that nothing can set to `true` would be a fake assurance.
use crate::{
    Result, checkpoint, error,
    manifest::UpgradePackage,
    revocation::RevocationList,
    signature::SignatureVerifier,
    verify::{PackageFile, VerifiedPackage, VerifyLimits, verify_package},
};
use nai_contracts::v1::ErrorCode;
use std::{collections::BTreeSet, sync::atomic::AtomicBool, time::Instant};

/// One artifact present on local media.
pub struct OfflineArtifact<'a> {
    pub path: &'a str,
    pub content: &'a [u8],
}

/// A completed offline install: the verified package plus how many local
/// artifacts it consumed.
#[derive(Debug)]
pub struct OfflineInstall {
    pub verified: VerifiedPackage,
    pub artifacts: usize,
}

/// Which declared artifacts are absent from the local set.
///
/// This is a separate query rather than a payload on the error because the
/// crate's error carries a stable `&'static str` token so reports can classify
/// outcomes without a debug build; a list of paths is not a token.
pub fn missing_artifacts(
    package: &UpgradePackage,
    artifacts: &[OfflineArtifact<'_>],
) -> Vec<String> {
    let local: BTreeSet<&str> = artifacts.iter().map(|artifact| artifact.path).collect();
    package
        .files
        .iter()
        .map(|file| file.path.as_str())
        .filter(|path| !local.contains(path))
        .map(str::to_owned)
        .collect()
}

/// Install from local media only.
///
/// Refuses with `CAPABILITY_NOT_READY` when anything declared is absent — the
/// same code the preflight uses for "not enough to proceed", because that is
/// what it is. On success the full verification chain runs unchanged: schema,
/// exact content set, per-file SHA256, signature, revocation.
#[allow(clippy::too_many_arguments)]
pub fn install_offline(
    manifest: &[u8],
    artifacts: &[OfflineArtifact<'_>],
    signature: &[u8],
    verifier: &dyn SignatureVerifier,
    revocations: &RevocationList,
    now_ms: i64,
    limits: VerifyLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<OfflineInstall> {
    checkpoint(deadline, cancelled)?;
    let package = UpgradePackage::parse(manifest)?;
    if !missing_artifacts(&package, artifacts).is_empty() {
        return Err(error(
            ErrorCode::CapabilityNotReady,
            "offline install lacks a local artifact",
        ));
    }
    let files: Vec<PackageFile<'_>> = artifacts
        .iter()
        .map(|artifact| PackageFile {
            path: artifact.path,
            content: artifact.content,
        })
        .collect();
    let verified = verify_package(
        manifest,
        &files,
        signature,
        verifier,
        revocations,
        now_ms,
        limits,
        deadline,
        cancelled,
    )?;
    Ok(OfflineInstall {
        verified,
        artifacts: package.files.len(),
    })
}
