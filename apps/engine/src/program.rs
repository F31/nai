//! Finite assisted producer. No automatic advance, resume, approval or hardware ACK.
use super::*;
use nai_control_api::ProgramRequest;
use nai_kernel::evaluate::selling::SEGMENT_ORDER;
use nai_runtime::{PitchInput, PitchRunner};

pub(super) struct Program {
    id: String,
    snapshot: String,
    epoch: u64,
    prompts: Vec<String>,
    runner: PitchRunner,
    started: Instant,
    deadline: Instant,
    phase: &'static str,
    confirmed: usize,
    request: String,
    candidate: String,
    confirmation: Option<Value>,
}

impl Program {
    fn new(r: &ProgramRequest) -> Result<Self> {
        if r.prompts.len() != 4
            || r.prompts
                .iter()
                .any(|p| p.trim().is_empty() || p.chars().count() > 512)
        {
            return Err(ErrorCode::InvalidArgument);
        }
        let segments: Vec<_> = SEGMENT_ORDER.iter().map(|s| s.label()).collect();
        let runner = PitchRunner::start(PitchInput {
            segments: &segments,
            checkpoint: None,
            resume: None,
            entrances: &[],
            // These are prompts, not verified action cues. Generated text still passes the live guards.
            cues: &[],
            current_facts: &[],
            rule_version: "engine-assisted-program-v1",
            cooldown_ms: 1000,
            min_switch_ms: 100,
        })
        .map_err(|e| e.code())?;
        let started = Instant::now();
        Ok(Self {
            id: r.program_id.clone(),
            snapshot: r.snapshot.clone(),
            epoch: r.epoch,
            prompts: r.prompts.clone(),
            runner,
            started,
            deadline: started + Duration::from_secs(900),
            phase: "ready",
            confirmed: 0,
            request: String::new(),
            candidate: String::new(),
            confirmation: None,
        })
    }
    fn view(&self) -> Value {
        let mut view = json!({"program_id":self.id,"snapshot":self.snapshot,"generation_epoch":self.epoch.to_string(),
            "phase":self.phase,"entered":self.runner.progress().0,"confirmed":self.confirmed,
            "max_attempts":4,"request_id":self.request,"candidate_hash":self.candidate,"playback_ack":false});
        if let Some(record) = &self.confirmation {
            view["last_confirmation"] = record.clone();
        }
        view
    }
    fn next(&mut self, r: &ProgramRequest) -> Result<ManualEventRequest> {
        if Instant::now() >= self.deadline {
            self.phase = "paused";
            return Err(ErrorCode::DeadlineExceeded);
        }
        if self.phase != "ready" {
            return Err(ErrorCode::Conflict);
        }
        let entered = self.runner.progress().0;
        if entered >= 4 {
            return Err(ErrorCode::BudgetExceeded);
        }
        self.runner
            .enter(self.started.elapsed().as_millis().min(u32::MAX as u128) as u32)
            .map_err(|e| e.code())?;
        self.phase = "generating";
        self.request = r.command_id.clone();
        self.candidate.clear();
        Ok(ManualEventRequest {
            command_id: r.command_id.clone(),
            epoch: self.epoch,
            text: format!(
                "讲品段落：{}。{}",
                SEGMENT_ORDER[entered].label(),
                self.prompts[entered]
            ),
            deadline: r.deadline.min(self.deadline),
        })
    }
    fn can_confirm(&self, hash: &str, submitted: bool) -> Result<()> {
        if self.phase != "candidate" || hash.is_empty() || self.candidate != hash || !submitted {
            return Err(ErrorCode::Conflict);
        }
        Ok(())
    }
    fn commit_confirmation(
        &mut self,
        hash: &str,
        mut record: Value,
        persist: impl FnOnce(&Value) -> Result<String>,
        recheck: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        self.can_confirm(hash, true)?;
        let reference = persist(&record)?;
        if reference.len() != 64
            || !reference
                .bytes()
                .all(|v| v.is_ascii_hexdigit() && !v.is_ascii_uppercase())
        {
            self.phase = "paused";
            return Err(ErrorCode::Internal);
        }
        record["confirmation_ref"] = json!(reference);
        self.confirmation = Some(record);
        if let Err(code) = recheck() {
            self.phase = "paused";
            return Err(code);
        }
        self.confirm(hash, true)
    }
    fn confirm(&mut self, hash: &str, submitted: bool) -> Result<()> {
        self.can_confirm(hash, submitted)?;
        // Human attestation only. Never feed completed sentence IDs into PitchRunner.
        self.confirmed += 1;
        self.phase = if self.confirmed == 4 { "done" } else { "ready" };
        self.request.clear();
        self.candidate.clear();
        Ok(())
    }
}

