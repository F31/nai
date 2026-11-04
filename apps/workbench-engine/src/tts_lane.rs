#[path = "media_pipe.rs"]
mod media_pipe;
#[path = "prepared_media.rs"]
mod prepared_media;
#[path = "public_approval.rs"]
mod public_approval;
#[path = "tts_audio.rs"]
mod tts_audio;
// Bounded fixed-phrase test lane; private cache after validation, never played.
use nai_contracts::workbench::*;
use nai_foundation_sqlite::{Error, Store};
use nai_runtime_core::foundation::Operation;
use sha2::{Digest, Sha256};
use std::sync::mpsc;
fn diagnostic_direct(command: &FoundationCommand) -> bool {
    cfg!(debug_assertions)
        && matches!(
            command.operation,
            Some(foundation_command::Operation::TestTts(_))
        )
        && std::env::var("NAI_TTS_DIAGNOSTIC_DIRECT").as_deref() == Ok("1")
}
struct ExecutionRegistration {
    key: zeroize::Zeroizing<String>,
    preparation_id: String,
    seq: u64,
    active: bool,
    requested: bool,
}
enum PublicAction<'a> {
    Inspect,
    AutoConfirm(public_approval::Draft, &'a LiveAutoConfirm),
    Confirm(public_approval::Draft, &'a FoundationPublicApproval),
    Prepare(public_approval::Draft, &'a FoundationPublicWindowPrepare),
    Execute(
        public_approval::Draft,
        ExecutionRegistration,
        &'a FoundationPublicWindowPrepare,
        bool,
    ),
}
struct Reply {
    started: bool,
    error: String,
    count: u64,
    digest: String,
    samples: zeroize::Zeroizing<Vec<i16>>,
}
struct Active {
    ws: String,
    id: String,
    cancel: Operation,
    rx: mpsc::Receiver<Reply>,
    deadline: std::time::Instant,
    settled: bool,
}
pub struct Lane {
    active: Option<Active>,
    avatar: Result<nai_media_core::public_avatar::BuiltinAvatar, Error>,
    approval_draft: Option<public_approval::Draft>,
    approved: Option<public_approval::Draft>,
    auto_approval_id: Option<String>,
    window_file: Option<tempfile::NamedTempFile>,
    execution: Option<ExecutionRegistration>,
    allow: bool,
    cache: tts_audio::Cache,
    player: crate::audition::Player,
    output_binding: nai_runtime_core::public_output_binding::OutputBindingGate,
    public_stop: std::path::PathBuf,
    prepared: prepared_media::Bank,
}
impl Lane {
    pub fn public_candidate(
        &mut self,
        store: &mut Store,
        actor: &str,
        op: &Operation,
        request: &FoundationPublicCandidate,
    ) -> Result<FoundationResult, Error> {
        self.approval_draft = None;
        self.inspect_public_candidate(store, actor, op, request, PublicAction::Inspect)
    }
    pub fn confirm_public_approval(
        &mut self,
        store: &mut Store,
        actor: &str,
        op: &Operation,
        x: &FoundationPublicApproval,
    ) -> Result<FoundationResult, Error> {
        let draft = self.approval_draft.take().ok_or(Error::RevisionConflict)?;
        let request = x.candidate.as_ref().ok_or(Error::InvalidArgument)?;
        self.inspect_public_candidate(store, actor, op, request, PublicAction::Confirm(draft, x))
    }
    pub fn confirm_live_auto(
        &mut self,
        store: &mut Store,
        actor: &str,
        op: &Operation,
        x: &LiveAutoConfirm,
    ) -> Result<FoundationResult, Error> {
        let draft = self.approval_draft.take().ok_or(Error::RevisionConflict)?;
        self.inspect_public_candidate(
            store,
            actor,
            op,
            x.candidate.as_ref().ok_or(Error::InvalidArgument)?,
            PublicAction::AutoConfirm(draft, x),
        )
    }
    pub fn prepare_public_window(
        &mut self,
        store: &mut Store,
        actor: &str,
        op: &Operation,
        x: &FoundationPublicWindowPrepare,
    ) -> Result<FoundationResult, Error> {
        let approved = self.approved.clone().ok_or(Error::PermissionDenied)?;
        let request = x.candidate.as_ref().ok_or(Error::InvalidArgument)?;
        self.inspect_public_candidate(
            store,
            actor,
            op,
            request,
            PublicAction::Prepare(approved, x),
        )
    }
    pub fn execute_public(
        &mut self,
        store: &mut Store,
        actor: &str,
        op: &Operation,
        x: &FoundationPublicWindowPrepare,
        start: bool,
    ) -> Result<FoundationResult, Error> {
        let registration = self.execution.take().ok_or(Error::PermissionDenied)?;
        let approved = self.approved.clone().ok_or(Error::PermissionDenied)?;
        let request = x.candidate.as_ref().ok_or(Error::InvalidArgument)?;
        self.inspect_public_candidate(
            store,
            actor,
            op,
            request,
            PublicAction::Execute(approved, registration, x, start),
        )
    }
    pub fn stop_public(
        &mut self,
        store: &mut Store,
        actor: &str,
        op: &Operation,
        x: &PublicApprovalRevoke,
    ) -> Result<FoundationResult, Error> {
        let b = self
            .approved
            .as_ref()
            .and_then(|a| a.view.bundle.as_ref())
            .ok_or(Error::PermissionDenied)?;
        if b.workspace_id != x.workspace_id || b.approval_id != x.approval_id {
            return Err(Error::PermissionDenied);
        }
        let b = b.clone();
        self.execution = None;
        self.window_file = None;
        store.record_public_execution(op, actor, &b, "public_execution_stopped")?;
        Ok(FoundationResult::default())
    }
    pub fn revoke_public_approval(
        &mut self,
        store: &mut Store,
        actor: &str,
        op: &Operation,
        x: &PublicApprovalRevoke,
    ) -> Result<FoundationResult, Error> {
        self.approval_draft = None;
        self.approved = None;
        self.window_file = None;
        self.execution = None;
        store.revoke_public_media(op, actor, &x.workspace_id, &x.approval_id)?;
        Ok(FoundationResult {
            resource_id: x.approval_id.clone(),
            public_approval: Some(Box::new(PublicApprovalView {
                revoked: true,
                ..Default::default()
            })),
            ..Default::default()
        })
    }
    fn inspect_public_candidate(
        &mut self,
        store: &mut Store,
        actor: &str,
        op: &Operation,
        request: &FoundationPublicCandidate,
        action: PublicAction<'_>,
    ) -> Result<FoundationResult, Error> {
        let bound = self.output_binding.consume(
            request.output_observation.as_ref(),
            store.public_media_tick(),
            &crate::audition::nonce(&self.public_stop)?,
            op,
        )?;
        let source = store.public_candidate_review(op, actor, request)?;
        let mut check = source.check().clone();
        let expected = check.audio.take().ok_or(Error::IntegrityFailed)?;
        let axis = check
            .axes
            .iter_mut()
            .find(|a| a.axis == "audio")
            .ok_or(Error::IntegrityFailed)?;
        match self.cache.observe(
            &request.workspace_id,
            &request.task_id,
            &request.task_sha256,
            std::time::Instant::now(),
            op,
        ) {
            Ok(actual) => {
                if actual != expected {
                    return Err(Error::IntegrityFailed);
                }
                axis.state = PublicCandidateAxisState::Passed as i32;
                axis.reason = "ACTUAL_PCM_VERIFIED".into();
                check.audio = Some(actual);
            }
            Err(Error::CapabilityUnavailable | Error::DeadlineExceeded) => {
                axis.reason = "PCM_EXPIRED_OR_RELEASED".into();
            }
            Err(e) => return Err(e),
        }
        let avatar = self.avatar.as_ref().map_err(|e| *e)?;
        check.visual = Some(avatar.observe(&|| op.check())?);
        check.visual_name = nai_media_core::public_avatar::NAME.into();
        check.visual_license = nai_media_core::public_avatar::LICENSE_ID.into();
        let visual = check
            .axes
            .iter_mut()
            .find(|a| a.axis == "visual")
            .ok_or(Error::IntegrityFailed)?;
        visual.state = PublicCandidateAxisState::Passed as i32;
        visual.reason = "BUILTIN_VISUAL_AND_LICENSE_VERIFIED".into();
        let media = check
            .axes
            .iter_mut()
            .find(|a| a.axis == "media")
            .ok_or(Error::IntegrityFailed)?;
        if check.audio.is_some() {
            match self.prepared.observe(
                op,
                &request.workspace_id,
                &request.task_id,
                &request.task_sha256,
                check.visual.as_ref().ok_or(Error::IntegrityFailed)?,
            ) {
                Ok((view, receipt)) => {
                    check.prepared_media = Some(view);
                    check.prepared_media_receipt = receipt;
                    media.state = PublicCandidateAxisState::Passed as i32;
                    media.reason = "MEDIA_ASSET_PREPARED".into();
                }
                Err(Error::CapabilityUnavailable) => {
                    media.reason = "MEDIA_ASSET_NOT_PREPARED".into()
                }
                Err(e) => return Err(e),
            }
        }
        let output = check
            .axes
            .iter_mut()
            .find(|a| a.axis == "output")
            .ok_or(Error::IntegrityFailed)?;
        if let Some(b) = &bound {
            b.recheck(
                source.tick(),
                &crate::audition::nonce(&self.public_stop)?,
                op,
            )?;
            if b.status().visual != check.visual
                || b.status().visual_name != check.visual_name
                || b.status().visual_license != check.visual_license
            {
                return Err(Error::IntegrityFailed);
            }
            check.output_instance = Some(b.status().clone());
            output.reason = "PUBLIC_OUTPUT_STANDBY_ONLY".into();
        } else {
            output.reason = "PUBLIC_OUTPUT_NOT_BOUND".into();
        }
        let (store, mut review) = source.review(&check)?;
        // A renewal already has a playing cursor; only new preparation/start
        // requires enough time for the entire source clip.
        if review.policy_passed && !matches!(&action, PublicAction::Execute(_, _, _, false)) {
            let required = nai_runtime_core::public_guard::required_playback_window_ms(
                check.audio.as_ref().ok_or(Error::IntegrityFailed)?,
            )?;
            if review.valid_until_ms.saturating_sub(review.checked_at_ms) < required {
                review.policy_passed = false;
                review.reason = PublicGuardReason::PlaybackWindowTooShort as i32;
            }
        }
        let guard = check
            .axes
            .iter_mut()
            .find(|a| a.axis == "guard")
            .ok_or(Error::IntegrityFailed)?;
        guard.state = if review.policy_passed {
            PublicCandidateAxisState::Passed as i32
        } else if review.reason == PublicGuardReason::MediaNotReady as i32 {
            PublicCandidateAxisState::NotReady as i32
        } else {
            PublicCandidateAxisState::Blocked as i32
        };
        guard.reason =
            match PublicGuardReason::try_from(review.reason).map_err(|_| Error::IntegrityFailed)? {
                PublicGuardReason::ExactSpecPassed => "GUARD_EXACT_SPEC_PASSED",
                PublicGuardReason::ExactFeaturePassed => "GUARD_EXACT_FEATURE_PASSED",
                PublicGuardReason::PlaybackWindowTooShort => "GUARD_PLAYBACK_WINDOW_TOO_SHORT",
                PublicGuardReason::UnsupportedSpeech => "GUARD_UNSUPPORTED_SPEECH",
                PublicGuardReason::RiskyContent => "GUARD_RISKY_CONTENT",
                PublicGuardReason::UnsupportedRule => "GUARD_UNSUPPORTED_RULE",
                PublicGuardReason::ProhibitedPhrase => "GUARD_PROHIBITED_PHRASE",
                PublicGuardReason::MediaNotReady => "GUARD_MEDIA_NOT_READY",
                PublicGuardReason::OutputNotBound => "GUARD_OUTPUT_NOT_BOUND",
                _ => return Err(Error::IntegrityFailed),
            }
            .into();
        check.guard_review = Some(review);
        if let Some(b) = &bound {
            b.recheck(
                store.public_media_tick(),
                &crate::audition::nonce(&self.public_stop)?,
                op,
            )?;
        }
        if let PublicAction::AutoConfirm(draft, x) = action {
            store.validate_live_auto_candidate(op, actor, x, &check)?;
            let confirmation = FoundationPublicApproval {
                candidate: x.candidate.clone(),
                bundle_sha256: x.bundle_sha256.clone(),
                acknowledge_public_rights: true,
            };
            let retained = draft.clone();
            let result = draft.confirm(public_approval::Confirmation {
                check: &check,
                request: &confirmation,
                bound: bound.as_ref().ok_or(Error::CapabilityUnavailable)?,
                store,
                actor,
                op,
                path: &self.public_stop,
            })?;
            self.auto_approval_id = result
                .public_approval
                .as_ref()
                .and_then(|a| a.bundle.as_ref())
                .map(|b| b.approval_id.clone());
            self.approved = Some(retained);
            return Ok(result);
        }
        if let PublicAction::Confirm(draft, x) = action {
            let retained = draft.clone();
            let result = draft.confirm(public_approval::Confirmation {
                check: &check,
                request: x,
                bound: bound.as_ref().ok_or(Error::CapabilityUnavailable)?,
                store,
                actor,
                op,
                path: &self.public_stop,
            })?;
            self.approved = Some(retained);
            return Ok(result);
        }
        if let PublicAction::Execute(approved, mut registration, x, start) = action {
            let binding = bound.as_ref().ok_or(Error::CapabilityUnavailable)?;
            let b = approved
                .view
                .bundle
                .as_ref()
                .ok_or(Error::IntegrityFailed)?;
            let p = binding
                .status()
                .window_prepared
                .as_ref()
                .ok_or(Error::CapabilityUnavailable)?;
            let receipt = p.receipt.as_ref().ok_or(Error::IntegrityFailed)?;
            if p.submission_complete
                && registration.active
                && !start
                && p.preparation_id == registration.preparation_id
                && p.execution_seq > 0
                && p.execution_seq <= registration.seq
                && p.submitted_samples == receipt.sample_count
                && receipt.consumer_instance_id == b.output_id
                && receipt.descriptor_sha256 == b.prepared_media_sha256
                && receipt.pcm_sha256 == b.audio.as_ref().ok_or(Error::IntegrityFailed)?.pcm_sha256
            {
                let confirmation = FoundationPublicApproval {
                    candidate: Some(request.clone()),
                    bundle_sha256: x.bundle_sha256.clone(),
                    acknowledge_public_rights: false,
                };
                approved.check_window_preparation(public_approval::Confirmation {
                    check: &check,
                    request: &confirmation,
                    bound: binding,
                    store,
                    actor,
                    op,
                    path: &self.public_stop,
                })?;
                self.window_file = None;
                store.record_public_execution(op, actor, b, "public_submission_complete")?;
                self.cache.revoke(&request.workspace_id, &request.task_id);
                self.prepared.retain(&self.cache.references());
                return Ok(FoundationResult {
                    public_window_prepared: Some(Box::new((**p).clone())),
                    ..Default::default()
                });
            }
            if p.preparation_id != registration.preparation_id
                || receipt.descriptor_sha256 != b.prepared_media_sha256
                || receipt.consumer_instance_id != b.output_id
                || receipt.pcm_sha256 != b.audio.as_ref().ok_or(Error::IntegrityFailed)?.pcm_sha256
                || receipt.sample_count
                    != b.audio.as_ref().ok_or(Error::IntegrityFailed)?.sample_count
                || !p.caption_ready
                || p.caption_state != PublicCaptionState::Ready as i32
                || p.caption_sha256 != b.caption_sha256
                || p.execution_stopped
                || !p.device_ready
                || p.execution_challenge == 0
                || p.execution_seq != registration.seq
                || start == registration.active
                || p.playing != registration.active
            {
                return Err(Error::PermissionDenied);
            }
            let confirmation = FoundationPublicApproval {
                candidate: Some(request.clone()),
                bundle_sha256: x.bundle_sha256.clone(),
                acknowledge_public_rights: false,
            };
            if start {
                approved.check_start_window(store.public_media_tick())?;
            }
            registration.seq = registration
                .seq
                .checked_add(1)
                .ok_or(Error::ResourceExhausted)?;
            let lease = approved.execution(
                public_approval::Confirmation {
                    check: &check,
                    request: &confirmation,
                    bound: binding,
                    store,
                    actor,
                    op,
                    path: &self.public_stop,
                },
                nai_runtime_core::public_execution::IssueRequest {
                    authorization: registration.key.as_str(),
                    preparation_id: &registration.preparation_id,
                    challenge: p.execution_challenge,
                    seq: registration.seq,
                    max_lease_ms: 500,
                },
            )?;
            if start && !registration.requested {
                store.record_public_execution(op, actor, b, "public_execution_requested")?;
            }
            registration.active = true;
            self.execution = Some(registration);
            return Ok(FoundationResult {
                private_execution_lease: Some(Box::new(lease)),
                ..Default::default()
            });
        }
        if let PublicAction::Prepare(approved, x) = action {
            if self.execution.as_ref().is_some_and(|e| e.active) {
                if bound.as_ref().is_some_and(|b| {
                    b.status()
                        .window_prepared
                        .as_ref()
                        .is_some_and(|p| p.playing)
                }) {
                    return Err(Error::Busy);
                }
                self.execution = None;
            }
            let binding = bound.as_ref().ok_or(Error::CapabilityUnavailable)?;
            let confirmation = FoundationPublicApproval {
                candidate: Some(request.clone()),
                bundle_sha256: x.bundle_sha256.clone(),
                acknowledge_public_rights: false,
            };
            approved.check_window_preparation(public_approval::Confirmation {
                check: &check,
                request: &confirmation,
                bound: binding,
                store,
                actor,
                op,
                path: &self.public_stop,
            })?;
            let (file, descriptor) = self.prepared.export(
                op,
                &request.workspace_id,
                &request.task_id,
                &request.task_sha256,
                check.visual.as_ref().ok_or(Error::IntegrityFailed)?,
            )?;
            binding.recheck(
                store.public_media_tick(),
                &crate::audition::nonce(&self.public_stop)?,
                op,
            )?;
            let mut bytes = [0u8; 32];
            ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut bytes)
                .map_err(|_| Error::CapabilityUnavailable)?;
            let key = zeroize::Zeroizing::new(
                bytes.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            );
            bytes.fill(0);
            let preparation_id = media_pipe::random_id()?;
            let transfer = PublicWindowPreparation {
                bundle: approved.view.bundle.clone(),
                preparation_id: preparation_id.clone(),
                execution_key: key.to_string(),
                caption_text: approved.view.text.clone(),
                descriptor: Some(descriptor),
                data_path: file.path().to_str().ok_or(Error::DataRootRejected)?.into(),
                output_id: binding.status().output_id.clone(),
                bundle_sha256: x.bundle_sha256.clone(),
            };
            let bundle = approved
                .view
                .bundle
                .as_ref()
                .ok_or(Error::IntegrityFailed)?;
            let requested = self.auto_approval_id.as_deref() == Some(bundle.approval_id.as_str());
            if requested {
                store.record_public_execution(op, actor, bundle, "public_execution_requested")?;
            }
            self.execution = Some(ExecutionRegistration {
                requested,
                key,
                preparation_id,
                seq: 0,
                active: false,
            });
            self.window_file = Some(file);
            return Ok(FoundationResult {
                private_window_preparation: Some(Box::new(transfer)),
                ..Default::default()
            });
        }
        self.approval_draft = if store.can_approve_public_media(op, actor, &request.workspace_id)? {
            public_approval::Draft::new(&check, store.public_media_tick())?
        } else {
            None
        };
        check.approval_draft = self
            .approval_draft
            .as_ref()
            .map(|d| Box::new(d.view.clone()));
        check.approval_available = check.approval_draft.is_some();
        // Business output remains not-ready. Content approval is not a public execution permit.
        op.check()?;
        Ok(FoundationResult {
            public_candidate_check: Some(Box::new(check)),
            ..Default::default()
        })
    }
    pub fn prepare_output_binding(&mut self, tick: u64) -> Result<FoundationResult, Error> {
        Ok(FoundationResult {
            public_output_challenge: Some(self.output_binding.prepare(tick)?),
            ..Default::default()
        })
    }
    pub fn new(
        allow: bool,
        generation: &str,
        fence: std::sync::Arc<nai_runtime_core::foundation::StopFence>,
        root: &std::path::Path,
    ) -> Self {
        let digest = Sha256::digest(generation.as_bytes());
        let mut seed = [0u8; 8];
        seed.copy_from_slice(&digest[..8]);
        Self {
            active: None,
            avatar: nai_media_core::public_avatar::BuiltinAvatar::new(&|| Ok(())),
            approval_draft: None,
            approved: None,
            auto_approval_id: None,
            window_file: None,
            execution: None,
            allow,
            cache: tts_audio::Cache::new(u64::from_le_bytes(seed), fence.clone()),
            player: crate::audition::Player::new(root),
            output_binding: Default::default(),
            public_stop: root.join("public-output.stop"),
            prepared: prepared_media::Bank::new(root, fence),
        }
    }
    pub fn before(
        &mut self,
        store: &mut Store,
        actor: &str,
        command: &FoundationCommand,
    ) -> Result<(), Error> {
        use foundation_command::Operation::*;
        if matches!(
            command.operation,
            Some(TestTts(_) | SynthesizePreviewSpeech(_) | PrepareLiveAuto(_))
        ) {
            self.cache.available(std::time::Instant::now())?;
        }
        if matches!(
            &command.operation,
            Some(
                TestTts(_)
                    | SynthesizePreviewSpeech(_)
                    | PrepareLiveAuto(_)
                    | StartLiveAuto(_)
                    | StartLlm(_)
                    | TestLlm(_)
            )
        ) && self.active.is_some()
        {
            return Err(Error::Busy);
        }
        if let Some(CancelTts(x)) = &command.operation {
            store.tts_current(&Operation::new(4000)?, actor, &x.workspace_id, &x.task_id)?;
            if let Some(a) = &self.active
                && a.ws == x.workspace_id
                && a.id == x.task_id
            {
                a.cancel.cancel();
            }
        }
        Ok(())
    }
    pub fn after(
        &mut self,
        store: &mut Store,
        actor: &str,
        command: &FoundationCommand,
        result: &mut FoundationResult,
    ) -> Result<(), Error> {
        if let Some(foundation_command::Operation::WorkspaceLifecycle(x)) = &command.operation
            && x.action == WorkspaceAction::Recycle as i32
            && let Some(a) = &self.active
            && a.ws == x.workspace_id
        {
            a.cancel.cancel();
        }
        // Retire only artifacts invalidated by the completed mutation, keeping
        // unrelated valid audio intact. The cache holds at most two entries.
        if matches!(
            command.operation,
            Some(
                foundation_command::Operation::StopPreview(_)
                    | foundation_command::Operation::StopLiveAuto(_)
                    | foundation_command::Operation::ControlLiveQuestion(_)
                    | foundation_command::Operation::ControlLiveTakeover(_)
                    | foundation_command::Operation::WorkspaceLifecycle(_)
                    | foundation_command::Operation::RevokePublication(_)
                    | foundation_command::Operation::SaveTtsModel(_)
                    | foundation_command::Operation::DeleteTtsModel(_)
                    | foundation_command::Operation::Grant(_)
                    | foundation_command::Operation::DeleteObject(_)
                    | foundation_command::Operation::Stop(_)
            )
        ) {
            for (ws, id) in self.cache.references() {
                if store
                    .tts_current(&Operation::new(4000)?, actor, &ws, &id)
                    .map_or(true, |r| r.body.is_empty())
                {
                    if self
                        .player
                        .active
                        .as_ref()
                        .is_some_and(|a| a.ws == ws && a.id == id)
                    {
                        self.player.stop("CONTEXT_REVOKED");
                    }
                    self.cache.revoke(&ws, &id);
                }
            }
        }
        if matches!(
            command.operation,
            Some(foundation_command::Operation::StartLiveAuto(_))
        ) {
            self.player.stop("AUTO_START");
            self.cache.clear();
            self.prepared.clear();
        }
        if matches!(
            command.operation,
            Some(foundation_command::Operation::StopLiveAuto(_))
                | Some(foundation_command::Operation::ControlLiveTakeover(_))
                | Some(foundation_command::Operation::ControlLiveQuestion(
                    LiveQuestionControl { cancel: true, .. }
                ))
        ) {
            self.execution = None;
            self.approved = None;
            self.approval_draft = None;
            self.window_file = None;
        }
        let x = match &command.operation {
            Some(foundation_command::Operation::TestTts(x)) => x.clone(),
            Some(foundation_command::Operation::PrepareLiveAuto(x)) => {
                let a = result.live_auto.as_ref().ok_or(Error::IntegrityFailed)?;
                FoundationTtsTest {
                    workspace_id: x
                        .run
                        .as_ref()
                        .ok_or(Error::InvalidArgument)?
                        .workspace_id
                        .clone(),
                    task_id: x.task_id.clone(),
                    model_id: a.model_id.clone(),
                    model_revision: a.model_revision,
                }
            }
            Some(foundation_command::Operation::SynthesizePreviewSpeech(x)) => {
                let p = x.plan.as_ref().ok_or(Error::InvalidArgument)?;
                FoundationTtsTest {
                    workspace_id: p.workspace_id.clone(),
                    task_id: x.task_id.clone(),
                    model_id: p.model_id.clone(),
                    model_revision: p.model_revision,
                }
            }
            _ => return Ok(()),
        };
        if result.task_state != AgentTaskState::Queued as i32 {
            return Ok(());
        }
        let deadline_ms = if matches!(
            command.operation,
            Some(foundation_command::Operation::TestTts(_))
        ) {
            nai_runtime_core::tts::MODEL_TEST_DEADLINE_MS
        } else {
            nai_runtime_core::tts::PREVIEW_DEADLINE_MS
        };
        let prepared =
            store.tts_dispatch(&Operation::new(4000)?, actor, &x.workspace_id, &x.task_id);
        match prepared {
            Ok(p) => {
                let deadline_ms = deadline_ms.min(p.deadline_ms);
                let trace_id = x.task_id.clone();
                let direct = diagnostic_direct(command);
                let cancel = Operation::model_task(deadline_ms)?;
                let flag = cancel.model_child(deadline_ms)?;

                let (tx, rx) = mpsc::sync_channel(1);
                let allow = self.allow;
                let spawn = std::thread::Builder::new()
                    .name("nai-private-tts-test".into())
                    .spawn(move || {
                        let reply = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            let r = nai_llm_openai::tts::run(
                                nai_llm_openai::tts::Request {
                                    protocol: p.protocol,
                                    language: p.language,
                                    base_url: p.base_url,
                                    model: p.model,
                                    voice: p.voice,
                                    input: p.text,
                                    api_key: p.api_key,
                                    deadline_ms,
                                    trace_id,
                                    direct,
                                },
                                flag,
                                allow,
                            );
                            let count = r.samples.len() as u64;
                            let mut hash = Sha256::new();
                            for sample in &r.samples {
                                hash.update(sample.to_le_bytes());
                            }
                            let digest = if count > 0 {
                                hash.finalize().iter().map(|b| format!("{b:02x}")).collect()
                            } else {
                                String::new()
                            };
                            Reply {
                                started: r.request_started,
                                error: if !r.failure_code.is_empty() {
                                    r.failure_code
                                } else {
                                    r.error
                                        .map(|e| e.as_str_name().to_owned())
                                        .unwrap_or_default()
                                },
                                count,
                                digest,
                                samples: zeroize::Zeroizing::new(r.samples),
                            }
                        }))
                        .unwrap_or(Reply {
                            started: true,
                            error: "WORKER_LOST".into(),
                            count: 0,
                            digest: String::new(),
                            samples: zeroize::Zeroizing::new(Vec::new()),
                        });
                        let _ = tx.send(reply);
                    });
                if spawn.is_ok() {
                    self.active = Some(Active {
                        ws: x.workspace_id.clone(),
                        id: x.task_id.clone(),
                        cancel,
                        rx,
                        deadline: std::time::Instant::now()
                            + std::time::Duration::from_millis(u64::from(deadline_ms) + 250),
                        settled: false,
                    });
                } else {
                    store.finish_tts(
                        &Operation::new(4000)?,
                        actor,
                        &x.workspace_id,
                        &x.task_id,
                        false,
                        "WORKER_UNAVAILABLE",
                        0,
                        "",
                    )?;
                }
            }
            Err(_) => store.finish_tts(
                &Operation::new(4000)?,
                actor,
                &x.workspace_id,
                &x.task_id,
                false,
                "PREPARE_FAILED",
                0,
                "",
            )?,
        }
        *result = store.tts_current(&Operation::new(4000)?, actor, &x.workspace_id, &x.task_id)?;
        Ok(())
    }
    pub fn poll(&mut self, store: &mut Store, actor: &str) -> Result<(), Error> {
        if self
            .approved
            .as_ref()
            .is_some_and(|a| a.expired(store.public_media_tick()))
        {
            self.approved = None;
            self.window_file = None;
            self.execution = None;
        }
        self.cache.purge(std::time::Instant::now());
        self.prepared.retain(&self.cache.references());
        if let Some(challenge) = self.player.next()? {
            let a = self
                .player
                .active
                .as_ref()
                .ok_or(Error::CapabilityUnavailable)?;
            let x = FoundationTtsAudio {
                workspace_id: a.ws.clone(),
                task_id: a.id.clone(),
                task_sha256: a.sha.clone(),
            };
            let c = FoundationCommand {
                operation: Some(foundation_command::Operation::AuthorizeTtsAudio(x)),
                ..Default::default()
            };
            match self
                .audio(store, actor, &Operation::new(4000)?, &c)
                .and_then(|r| {
                    self.player.renew_worker(
                        challenge,
                        r.tts_audio.as_ref().ok_or(Error::CapabilityUnavailable)?,
                    )
                }) {
                Ok(()) => (),
                Err(_) => self.player.fail(),
            }
        }

        let Some(a) = &mut self.active else {
            return Ok(());
        };
        let current = store.tts_current(&Operation::new(4000)?, actor, &a.ws, &a.id);
        if current.as_ref().map_or(true, |r| {
            r.task_state != AgentTaskState::Running as i32 || r.task_reason == "RESULT_UNAVAILABLE"
        }) {
            a.cancel.cancel();
        }
        match a.rx.try_recv() {
            Ok(r) => {
                if !a.settled {
                    store.finish_tts(
                        &Operation::new(4000)?,
                        actor,
                        &a.ws,
                        &a.id,
                        r.started,
                        &r.error,
                        r.count,
                        &r.digest,
                    )?;
                    let completed =
                        store.tts_current(&Operation::new(4000)?, actor, &a.ws, &a.id)?;
                    if completed.task_state == AgentTaskState::Completed as i32
                        && !completed.body.is_empty()
                    {
                        self.cache.insert(
                            &a.ws,
                            &a.id,
                            &completed.sha256,
                            &r.digest,
                            r.samples,
                            std::time::Instant::now(),
                        )?;
                        let op = Operation::new(4000)?;
                        let prepared = self
                            .cache
                            .preparation_samples(
                                &a.ws,
                                &a.id,
                                &completed.sha256,
                                std::time::Instant::now(),
                                &op,
                            )
                            .and_then(|samples| {
                                self.prepared.prepare(
                                    &op,
                                    prepared_media::Source {
                                        workspace: &a.ws,
                                        task: &a.id,
                                        task_sha: &completed.sha256,
                                        samples,
                                    },
                                )
                            });
                        if let Err(e) = prepared {
                            eprintln!("MEDIA_PREPARATION_UNAVAILABLE: {}", e.as_str_name());
                        }
                    }
                }
                self.active = None;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                if !a.settled {
                    store.finish_tts(
                        &Operation::new(4000)?,
                        actor,
                        &a.ws,
                        &a.id,
                        true,
                        "WORKER_LOST",
                        0,
                        "",
                    )?;
                }
                self.active = None;
            }
            Err(mpsc::TryRecvError::Empty) => {
                if !a.settled && std::time::Instant::now() >= a.deadline {
                    a.cancel.cancel();
                    store.finish_tts(
                        &Operation::new(4000)?,
                        actor,
                        &a.ws,
                        &a.id,
                        true,
                        "WORKER_STALLED",
                        0,
                        "",
                    )?;
                    a.settled = true;
                }
            }
        }
        Ok(())
    }
    pub fn audio(
        &mut self,
        store: &mut Store,
        actor: &str,
        op: &Operation,
        command: &FoundationCommand,
    ) -> Result<FoundationResult, Error> {
        use foundation_command::Operation::*;
        if let Some(SelectOutputDevice(x)) = command.operation.as_ref() {
            let a = self
                .player
                .active
                .as_ref()
                .ok_or(Error::CapabilityUnavailable)?;
            let r = store.tts_current(op, actor, &x.workspace_id, &x.task_id)?;
            if a.sha != r.sha256 || r.body.is_empty() {
                return Err(Error::CapabilityUnavailable);
            }
            self.player.select(x)?;
            return Ok(FoundationResult {
                audition: Some(self.player.view.clone()),
                ..Default::default()
            });
        }
        if let Some(StopTtsAudition(_)) = command.operation.as_ref() {
            self.player.stop("USER_STOPPED");
            return Ok(FoundationResult {
                audition: Some(self.player.view.clone()),
                ..Default::default()
            });
        }
        if let Some(RenewTtsAudition(x)) = command.operation.as_ref() {
            if let Some(a) = self.player.active.as_mut() {
                if a.ws != x.workspace_id || a.id != x.task_id {
                    return Err(Error::RevisionConflict);
                }
                if std::time::Instant::now() >= a.until {
                    self.player.fail();
                    self.player.view.reason = "GUI_OR_INIT_DEADLINE".into();
                } else {
                    a.until = std::time::Instant::now() + std::time::Duration::from_millis(600);
                }
            }
            return Ok(FoundationResult {
                audition: Some(self.player.view.clone()),
                ..Default::default()
            });
        }
        let start = matches!(command.operation, Some(StartTtsAudition(_)));
        let expected = if let Some(StartTtsAudition(x)) = command.operation.as_ref() {
            x.expected_stop_nonce.as_str()
        } else {
            ""
        };
        let (x, revoke) = match command.operation.as_ref() {
            Some(AuthorizeTtsAudio(x)) => (x, false),
            Some(StartTtsAudition(x)) => (x.audio.as_ref().ok_or(Error::InvalidArgument)?, false),
            Some(RevokeTtsAudio(x)) => (x, true),
            _ => return Err(Error::InvalidArgument),
        };
        let mut current = store.tts_current(op, actor, &x.workspace_id, &x.task_id)?;
        if current.sha256 != x.task_sha256 {
            return Err(Error::RevisionConflict);
        }
        if revoke {
            self.player.stop("AUDIO_RELEASED");
            self.cache.revoke(&x.workspace_id, &x.task_id);
            self.prepared.retain(&self.cache.references());
            return Ok(current);
        }
        if current.task_state != AgentTaskState::Completed as i32 || current.body.is_empty() {
            return Err(Error::CapabilityUnavailable);
        }
        let body: serde_json::Value =
            serde_json::from_str(&current.body).map_err(|_| Error::IntegrityFailed)?;
        let count = body["sample_count"]
            .as_u64()
            .ok_or(Error::IntegrityFailed)?;
        let hash = body["audio_sha256"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?;
        let remaining = body["expires_at_ms"]
            .as_u64()
            .ok_or(Error::IntegrityFailed)?
            .saturating_sub(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|_| Error::DeadlineExceeded)?
                    .as_millis() as u64,
            );
        if remaining == 0 {
            return Err(Error::DeadlineExceeded);
        }
        current.tts_audio = Some(self.cache.authorize(
            &x.workspace_id,
            &x.task_id,
            &current.sha256,
            hash,
            count,
            std::time::Instant::now(),
            remaining.min(500) as u32,
        )?);
        if start {
            let p = current
                .tts_audio
                .as_ref()
                .ok_or(Error::CapabilityUnavailable)?;
            let samples = self
                .cache
                .read(&x.workspace_id, p, std::time::Instant::now())?;
            if let Err(e) = self.player.start(
                &x.workspace_id,
                &x.task_id,
                &current.sha256,
                p,
                samples,
                expected,
            ) {
                self.player.fail();
                return Err(e);
            }
            current.audition = Some(self.player.view.clone());
        }
        op.check()?;
        Ok(current)
    }
    pub fn stop(&mut self, store: &mut Store, actor: &str) {
        self.player.stop("OWNER_STOPPED");
        self.cache.clear();
        self.prepared.clear();
        self.approved = None;
        self.window_file = None;
        self.execution = None;
        self.approval_draft = None;
        if let Some(a) = self.active.take() {
            a.cancel.cancel();
            let _ = store.finish_tts(
                &Operation::new(4000).expect("fixed deadline"),
                actor,
                &a.ws,
                &a.id,
                true,
                "OWNER_STOPPED",
                0,
                "",
            );
        }
    }
}
