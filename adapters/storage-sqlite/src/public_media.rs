//! Root-only public approval ledger. Not exposed as a client command or playback capability.
use super::*;
use nai_contracts::public_media_hash;
use nai_runtime_core::public_media::{
    PublicMediaAuthority, PublicMediaAuthorityState, review_public_media_approval,
};

/// A verified source held under the single writer's exclusive borrow until guard review.
/// No Store command can mutate models, roles or facts between observation and review.
/// Cancellation and external scope fences are still checked by the review.
pub struct PublicCandidateSource<'a> {
    store: &'a mut Store,
    op: Operation,
    actor: String,
    request: FoundationPublicCandidate,
    current: PublicCandidateCheck,
    generation: u64,
}
impl<'a> PublicCandidateSource<'a> {
    pub fn check(&self) -> &PublicCandidateCheck {
        &self.current
    }
    pub fn tick(&self) -> u64 {
        self.store.public_media_tick()
    }
    pub fn review(
        self,
        observed: &PublicCandidateCheck,
    ) -> Result<(&'a mut Store, PublicGuardReview)> {
        self.op.check()?;
        self.store.fence.check(self.generation)?;
        owner(self.store, &self.request.workspace_id, &self.actor)?;
        let review = self.store.review_bound_public_candidate(
            &self.op,
            &self.actor,
            &self.request,
            self.current,
            observed,
        )?;
        self.store.fence.check(self.generation)?;
        Ok((self.store, review))
    }
}
pub struct PublicMediaApprovalRequest {
    pub bundle: PublicMediaBundle,
    /// Explicit confirmation of the reviewed complete bundle, not an inferred default.
    pub expected_sha256: String,
    pub ttl_ms: u32,
}
pub struct StoredPublicMediaAuthority<'a> {
    store: &'a Store,
    actor: &'a str,
    observations: Option<&'a dyn PublicMediaAuthority>,
}
struct ApprovalRow {
    actor: String,
    role_revision: u64,
    digest: String,
    bytes: Vec<u8>,
    status: String,
    expires: u64,
    expires_at: i64,
    created: i64,
}
fn scope(b: &PublicMediaBundle) -> String {
    format!("public:{}:{}", b.workspace_id, b.approval_id)
}
fn owner(s: &Store, ws: &str, actor: &str) -> Result<u64> {
    let (r, revision) = role(&s.conn, ws, actor)?;
    if r != FoundationRole::Admin {
        return Err(Error::PermissionDenied);
    }
    s.fence.check_scope(&role_scope(ws, actor, revision))?;
    Ok(revision)
}
fn source(s: &Store, op: &Operation, actor: &str, b: &PublicMediaBundle) -> Result<u64> {
    Ok(source_with_expiry(s, op, actor, b)?.0)
}
fn source_with_expiry(
    s: &Store,
    op: &Operation,
    actor: &str,
    b: &PublicMediaBundle,
) -> Result<(u64, Option<i64>)> {
    let auto_expires = s.validate_auto_bundle(op, actor, b)?;
    op.check()?;
    s.fence.check(s.fence.generation())?;
    s.fence.check_scope(&scope(b))?;
    let role_revision = owner(s, &b.workspace_id, actor)?;
    let session: Option<(String, u64, bool)> = s
        .conn
        .query_row(
            "SELECT owner,epoch,stopped FROM sessions WHERE workspace_id=?1 AND id=?2",
            params![b.workspace_id, b.session_id],
            |r| Ok((r.get(0)?, uint(r, 1)?, r.get(2)?)),
        )
        .optional()
        .wb()?;
    let (session_owner, epoch, stopped) = session.ok_or(Error::NotFound)?;
    if session_owner != actor || stopped || epoch != b.epoch {
        return Err(Error::RevisionConflict);
    }
    let snapshot = publication::read(
        &s.conn,
        &s.root,
        &s.fence,
        op,
        &b.workspace_id,
        &b.snapshot_id,
        utc()?,
    )?;
    if snapshot.publication_status != PublicationStatus::PrivateContent as i32 {
        return Err(Error::PermissionDenied);
    }
    if snapshot.sha256 != b.snapshot_sha256 {
        return Err(Error::IntegrityFailed);
    }
    let value: serde_json::Value =
        serde_json::from_str(&snapshot.body).map_err(|_| Error::IntegrityFailed)?;
    // Entire immutable source closure: show/product/host/series version references.
    if hash(value["refs"].to_string().as_bytes()) != b.facts_sha256 {
        return Err(Error::IntegrityFailed);
    }
    op.check()?;
    let expires = match value.get("valid_until_ms") {
        Some(v) if !v.is_null() => Some(v.as_i64().ok_or(Error::IntegrityFailed)?),
        _ => None,
    };
    let expires = match (expires, auto_expires) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    Ok((role_revision, expires))
}
/// Only approval is supplied by the ledger. Public rights, actual output, assets and guard
/// must come from the trusted root observer; none are inferred from the stored bundle.
// Each authority read checks source exactly once before binding its live observation.
fn bind_observation(
    b: &PublicMediaBundle,
    op: &Operation,
    observations: Option<&dyn PublicMediaAuthority>,
) -> Result<PublicMediaAuthorityState> {
    let mut state = observations
        .ok_or(Error::CapabilityUnavailable)?
        .read(b, op)?;
    state.approval_id = b.approval_id.clone();
    state.approved_sha256 = b.approved_sha256.clone();
    state.approval_allowed = true;
    op.check()?;
    Ok(state)
}
struct Confirmation<'a> {
    store: &'a Store,
    actor: &'a str,
    observations: Option<&'a dyn PublicMediaAuthority>,
}
impl PublicMediaAuthority for Confirmation<'_> {
    fn read(&self, b: &PublicMediaBundle, op: &Operation) -> Result<PublicMediaAuthorityState> {
        source(self.store, op, self.actor, b)?;
        bind_observation(b, op, self.observations)
    }
}
impl Store {
    pub fn can_approve_public_media(&self, op: &Operation, actor: &str, ws: &str) -> Result<bool> {
        op.check()?;
        match owner(self, ws, actor) {
            Ok(_) => Ok(true),
            Err(Error::PermissionDenied) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn public_candidate_review<'a>(
        &'a mut self,
        op: &Operation,
        actor: &str,
        request: &FoundationPublicCandidate,
    ) -> Result<PublicCandidateSource<'a>> {
        let generation = self.fence.generation();
        let current = self.public_candidate_source(op, actor, request)?;
        self.fence.check(generation)?;
        Ok(PublicCandidateSource {
            store: self,
            op: op.clone(),
            actor: actor.into(),
            request: request.clone(),
            current,
            generation,
        })
    }
    /// Trusted root-only preparation review; not exposed as a standalone client command.
    pub fn review_public_candidate(
        &mut self,
        op: &Operation,
        actor: &str,
        request: &FoundationPublicCandidate,
        observed: &PublicCandidateCheck,
    ) -> Result<PublicGuardReview> {
        self.public_candidate_review(op, actor, request)?
            .review(observed)
            .map(|(_, review)| review)
    }
    fn review_bound_public_candidate(
        &mut self,
        op: &Operation,
        _actor: &str,
        request: &FoundationPublicCandidate,
        current: PublicCandidateCheck,
        observed: &PublicCandidateCheck,
    ) -> Result<PublicGuardReview> {
        op.check()?;
        if current.preview_id != observed.preview_id
            || current.preview_sha256 != observed.preview_sha256
            || current.task_id != observed.task_id
            || current.task_sha256 != observed.task_sha256
            || current.session_id != observed.session_id
            || current.epoch != observed.epoch
            || current.snapshot_id != observed.snapshot_id
            || current.snapshot_sha256 != observed.snapshot_sha256
            || current.facts_sha256 != observed.facts_sha256
            || current.caption_sha256 != observed.caption_sha256
            || current.text != observed.text
        {
            return Err(Error::IntegrityFailed);
        }
        let now = utc()?;
        let Some(output) = observed.output_instance.as_ref() else {
            return Ok(PublicGuardReview {
                reason: PublicGuardReason::OutputNotBound as i32,
                checked_at_ms: now as u64,
                valid_until_ms: now as u64,
                ..Default::default()
            });
        };
        uuid(&output.output_id)?;
        if !nai_runtime_core::public_output_binding::healthy(output)
            || output.visual != observed.visual
        {
            return Err(Error::CapabilityUnavailable);
        }
        if observed.audio.is_none() || observed.visual.is_none() {
            return Ok(PublicGuardReview {
                reason: PublicGuardReason::MediaNotReady as i32,
                checked_at_ms: now as u64,
                valid_until_ms: now as u64,
                ..Default::default()
            });
        }
        if current.audio != observed.audio {
            return Err(Error::IntegrityFailed);
        }
        let (card, _) = publication::stored(
            &self.conn,
            &self.root,
            &request.workspace_id,
            &request.preview_id,
            "publication_private_preview",
        )?;
        let (snapshot, _) = publication::stored(
            &self.conn,
            &self.root,
            &request.workspace_id,
            &current.snapshot_id,
            "publication_snapshot",
        )?;
        let docs = publication::documents(
            &self.conn,
            &self.root,
            &self.fence,
            &request.workspace_id,
            &snapshot["refs"],
            false,
        )?;
        let object = PublicGuardObject {
            protocol_major: 1,
            policy_id: nai_runtime_core::public_guard::policy_id().into(),
            workspace_id: request.workspace_id.clone(),
            session_id: current.session_id,
            epoch: current.epoch,
            preview_id: current.preview_id,
            preview_sha256: current.preview_sha256,
            task_id: current.task_id,
            task_sha256: current.task_sha256,
            snapshot_id: current.snapshot_id,
            snapshot_sha256: current.snapshot_sha256,
            facts_sha256: current.facts_sha256,
            caption_sha256: current.caption_sha256,
            output_target_id: output.output_id.clone(),
            audio: observed.audio.clone(),
            visual: observed.visual.clone(),
        };
        let input = nai_runtime_core::public_guard::ReviewInput {
            documents: [&docs[0], &docs[1], &docs[2], &docs[3]],
            cue_id: card["cue_id"].as_str().ok_or(Error::IntegrityFailed)?,
            faq_id: card.get("live_question").and_then(|q| q["faq_id"].as_str()),
            text: &current.text,
            now,
            source_valid_until: card["expires_at_ms"]
                .as_i64()
                .ok_or(Error::IntegrityFailed)?,
            output_id: &output.output_id,
        };
        let review = nai_runtime_core::public_guard::review(op, object, &input)?;
        self.begin(op)?;
        if utc()? as u64 >= review.valid_until_ms {
            return Err(Error::DeadlineExceeded);
        }
        Ok(review)
    }
    /// Clock used by this store's authority; callers must not mix another origin with it.
    pub fn public_media_tick(&self) -> u64 {
        u64::try_from(self.origin.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
    pub fn public_media_authority<'a>(
        &'a self,
        actor: &'a str,
        observations: Option<&'a dyn PublicMediaAuthority>,
    ) -> StoredPublicMediaAuthority<'a> {
        StoredPublicMediaAuthority {
            store: self,
            actor,
            observations,
        }
    }
    pub fn approve_public_media(
        &mut self,
        op: &Operation,
        actor: &str,
        request: &PublicMediaApprovalRequest,
        observations: Option<&dyn PublicMediaAuthority>,
    ) -> Result<()> {
        let b = &request.bundle;
        if !(1..=30_000).contains(&request.ttl_ms)
            || b.encoded_len() > 8192
            || request.expected_sha256 != b.approved_sha256
            || public_media_hash(b) != request.expected_sha256
        {
            return Err(Error::InvalidArgument);
        }
        let role_revision = source(self, op, actor, b)?;
        let tick = self.public_media_tick();
        let now = utc()?;
        let until = tick
            .checked_add(request.ttl_ms.into())
            .ok_or(Error::DeadlineExceeded)?;
        let count:u64=self.conn.query_row("SELECT COUNT(*) FROM public_media_approvals WHERE workspace_id=?1 AND state='approved' AND expires_tick>?2",params![b.workspace_id,int(tick)?],|r|uint(r,0)).wb()?;
        if count >= 200 {
            return Err(Error::ResourceExhausted);
        }
        let confirmation = Confirmation {
            store: self,
            actor,
            observations,
        };
        let clock = || self.public_media_tick();
        review_public_media_approval(b, Some(&confirmation), &self.fence, op, &clock)?;
        review_public_media_approval(b, Some(&confirmation), &self.fence, op, &clock)?;
        // No network/model calls or extra owner are introduced by this transaction.
        let tx = self.conn.transaction().wb()?;
        tx.execute("INSERT INTO public_media_approvals(workspace_id,approval_id,session_id,epoch,principal_id,role_revision,bundle_sha256,bundle,state,expires_tick,expires_at,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'approved',?9,?10,?11,?11)",params![b.workspace_id,b.approval_id,b.session_id,int(b.epoch)?,actor,int(role_revision)?,b.approved_sha256,b.encode_to_vec(),int(until)?,now+i64::from(request.ttl_ms),now]).wb()?;
        audit(
            &tx,
            &b.workspace_id,
            actor,
            "public_media_approve",
            &b.approval_id,
            now,
        )?;
        op.check()?;
        self.fence.check(self.fence.generation())?;
        tx.commit().wb()?;
        Ok(())
    }
    /// Records a request or observed submission, never a hardware completion ACK.
    pub fn record_public_execution(
        &mut self,
        op: &Operation,
        actor: &str,
        b: &PublicMediaBundle,
        action: &str,
    ) -> Result<()> {
        op.check()?;
        owner(self, &b.workspace_id, actor)?;
        if ![
            "public_execution_requested",
            "public_execution_stopped",
            "public_submission_complete",
        ]
        .contains(&action)
        {
            return Err(Error::InvalidArgument);
        }
        let now = utc()?;
        let tx = self.conn.transaction().wb()?;
        crate::live::output(&tx, &self.root, &self.fence, op, actor, b, action, now)?;
        audit(&tx, &b.workspace_id, actor, action, &b.approval_id, now)?;
        op.check()?;
        tx.commit().wb()?;
        Ok(())
    }
    pub fn revoke_public_media(
        &mut self,
        op: &Operation,
        actor: &str,
        ws: &str,
        id: &str,
    ) -> Result<()> {
        op.check()?;
        owner(self, ws, actor)?;
        uuid(id)?;
        let exists:bool=self.conn.query_row("SELECT EXISTS(SELECT 1 FROM public_media_approvals WHERE workspace_id=?1 AND approval_id=?2)",params![ws,id],|r|r.get(0)).wb()?;
        if !exists {
            return Err(Error::NotFound);
        }
        // Durable deny first. An audit/commit failure never restores the revoked permission.
        root::revoke(&self.root, &self.fence, &format!("public:{ws}:{id}"))?;
        let now = utc()?;
        let tx = self.conn.transaction().wb()?;
        tx.execute("UPDATE public_media_approvals SET state='revoked',updated_at=?1 WHERE workspace_id=?2 AND approval_id=?3",params![now,ws,id]).wb()?;
        audit(&tx, ws, actor, "public_media_revoke", id, now)?;
        op.check()?;
        tx.commit().wb()?;
        Ok(())
    }
}
impl PublicMediaAuthority for StoredPublicMediaAuthority<'_> {
    fn read(&self, b: &PublicMediaBundle, op: &Operation) -> Result<PublicMediaAuthorityState> {
        let revision = source(self.store, op, self.actor, b)?;
        self.read_ledger(b, op, revision)
    }
}
impl StoredPublicMediaAuthority<'_> {
    fn read_ledger(
        &self,
        b: &PublicMediaBundle,
        op: &Operation,
        revision: u64,
    ) -> Result<PublicMediaAuthorityState> {
        let s = self.store;
        let row:Option<ApprovalRow>=s.conn.query_row(
            "SELECT principal_id,role_revision,bundle_sha256,bundle,state,expires_tick,expires_at,created_at FROM public_media_approvals WHERE workspace_id=?1 AND approval_id=?2",
            params![b.workspace_id,b.approval_id],|r|Ok(ApprovalRow {actor:r.get(0)?,role_revision:uint(r,1)?,digest:r.get(2)?,bytes:r.get(3)?,status:r.get(4)?,expires:uint(r,5)?,expires_at:r.get(6)?,created:r.get(7)?}),
        ).optional().wb()?;
        let ApprovalRow {
            actor,
            role_revision,
            digest,
            bytes,
            status,
            expires,
            expires_at,
            created,
        } = row.ok_or(Error::PermissionDenied)?;
        if actor != self.actor || role_revision != revision {
            return Err(Error::RevisionConflict);
        }
        let now = utc()?;
        if status != "approved"
            || now < created
            || now >= expires_at
            || s.public_media_tick() >= expires
        {
            return Err(Error::PermissionDenied);
        }
        if digest != public_media_hash(b) || bytes != b.encode_to_vec() {
            return Err(Error::IntegrityFailed);
        }
        let mut state = bind_observation(b, op, self.observations)?;
        state.valid_until_tick = state.valid_until_tick.min(expires);
        s.fence.check_scope(&scope(b))?;
        s.fence.check(s.fence.generation())?;
        Ok(state)
    }
}