impl OperatorLane {
    fn refresh_program(&self, p: &mut Program) -> Result<()> {
        let checked = self.inner.check_epoch(p.epoch, p.deadline);
        if let Err(code) = checked {
            // A contended lock is not evidence of revocation. Other failures permanently pause.
            if code != ErrorCode::Conflict {
                p.phase = "paused";
            }
            return Err(code);
        }
        let knowledge = self
            .inner
            .knowledge
            .try_lock()
            .map_err(|_| ErrorCode::Conflict)?;
        if knowledge.1 != p.snapshot {
            p.phase = "paused";
            return Err(ErrorCode::StaleEpoch);
        }
        if p.phase == "candidate" {
            let previews = self
                .inner
                .previews
                .try_lock()
                .map_err(|_| ErrorCode::Conflict)?;
            if previews
                .iter()
                .any(|v| v.candidate_hash == p.candidate && v.status == "failed")
            {
                p.phase = "failed";
            }
        }
        Ok(())
    }
    pub(super) fn ensure_manual_allowed(&self) -> Result<()> {
        let mut slot = self
            .inner
            .program
            .try_lock()
            .map_err(|_| ErrorCode::Conflict)?;
        if let Some(p) = slot.as_mut() {
            if let Err(ErrorCode::Conflict) = self.refresh_program(p) {
                return Err(ErrorCode::Conflict);
            }
            if !matches!(p.phase, "done" | "failed" | "paused") {
                return Err(ErrorCode::Conflict);
            }
        }
        Ok(())
    }
    pub(super) fn read_program(&self) -> Result<Value> {
        let mut slot = self
            .inner
            .program
            .try_lock()
            .map_err(|_| ErrorCode::Conflict)?;
        if let Some(p) = slot.as_mut() {
            if let Err(ErrorCode::Conflict) = self.refresh_program(p) {
                return Err(ErrorCode::Conflict);
            }
            return Ok(p.view());
        }
        let mut view = json!({"program_id":"","snapshot":"","generation_epoch":"0","phase":"absent",
            "entered":0,"confirmed":0,"max_attempts":4,"request_id":"","candidate_hash":"","playback_ack":false});
        let store = self
            .inner
            .store
            .try_lock()
            .map_err(|_| ErrorCode::Conflict)?;
        if let Some((reference, mut record)) = store
            .recent_program_confirmation(&op()?, &self.inner.workspace)
            .map_err(|e| e.code)?
        {
            record["confirmation_ref"] = json!(reference);
            validate_confirmation(&record)?;
            view["last_confirmation"] = record;
        }
        Ok(view)
    }
    pub(super) fn program_command(&self, r: ProgramRequest) -> Result<()> {
        if (r.action != "start" && !r.prompts.is_empty())
            || (r.action != "confirm" && !r.candidate_hash.is_empty())
            || (r.action == "confirm" && r.candidate_hash.is_empty())
        {
            return Err(ErrorCode::InvalidArgument);
        }
        self.inner.check_epoch(r.epoch, r.deadline)?;
        let mut slot = self
            .inner
            .program
            .try_lock()
            .map_err(|_| ErrorCode::Conflict)?;
        if r.action == "start" {
            // Reserve the single generation lane while checking start. An ordinary manual request
            // already in flight cannot silently overlap the first segment.
            let _permit = self
                .inner
                .capacity
                .clone()
                .try_acquire_owned()
                .map_err(|_| ErrorCode::Conflict)?;
            if slot.as_ref().is_some_and(|p| p.epoch == r.epoch) {
                return Err(ErrorCode::Conflict);
            }
            let knowledge = self
                .inner
                .knowledge
                .try_lock()
                .map_err(|_| ErrorCode::Conflict)?;
            if knowledge.1 != r.snapshot {
                return Err(ErrorCode::StaleEpoch);
            }
            *slot = Some(Program::new(&r)?);
            return Ok(());
        }
        let p = slot.as_mut().ok_or(ErrorCode::NotFound)?;
        self.refresh_program(p)?;
        if p.id != r.program_id || p.snapshot != r.snapshot || p.epoch != r.epoch {
            return Err(ErrorCode::StaleEpoch);
        }
        match r.action.as_str() {
            "next" => {
                let request = p.next(&r)?;
                if let Err(code) = self.enqueue_generation(request, true) {
                    p.phase = "failed";
                    return Err(code);
                }
                Ok(())
            }
            "confirm" => {
                let preview = self
                    .inner
                    .previews
                    .try_lock()
                    .map_err(|_| ErrorCode::Conflict)?
                    .iter()
                    .find(|v| {
                        v.local_playback
                            && v.candidate_hash == r.candidate_hash
                            && v.status == "submitted"
                    })
                    .cloned()
                    .ok_or(ErrorCode::Conflict)?;
                p.can_confirm(&r.candidate_hash, true)?;
                let report = verified_report(&self.inner.artifacts, &preview, r.epoch)?;
                // Reuse the live approval/cue/fact/expiry guard, not just a saved approval flag.
                self.inner
                    .playback_grant(&r.candidate_hash, r.epoch, 1, &report.media_sha256)?;
                self.inner
                    .check_epoch(r.epoch, r.deadline.min(p.deadline))?;
                let record = json!({"event":"program_human_confirmation", "program_id":p.id,
                    "session_id":self.inner.session,"generation_epoch":p.epoch.to_string(),"snapshot":p.snapshot,
                    "segment_number":p.confirmed+1,"candidate_hash":p.candidate,"operator_id":self.inner.actor,
                    "device_played_samples":preview.sample_count.to_string(),"report_sha256":preview.report_sha256,
                    "media_sha256":report.media_sha256,"window_updates":report.window_updates.unwrap_or(0).to_string(),
                    "confirmed_at_utc_ms":now().to_string(),"human_confirmed":true,"playback_ack":false});
                validate_confirmation(&record)?;
                let deadline = r.deadline.min(p.deadline);
                let operation = Operation::new(
                    deadline
                        .saturating_duration_since(Instant::now())
                        .min(Duration::from_secs(5)),
                )
                .map_err(|e| e.code)?;
                p.commit_confirmation(
                    &r.candidate_hash,
                    record,
                    |record| {
                        let ack = self
                            .inner
                            .store
                            .try_lock()
                            .map_err(|_| ErrorCode::Conflict)?
                            .append_decision(
                                &operation,
                                &self.inner.workspace,
                                Some(&self.inner.session),
                                nai_storage::DecisionKind::Cue,
                                record,
                                &r.command_id,
                            )
                            .map_err(|e| e.code)?;
                        if !ack.persisted {
                            return Err(ErrorCode::Internal);
                        }
                        ack.entry_hash.ok_or(ErrorCode::Internal)
                    },
                    || self.inner.check_epoch(r.epoch, deadline),
                )
            }
            _ => Err(ErrorCode::InvalidArgument),
        }
    }
}

