//! Downloading: the phase in front of verification, and the reason a dropped
//! connection cannot disturb the running version.
//!
//! The orchestrator owns the *decisions* a download needs — which window to ask
//! for, whether a short read may be re-asked, when to give up — and no
//! transport. Two facts about the world enter through seams: how many bytes of
//! an artifact are already durable on local media ([`ResumeLedger`]) and whether
//! a window arrived ([`PackageFetcher`]). As with [`crate::SignatureVerifier`],
//! the transport is an injected implementation, so the crate keeps its
//! no-network property: a real downloader is a later injection rather than a
//! rewrite of this module.
//!
//! ## Which failures are re-asked, and which are refused
//!
//! A **short read** (fewer bytes than the window asked for) is a *value*, not an
//! error. An interrupted stream is the normal case this module exists to
//! survive, so it is counted and re-asked from the offset the ledger reports.
//! Anything the transport returns as `Err` is **not** re-asked: that is the
//! transport saying "asking again will not help" (bad credentials, an artifact
//! the source does not have), and retrying it would only burn the budget.
//!
//! A **complete but wrong artifact** (the assembled bytes hash to something
//! other than the signed declaration) is refused — and its durable bytes are
//! discarded first. Keeping them would let the *next* run resume at the end of a
//! corrupt file and never refetch it: a poison that survives restarts.
//!
//! ## Why the offset is read back every time
//!
//! Each window's offset comes from `ledger.durable_bytes(path)` — a read of
//! storage — never from the orchestrator's own running total, and every append
//! is confirmed by reading the count back. Likewise the bytes handed to the
//! final hash gate are read back from the ledger, not taken from the buffers
//! this module happens to hold. A ledger that silently drops a chunk therefore
//! produces a *named failure* instead of phantom progress, and "the artifact
//! matches its declaration" is a statement about local media rather than about
//! the caller's bookkeeping.
//!
//! ## What failure costs
//!
//! Nothing. This module cannot stage, journal or swap anything: its inputs are
//! bytes, two seams and a budget, and its only output on success is a
//! [`VerifiedPackage`] — the same value [`verify_package`] returns. A download
//! that dies therefore leaves the running version untouched, which is the
//! structural half of "a dropped network does not disturb the running version".
use crate::{
    Result, checkpoint, error,
    manifest::UpgradePackage,
    revocation::RevocationList,
    signature::SignatureVerifier,
    verify::{PackageFile, VerifiedPackage, VerifyLimits, sha256_hex, verify_package},
};
use nai_contracts::v1::ErrorCode;
use nai_security::Operation;
use std::{sync::atomic::AtomicBool, time::Instant};

/// One artifact window to ask a transport for. `offset + length` is exactly the
/// declared size of the artifact, so a window never crosses a file boundary and
/// the last window of an artifact always ends at its declared end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FetchWindow<'a> {
    pub path: &'a str,
    pub offset: u64,
    pub length: u64,
}

/// What one attempt produced. `bytes.len()` may be **less** than the requested
/// length: that is an interrupted stream, reported as a value so the decision to
/// re-ask stays here rather than in the transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    pub bytes: Vec<u8>,
}

