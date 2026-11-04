//! Public AV admission only. No production authority or media transport is installed here.
use crate::foundation::{Operation, Result, StopFence, uuid};
use nai_contracts::{
    public_media_hash,
    workbench::{
        DeliveryScope, PublicMediaAudio, PublicMediaBundle, PublicMediaVisual,
        WorkbenchError as Error,
    },
};
use std::sync::Arc;

/// Root-observed evidence, never a deserialized client permission claim.
#[derive(Clone)]
pub struct PublicMediaAuthorityState {
    pub workspace_id: String,
    pub session_id: String,
    pub epoch: u64,
    pub output_id: String,
    pub snapshot_id: String,
    pub snapshot_sha256: String,
    pub facts_sha256: String,
    pub approval_id: String,
    pub approved_sha256: String,
    pub guard_permit_id: String,
    pub guard_bundle_sha256: String,
    pub prepared_media_sha256: String,
    pub audio: PublicMediaAudio,
    pub visual: PublicMediaVisual,
    pub caption_sha256: String,
    pub public_rights: bool,
    pub approval_allowed: bool,
    pub guard_allowed: bool,
    pub assets_licensed: bool,
    pub output_supported: bool,
    pub valid_until_tick: u64,
}
pub trait PublicMediaAuthority {
    fn read(&self, bundle: &PublicMediaBundle, op: &Operation)
    -> Result<PublicMediaAuthorityState>;
}
pub struct PublicMediaPermit {
    bundle: PublicMediaBundle,
    digest: String,
    fence: Arc<StopFence>,
    generation: u64,
    issued: u64,
    until: u64,
    invalid: bool,
}
pub(crate) fn sha(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub(crate) fn structure(b: &PublicMediaBundle) -> Result<()> {
    if b.protocol_major != 1
        || b.scope != DeliveryScope::Public as i32
        || b.epoch == 0
        || b.lease_ms == 0
        || b.lease_ms > 500
    {
        return Err(Error::InvalidArgument);
    }
    for id in [
        &b.bundle_id,
        &b.workspace_id,
        &b.session_id,
        &b.output_id,
        &b.snapshot_id,
        &b.approval_id,
        &b.guard_permit_id,
    ] {
        uuid(id)?;
    }
    for hash in [
        &b.snapshot_sha256,
        &b.facts_sha256,
        &b.approved_sha256,
        &b.caption_sha256,
        &b.prepared_media_sha256,
    ] {
        if !sha(hash) {
            return Err(Error::InvalidArgument);
        }
    }
    let audio = b.audio.as_ref().ok_or(Error::InvalidArgument)?;
    let visual = b.visual.as_ref().ok_or(Error::InvalidArgument)?;
    resource_axes(audio, visual)
}
pub(crate) fn resource_axes(audio: &PublicMediaAudio, visual: &PublicMediaVisual) -> Result<()> {
    uuid(&audio.artifact_id)?;
    uuid(&visual.artifact_id)?;
    if audio.artifact_id == visual.artifact_id
        || !sha(&audio.pcm_sha256)
        || !sha(&visual.package_sha256)
        || !sha(&visual.license_sha256)
        || audio.sample_count == 0
        || audio.sample_count > 480_000
        || audio.sample_rate != 24_000
        || audio.channels != 1
        || visual.width == 0
        || visual.width > 1280
        || visual.height == 0
        || visual.height > 720
        || visual.fps == 0
        || visual.fps > 60
    {
        return Err(Error::InvalidArgument);
    }
    Ok(())
}
#[derive(Clone, Copy)]
enum Use {
    Approval,
    Playback,
}
fn evidence(
    b: &PublicMediaBundle,
    hash: &str,
    s: &PublicMediaAuthorityState,
    now: u64,
    purpose: Use,
) -> Result<()> {
    if !s.public_rights || !s.approval_allowed || !s.guard_allowed || !s.assets_licensed {
        return Err(Error::PermissionDenied);
    }
    if matches!(purpose, Use::Playback) && !s.output_supported {
        return Err(Error::CapabilityUnavailable);
    }
    if now >= s.valid_until_tick {
        return Err(Error::DeadlineExceeded);
    }
    if b.workspace_id != s.workspace_id
        || b.session_id != s.session_id
        || b.epoch != s.epoch
        || b.output_id != s.output_id
        || b.snapshot_id != s.snapshot_id
        || b.snapshot_sha256 != s.snapshot_sha256
        || b.facts_sha256 != s.facts_sha256
        || b.approval_id != s.approval_id
        || b.guard_permit_id != s.guard_permit_id
    {
        return Err(Error::RevisionConflict);
    }
    if hash != s.approved_sha256
        || b.prepared_media_sha256 != s.prepared_media_sha256
        || hash != s.guard_bundle_sha256
        || b.caption_sha256 != s.caption_sha256
        || b.audio.as_ref() != Some(&s.audio)
        || b.visual.as_ref() != Some(&s.visual)
    {
        return Err(Error::IntegrityFailed);
    }
    Ok(())
}
/// Content review only: no permit is returned and output readiness is never inferred.
pub fn review_public_media_approval(
    bundle: &PublicMediaBundle,
    authority: Option<&dyn PublicMediaAuthority>,
    fence: &StopFence,
    op: &Operation,
    clock: &dyn Fn() -> u64,
) -> Result<()> {
    let generation = fence.generation();
    fence.check(generation)?;
    op.check()?;
    structure(bundle)?;
    let digest = public_media_hash(bundle);
    if digest != bundle.approved_sha256 {
        return Err(Error::IntegrityFailed);
    }
    let issued = clock();
    let state = authority
        .ok_or(Error::CapabilityUnavailable)?
        .read(bundle, op)?;
    fence.check(generation)?;
    op.check()?;
    let now = clock();
    if now < issued || now - issued >= 500 {
        return Err(Error::DeadlineExceeded);
    }
    evidence(bundle, &digest, &state, now, Use::Approval)
}
pub fn admit_public_media(
    bundle: PublicMediaBundle,
    authority: Option<&dyn PublicMediaAuthority>,
    fence: Arc<StopFence>,
    op: &Operation,
    clock: &dyn Fn() -> u64,
) -> Result<PublicMediaPermit> {
    let generation = fence.generation();
    fence.check(generation)?;
    op.check()?;
    structure(&bundle)?;
    let digest = public_media_hash(&bundle);
    if digest != bundle.approved_sha256 {
        return Err(Error::IntegrityFailed);
    }
    let issued = clock();
    let until = issued
        .checked_add(bundle.lease_ms.into())
        .ok_or(Error::DeadlineExceeded)?;
    let observed = authority
        .ok_or(Error::CapabilityUnavailable)?
        .read(&bundle, op)?;
    // Read operations may be slow: neither their completion nor a delayed message renews the lease.
    fence.check(generation)?;
    op.check()?;
    let now = clock();
    if now < issued || now >= until {
        return Err(Error::DeadlineExceeded);
    }
    evidence(&bundle, &digest, &observed, now, Use::Playback)?;
    Ok(PublicMediaPermit {
        bundle,
        digest,
        fence,
        generation,
        issued,
        until: until.min(observed.valid_until_tick),
        invalid: false,
    })
}
impl PublicMediaPermit {
    pub fn checked_bundle(
        &mut self,
        authority: Option<&dyn PublicMediaAuthority>,
        op: &Operation,
        clock: &dyn Fn() -> u64,
    ) -> Result<&PublicMediaBundle> {
        self.recheck(authority, op, clock)?;
        Ok(&self.bundle)
    }
    /// Must be used before every consumption/renewal; not a cached Ready bit.
    pub fn recheck(
        &mut self,
        authority: Option<&dyn PublicMediaAuthority>,
        op: &Operation,
        clock: &dyn Fn() -> u64,
    ) -> Result<()> {
        let result = (|| {
            if self.invalid {
                return Err(Error::Cancelled);
            }
            self.fence.check(self.generation)?;
            op.check()?;
            let now = clock();
            if now < self.issued || now >= self.until {
                return Err(Error::DeadlineExceeded);
            }
            let observed = authority
                .ok_or(Error::CapabilityUnavailable)?
                .read(&self.bundle, op)?;
            self.fence.check(self.generation)?;
            op.check()?;
            let now = clock();
            if now < self.issued || now >= self.until {
                return Err(Error::DeadlineExceeded);
            }
            evidence(&self.bundle, &self.digest, &observed, now, Use::Playback)
        })();
        if result.is_err() {
            self.invalid = true;
        }
        result
    }
}

impl PublicMediaPermit {
    /// Root-only issuance after current authority recheck. The DTO itself is not authority.
    pub fn execution_lease(
        &mut self,
        authority: Option<&dyn PublicMediaAuthority>,
        op: &Operation,
        clock: &dyn Fn() -> u64,
        x: &crate::public_execution::IssueRequest<'_>,
    ) -> Result<nai_contracts::workbench::PublicExecutionLease> {
        self.recheck(authority, op, clock)?;
        uuid(x.preparation_id)?;
        if x.authorization.len() != 64
            || !x.authorization.bytes().all(|b| b.is_ascii_hexdigit())
            || x.challenge == 0
            || x.seq == 0
            || x.max_lease_ms == 0
            || x.max_lease_ms > 500
        {
            return Err(Error::InvalidArgument);
        }
        let now = clock();
        if now < self.issued || now >= self.until {
            self.invalid = true;
            return Err(Error::DeadlineExceeded);
        }
        let ttl = u64::from(x.max_lease_ms)
            .min(self.until - now)
            .min(u64::from(self.bundle.lease_ms));
        Ok(nai_contracts::workbench::PublicExecutionLease {
            authorization: x.authorization.into(),
            output_id: self.bundle.output_id.clone(),
            bundle_sha256: self.digest.clone(),
            prepared_media_sha256: self.bundle.prepared_media_sha256.clone(),
            session_id: self.bundle.session_id.clone(),
            epoch: self.bundle.epoch,
            challenge: x.challenge,
            seq: x.seq,
            lease_ms: ttl as u32,
            purpose: crate::public_execution::PURPOSE.into(),
            preparation_id: x.preparation_id.into(),
        })
    }
}