fn validate_confirmation(record: &Value) -> Result<()> {
    static SCHEMA: std::sync::OnceLock<std::result::Result<jsonschema::Validator, ErrorCode>> =
        std::sync::OnceLock::new();
    let validator = SCHEMA
        .get_or_init(|| {
            let mut schema: Value = serde_json::from_str(nai_contracts::control_http::SCHEMAS_JSON)
                .map_err(|_| ErrorCode::Internal)?;
            schema["$ref"] = json!("#/$defs/ProgramConfirmation");
            jsonschema::validator_for(&schema).map_err(|_| ErrorCode::Internal)
        })
        .as_ref()
        .map_err(|e| *e)?;
    if !validator.is_valid(record) {
        return Err(ErrorCode::InvalidArgument);
    }
    Ok(())
}

fn verified_report(
    artifacts: &Path,
    preview: &PreviewRecord,
    epoch: u64,
) -> Result<LocalPlaybackStatus> {
    use std::io::Read;
    let expected = format!("{}-{epoch}.playback.pb", preview.candidate_hash);
    if preview.artifact_ref != expected || preview.report_sha256.len() != 64 {
        return Err(ErrorCode::Conflict);
    }
    let path = artifacts.join(expected);
    let metadata = fs::symlink_metadata(&path).map_err(|_| ErrorCode::CapabilityNotReady)?;
    if !metadata.file_type().is_file() || metadata.len() > 16384 {
        return Err(ErrorCode::InvalidArgument);
    }
    let mut bytes = Vec::new();
    fs::File::open(&path)
        .map_err(|_| ErrorCode::CapabilityNotReady)?
        .take(16385)
        .read_to_end(&mut bytes)
        .map_err(|_| ErrorCode::Internal)?;
    if bytes.len() > 16384 || hex_digest(&bytes) != preview.report_sha256 {
        return Err(ErrorCode::Conflict);
    }
    let report =
        LocalPlaybackStatus::decode(bytes.as_slice()).map_err(|_| ErrorCode::InvalidArgument)?;
    crate::local_playback::validate_submission(
        &report,
        &crate::local_playback::CueTarget {
            descriptor: &path,
            media_sha256: &report.media_sha256,
            expected_samples: preview.sample_count,
            expected_frames: preview.frame_count,
        },
    )?;
    Ok(report)
}