/// The transport seam. An implementation decides *how* a window is obtained
/// (HTTP `Range`, a local mirror, removable media); it does not decide retries,
/// resumption or verification.
pub trait PackageFetcher {
    /// Obtain `window`. Returning fewer bytes than requested is an interruption
    /// and is re-asked; returning `Err` is treated as final. An implementation
    /// SHOULD honour `op` (a cancelled or expired operation must not keep
    /// transferring).
    fn fetch(&mut self, op: &Operation, window: &FetchWindow<'_>) -> Result<Fetched>;
}

/// Durable local state of a partially downloaded artifact.
///
/// Every method is a storage operation, and the orchestrator reads rather than
/// remembers: the facts it acts on must come from here. An implementation MUST
/// refuse an append at an offset that does not continue the durable prefix
/// instead of silently realigning it.
pub trait ResumeLedger {
    /// Bytes of `path` already durable. A read of storage, not a cached total.
    fn durable_bytes(&self, path: &str) -> u64;
    /// Append `chunk`, which begins at `offset`, and make it durable.
    fn append(&mut self, op: &Operation, path: &str, offset: u64, chunk: &[u8]) -> Result<()>;
    /// The durable prefix of `path`, read back for the hash gate.
    fn assembled(&self, path: &str) -> Vec<u8>;
    /// Drop everything durable for `path`. Called when the assembled bytes fail
    /// the signed declaration, so a corrupt prefix cannot become the resume
    /// point of the next run.
    fn discard(&mut self, op: &Operation, path: &str) -> Result<()>;
}

/// How many attempts one window may take. Exhausting it is a *named* refusal,
/// never a silent loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FetchPolicy {
    /// Attempts per window. `1` means "no re-ask": a single short read refuses.
    pub max_attempts_per_window: u32,
}
impl Default for FetchPolicy {
    fn default() -> Self {
        Self {
            max_attempts_per_window: 3,
        }
    }
}

/// Everything the download needs besides the two seams. The deadline and
/// cancellation are the ones the terminal verification must honour; the loop
/// itself stops on `op`, exactly as [`crate::UpgradeTransaction::prepare`] does.
pub struct DownloadRequest<'a> {
    pub manifest: &'a [u8],
    pub signature: &'a [u8],
    pub verifier: &'a dyn SignatureVerifier,
    pub revocations: &'a RevocationList,
    pub now_ms: i64,
    pub policy: FetchPolicy,
    pub limits: VerifyLimits,
    pub deadline: Instant,
    pub cancelled: &'a AtomicBool,
}

/// A completed download. `verified` is the value the rest of the upgrade flow
/// consumes, so a download that did not reach the hash gate has nothing to
/// hand over.
#[derive(Debug)]
pub struct DownloadReport {
    pub verified: VerifiedPackage,
    /// Windows the transport was actually asked for, in order. Counted rather
    /// than asserted so a report can state what happened.
    pub windows_requested: usize,
    /// Attempts that came back short — the interruptions that were survived.
    pub interruptions: usize,
    /// Bytes already durable when this run started: what resumption saved.
    pub resumed_bytes: u64,
}

/// Bound what the source is allowed to make us move, from the *declaration*,
/// before a single byte is requested.
///
/// The refusals use the same tokens as the equivalent bounds inside
/// [`verify_package`], but they are not the same rule: that one bounds what
/// actually arrived, this one bounds what the signed manifest is about to ask
/// for. Sharing a function would mean pretending two different inputs are one.
fn bound_declared(package: &UpgradePackage, limits: VerifyLimits) -> Result<()> {
    if package.files.len() > limits.max_files {
        return Err(error(
            ErrorCode::ResourceExhausted,
            "package file count limit",
        ));
    }
    let mut total: u64 = 0;
    for file in &package.files {
        total = total
            .checked_add(file.bytes)
            .filter(|total| *total <= limits.max_bytes)
            .ok_or_else(|| error(ErrorCode::ResourceExhausted, "package content byte limit"))?;
    }
    Ok(())
}

