//! Durable auto-run scope. Scope is not an actual media permit; every consume is rechecked.
use super::*;
use serde_json::{Value, json};
fn nonce(root: &Path) -> Result<String> {
    let path = root.join("public-output.stop");
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Ok(m) if m.is_file() && !m.file_type().is_symlink() && m.len() <= 64 => {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if m.file_attributes() & 0x400 != 0 {
                    return Err(Error::DataRootRejected);
                }
            }
            let n = fs::read_to_string(path).map_err(|_| Error::IoFailed)?;
            if !n.is_empty() {
                uuid(&n)?;
            }
            Ok(n)
        }
        _ => Err(Error::DataRootRejected),
    }
}
struct AutoPosition {
    state: i32,
    sentence_cursor: u32,
}
#[allow(clippy::too_many_arguments)]
fn scope(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    run: &str,
    grant: &str,
    now: i64,
) -> Result<(Value, u64, AutoPosition)> {
    let (v, revision) = live::load(conn, root, ws, run, actor)?;
    let a = v.get("auto_grant").ok_or(Error::PermissionDenied)?;
    if a["grant_id"] != grant
        || a["state"] != "active"
        || now < a["created_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?
        || now >= a["expires_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?
        || a["stop_nonce"] != nonce(root)?
    {
        return Err(Error::Cancelled);
    }
    let batch = v.get("batch_approval").ok_or(Error::PermissionDenied)?;
    let (epoch, stopped) = live::session(conn, ws, run, actor)?;
    if stopped
        || v["session_epoch"].as_u64() != Some(epoch)
        || a["epoch"].as_u64() != Some(epoch)
        || batch["epoch"].as_u64() != Some(epoch)
        || batch["revoked"] != false
        || batch["plan_sha256"] != a["plan_sha256"]
        || now
            < batch["created_at_ms"]
                .as_i64()
                .ok_or(Error::IntegrityFailed)?
        || now
            >= batch["expires_at_ms"]
                .as_i64()
                .ok_or(Error::IntegrityFailed)?
    {
        return Err(Error::Cancelled);
    }
    let mid = a["model_id"].as_str().ok_or(Error::IntegrityFailed)?;
    let (m, r, sha) = llm::raw(conn, root, ws, mid, 0, "publication_tts_config")?;
    fence.check_scope(&object_scope(ws, mid))?;
    if m["enabled"] != true
        || m["removed"] == true
        || a["model_revision"].as_u64() != Some(r)
        || a["model_sha256"] != sha
        || a["voice"] != m["voice"]
    {
        return Err(Error::RevisionConflict);
    }
    // This is scope-only metadata; dispatch and media consumers separately verify full source.
    let c = AutoPosition {
        state: v["state"].as_i64().ok_or(Error::IntegrityFailed)? as i32,
        sentence_cursor: v["sentence_cursor"]
            .as_u64()
            .ok_or(Error::IntegrityFailed)? as u32,
    };
    if matches!(
        LiveRunState::try_from(c.state).map_err(|_| Error::IntegrityFailed)?,
        LiveRunState::Interrupted | LiveRunState::Ended | LiveRunState::Unavailable
    ) {
        return Err(Error::Cancelled);
    }
    op.check()?;
    Ok((v, revision, c))
}
#[allow(clippy::too_many_arguments)]
fn authority(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    run: &str,
    grant: &str,
    now: i64,
) -> Result<(Value, u64, LiveSpeechPlan, AutoPosition)> {
    let (v, revision, c) = scope(conn, root, fence, op, actor, ws, run, grant, now)?;
    let p = live::speech_plan(conn, root, fence, op, actor, ws, run, now)?
        .live_speech_plan
        .ok_or(Error::IntegrityFailed)?;
    if v["auto_grant"]["plan_sha256"] != p.plan_sha256
        || !p
            .batch_approval
            .as_ref()
            .is_some_and(|b| b.approved && !b.revoked)
    {
        return Err(Error::PermissionDenied);
    }
    Ok((v, revision, p, c))
}
#[allow(clippy::too_many_arguments)]
pub(super) fn read(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    run: &str,
    now: i64,
) -> Result<FoundationResult> {
    let (v, revision) = live::load(conn, root, ws, run, actor)?;
    let Some(a) = v.get("auto_grant") else {
        return Ok(FoundationResult::default());
    };
    let grant = a["grant_id"].as_str().ok_or(Error::IntegrityFailed)?;
    // Status is metadata, never an audio/public authority. Actual dispatch/renewal rechecks source.
    let checked = scope(conn, root, fence, op, actor, ws, run, grant, now);
    let mut state = a["state"]
        .as_str()
        .ok_or(Error::IntegrityFailed)?
        .to_string();
    let mut reason = a["reason"]
        .as_str()
        .ok_or(Error::IntegrityFailed)?
        .to_string();
    let mut tasks = Vec::new();
    if state == "stopped" && reason == "COMPLETED" {
        state = "completed".into();
    }
    if state == "active" {
        match checked {
            Ok((_, _, c)) => {
                state =
                    match LiveRunState::try_from(c.state).map_err(|_| Error::IntegrityFailed)? {
                        LiveRunState::Completed => "completed",
                        LiveRunState::Paused => "paused",
                        LiveRunState::PauseRequested => "pausing",
                        LiveRunState::Dispatching => "playing",
                        _ => "preparing",
                    }
                    .into();
                for t in a["tasks"]
                    .as_array()
                    .ok_or(Error::IntegrityFailed)?
                    .iter()
                    .filter(|t| {
                        t["index"].as_u64().is_some_and(|i| {
                            i >= u64::from(c.sentence_cursor)
                                && i < u64::from(c.sentence_cursor) + 2
                        })
                    })
                {
                    let id = t["task_id"].as_str().ok_or(Error::IntegrityFailed)?;
                    let (task, _, task_sha256) = tts_task::task(conn, root, actor, ws, id)?;
                    let task_state = match task["state"].as_str() {
                        Some("queued") => AgentTaskState::Queued,
                        Some("running") => AgentTaskState::Running,
                        Some("completed") => AgentTaskState::Completed,
                        Some("cancelled") => AgentTaskState::Cancelled,
                        _ => AgentTaskState::Failed,
                    };
                    tasks.push(LiveAutoTask {
                        sentence_cursor: t["index"].as_u64().ok_or(Error::IntegrityFailed)? as u32,
                        preview_id: t["preview_id"]
                            .as_str()
                            .ok_or(Error::IntegrityFailed)?
                            .into(),
                        preview_sha256: t["preview_sha256"]
                            .as_str()
                            .ok_or(Error::IntegrityFailed)?
                            .into(),
                        task_id: id.into(),
                        task_sha256,
                        task_state: task_state as i32,
                        model_id: a["model_id"].as_str().ok_or(Error::IntegrityFailed)?.into(),
                        model_revision: a["model_revision"]
                            .as_u64()
                            .ok_or(Error::IntegrityFailed)?,
                    });
                }
            }
            Err(e) => {
                state = "blocked".into();
                reason = e.as_str_name().into();
            }
        }
    }
    let q = a.get("question");
    let mut question_text = String::new();
    let mut answer_text = String::new();
    if let Some(q) = q {
        if let Some(id) = q["faq_task_id"].as_str()
            && let Ok((f, _)) = publication::stored(conn, root, ws, id, "publication_faq_task")
        {
            question_text = f["question"].as_str().unwrap_or("").into();
            answer_text = f["text"].as_str().unwrap_or("").into();
        }
        if crate::live_question::active(&v) && state != "blocked" {
            if question_text.is_empty() || q["expires_at_ms"].as_i64().is_none_or(|t| now >= t) {
                state = "blocked".into();
                reason = "QUESTION_UNAVAILABLE".into();
            } else if q["state"] == "pending" {
                state = if v["state"] == LiveRunState::Paused as i32 {
                    "question_waiting"
                } else {
                    "question_pausing"
                }
                .into();
                tasks.clear();
            } else {
                state = if q["state"] == "playing" {
                    "playing"
                } else {
                    "preparing"
                }
                .into();
                tasks.clear();
                if let Some(t) = q.get("task") {
                    let id = t["task_id"].as_str().ok_or(Error::IntegrityFailed)?;
                    let (task, _, sha) = tts_task::task(conn, root, actor, ws, id)?;
                    tasks.push(LiveAutoTask {
                        sentence_cursor: v["sentence_cursor"].as_u64().unwrap_or(0) as u32,
                        preview_id: t["preview_id"]
                            .as_str()
                            .ok_or(Error::IntegrityFailed)?
                            .into(),
                        preview_sha256: t["preview_sha256"]
                            .as_str()
                            .ok_or(Error::IntegrityFailed)?
                            .into(),
                        task_id: id.into(),
                        task_sha256: sha,
                        task_state: match task["state"].as_str() {
                            Some("queued") => AgentTaskState::Queued,
                            Some("running") => AgentTaskState::Running,
                            Some("completed") => AgentTaskState::Completed,
                            Some("cancelled") => AgentTaskState::Cancelled,
                            _ => AgentTaskState::Failed,
                        } as i32,
                        model_id: a["model_id"].as_str().ok_or(Error::IntegrityFailed)?.into(),
                        model_revision: a["model_revision"]
                            .as_u64()
                            .ok_or(Error::IntegrityFailed)?,
                    });
                }
            }
        }
    }
    Ok(FoundationResult {
        resource_id: run.into(),
        epoch: a["epoch"].as_u64().ok_or(Error::IntegrityFailed)?,
        revision,
        live_auto: Some(LiveAutoView {
            run_id: run.into(),
            grant_id: grant.into(),
            state,
            reason,
            model_id: a["model_id"].as_str().ok_or(Error::IntegrityFailed)?.into(),
            model_revision: a["model_revision"].as_u64().ok_or(Error::IntegrityFailed)?,
            voice: a["voice"].as_str().ok_or(Error::IntegrityFailed)?.into(),
            output_id: a["output_id"]
                .as_str()
                .ok_or(Error::IntegrityFailed)?
                .into(),
            max_cost_minor: a["max_cost_minor"].as_u64().ok_or(Error::IntegrityFailed)?,
            reserved_minor: a["reserved_minor"].as_u64().ok_or(Error::IntegrityFailed)?,
            expires_at_ms: a["expires_at_ms"].as_u64().ok_or(Error::IntegrityFailed)?,
            tasks,
            sentence_cursor: v["sentence_cursor"].as_u64().unwrap_or(0) as u32,
            sentence_total: if crate::live_question::active(&v) {
                v["sentence_cursor"].as_u64().unwrap_or(0) as u32 + 1
            } else {
                v["sentence_total"].as_u64().unwrap_or(0) as u32
            },
            interruption_id: q
                .and_then(|q| q["interruption_id"].as_str())
                .unwrap_or("")
                .into(),
            interruption_state: q.and_then(|q| q["state"].as_str()).unwrap_or("").into(),
            question: question_text,
            answer: answer_text,
            return_cursor: q.and_then(|q| q["return_cursor"].as_u64()).unwrap_or(0) as u32,
        }),
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
    x: &LiveAutoStart,
    now: i64,
) -> Result<FoundationResult> {
    let r = x.run.as_ref().ok_or(Error::InvalidArgument)?;
    uuid(&x.grant_id)?;
    uuid(&x.model_id)?;
    uuid(&x.output_id)?;
    if !x.acknowledge_rights || role(tx, &r.workspace_id, actor)?.0 != FoundationRole::Admin {
        return Err(Error::PermissionDenied);
    }
    if x.max_cost_minor == 0
        || x.max_cost_minor > 1000000
        || x.visual_sha256.len() != 64
        || x.license_sha256.len() != 64
        || x.stop_nonce != nonce(root)?
    {
        return Err(Error::InvalidArgument);
    }
    let (mut v, revision) = live::load(tx, root, &r.workspace_id, &r.run_id, actor)?;
    let p = live::speech_plan(tx, root, fence, op, actor, &r.workspace_id, &r.run_id, now)?
        .live_speech_plan
        .ok_or(Error::IntegrityFailed)?;
    if v["takeover"]["active"] == true {
        return Err(Error::PermissionDenied);
    }
    let c = live::read(tx, root, fence, op, actor, &r.workspace_id, &r.run_id, now)?
        .live_context
        .ok_or(Error::IntegrityFailed)?;
    if revision != x.expected_revision
        || p.epoch != x.expected_epoch
        || p.plan_sha256 != x.plan_sha256
        || c.state != LiveRunState::Ready as i32
    {
        return Err(Error::RevisionConflict);
    }
    let b = p
        .batch_approval
        .as_ref()
        .filter(|b| b.approved && !b.revoked)
        .ok_or(Error::PermissionDenied)?;
    if let Some(a) = v.get("auto_grant")
        && a["state"] == "active"
        && authority(
            tx,
            root,
            fence,
            op,
            actor,
            &r.workspace_id,
            &r.run_id,
            a["grant_id"].as_str().ok_or(Error::IntegrityFailed)?,
            now,
        )
        .is_ok()
    {
        return Err(Error::Busy);
    }
    let mut ids = v
        .get("auto_ids")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if ids.iter().any(|i| i == &x.grant_id) {
        return Err(Error::RevisionConflict);
    }
    if ids.len() >= 32 {
        return Err(Error::ResourceExhausted);
    }
    ids.push(x.grant_id.clone().into());
    let (m, mr, sha) = llm::raw(
        tx,
        root,
        &r.workspace_id,
        &x.model_id,
        0,
        "publication_tts_config",
    )?;
    if mr != x.model_revision || m["enabled"] != true || m["removed"] == true {
        return Err(Error::RevisionConflict);
    }
    v["auto_ids"] = ids.into();
    v["auto_grant"] = json!({"grant_id":x.grant_id,"model_id":x.model_id,"model_revision":mr,"model_sha256":sha,"voice":m["voice"],"plan_sha256":p.plan_sha256,"output_id":x.output_id,"visual_sha256":x.visual_sha256,"license_sha256":x.license_sha256,"stop_nonce":x.stop_nonce,"created_at_ms":now,"expires_at_ms":(now+300000).min(b.expires_at_ms as i64),"epoch":p.epoch,"max_cost_minor":x.max_cost_minor,"reserved_minor":0,"state":"active","reason":"AUTHORIZED","tasks":[]});
    v["updated_at_ms"] = now.into();
    llm::put(tx, root, &r.workspace_id, &r.run_id, revision, &v, now)?;
    read(tx, root, fence, op, actor, &r.workspace_id, &r.run_id, now)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn prepare(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    role_rev: u64,
    tick: u64,
    x: &LiveAutoPrepare,
    now: i64,
) -> Result<FoundationResult> {
    let r = x.run.as_ref().ok_or(Error::InvalidArgument)?;
    let (mut v, revision, _, c) = authority(
        tx,
        root,
        fence,
        op,
        actor,
        &r.workspace_id,
        &r.run_id,
        &x.grant_id,
        now,
    )?;
    let answering = crate::live_question::active(&v);
    if answering {
        if v["auto_grant"]["question"]["state"] != "pending"
            || c.state != LiveRunState::Paused as i32
            || x.sentence_cursor != c.sentence_cursor
        {
            return Err(Error::Cancelled);
        }
        crate::live_question::answer(tx, root, fence, op, actor, &r.workspace_id, &v, now)?;
        v["auto_grant"]["question"]["return_cursor"] = c.sentence_cursor.into();
        v["auto_grant"]["question"]["state"] = "preparing".into();
        v["state"] = (LiveRunState::Ready as i32).into();
        llm::put(tx, root, &r.workspace_id, &r.run_id, revision, &v, now)?;
    }
    if !answering
        && !matches!(
            LiveRunState::try_from(c.state),
            Ok(LiveRunState::Ready | LiveRunState::Dispatching)
        )
    {
        return Err(Error::Cancelled);
    }
    if x.sentence_cursor < c.sentence_cursor || x.sentence_cursor >= c.sentence_cursor + 2 {
        return Err(Error::RevisionConflict);
    }
    let a = &v["auto_grant"];
    let tasks = a["tasks"].as_array().ok_or(Error::IntegrityFailed)?;
    if !answering
        && tasks
            .iter()
            .any(|t| t["index"].as_u64() == Some(u64::from(x.sentence_cursor)))
    {
        return Err(Error::RevisionConflict);
    }
    let cap = 10.min(
        a["max_cost_minor"]
            .as_u64()
            .ok_or(Error::IntegrityFailed)?
            .saturating_sub(a["reserved_minor"].as_u64().ok_or(Error::IntegrityFailed)?),
    );
    if cap == 0 {
        return Err(Error::ResourceExhausted);
    }
    let card = if answering {
        crate::live_question::preview(
            tx,
            root,
            fence,
            op,
            actor,
            role_rev,
            &r.workspace_id,
            &r.run_id,
            &x.preview_id,
            &v,
            now,
        )?
    } else {
        live::preview_sentence(
            tx,
            root,
            fence,
            op,
            actor,
            role_rev,
            tick,
            now,
            &LiveSentencePreview {
                run: Some(r.clone()),
                preview_id: x.preview_id.clone(),
                sentence_cursor: x.sentence_cursor,
            },
        )?
    };
    let request = FoundationSpeechPlan {
        workspace_id: r.workspace_id.clone(),
        preview_id: x.preview_id.clone(),
        preview_sha256: card.sha256.clone(),
        model_id: a["model_id"].as_str().ok_or(Error::IntegrityFailed)?.into(),
        model_revision: a["model_revision"].as_u64().ok_or(Error::IntegrityFailed)?,
        max_cost_minor: cap,
        task_id: x.task_id.clone(),
    };
    let p = tts_task::speech_plan(tx, root, fence, op, actor, &request, now, true)?;
    let a = &mut v["auto_grant"];
    // Consumed history lives in task objects and audit; keep only the two live slots here.
    let slots = a["tasks"].as_array_mut().ok_or(Error::IntegrityFailed)?;
    slots.retain(|t| {
        t["index"]
            .as_u64()
            .is_some_and(|i| i >= u64::from(c.sentence_cursor))
    });
    if !answering && slots.len() >= 2 {
        return Err(Error::ResourceExhausted);
    }
    let slot = json!({"index":x.sentence_cursor,"preview_id":x.preview_id,"preview_sha256":card.sha256,"task_id":x.task_id});
    if answering {
        a["question"]["task"] = slot;
    } else {
        slots.push(slot);
    }

    a["reserved_minor"] = (a["reserved_minor"].as_u64().unwrap() + cap).into();
    v["updated_at_ms"] = now.into();
    llm::put(
        tx,
        root,
        &r.workspace_id,
        &r.run_id,
        revision + u64::from(answering),
        &v,
        now,
    )?;
    let start = FoundationSpeechStart {
        plan: Some(request.clone()),
        task_id: x.task_id.clone(),
        approval_sha256: p.approval_sha256,
    };
    let test = FoundationTtsTest {
        workspace_id: r.workspace_id.clone(),
        task_id: x.task_id.clone(),
        model_id: request.model_id,
        model_revision: request.model_revision,
    };
    let mut result = tts_task::start(
        tx,
        root,
        fence,
        op,
        actor,
        role_rev,
        now,
        &test,
        Some(&start),
        Some((&r.run_id, &x.grant_id)),
    )?;
    result.live_auto = read(tx, root, fence, op, actor, &r.workspace_id, &r.run_id, now)?.live_auto;
    Ok(result)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn stop(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    x: &LiveAutoStop,
    now: i64,
) -> Result<FoundationResult> {
    let r = x.run.as_ref().ok_or(Error::InvalidArgument)?;
    let (mut v, revision) = live::load(tx, root, &r.workspace_id, &r.run_id, actor)?;
    let question_active = crate::live_question::active(&v);
    let a = v.get_mut("auto_grant").ok_or(Error::NotFound)?;
    if a["grant_id"] != x.grant_id {
        return Err(Error::RevisionConflict);
    }
    if ![
        "OPERATOR_STOP",
        "AUTO_FAILED",
        "OUTPUT_CHANGED",
        "COMPLETED",
        "RESOURCE_EXHAUSTED",
        "TTS_FAILED",
        "POLICY_BLOCKED",
        "MEDIA_NOT_READY",
        "AUTHORITY_UNAVAILABLE",
    ]
    .contains(&x.reason.as_str())
    {
        return Err(Error::InvalidArgument);
    }
    a["state"] = if ["OPERATOR_STOP", "COMPLETED"].contains(&x.reason.as_str()) {
        "stopped".into()
    } else {
        "failed".into()
    };
    a["reason"] = x.reason.clone().into();
    if question_active {
        a["question"]["state"] = "cancelled".into();
        if v["active_approval"] == "" {
            v["state"] = (LiveRunState::Paused as i32).into();
        }
    }

    if v["active_approval"] != "" && matches!(v["state"].as_i64(), Some(2 | 3)) {
        v["state"] = (LiveRunState::Interrupted as i32).into();
    }
    v["updated_at_ms"] = now.into();
    llm::put(tx, root, &r.workspace_id, &r.run_id, revision, &v, now)?;
    read(tx, root, fence, op, actor, &r.workspace_id, &r.run_id, now)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn task_gate(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    id: &str,
    t: &Value,
    now: i64,
) -> Result<()> {
    let Some(grant) = t.get("auto_grant_id") else {
        return Ok(());
    };
    let run = t["auto_run_id"].as_str().ok_or(Error::IntegrityFailed)?;
    let (v, _, _) = scope(
        conn,
        root,
        fence,
        op,
        actor,
        ws,
        run,
        grant.as_str().ok_or(Error::IntegrityFailed)?,
        now,
    )?;
    let a = &v["auto_grant"];
    let item = selected_task(a, id).ok_or(Error::PermissionDenied)?;
    if crate::live_question::active(&v)
        && a["question"]["state"] != "pending"
        && a["question"]["task"]["task_id"] != id
    {
        return Err(Error::Cancelled);
    }
    if t["model_ref"]["object_id"] != a["model_id"]
        || t["model_ref"]["sha256"] != a["model_sha256"]
        || t["source"]["object_id"] != item["preview_id"]
        || t["source"]["sha256"] != item["preview_sha256"]
    {
        return Err(Error::RevisionConflict);
    }
    Ok(())
}
impl Store {
    pub(super) fn validate_auto_bundle(
        &self,
        op: &Operation,
        actor: &str,
        b: &PublicMediaBundle,
    ) -> Result<Option<i64>> {
        let Some(audio) = b.audio.as_ref() else {
            return Ok(None);
        };
        let exists: bool = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM objects WHERE workspace_id=?1 AND id=?2)",
                params![b.workspace_id, audio.artifact_id],
                |r| r.get(0),
            )
            .wb()?;
        if !exists {
            return Ok(None);
        }
        let (_, sha) = object(&self.conn, &b.workspace_id, &audio.artifact_id, 0)?;
        let t: Value =
            serde_json::from_str(&body(&self.root, &sha)?).map_err(|_| Error::IntegrityFailed)?;
        if t.get("auto_grant_id").is_none() {
            return Ok(None);
        }
        task_gate(
            &self.conn,
            &self.root,
            &self.fence,
            op,
            actor,
            &b.workspace_id,
            &audio.artifact_id,
            &t,
            utc()?,
        )?;
        let (v, _) = live::load(
            &self.conn,
            &self.root,
            &b.workspace_id,
            t["auto_run_id"].as_str().ok_or(Error::IntegrityFailed)?,
            actor,
        )?;
        let a = &v["auto_grant"];
        let item = selected_task(a, &audio.artifact_id).ok_or(Error::PermissionDenied)?;
        if item["index"] != v["sentence_cursor"] {
            return Err(Error::PermissionDenied);
        }
        let visual = b.visual.as_ref().ok_or(Error::IntegrityFailed)?;
        if a["output_id"] != b.output_id
            || a["visual_sha256"] != visual.package_sha256
            || a["license_sha256"] != visual.license_sha256
        {
            return Err(Error::PermissionDenied);
        }
        let until = a["expires_at_ms"]
            .as_i64()
            .ok_or(Error::IntegrityFailed)?
            .min(
                v["batch_approval"]["expires_at_ms"]
                    .as_i64()
                    .ok_or(Error::IntegrityFailed)?,
            )
            .min(t["expires_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?);
        if utc()? >= until {
            return Err(Error::Cancelled);
        }
        Ok(Some(until))
    }
    pub fn validate_live_auto_candidate(
        &self,
        op: &Operation,
        actor: &str,
        x: &LiveAutoConfirm,
        check: &PublicCandidateCheck,
    ) -> Result<()> {
        let r = x.run.as_ref().ok_or(Error::InvalidArgument)?;
        let (v, _, _, c) = authority(
            &self.conn,
            &self.root,
            &self.fence,
            op,
            actor,
            &r.workspace_id,
            &r.run_id,
            &x.grant_id,
            utc()?,
        )?;
        if c.state != LiveRunState::Ready as i32 {
            return Err(Error::Cancelled);
        }
        let a = &v["auto_grant"];
        let request = x.candidate.as_ref().ok_or(Error::InvalidArgument)?;
        let item = selected_task(a, &request.task_id).ok_or(Error::PermissionDenied)?;
        if item["index"].as_u64() != Some(u64::from(c.sentence_cursor)) {
            return Err(Error::PermissionDenied);
        }
        if crate::live_question::active(&v) && a["question"]["task"]["task_id"] != request.task_id {
            return Err(Error::PermissionDenied);
        }
        let visual = check.visual.as_ref().ok_or(Error::CapabilityUnavailable)?;
        if request.workspace_id != r.workspace_id
            || item["preview_id"] != request.preview_id
            || item["preview_sha256"] != request.preview_sha256
            || item["task_id"] != request.task_id
            || check
                .output_instance
                .as_ref()
                .is_none_or(|o| a["output_id"] != o.output_id)
            || a["visual_sha256"] != visual.package_sha256
            || a["license_sha256"] != visual.license_sha256
        {
            return Err(Error::PermissionDenied);
        }
        Ok(())
    }
}
use std::fs;

fn selected_task<'a>(a: &'a Value, id: &str) -> Option<&'a Value> {
    if let Some(t) = a.get("question").and_then(|q| q.get("task"))
        && t["task_id"] == id
    {
        return Some(t);
    }
    a["tasks"].as_array()?.iter().find(|t| t["task_id"] == id)
}
