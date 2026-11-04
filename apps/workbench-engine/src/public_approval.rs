//! Content approval and current-authority execution issuance; no device SDK dependency.
use super::*;
use nai_contracts::{prepared_media_hash, public_guard_hash, public_media_hash};
use nai_runtime_core::{
    public_media::{PublicMediaAuthority, PublicMediaAuthorityState},
    public_output_binding::BoundPublicOutput,
};
pub struct Confirmation<'a> {
    pub check: &'a PublicCandidateCheck,
    pub request: &'a FoundationPublicApproval,
    pub bound: &'a BoundPublicOutput,
    pub store: &'a mut Store,
    pub actor: &'a str,
    pub op: &'a Operation,
    pub path: &'a std::path::Path,
}
#[derive(Clone)]
pub struct Draft {
    pub view: PublicApprovalDraft,
    guard_sha: String,
    until: u64,
}
impl Draft {
    pub fn expired(&self, now: u64) -> bool {
        now >= self.until
    }
    fn check_time(&self, now: u64) -> Result<(), Error> {
        if now >= self.until {
            Err(Error::DeadlineExceeded)
        } else {
            Ok(())
        }
    }
    pub fn check_start_window(&self, now: u64) -> Result<(), Error> {
        self.check_time(now)?;
        let audio = self
            .view
            .bundle
            .as_ref()
            .and_then(|b| b.audio.as_ref())
            .ok_or(Error::IntegrityFailed)?;
        let required = nai_runtime_core::public_guard::required_playback_window_ms(audio)?;
        if self.until - now < required {
            return Err(Error::DeadlineExceeded);
        }
        Ok(())
    }
    pub fn new(c: &PublicCandidateCheck, now: u64) -> Result<Option<Self>, Error> {
        let Some(g) = c.guard_review.as_ref().filter(|g| g.policy_passed) else {
            return Ok(None);
        };
        let Some(o) = c.output_instance.as_ref() else {
            return Ok(None);
        };
        let Some(p) = c.prepared_media.as_ref() else {
            return Ok(None);
        };
        if c.prepared_media_receipt.is_none()
            || o.terminal
            || o.live_ready
            || !o.standby
            || c.axes
                .iter()
                .filter(|a| a.axis != "output")
                .any(|a| a.state != PublicCandidateAxisState::Passed as i32)
        {
            return Ok(None);
        }
        let remaining = 30000u64.min(g.valid_until_ms.saturating_sub(g.checked_at_ms));
        if remaining == 0 {
            return Ok(None);
        }
        let mut b = PublicMediaBundle {
            protocol_major: 1,
            bundle_id: media_pipe::random_id()?,
            workspace_id: p.workspace_id.clone(),
            session_id: c.session_id.clone(),
            epoch: c.epoch,
            output_id: o.output_id.clone(),
            snapshot_id: c.snapshot_id.clone(),
            snapshot_sha256: c.snapshot_sha256.clone(),
            facts_sha256: c.facts_sha256.clone(),
            approval_id: media_pipe::random_id()?,
            approved_sha256: String::new(),
            guard_permit_id: media_pipe::random_id()?,
            caption_sha256: c.caption_sha256.clone(),
            audio: c.audio.clone(),
            visual: c.visual.clone(),
            lease_ms: 500,
            scope: DeliveryScope::Public as i32,
            prepared_media_sha256: prepared_media_hash(p),
        };
        b.approved_sha256 = public_media_hash(&b);
        Ok(Some(Self {
            view: PublicApprovalDraft {
                bundle: Some(b),
                text: c.text.clone(),
                expires_at_ms: c
                    .checked_at_ms
                    .checked_add(remaining)
                    .ok_or(Error::DeadlineExceeded)?,
            },
            guard_sha: g.object_sha256.clone(),
            until: now.checked_add(remaining).ok_or(Error::DeadlineExceeded)?,
        }))
    }
    pub fn check_window_preparation(
        &self,
        context: Confirmation<'_>,
    ) -> Result<PublicMediaAuthorityState, Error> {
        let Confirmation {
            check: c,
            request: x,
            bound,
            store,
            actor,
            op,
            path,
        } = context;
        self.check_time(store.public_media_tick())?;
        let b = self.view.bundle.as_ref().ok_or(Error::IntegrityFailed)?;
        let g = c.guard_review.as_ref().ok_or(Error::PermissionDenied)?;
        let p = c
            .prepared_media
            .as_ref()
            .ok_or(Error::CapabilityUnavailable)?;
        if !g.policy_passed
            || g.object_sha256 != self.guard_sha
            || x.bundle_sha256 != b.approved_sha256
            || c.text != self.view.text
            || bound.status().output_id != b.output_id
            || prepared_media_hash(p) != b.prepared_media_sha256
            || c.session_id != b.session_id
            || c.epoch != b.epoch
            || c.snapshot_sha256 != b.snapshot_sha256
            || c.facts_sha256 != b.facts_sha256
            || c.audio != b.audio
            || c.visual != b.visual
        {
            return Err(Error::RevisionConflict);
        }
        let state = PublicMediaAuthorityState {
            workspace_id: b.workspace_id.clone(),
            session_id: c.session_id.clone(),
            epoch: c.epoch,
            output_id: bound.status().output_id.clone(),
            snapshot_id: c.snapshot_id.clone(),
            snapshot_sha256: c.snapshot_sha256.clone(),
            facts_sha256: c.facts_sha256.clone(),
            approval_id: b.approval_id.clone(),
            approved_sha256: b.approved_sha256.clone(),
            guard_permit_id: b.guard_permit_id.clone(),
            guard_bundle_sha256: b.approved_sha256.clone(),
            prepared_media_sha256: prepared_media_hash(p),
            audio: c.audio.clone().ok_or(Error::IntegrityFailed)?,
            visual: c.visual.clone().ok_or(Error::IntegrityFailed)?,
            caption_sha256: c.caption_sha256.clone(),
            public_rights: true,
            approval_allowed: false,
            guard_allowed: g.policy_passed,
            assets_licensed: true,
            output_supported: false,
            valid_until_tick: self.until.min(bound.valid_until_tick()),
        };
        let observation = Observed {
            state: state.clone(),
            bound,
            path,
            started: std::time::Instant::now(),
            tick: store.public_media_tick(),
        };
        let authority = store.scoped_public_media_authority(actor, op, Some(&observation));
        nai_runtime_core::public_media::review_public_media_approval(
            b,
            Some(&authority),
            &store.fence(),
            op,
            &|| store.public_media_tick(),
        )?;
        Ok(state)
    }
    pub fn execution(
        &self,
        context: Confirmation<'_>,
        issue: nai_runtime_core::public_execution::IssueRequest<'_>,
    ) -> Result<PublicExecutionLease, Error> {
        let Confirmation {
            check,
            request,
            bound,
            store,
            actor,
            op,
            path,
        } = context;
        let mut state = self.check_window_preparation(Confirmation {
            check,
            request,
            bound,
            store,
            actor,
            op,
            path,
        })?;
        state.output_supported = true;
        state.valid_until_tick = self.until;
        let observation = Observed {
            state,
            bound,
            path,
            started: std::time::Instant::now(),
            tick: store.public_media_tick(),
        };
        let authority = store.scoped_public_media_authority(actor, op, Some(&observation));
        let bundle = self.view.bundle.as_ref().ok_or(Error::IntegrityFailed)?;
        let clock = || store.public_media_tick();
        let mut permit = nai_runtime_core::public_media::admit_public_media(
            bundle.clone(),
            Some(&authority),
            store.fence(),
            op,
            &clock,
        )?;
        permit.execution_lease(Some(&authority), op, &clock, &issue)
    }
    pub fn confirm(self, context: Confirmation<'_>) -> Result<FoundationResult, Error> {
        let Confirmation {
            check: c,
            request: x,
            bound,
            store,
            actor,
            op,
            path,
        } = context;
        let b = self.view.bundle.as_ref().ok_or(Error::IntegrityFailed)?;
        let now = store.public_media_tick();
        self.check_time(now)?;
        if !x.acknowledge_public_rights {
            return Err(Error::PermissionDenied);
        }
        let g = c.guard_review.as_ref().ok_or(Error::PermissionDenied)?;
        let p = c
            .prepared_media
            .as_ref()
            .ok_or(Error::CapabilityUnavailable)?;
        if x.bundle_sha256 != b.approved_sha256
            || public_media_hash(b) != x.bundle_sha256
            || !g.policy_passed
            || g.object_sha256 != self.guard_sha
            || g.object.as_ref().map(public_guard_hash).as_deref() != Some(&self.guard_sha)
            || c.text != self.view.text
            || c.session_id != b.session_id
            || c.epoch != b.epoch
            || c.snapshot_id != b.snapshot_id
            || c.snapshot_sha256 != b.snapshot_sha256
            || c.facts_sha256 != b.facts_sha256
            || c.caption_sha256 != b.caption_sha256
            || c.audio != b.audio
            || c.visual != b.visual
            || prepared_media_hash(p) != b.prepared_media_sha256
            || bound.status().output_id != b.output_id
            || c.prepared_media_receipt.is_none()
        {
            return Err(Error::RevisionConflict);
        }
        let state = PublicMediaAuthorityState {
            workspace_id: p.workspace_id.clone(),
            session_id: c.session_id.clone(),
            epoch: c.epoch,
            output_id: bound.status().output_id.clone(),
            snapshot_id: c.snapshot_id.clone(),
            snapshot_sha256: c.snapshot_sha256.clone(),
            facts_sha256: c.facts_sha256.clone(),
            approval_id: b.approval_id.clone(),
            approved_sha256: b.approved_sha256.clone(),
            guard_permit_id: b.guard_permit_id.clone(),
            guard_bundle_sha256: b.approved_sha256.clone(),
            prepared_media_sha256: prepared_media_hash(p),
            audio: c.audio.clone().ok_or(Error::IntegrityFailed)?,
            visual: c.visual.clone().ok_or(Error::IntegrityFailed)?,
            caption_sha256: c.caption_sha256.clone(),
            public_rights: x.acknowledge_public_rights,
            approval_allowed: false,
            guard_allowed: g.policy_passed,
            assets_licensed: true,
            output_supported: false,
            valid_until_tick: self.until.min(bound.valid_until_tick()),
        };
        let observations = Observed {
            state,
            bound,
            path,
            started: std::time::Instant::now(),
            tick: now,
        };
        store.approve_public_media(
            op,
            actor,
            &nai_foundation_sqlite::PublicMediaApprovalRequest {
                bundle: b.clone(),
                expected_sha256: x.bundle_sha256.clone(),
                ttl_ms: (self.until - now) as u32,
            },
            Some(&observations),
        )?;
        Ok(FoundationResult {
            resource_id: b.approval_id.clone(),
            sha256: b.approved_sha256.clone(),
            public_approval: Some(Box::new(PublicApprovalView {
                bundle: Some(b.clone()),
                approved: true,
                revoked: false,
                expires_at_ms: self.view.expires_at_ms,
                playback_available: false,
            })),
            ..Default::default()
        })
    }
}
struct Observed<'a> {
    state: PublicMediaAuthorityState,
    bound: &'a BoundPublicOutput,
    path: &'a std::path::Path,
    started: std::time::Instant,
    tick: u64,
}
impl PublicMediaAuthority for Observed<'_> {
    fn read(
        &self,
        _: &PublicMediaBundle,
        op: &Operation,
    ) -> Result<PublicMediaAuthorityState, Error> {
        self.bound
            .recheck(
                self.tick
                    .saturating_add(self.started.elapsed().as_millis() as u64),
                &crate::audition::nonce(self.path)?,
                op,
            )
            .inspect_err(|e| {
                eprintln!(
                    "PUBLIC_APPROVAL_OBSERVATION: {} now={} until={}",
                    e.as_str_name(),
                    self.tick
                        .saturating_add(self.started.elapsed().as_millis() as u64),
                    self.bound.valid_until_tick()
                );
            })?;
        Ok(self.state.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn draft_deadline_does_not_renew_after_confirmation_delay() {
        let d = Draft {
            view: PublicApprovalDraft::default(),
            guard_sha: String::new(),
            until: 100,
        };
        assert!(d.check_time(99).is_ok());
        assert_eq!(d.check_time(100), Err(Error::DeadlineExceeded));
        assert_eq!(d.check_time(101), Err(Error::DeadlineExceeded));
    }
    #[test]
    fn start_window_rechecks_delay_and_does_not_change_the_approval_deadline() {
        let d = Draft {
            view: PublicApprovalDraft {
                bundle: Some(PublicMediaBundle {
                    audio: Some(PublicMediaAudio {
                        sample_count: 48000,
                        sample_rate: 24000,
                        channels: 1,
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
            guard_sha: String::new(),
            until: 5000,
        };
        assert!(d.check_start_window(2500).is_ok());
        assert_eq!(d.check_start_window(2501), Err(Error::DeadlineExceeded));
        assert_eq!(d.check_start_window(5000), Err(Error::DeadlineExceeded));
        assert_eq!(d.until, 5000);
    }
}