/// Fetch every declared artifact, then run the one verification chain.
///
/// Resumption, bounded re-asking and the per-artifact hash gate are decisions
/// this function makes; how bytes travel is the transport's business. On any
/// failure nothing is returned, so no staging, journal record or manifest swap
/// can exist downstream of a failed download.
pub fn download_package(
    op: &Operation,
    request: &DownloadRequest<'_>,
    fetcher: &mut dyn PackageFetcher,
    ledger: &mut dyn ResumeLedger,
) -> Result<DownloadReport> {
    checkpoint(request.deadline, request.cancelled)?;
    op.check()?;

    // Schema first, the same order verification uses: the declared set is only
    // known to be well formed once it has been parsed.
    let package = UpgradePackage::parse(request.manifest)?;
    bound_declared(&package, request.limits)?;

    let mut windows_requested = 0usize;
    let mut interruptions = 0usize;
    let mut resumed_bytes = 0u64;
    let mut owned: Vec<(String, Vec<u8>)> = Vec::with_capacity(package.files.len());

    for file in &package.files {
        op.check()?;
        // Counted for the report; the *gate* on this value is inside the loop
        // below, so the check cannot be removed from one place and survive in
        // another.
        let start = ledger.durable_bytes(&file.path);
        resumed_bytes = resumed_bytes.saturating_add(start);

        let mut attempts: u32 = 0;
        loop {
            op.check()?;
            let have = ledger.durable_bytes(&file.path);
            if have == file.bytes {
                break;
            }
            // A resume point past the declared end is not a number to clamp: the
            // ledger is describing an artifact that cannot be this one, and
            // pretending otherwise would verify something nobody asked for.
            if have > file.bytes {
                return Err(error(
                    ErrorCode::InvalidArgument,
                    "resume ledger exceeds declared size",
                ));
            }
            if attempts >= request.policy.max_attempts_per_window {
                return Err(error(
                    ErrorCode::ResourceExhausted,
                    "download retry budget exhausted",
                ));
            }
            attempts += 1;
            windows_requested += 1;

            let window = FetchWindow {
                path: &file.path,
                offset: have,
                length: file.bytes - have,
            };
            let fetched = fetcher.fetch(op, &window)?;
            if fetched.bytes.len() as u64 > window.length {
                return Err(error(
                    ErrorCode::InvalidArgument,
                    "download returned more than requested",
                ));
            }
            let short = (fetched.bytes.len() as u64) < window.length;
            if short {
                interruptions += 1;
            }
            if fetched.bytes.is_empty() {
                // Nothing arrived: either a stream that died before its first
                // byte or a source with nothing to give. Both are re-asked
                // until the budget runs out; neither may be treated as done.
                continue;
            }
            ledger.append(op, &file.path, have, &fetched.bytes)?;
            // Read the count back rather than trusting the append: a ledger that
            // dropped the chunk must not be mistaken for progress.
            if ledger.durable_bytes(&file.path) != have + fetched.bytes.len() as u64 {
                return Err(error(
                    ErrorCode::Internal,
                    "resume ledger did not persist the chunk",
                ));
            }
        }

        // The hash gate reads what is durable, not what this run happened to
        // buffer: across a restart the two are not the same thing.
        let assembled = ledger.assembled(&file.path);
        if assembled.len() as u64 != file.bytes {
            return Err(error(
                ErrorCode::Internal,
                "resume ledger disagrees with its own byte count",
            ));
        }
        if sha256_hex(&assembled) != file.sha256 {
            ledger.discard(op, &file.path)?;
            if ledger.durable_bytes(&file.path) != 0 {
                return Err(error(
                    ErrorCode::Internal,
                    "resume ledger did not discard the corrupt artifact",
                ));
            }
            return Err(error(
                ErrorCode::InvalidArgument,
                "package content hash mismatch",
            ));
        }
        owned.push((file.path.clone(), assembled));
    }

    checkpoint(request.deadline, request.cancelled)?;
    let files: Vec<PackageFile<'_>> = owned
        .iter()
        .map(|(path, content)| PackageFile {
            path: path.as_str(),
            content: content.as_slice(),
        })
        .collect();
    let verified = verify_package(
        request.manifest,
        &files,
        request.signature,
        request.verifier,
        request.revocations,
        request.now_ms,
        request.limits,
        request.deadline,
        request.cancelled,
    )?;
    Ok(DownloadReport {
        verified,
        windows_requested,
        interruptions,
        resumed_bytes,
    })
}