pub(super) fn generation_finished(
    inner: &Inner,
    r: &ManualEventRequest,
    result: &std::result::Result<String, GenerationFailure>,
) {
    if let Ok(mut slot) = inner.program.lock()
        && let Some(p) = slot.as_mut()
        && p.request == r.command_id
        && p.epoch == r.epoch
        && p.phase == "generating"
    {
        match result {
            Ok(hash) => {
                p.candidate = hash.clone();
                p.phase = "candidate";
            }
            Err(_) => p.phase = "failed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn confirmation_write_failure_does_not_advance_and_revocation_preserves_audit_only() {
        let mut p = Program::new(&request("start")).unwrap();
        p.next(&request("next")).unwrap();
        p.phase = "candidate";
        p.candidate = "exact".into();
        assert_eq!(
            p.commit_confirmation(
                "exact",
                json!({}),
                |_| Err(ErrorCode::Internal),
                || panic!("must not advance after write failure")
            ),
            Err(ErrorCode::Internal)
        );
        assert_eq!(p.confirmed, 0);
        assert_eq!(p.phase, "candidate");
        assert!(p.confirmation.is_none());
        assert_eq!(
            p.commit_confirmation(
                "exact",
                json!({}),
                |_| Ok("a".repeat(64)),
                || Err(ErrorCode::StaleEpoch)
            ),
            Err(ErrorCode::StaleEpoch)
        );
        assert_eq!(p.confirmed, 0);
        assert_eq!(p.phase, "paused");
        assert_eq!(
            p.confirmation.as_ref().unwrap()["confirmation_ref"],
            "a".repeat(64)
        );
        assert!(p.next(&request("next")).is_err());
    }
    #[test]
    fn persisted_confirmation_advances_exactly_once() {
        let mut p = Program::new(&request("start")).unwrap();
        p.next(&request("next")).unwrap();
        p.phase = "candidate";
        p.candidate = "exact".into();
        p.commit_confirmation("exact", json!({}), |_| Ok("b".repeat(64)), || Ok(()))
            .unwrap();
        assert_eq!(p.confirmed, 1);
        assert_eq!(p.phase, "ready");
        assert!(
            p.commit_confirmation(
                "exact",
                json!({}),
                |_| panic!("duplicate must not persist"),
                || Ok(())
            )
            .is_err()
        );
    }
    #[test]
    fn report_is_bounded_bound_to_epoch_and_rejects_replacement_or_incomplete_progress() {
        let dir = tempfile::tempdir().unwrap();
        let report = LocalPlaybackStatus {
            version: 1,
            state: "submitted".into(),
            copied_samples: Some(24000),
            device_played_samples: Some(24000),
            device_position_observed: true,
            audio_backend: "wasapi-iaudioclock".into(),
            audio_stream_open_count: 1,
            window_updates: Some(10),
            media_sha256: "a".repeat(64),
            video_frame_index: Some(11),
            video_device_samples: Some(24000),
            video_tail_submitted: true,
            ..Default::default()
        };
        let bytes = report.encode_to_vec();
        let mut preview = PreviewRecord {
            local_playback: true,
            candidate_hash: "c".repeat(64),
            status: "submitted".into(),
            message: String::new(),
            sample_count: 24000,
            frame_count: 12,
            artifact_ref: format!("{}-1.playback.pb", "c".repeat(64)),
            audio_sha256: String::new(),
            report_sha256: hex_digest(&bytes),
        };
        let path = dir.path().join(&preview.artifact_ref);
        fs::write(&path, &bytes).unwrap();
        assert!(verified_report(dir.path(), &preview, 1).is_ok());
        assert!(verified_report(dir.path(), &preview, 2).is_err());
        fs::write(&path, b"replaced").unwrap();
        assert!(verified_report(dir.path(), &preview, 1).is_err());
        let mut bad = report.clone();
        bad.device_played_samples = Some(23999);
        let bytes = bad.encode_to_vec();
        preview.report_sha256 = hex_digest(&bytes);
        fs::write(&path, bytes).unwrap();
        assert!(verified_report(dir.path(), &preview, 1).is_err());
        fs::write(&path, vec![0u8; 16385]).unwrap();
        assert!(verified_report(dir.path(), &preview, 1).is_err());
        fs::remove_file(&path).unwrap();
        assert!(verified_report(dir.path(), &preview, 1).is_err());
    }
    fn request(action: &str) -> ProgramRequest {
        ProgramRequest {
            command_id: "next-1".into(),
            epoch: 1,
            deadline: Instant::now() + Duration::from_secs(30),
            action: action.into(),
            program_id: "finite".into(),
            snapshot: "published".into(),
            prompts: vec!["卖点".into(), "规格".into(), "场景".into(), "售后".into()],
            candidate_hash: String::new(),
        }
    }
    #[test]
    fn fixed_order_single_attempt_and_human_confirmation_are_distinct() {
        let mut p = Program::new(&request("start")).unwrap();
        for kind in SEGMENT_ORDER {
            p.started = Instant::now() - Duration::from_secs((p.confirmed as u64 + 1) * 2);
            let r = p.next(&request("next")).unwrap();
            assert!(r.text.contains(kind.label()));
            assert!(p.next(&request("next")).is_err());
            p.phase = "candidate";
            p.candidate = "exact".into();
            assert!(p.confirm("other", true).is_err());
            assert!(p.confirm("exact", false).is_err());
            p.confirm("exact", true).unwrap();
        }
        assert_eq!(p.phase, "done");
        assert_eq!(p.runner.progress().0, 4);
        assert!(p.next(&request("next")).is_err());
        assert_eq!(p.view()["playback_ack"], false);
    }
    #[test]
    fn plan_is_bounded_and_failed_attempt_never_retries() {
        let mut r = request("start");
        r.prompts.pop();
        assert!(Program::new(&r).is_err());
        let mut p = Program::new(&request("start")).unwrap();
        p.next(&request("next")).unwrap();
        p.phase = "failed";
        assert!(p.next(&request("next")).is_err());
        assert_eq!(p.runner.progress().0, 1);
        assert!(p.deadline.duration_since(p.started) <= Duration::from_secs(900));
    }
    #[test]
    fn expired_program_cannot_dispatch_and_entry_cooldown_is_enforced() {
        let mut p = Program::new(&request("start")).unwrap();
        p.deadline = Instant::now();
        assert_eq!(
            p.next(&request("next")).err(),
            Some(ErrorCode::DeadlineExceeded)
        );
        assert_eq!(p.runner.progress().0, 0);
        let mut p = Program::new(&request("start")).unwrap();
        p.next(&request("next")).unwrap();
        p.phase = "candidate";
        p.candidate = "exact".into();
        p.confirm("exact", true).unwrap();
        assert_eq!(p.next(&request("next")).err(), Some(ErrorCode::Conflict));
        assert_eq!(p.runner.progress().0, 1);
    }
}