// A request-scoped proof cannot outlive the Operation or immutable Store borrow.
// No decision, current version, or file body is reused across ordinary requests.
struct SourceProof {
    digest: String,
    revision: u64,
    expires: Option<i64>,
    checked: i64,
    generation: u64,
    scopes: Vec<String>,
}
pub struct ScopedPublicMediaAuthority<'a> {
    base: StoredPublicMediaAuthority<'a>,
    operation: &'a Operation,
    proof: std::cell::RefCell<Option<SourceProof>>,
}
impl Store {
    pub fn scoped_public_media_authority<'a>(
        &'a self,
        actor: &'a str,
        op: &'a Operation,
        observations: Option<&'a dyn PublicMediaAuthority>,
    ) -> ScopedPublicMediaAuthority<'a> {
        ScopedPublicMediaAuthority {
            base: self.public_media_authority(actor, observations),
            operation: op,
            proof: Default::default(),
        }
    }
}
impl PublicMediaAuthority for ScopedPublicMediaAuthority<'_> {
    fn read(&self, b: &PublicMediaBundle, op: &Operation) -> Result<PublicMediaAuthorityState> {
        if !std::ptr::eq(op, self.operation) {
            return self.base.read(b, op);
        }
        op.check()?;
        let store = self.base.store;
        let digest = public_media_hash(b);
        let now = utc()?;
        let mut proof = self.proof.try_borrow_mut().map_err(|_| Error::Busy)?;
        if let Some(p) = proof.as_ref().filter(|p| p.digest == digest) {
            p.check(&store.fence, op, now)?;
            let state = self.base.read_ledger(b, op, p.revision)?;
            p.check(&store.fence, op, utc()?)?;
            return Ok(state);
        }
        let generation = store.fence.generation();
        let scopes = store.fence.revocation_scopes()?;
        let (revision, expires) = source_with_expiry(store, op, self.base.actor, b)?;
        store.fence.check(generation)?;
        if store.fence.revocation_scopes()? != scopes
            || expires.is_some_and(|until| utc().map_or(true, |now| now >= until))
        {
            return Err(Error::Cancelled);
        }
        let checked = utc()?;
        if checked < now || expires.is_some_and(|until| checked >= until) {
            return Err(Error::Cancelled);
        }
        *proof = Some(SourceProof {
            digest,
            revision,
            expires,
            checked,
            generation,
            scopes,
        });
        let state = self.base.read_ledger(b, op, revision)?;
        proof
            .as_ref()
            .ok_or(Error::IntegrityFailed)?
            .check(&store.fence, op, utc()?)?;
        Ok(state)
    }
}
impl SourceProof {
    fn check(&self, fence: &StopFence, op: &Operation, now: i64) -> Result<()> {
        op.check()?;
        fence.check(self.generation)?;
        if now < self.checked
            || self.expires.is_some_and(|until| now >= until)
            || fence.revocation_scopes()? != self.scopes
        {
            return Err(Error::Cancelled);
        }
        Ok(())
    }
}
#[cfg(test)]
mod source_proof_tests {
    use super::*;
    #[test]
    fn source_proof_does_not_cache_time_cancellation_or_scoped_revocation() {
        let fence = StopFence::default();
        let op = Operation::new(4000).unwrap();
        let p = SourceProof {
            digest: "x".into(),
            revision: 1,
            expires: Some(150),
            checked: 100,
            generation: 0,
            scopes: vec![],
        };
        assert!(p.check(&fence, &op, 100).is_ok());
        assert!(p.check(&fence, &op, 149).is_ok());
        assert!(p.check(&fence, &op, 99).is_err());
        assert!(p.check(&fence, &op, 150).is_err());
        fence.revoke_scope("changed").unwrap();
        assert!(p.check(&fence, &op, 110).is_err());
        let fresh = StopFence::default();
        op.cancel();
        assert!(p.check(&fresh, &op, 110).is_err());
    }
}
