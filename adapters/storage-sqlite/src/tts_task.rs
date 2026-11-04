//! Single-writer fixed-phrase TTS tests. PCM never enters the object/control bus.
use super::*;
use crate::llm;
use serde_json::{Value, json};
const KIND: &str = "publication_tts_test";
pub const TEST_TEXT: &str = "你好，这是语音合成测试。";
pub struct TtsDispatch {
    pub deadline_ms: u32,
    pub protocol: String,
    pub language: String,
    pub text: String,
    pub base_url: String,
    pub model: String,
    pub voice: String,
    pub api_key: zeroize::Zeroizing<Vec<u8>>,
}
fn local_profile(m: &Value) -> Result<bool> {
    let local = nai_runtime_core::tts::validate_profile(
        m["protocol"].as_str().unwrap_or(""),
        m["base_url"].as_str().ok_or(Error::IntegrityFailed)?,
        m["language"].as_str().unwrap_or(""),
    )?;
    if local && !m["secret"].is_null() {
        return Err(Error::IntegrityFailed);
    }
    Ok(local)
}
pub(super) fn task(
    conn: &Connection,
    root: &Path,
    actor: &str,
    ws: &str,
    id: &str,
) -> Result<(Value, u64, String)> {
    let (r, s) = object(conn, ws, id, 0)?;
    let v = nai_runtime_core::publication::artifact(&body(root, &s)?)?;
    if !matches!(v["kind"].as_str(), Some(KIND | "publication_tts_preview")) {
        return Err(Error::InvalidArgument);
    }
    if v["created_by"] != actor {
        return Err(Error::PermissionDenied);
    }
    Ok((v, r, s))
}
#[allow(clippy::too_many_arguments)]
fn gate(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    id: &str,
    v: &Value,
    now: i64,
) -> Result<Value> {
    op.check()?;
    fence.check(fence.generation())?;
    crate::live_auto::task_gate(conn, root, fence, op, actor, ws, id, v, now)?;
    let (r, rev) = role(conn, ws, actor)?;
    writer(r)?;
    if v["role_revision"].as_u64() != Some(rev) {
        return Err(Error::PermissionDenied);
    }
    fence.check_scope(&role_scope(ws, actor, rev))?;
    fence.check_scope(&object_scope(ws, id))?;
    if now < v["created_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?
        || now >= v["expires_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?
    {
        return Err(Error::DeadlineExceeded);
    }
    let mid = v["model_ref"]["object_id"]
        .as_str()
        .ok_or(Error::IntegrityFailed)?;
    let (m, r, sha) = llm::raw(conn, root, ws, mid, 0, "publication_tts_config")?;
    let local = local_profile(&m)?;
    fence.check_scope(&object_scope(ws, mid))?;
    if v["model_ref"]["revision"].as_u64() != Some(r)
        || v["model_ref"]["sha256"] != sha
        || m["enabled"] != true
        || m["removed"] == true
        || (m["secret"].is_null() && !local)
    {
        return Err(Error::RevisionConflict);
    }
    if v["kind"] == "publication_tts_preview" {
        let request = FoundationSpeechPlan {
            task_id: id.into(),
            workspace_id: ws.into(),
            preview_id: v["source"]["object_id"]
                .as_str()
                .ok_or(Error::IntegrityFailed)?
                .into(),
            preview_sha256: v["source"]["sha256"]
                .as_str()
                .ok_or(Error::IntegrityFailed)?
                .into(),
            model_id: mid.into(),
            model_revision: r,
            max_cost_minor: v["max_cost_minor"].as_u64().ok_or(Error::IntegrityFailed)?,
        };
        let p = speech_plan(conn, root, fence, op, actor, &request, now, false)?;
        crate::model_context::verify(
            &v["task_context"],
            p.agent_context.as_ref().ok_or(Error::IntegrityFailed)?,
        )?;
        if p.approval_sha256 != v["approval_sha256"] {
            return Err(Error::RevisionConflict);
        }
    }
    Ok(m)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn speech_plan(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    x: &FoundationSpeechPlan,
    now: i64,
    check_budget: bool,
) -> Result<SpeechPlanView> {
    op.check()?;
    uuid(&x.task_id)?;
    let (r, role_rev) = role(conn, &x.workspace_id, actor)?;
    writer(r)?;
    fence.check(fence.generation())?;
    fence.check_scope(&role_scope(&x.workspace_id, actor, role_rev))?;
    let preview = preview::read(
        conn,
        root,
        fence,
        op,
        actor,
        &x.workspace_id,
        &x.preview_id,
        0,
        now,
    )?;
    if preview.state != DeliveryState::Delivering as i32 {
        return Err(Error::CapabilityUnavailable);
    }
    if preview.sha256 != x.preview_sha256 {
        return Err(Error::RevisionConflict);
    }
    let card: Value = serde_json::from_str(&preview.body).map_err(|_| Error::IntegrityFailed)?;
    let context = crate::model_context::from_snapshot(
        conn,
        root,
        fence,
        op,
        &x.workspace_id,
        &card["snapshot"],
        now,
    )?;
    let text = card["text"].as_str().ok_or(Error::IntegrityFailed)?;
    let characters = nai_runtime_core::tts::characters(text)?;
    let (m, rev, sha) = llm::raw(
        conn,
        root,
        &x.workspace_id,
        &x.model_id,
        0,
        "publication_tts_config",
    )?;
    fence.check_scope(&object_scope(&x.workspace_id, &x.model_id))?;
    let local = local_profile(&m)?;
    if rev != x.model_revision
        || m["enabled"] != true
        || m["removed"] == true
        || (m["secret"].is_null() && !local)
    {
        return Err(Error::RevisionConflict);
    }
    let estimate_known = m["price_known"] == true;
    let reserve = nai_runtime_core::tts::reservation(
        text,
        if estimate_known {
            m["character_minor_per_million"].as_u64()
        } else {
            None
        },
        x.max_cost_minor,
    )?;
    if check_budget {
        if llm::active(conn)? {
            return Err(Error::Busy);
        }
        let b = llm::budget(conn, &x.workspace_id)?;
        llm::admit_budget(&b, reserve, x.max_cost_minor)?;
    }
    let approval = json!({"task_context":nai_runtime_core::task_context::to_value(&context),"task_id":x.task_id,"workspace_id":x.workspace_id,"actor":actor,"role_revision":role_rev,"preview_id":x.preview_id,"preview_sha256":preview.sha256,"session_id":card["session_id"],"epoch":card["epoch"],"snapshot":card["snapshot"],"text":text,"model_ref":{"object_id":x.model_id,"revision":rev,"sha256":sha},"voice":m["voice"],"target":"private_native_audio","expires_at_ms":card["expires_at_ms"],"max_cost_minor":x.max_cost_minor,"reserve_minor":reserve,"estimate_known":estimate_known});
    Ok(SpeechPlanView {
        agent_context: Some(context),
        approval_sha256: hash(approval.to_string().as_bytes()),
        text: text.into(),
        characters,
        reserve_minor: reserve,
        expires_at_ms: card["expires_at_ms"]
            .as_u64()
            .ok_or(Error::IntegrityFailed)?,
        target: "private_native_audio".into(),
        voice: m["voice"].as_str().ok_or(Error::IntegrityFailed)?.into(),
        estimate_known,
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn read(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    id: &str,
    now: i64,
) -> Result<FoundationResult> {
    let (v, revision, sha) = task(conn, root, actor, ws, id)?;
    let outbox: String = conn
        .query_row(
            "SELECT state FROM outbox WHERE event_id=?1",
            [format!("tts:{id}")],
            |r| r.get(0),
        )
        .wb()?;
    let state = if matches!(v["state"].as_str(), Some("queued" | "running"))
        && !matches!(outbox.as_str(), "pending" | "dispatching")
    {
        "failed"
    } else {
        v["state"].as_str().ok_or(Error::IntegrityFailed)?
    };
    let testing = v["kind"] == KIND;
    let (status, reserved): (String, u64) = if testing {
        (
            if v["request_started"] != false {
                "unknown"
            } else {
                "not_sent"
            }
            .into(),
            0,
        )
    } else {
        conn.query_row(
            "SELECT status,reserved_minor FROM budget_reservations WHERE workspace_id=?1 AND id=?2",
            params![ws, id],
            |r| Ok((r.get(0)?, uint(r, 1)?)),
        )
        .wb()?
    };
    let valid = gate(conn, root, fence, op, actor, ws, id, &v, now).is_ok();
    Ok(FoundationResult {
        resource_id: id.into(),
        agent_context: if valid && !testing {
            Some(nai_runtime_core::task_context::from_value(
                &v["task_context"],
            )?)
        } else {
            None
        },
        revision,
        sha256: sha,
        task_state: match state {
            "queued" => AgentTaskState::Queued,
            "running" => AgentTaskState::Running,
            "completed" => AgentTaskState::Completed,
            "cancelled" => AgentTaskState::Cancelled,
            _ => AgentTaskState::Failed,
        } as i32,
        task_reason: if state == "failed" && v["state"] != "failed" {
            "RESTARTED_NO_RETRY".into()
        } else if !valid {
            "RESULT_UNAVAILABLE".into()
        } else {
            v["reason"].as_str().unwrap_or("UNAVAILABLE").into()
        },
        body: if state == "completed" && valid {
            v.to_string()
        } else {
            String::new()
        },
        charge_status: status,
        reserved_minor: reserved,
        llm_budget: if testing {
            None
        } else {
            Some(llm::budget(conn, ws)?)
        },
        ..Default::default()
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn start(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    role_rev: u64,
    now: i64,
    x: &FoundationTtsTest,
    speech: Option<&FoundationSpeechStart>,
    auto: Option<(&str, &str)>,
) -> Result<FoundationResult> {
    uuid(&x.task_id)?;
    // Shared persistent dispatch limit across text and speech.
    if llm::active(tx)? {
        return Err(Error::Busy);
    }
    let (m, rev, sha) = llm::raw(
        tx,
        root,
        &x.workspace_id,
        &x.model_id,
        0,
        "publication_tts_config",
    )?;
    if rev != x.model_revision {
        return Err(Error::RevisionConflict);
    }
    let rate = if m["price_known"] == true {
        m["character_minor_per_million"].as_u64()
    } else {
        None
    };
    let p = if let Some(s) = speech {
        if s.task_id != s.plan.as_ref().ok_or(Error::InvalidArgument)?.task_id {
            return Err(Error::RevisionConflict);
        }
        let plan = speech_plan(
            tx,
            root,
            fence,
            op,
            actor,
            s.plan.as_ref().ok_or(Error::InvalidArgument)?,
            now,
            true,
        )?;
        if plan.approval_sha256 != s.approval_sha256 {
            return Err(Error::RevisionConflict);
        }
        Some(plan)
    } else {
        None
    };
    let text = p.as_ref().map_or(TEST_TEXT, |p| p.text.as_str());
    let cap = speech
        .and_then(|s| s.plan.as_ref())
        .map(|s| s.max_cost_minor);
    let reserved = if let Some(cap) = cap {
        let reserved = nai_runtime_core::tts::reservation(text, rate, cap)?;
        llm::admit_budget(&llm::budget(tx, &x.workspace_id)?, reserved, cap)?;
        Some(reserved)
    } else {
        None
    };
    let mut v = json!({"schema_version":1,"kind":KIND,"created_at_ms":now,"created_by":actor,"role_revision":role_rev,"model_ref":{"object_id":x.model_id,"revision":rev,"sha256":sha},"state":"queued","reason":"QUEUED","expires_at_ms":now+60000,"request_started":false,"target":"private_workbench","sample_count":0,"audio_sha256":""});
    if let (Some(s), Some(p)) = (speech, p) {
        let source = s.plan.as_ref().ok_or(Error::InvalidArgument)?;
        v["kind"] = "publication_tts_preview".into();
        v["task_context"] = nai_runtime_core::task_context::to_value(
            p.agent_context.as_ref().ok_or(Error::IntegrityFailed)?,
        );
        v.as_object_mut()
            .ok_or(Error::IntegrityFailed)?
            .remove("request_started");
        v["reserved_minor"] = reserved.ok_or(Error::IntegrityFailed)?.into();
        v["source"] =
            json!({"object_id":source.preview_id,"revision":1,"sha256":source.preview_sha256});
        v["approval_sha256"] = p.approval_sha256.into();
        v["max_cost_minor"] = source.max_cost_minor.into();
        v["expires_at_ms"] = p.expires_at_ms.into();
    }
    if let Some((run, grant)) = auto {
        v["auto_run_id"] = run.into();
        v["auto_grant_id"] = grant.into();
    }
    gate(
        tx,
        root,
        fence,
        op,
        actor,
        &x.workspace_id,
        &x.task_id,
        &v,
        now,
    )?;
    llm::put(tx, root, &x.workspace_id, &x.task_id, 0, &v, now)?;
    if let Some(reserved) = reserved {
        tx.execute(
            "INSERT INTO budget_reservations VALUES(?1,?2,?3,?4,NULL,'reserved','CNY',1,?5,?5)",
            params![x.workspace_id, x.task_id, actor, int(reserved)?, now],
        )
        .wb()?;
    }
    tx.execute(
        "INSERT INTO outbox VALUES(?1,?2,?3,'pending',0,1,?4,?4)",
        params![format!("tts:{}", x.task_id), x.workspace_id, x.task_id, now],
    )
    .wb()?;
    read(tx, root, fence, op, actor, &x.workspace_id, &x.task_id, now)
}
pub(super) fn cancel(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    actor: &str,
    now: i64,
    x: &FoundationLlmTaskRead,
) -> Result<FoundationResult> {
    let (mut v, rev, _) = task(tx, root, actor, &x.workspace_id, &x.task_id)?;
    if !matches!(
        v["state"].as_str(),
        Some("queued" | "running" | "cancelled")
    ) {
        return Err(Error::RevisionConflict);
    }
    root::revoke(root, fence, &object_scope(&x.workspace_id, &x.task_id))?;
    let state: String = tx
        .query_row(
            "SELECT state FROM outbox WHERE event_id=?1",
            [format!("tts:{}", x.task_id)],
            |r| r.get(0),
        )
        .wb()?;
    let charge = if state == "pending" {
        "released"
    } else {
        "unknown"
    };
    if v["kind"] == KIND {
        v["request_started"] = (state != "pending").into();
    } else {
        tx.execute("UPDATE budget_reservations SET status=?1,revision=revision+1,updated_at=?2 WHERE workspace_id=?3 AND id=?4 AND status='reserved'",params![charge,now,x.workspace_id,x.task_id]).wb()?;
    }
    tx.execute(
        "UPDATE outbox SET state='cancelled',revision=revision+1,updated_at=?1 WHERE event_id=?2",
        params![now, format!("tts:{}", x.task_id)],
    )
    .wb()?;
    v["state"] = "cancelled".into();
    v["reason"] = "CANCELLED".into();
    v["sample_count"] = 0.into();
    v["audio_sha256"] = "".into();
    llm::put(tx, root, &x.workspace_id, &x.task_id, rev, &v, now)?;
    read(
        tx,
        root,
        fence,
        &Operation::new(4000)?,
        actor,
        &x.workspace_id,
        &x.task_id,
        now,
    )
}
impl Store {
    /// Root-only source observation. The audio metadata remains unverified until the live cache
    /// is inspected; Store::execute deliberately cannot return this as a completed check.
    pub fn public_candidate_source(
        &mut self,
        op: &Operation,
        actor: &str,
        x: &FoundationPublicCandidate,
    ) -> Result<PublicCandidateCheck> {
        self.begin(op)?;
        uuid(&x.workspace_id)?;
        uuid(&x.preview_id)?;
        uuid(&x.task_id)?;
        let now = utc()?;
        let (v, _, task_sha) = task(&self.conn, &self.root, actor, &x.workspace_id, &x.task_id)?;
        if v["kind"] != "publication_tts_preview"
            || task_sha != x.task_sha256
            || v["source"]["object_id"] != x.preview_id
            || v["source"]["sha256"] != x.preview_sha256
        {
            return Err(Error::RevisionConflict);
        }
        gate(
            &self.conn,
            &self.root,
            &self.fence,
            op,
            actor,
            &x.workspace_id,
            &x.task_id,
            &v,
            now,
        )?;
        if v["state"] != "completed" {
            return Err(Error::CapabilityUnavailable);
        }
        let preview = preview::read(
            &self.conn,
            &self.root,
            &self.fence,
            op,
            actor,
            &x.workspace_id,
            &x.preview_id,
            self.tick(),
            now,
        )?;
        if preview.state != DeliveryState::Delivering as i32 || preview.sha256 != x.preview_sha256 {
            return Err(Error::PermissionDenied);
        }
        let card: Value =
            serde_json::from_str(&preview.body).map_err(|_| Error::IntegrityFailed)?;
        let snapshot_id = card["snapshot"]["object_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?;
        let (snapshot, _) = publication::stored(
            &self.conn,
            &self.root,
            &x.workspace_id,
            snapshot_id,
            "publication_snapshot",
        )?;
        let text = card["text"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?
            .to_owned();
        let count = v["sample_count"].as_u64().ok_or(Error::IntegrityFailed)?;
        if count == 0 || count > nai_runtime_core::tts::MAX_SAMPLES as u64 {
            return Err(Error::IntegrityFailed);
        }
        let audio = PublicMediaAudio {
            artifact_id: x.task_id.clone(),
            pcm_sha256: v["audio_sha256"]
                .as_str()
                .ok_or(Error::IntegrityFailed)?
                .into(),
            sample_count: count,
            sample_rate: nai_runtime_core::tts::SAMPLE_RATE,
            channels: 1,
        };
        let axis =
            |axis: &str, state: PublicCandidateAxisState, reason: &str| PublicCandidateAxis {
                axis: axis.into(),
                state: state as i32,
                reason: reason.into(),
            };
        op.check()?;
        self.fence.check(self.fence.generation())?;
        Ok(PublicCandidateCheck {
            preview_id: x.preview_id.clone(),
            preview_sha256: x.preview_sha256.clone(),
            task_id: x.task_id.clone(),
            task_sha256: task_sha,
            caption_sha256: hash(text.as_bytes()),
            text,
            snapshot_id: snapshot_id.into(),
            snapshot_sha256: card["snapshot"]["sha256"]
                .as_str()
                .ok_or(Error::IntegrityFailed)?
                .into(),
            facts_sha256: hash(snapshot["refs"].to_string().as_bytes()),
            session_id: card["session_id"]
                .as_str()
                .ok_or(Error::IntegrityFailed)?
                .into(),
            epoch: card["epoch"].as_u64().ok_or(Error::IntegrityFailed)?,
            audio: Some(audio),
            visual: None,
            visual_name: String::new(),
            visual_license: String::new(),
            guard_review: None,
            output_instance: None,
            prepared_media: None,
            prepared_media_receipt: None,
            approval_draft: None,
            axes: vec![
                axis(
                    "source",
                    PublicCandidateAxisState::Passed,
                    "SOURCE_AND_SENTENCE_BOUND",
                ),
                axis(
                    "audio",
                    PublicCandidateAxisState::NotReady,
                    "PCM_NOT_OBSERVED",
                ),
                axis(
                    "media",
                    PublicCandidateAxisState::NotReady,
                    "MEDIA_ASSET_NOT_PREPARED",
                ),
                axis(
                    "visual",
                    PublicCandidateAxisState::NotReady,
                    "PUBLIC_VISUAL_LICENSE_NOT_CONFIGURED",
                ),
                axis(
                    "guard",
                    PublicCandidateAxisState::NotReady,
                    "PUBLIC_GUARD_NOT_INSTALLED",
                ),
                axis(
                    "output",
                    PublicCandidateAxisState::NotReady,
                    "PUBLIC_MEDIA_OUTPUT_NOT_INSTALLED",
                ),
            ],
            approval_available: false,
            checked_at_ms: now as u64,
        })
    }
    pub fn tts_current(
        &mut self,
        op: &Operation,
        actor: &str,
        ws: &str,
        id: &str,
    ) -> Result<FoundationResult> {
        self.begin(op)?;
        read(
            &self.conn,
            &self.root,
            &self.fence,
            op,
            actor,
            ws,
            id,
            utc()?,
        )
    }
    pub fn tts_dispatch(
        &mut self,
        op: &Operation,
        actor: &str,
        ws: &str,
        id: &str,
    ) -> Result<TtsDispatch> {
        self.begin(op)?;
        let now = utc()?;
        let tx = self.conn.transaction().wb()?;
        let (mut v, rev, _) = task(&tx, &self.root, actor, ws, id)?;
        let m = gate(&tx, &self.root, &self.fence, op, actor, ws, id, &v, now)?;
        if v["state"] != "queued" {
            return Err(Error::RevisionConflict);
        }
        let mid = v["model_ref"]["object_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?;
        let url = m["base_url"].as_str().ok_or(Error::IntegrityFailed)?;
        let secret = if local_profile(&m)? {
            zeroize::Zeroizing::new(Vec::new())
        } else {
            self.tts_vault
                .as_deref()
                .ok_or(Error::CapabilityUnavailable)?
                .open(
                    crate::shared_catalog::credential_scope(&tx, ws, mid)?,
                    mid,
                    url,
                    &m["secret"],
                )?
        };
        let changed=tx.execute("UPDATE outbox SET state='dispatching',attempts=1,revision=revision+1,updated_at=?1 WHERE event_id=?2 AND state='pending' AND attempts=0",params![now,format!("tts:{id}")]).wb()?;
        if changed != 1 {
            return Err(Error::RevisionConflict);
        }
        v["state"] = "running".into();
        v["reason"] = "RUNNING".into();
        if v["kind"] == KIND {
            v["request_started"] = true.into();
        }
        llm::put(&tx, &self.root, ws, id, rev, &v, now)?;
        audit(&tx, ws, actor, "tts_dispatch", id, now)?;
        gate(&tx, &self.root, &self.fence, op, actor, ws, id, &v, utc()?)?;
        op.check()?;
        tx.commit().wb()?;
        let remaining = v["expires_at_ms"].as_i64().ok_or(Error::IntegrityFailed)? - utc()?;
        if remaining <= 0 {
            return Err(Error::DeadlineExceeded);
        }
        Ok(TtsDispatch {
            deadline_ms: (remaining as u64).min(u64::from(if v["kind"] == KIND {
                nai_runtime_core::tts::MODEL_TEST_DEADLINE_MS
            } else {
                nai_runtime_core::tts::PREVIEW_DEADLINE_MS
            })) as u32,
            protocol: nai_runtime_core::tts::protocol(m["protocol"].as_str().unwrap_or(""))?.into(),
            language: m["language"].as_str().unwrap_or("").into(),
            text: if v["kind"] == "publication_tts_preview" {
                let source = v["source"]["object_id"]
                    .as_str()
                    .ok_or(Error::IntegrityFailed)?;
                let r = preview::read(
                    &self.conn,
                    &self.root,
                    &self.fence,
                    op,
                    actor,
                    ws,
                    source,
                    0,
                    utc()?,
                )?;
                let card: Value =
                    serde_json::from_str(&r.body).map_err(|_| Error::IntegrityFailed)?;
                card["text"].as_str().ok_or(Error::IntegrityFailed)?.into()
            } else {
                TEST_TEXT.into()
            },
            base_url: url.into(),
            model: m["model"].as_str().ok_or(Error::IntegrityFailed)?.into(),
            voice: m["voice"].as_str().ok_or(Error::IntegrityFailed)?.into(),
            api_key: secret,
        })
    }
    /// Trusted worker completion; no plaintext PCM persisted or returned over IPC.
    #[allow(clippy::too_many_arguments)]
    pub fn finish_tts(
        &mut self,
        op: &Operation,
        actor: &str,
        ws: &str,
        id: &str,
        started: bool,
        error: &str,
        samples: u64,
        digest: &str,
    ) -> Result<()> {
        self.begin(op)?;
        let now = utc()?;
        let tx = self.conn.transaction().wb()?;
        let (mut v, rev, _) = task(&tx, &self.root, actor, ws, id)?;
        let outbox: String = tx
            .query_row(
                "SELECT state FROM outbox WHERE event_id=?1",
                [format!("tts:{id}")],
                |r| r.get(0),
            )
            .wb()?;
        if !matches!(outbox.as_str(), "pending" | "dispatching") {
            return Ok(());
        }
        if samples > 480000
            || (!digest.is_empty()
                && (digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit())))
        {
            return Err(Error::InvalidArgument);
        }
        let valid = gate(&tx, &self.root, &self.fence, op, actor, ws, id, &v, now).is_ok();
        let success = started
            && error.is_empty()
            && samples > 0
            && !digest.is_empty()
            && valid
            && outbox == "dispatching";
        v["state"] = if success { "completed" } else { "failed" }.into();
        v["reason"] = if success {
            "SYNTHESIZED_BILLING_UNKNOWN"
        } else if !valid {
            "CONTEXT_UNAVAILABLE"
        } else {
            llm::safe_worker_failure(error)
        }
        .into();
        v["sample_count"] = if success { samples } else { 0 }.into();
        v["audio_sha256"] = if success { digest } else { "" }.into();
        // A committed dispatch is already an unknown side-effect boundary; release only if the worker proves no send.
        let charge = if started { "unknown" } else { "released" };
        if v["kind"] == KIND {
            v["request_started"] = started.into();
        } else {
            tx.execute("UPDATE budget_reservations SET status=?1,revision=revision+1,updated_at=?2 WHERE workspace_id=?3 AND id=?4 AND status='reserved'",params![charge,now,ws,id]).wb()?;
        }
        tx.execute(
            "UPDATE outbox SET state='applied',revision=revision+1,updated_at=?1 WHERE event_id=?2",
            params![now, format!("tts:{id}")],
        )
        .wb()?;
        llm::put(&tx, &self.root, ws, id, rev, &v, now)?;
        audit(&tx, ws, actor, "tts_finish", id, now)?;
        op.check()?;
        tx.commit().wb()?;
        Ok(())
    }
}

pub(super) fn commit_gate(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    id: &str,
) -> Result<()> {
    let (v, _, _) = task(conn, root, actor, ws, id)?;
    gate(conn, root, fence, op, actor, ws, id, &v, utc()?)?;
    Ok(())
}
