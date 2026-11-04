//! A single selected FAQ interruption. Original speech cursor is never consumed by an answer.
use super::*;
use serde_json::{Value, json};
pub(super) fn active(v: &Value) -> bool {
    v.get("auto_grant")
        .and_then(|a| a.get("question"))
        .is_some_and(|q| {
            matches!(
                q["state"].as_str(),
                Some("pending" | "preparing" | "playing")
            )
        })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn answer(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    v: &Value,
    now: i64,
) -> Result<Value> {
    let q = &v["auto_grant"]["question"];
    if !active(v)
        || now < q["created_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?
        || now >= q["expires_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?
    {
        return Err(Error::Cancelled);
    }
    let r = assistance::read(
        conn,
        root,
        fence,
        op,
        actor,
        ws,
        q["faq_task_id"].as_str().ok_or(Error::IntegrityFailed)?,
        now,
    )?;
    if r.sha256 != q["faq_task_sha256"] || r.suggestion_state != SuggestionState::Proposed as i32 {
        return Err(Error::Cancelled);
    }
    let f: Value = serde_json::from_str(&r.body).map_err(|_| Error::IntegrityFailed)?;
    let c = &v["context"];
    if f["snapshot"]["object_id"] != c["snapshot_id"]
        || f["snapshot"]["sha256"] != c["snapshot_sha256"]
        || f.get("parent_task").is_none()
    {
        return Err(Error::RevisionConflict);
    }
    Ok(f)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn control(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    x: &LiveQuestionControl,
    now: i64,
) -> Result<FoundationResult> {
    let r = x.run.as_ref().ok_or(Error::InvalidArgument)?;
    let (mut v, rev) = live::load(tx, root, &r.workspace_id, &r.run_id, actor)?;
    let view = live_auto::read(tx, root, fence, op, actor, &r.workspace_id, &r.run_id, now)?
        .live_auto
        .ok_or(Error::PermissionDenied)?;
    if (!x.cancel && rev != x.expected_revision)
        || view.grant_id != x.grant_id
        || v["session_epoch"].as_u64() != Some(x.expected_epoch)
    {
        return Err(Error::RevisionConflict);
    }
    uuid(&x.interruption_id)?;
    if x.cancel {
        if v["auto_grant"]["question"]["interruption_id"] != x.interruption_id {
            return Err(Error::RevisionConflict);
        }
        if v["auto_grant"]["question"]["state"] != "completed" {
            v["auto_grant"]["question"]["state"] = "cancelled".into();
        }
        v["auto_grant"]["state"] = "stopped".into();
        v["auto_grant"]["reason"] = "QUESTION_CANCELLED".into();
        v["state"] = if v["active_approval"] != "" {
            LiveRunState::Interrupted as i32
        } else {
            LiveRunState::Paused as i32
        }
        .into();
    } else {
        if !x.acknowledge_answer
            || active(&v)
            || !matches!(view.state.as_str(), "preparing" | "playing")
        {
            return Err(Error::PermissionDenied);
        }
        let ids = v["auto_grant"]
            .get("question_ids")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let task_ids = v["auto_grant"]
            .get("question_task_ids")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if task_ids.iter().any(|id| id == &x.faq_task_id) {
            return Err(Error::RevisionConflict);
        }
        if ids.len() >= 16 || ids.iter().any(|id| id == &x.interruption_id) {
            return Err(Error::ResourceExhausted);
        }
        let p = live::speech_plan(tx, root, fence, op, actor, &r.workspace_id, &r.run_id, now)?
            .live_speech_plan
            .ok_or(Error::IntegrityFailed)?;
        let cursor = v["sentence_cursor"]
            .as_u64()
            .ok_or(Error::IntegrityFailed)?;
        let cue = &p
            .sentences
            .get(cursor as usize)
            .ok_or(Error::Cancelled)?
            .cue_id;
        let (snap, _) = publication::stored(
            tx,
            root,
            &r.workspace_id,
            &p.source.as_ref().ok_or(Error::IntegrityFailed)?.snapshot_id,
            "publication_snapshot",
        )?;
        let docs = publication::documents(tx, root, fence, &r.workspace_id, &snap["refs"], true)?;
        if !docs[0]["cues"]
            .as_array()
            .ok_or(Error::IntegrityFailed)?
            .iter()
            .any(|c| c["id"] == *cue && c["allow_questions"] == true)
        {
            return Err(Error::PermissionDenied);
        }
        v["auto_grant"]["question"] = json!({"interruption_id":x.interruption_id,"faq_task_id":x.faq_task_id,"faq_task_sha256":x.faq_task_sha256,"state":"pending","created_at_ms":now,"expires_at_ms":now+nai_runtime_core::live::SELECTED_RESPONSE_WINDOW_MS,"return_cursor":cursor});
        let f = answer(tx, root, fence, op, actor, &r.workspace_id, &v, now)?;
        review_questions::check_answer_run(tx, root, actor, &r.workspace_id, &r.run_id, &f)?;
        let expires = v["auto_grant"]["question"]["expires_at_ms"]
            .as_i64()
            .ok_or(Error::IntegrityFailed)?
            .min(
                v["auto_grant"]["expires_at_ms"]
                    .as_i64()
                    .ok_or(Error::IntegrityFailed)?,
            )
            .min(
                v["batch_approval"]["expires_at_ms"]
                    .as_i64()
                    .ok_or(Error::IntegrityFailed)?,
            )
            .min(f["expires_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?);
        if expires <= now {
            return Err(Error::DeadlineExceeded);
        }
        v["auto_grant"]["question"]["expires_at_ms"] = expires.into();
        let text = f["text"].as_str().ok_or(Error::IntegrityFailed)?;
        if text.is_empty() || text.chars().count() > 120 {
            return Err(Error::ResourceExhausted);
        }
        let mut ids = ids;
        ids.push(x.interruption_id.clone().into());
        v["auto_grant"]["question_ids"] = ids.into();
        let mut task_ids = task_ids;
        task_ids.push(x.faq_task_id.clone().into());
        v["auto_grant"]["question_task_ids"] = task_ids.into();

        v["state"] = if v["state"] == LiveRunState::Dispatching as i32 {
            LiveRunState::PauseRequested as i32
        } else {
            LiveRunState::Paused as i32
        }
        .into();
    }
    v["updated_at_ms"] = now.into();
    llm::put(tx, root, &r.workspace_id, &r.run_id, rev, &v, now)?;
    live_auto::read(tx, root, fence, op, actor, &r.workspace_id, &r.run_id, now)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn preview(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    role_rev: u64,
    ws: &str,
    run: &str,
    id: &str,
    v: &Value,
    now: i64,
) -> Result<FoundationResult> {
    let f = answer(tx, root, fence, op, actor, ws, v, now)?;
    let p = live::speech_plan(tx, root, fence, op, actor, ws, run, now)?
        .live_speech_plan
        .ok_or(Error::IntegrityFailed)?;
    let index = (v["sentence_cursor"]
        .as_u64()
        .ok_or(Error::IntegrityFailed)? as usize)
        .min(p.sentences.len().saturating_sub(1));
    let q = &v["auto_grant"]["question"];
    let card = json!({"schema_version":1,"kind":"publication_private_preview","created_at_ms":now,"created_by":actor,"role_revision":role_rev,"snapshot":f["snapshot"],"session_id":run,"epoch":v["session_epoch"],"cue_id":p.sentences.get(index).ok_or(Error::Cancelled)?.cue_id,"text":f["text"],"target":"private_workbench","expires_at_ms":q["expires_at_ms"],"live_question":{"interruption_id":q["interruption_id"],"faq_task_id":q["faq_task_id"],"faq_task_sha256":q["faq_task_sha256"],"faq_id":f["faq_id"]}});
    publication::insert(tx, root, ws, id, &card, now)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn check_card(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    card: &Value,
    now: i64,
) -> Result<()> {
    let (v, _) = live::load(
        conn,
        root,
        ws,
        card["session_id"].as_str().ok_or(Error::IntegrityFailed)?,
        actor,
    )?;
    let f = answer(conn, root, fence, op, actor, ws, &v, now)?;
    let q = &v["auto_grant"]["question"];
    let i = &card["live_question"];
    if q["interruption_id"] != i["interruption_id"]
        || q["faq_task_id"] != i["faq_task_id"]
        || q["faq_task_sha256"] != i["faq_task_sha256"]
        || card["text"] != f["text"]
        || i["faq_id"] != f["faq_id"]
        || card["epoch"] != v["session_epoch"]
    {
        return Err(Error::RevisionConflict);
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub(super) fn output(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    b: &PublicMediaBundle,
    action: &str,
    mut v: Value,
    revision: u64,
    card: &Value,
    now: i64,
) -> Result<()> {
    if action != "public_execution_stopped" {
        check_card(tx, root, fence, op, actor, &b.workspace_id, card, now)?;
    }
    if action != "public_execution_stopped" {
        let audio = b.audio.as_ref().ok_or(Error::IntegrityFailed)?;
        let (t, _, _) = tts_task::task(tx, root, actor, &b.workspace_id, &audio.artifact_id)?;
        if t["state"] != "completed"
            || t["audio_sha256"] != audio.pcm_sha256
            || t["sample_count"].as_u64() != Some(audio.sample_count)
            || card["session_id"] != b.session_id
            || card["epoch"] != b.epoch
        {
            return Err(Error::RevisionConflict);
        }
    }
    let q = &v["auto_grant"]["question"];
    if q["task"]["preview_sha256"] != hash(card.to_string().as_bytes())
        || q["task"]["task_id"] != b.audio.as_ref().ok_or(Error::IntegrityFailed)?.artifact_id
        || q["return_cursor"] != v["sentence_cursor"]
    {
        return Err(Error::RevisionConflict);
    }
    if action == "public_execution_requested" {
        if q["state"] == "playing"
            && v["active_approval"] == b.approval_id
            && v["active_bundle"] == b.approved_sha256
        {
            return Ok(());
        }
        if q["state"] != "preparing" || v["state"] != LiveRunState::Ready as i32 {
            return Err(Error::Cancelled);
        }
        v["auto_grant"]["question"]["state"] = "playing".into();
        v["state"] = (LiveRunState::Dispatching as i32).into();
        v["active_approval"] = b.approval_id.clone().into();
        v["active_bundle"] = b.approved_sha256.clone().into();
    } else {
        if v["active_approval"] != b.approval_id || v["active_bundle"] != b.approved_sha256 {
            return Err(Error::RevisionConflict);
        }
        if action == "public_execution_stopped" {
            v["state"] = (LiveRunState::Interrupted as i32).into();
            v["auto_grant"]["question"]["state"] = "cancelled".into();
        } else {
            v["auto_grant"]["question"]["state"] = "completed".into();
            v["state"] = if v["state"] == LiveRunState::PauseRequested as i32 {
                LiveRunState::Paused as i32
            } else if v["sentence_cursor"] == v["sentence_total"] {
                LiveRunState::Completed as i32
            } else {
                LiveRunState::Ready as i32
            }
            .into();
            v["last_approval"] = b.approval_id.clone().into();
            v["last_bundle"] = b.approved_sha256.clone().into();
            v["active_approval"] = "".into();
            v["active_bundle"] = "".into();
        }
    }
    review_questions::delivery(
        tx,
        root,
        actor,
        &b.workspace_id,
        &b.session_id,
        v["auto_grant"]["question"]["faq_task_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?,
        if action == "public_execution_requested" {
            "requested"
        } else if action == "public_submission_complete" {
            "submitted"
        } else {
            "stopped"
        },
        now,
    )?;
    v["updated_at_ms"] = now.into();
    llm::put(tx, root, &b.workspace_id, &b.session_id, revision, &v, now)?;
    Ok(())
}
