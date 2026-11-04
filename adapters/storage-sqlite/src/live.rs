//! Durable bounded queue. Generic page writes cannot claim a submitted sentence.
use super::*;
use nai_contracts::workbench::{LiveRunAction as A, LiveRunState as S};
use serde_json::{Value, json};
pub(super) fn load(
    conn: &Connection,
    root: &Path,
    ws: &str,
    id: &str,
    actor: &str,
) -> Result<(Value, u64)> {
    let (v, r, _) = llm::raw(conn, root, ws, id, 0, "publication_live_run")?;
    if v["created_by"] != actor {
        return Err(Error::PermissionDenied);
    }
    let n = v["cue_ids"].as_array().ok_or(Error::IntegrityFailed)?.len();
    if v["cursor"].as_u64().ok_or(Error::IntegrityFailed)? as usize > n
        || v["submitted_count"]
            .as_u64()
            .ok_or(Error::IntegrityFailed)?
            + v["skipped_count"].as_u64().ok_or(Error::IntegrityFailed)?
            != v["cursor"].as_u64().unwrap()
    {
        return Err(Error::IntegrityFailed);
    }
    if v["sentence_mode"] == true {
        let cursor = v["sentence_cursor"]
            .as_u64()
            .ok_or(Error::IntegrityFailed)?;
        if v["submitted_sentences"]
            .as_u64()
            .ok_or(Error::IntegrityFailed)?
            + v["skipped_sentences"]
                .as_u64()
                .ok_or(Error::IntegrityFailed)?
            != cursor
            || cursor > v["sentence_total"].as_u64().ok_or(Error::IntegrityFailed)?
        {
            return Err(Error::IntegrityFailed);
        }
    }
    Ok((v, r))
}

/// Only a typed, protected live object can supply sentence identity to a preview.
#[allow(clippy::too_many_arguments)]
pub(super) fn preview_selection(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    x: &FoundationPreview,
    index: Option<u32>,
    now: i64,
) -> Result<Option<(String, Value)>> {
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM objects WHERE workspace_id=?1 AND id=?2)",
            params![x.workspace_id, x.session_id],
            |r| r.get(0),
        )
        .wb()?;
    if !exists {
        return if index.is_some() {
            Err(Error::NotFound)
        } else {
            Ok(None)
        };
    }
    let (_, sha) = object(conn, &x.workspace_id, &x.session_id, 0)?;
    let doc: Value =
        serde_json::from_str(&body(root, &sha)?).map_err(|_| Error::IntegrityFailed)?;
    if doc["kind"] != "publication_live_run" {
        return if index.is_some() {
            Err(Error::InvalidArgument)
        } else {
            Ok(None)
        };
    }
    let (v, _) = load(conn, root, &x.workspace_id, &x.session_id, actor)?;
    if v["sentence_mode"] != true {
        return Err(Error::CapabilityUnavailable);
    }
    if !matches!(
        state(&v)?,
        S::Ready | S::Dispatching | S::PauseRequested | S::Paused
    ) {
        return Err(Error::Cancelled);
    }
    let p = speech_plan(
        conn,
        root,
        fence,
        op,
        actor,
        &x.workspace_id,
        &x.session_id,
        now,
    )?
    .live_speech_plan
    .ok_or(Error::IntegrityFailed)?;
    let cursor = v["sentence_cursor"].as_u64().unwrap() as usize;
    let selected = index.map_or(cursor, |i| i as usize);
    if selected < cursor
        || selected >= cursor + 2
        || x.expected_epoch != p.epoch
        || x.snapshot_id != p.source.as_ref().unwrap().snapshot_id
        || x.snapshot_sha256 != p.source.as_ref().unwrap().snapshot_sha256
    {
        return Err(Error::RevisionConflict);
    }
    let s = p.sentences.get(selected).ok_or(Error::RevisionConflict)?;
    if s.cue_id != x.cue_id {
        return Err(Error::RevisionConflict);
    }
    Ok(Some((
        s.text.clone(),
        json!({"run_id":x.session_id,"plan_sha256":p.plan_sha256,"sentence_cursor":selected,"sentence_index":s.sentence_index}),
    )))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn preview_sentence(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    role_rev: u64,
    tick: u64,
    now: i64,
    x: &LiveSentencePreview,
) -> Result<FoundationResult> {
    let scope = x.run.as_ref().ok_or(Error::InvalidArgument)?;
    let p = speech_plan(
        tx,
        root,
        fence,
        op,
        actor,
        &scope.workspace_id,
        &scope.run_id,
        now,
    )?
    .live_speech_plan
    .ok_or(Error::IntegrityFailed)?;
    let s = p
        .sentences
        .get(x.sentence_cursor as usize)
        .ok_or(Error::InvalidArgument)?;
    let c = p.source.as_ref().ok_or(Error::IntegrityFailed)?;
    preview::submit(
        tx,
        root,
        fence,
        op,
        actor,
        role_rev,
        tick,
        now,
        &FoundationPreview {
            workspace_id: scope.workspace_id.clone(),
            preview_id: x.preview_id.clone(),
            snapshot_id: c.snapshot_id.clone(),
            snapshot_sha256: c.snapshot_sha256.clone(),
            cue_id: s.cue_id.clone(),
            session_id: scope.run_id.clone(),
            expected_epoch: p.epoch,
        },
        Some(x.sentence_cursor),
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn batch(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    x: &LiveBatchControl,
    now: i64,
) -> Result<FoundationResult> {
    let r = x.run.as_ref().ok_or(Error::InvalidArgument)?;
    let (mut v, revision) = load(tx, root, &r.workspace_id, &r.run_id, actor)?;
    let (epoch, stopped) = session(tx, &r.workspace_id, &r.run_id, actor)?;
    if revision != x.expected_revision || epoch != x.expected_epoch {
        return Err(Error::RevisionConflict);
    }
    if v["sentence_mode"] != true {
        return Err(Error::CapabilityUnavailable);
    }
    uuid(&x.approval_id)?;
    if x.revoke {
        let b = v.get_mut("batch_approval").ok_or(Error::NotFound)?;
        if b["approval_id"] != x.approval_id || b["plan_sha256"] != x.plan_sha256 {
            return Err(Error::RevisionConflict);
        }
        b["revoked"] = true.into();
    } else {
        let (r, _) = role(tx, &r.workspace_id, actor)?;
        if r != FoundationRole::Admin || !x.acknowledge_rights {
            return Err(Error::PermissionDenied);
        }
        if stopped || !matches!(state(&v)?, S::Ready | S::Paused) {
            return Err(Error::Cancelled);
        }
        let p = speech_plan(
            tx,
            root,
            fence,
            op,
            actor,
            &x.run.as_ref().unwrap().workspace_id,
            &x.run.as_ref().unwrap().run_id,
            now,
        )?
        .live_speech_plan
        .ok_or(Error::IntegrityFailed)?;
        if p.plan_sha256 != x.plan_sha256 {
            return Err(Error::RevisionConflict);
        }
        let mut used = v
            .get("batch_ids")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        // Keep bounded history even after replacement; revoked identifiers never become new grants.
        if used.iter().any(|id| id == &x.approval_id) {
            return Err(Error::RevisionConflict);
        }
        if used.len() >= 32 {
            return Err(Error::ResourceExhausted);
        }
        used.push(x.approval_id.clone().into());
        v["batch_ids"] = used.into();
        v["batch_approval"] = json!({"approval_id":x.approval_id,"plan_sha256":p.plan_sha256,"created_at_ms":now,"expires_at_ms":now+300000,"epoch":epoch,"revoked":false});
    }
    v["updated_at_ms"] = now.into();
    llm::put(tx, root, &r.workspace_id, &r.run_id, revision, &v, now)?;
    // Revocation remains available even if source permissions have expired.
    read(tx, root, fence, op, actor, &r.workspace_id, &r.run_id, now)
}
fn state(v: &Value) -> Result<S> {
    S::try_from(v["state"].as_i64().ok_or(Error::IntegrityFailed)? as i32)
        .map_err(|_| Error::IntegrityFailed)
}
/// Source-authoritative, read-only planning. This does not reserve cost or grant output rights.
#[allow(clippy::too_many_arguments)]
pub(super) fn speech_plan(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    id: &str,
    now: i64,
) -> Result<FoundationResult> {
    use nai_runtime_core::narration::{MAX_SENTENCE_CHARS, PREFETCH_LIMIT};
    let (v, revision) = load(conn, root, ws, id, actor)?;
    if v["sentence_mode"] != true {
        return Err(Error::CapabilityUnavailable);
    }
    let c = source(conn, root, fence, op, actor, ws, &v, now)?;
    let (epoch, stopped) = session(conn, ws, id, actor)?;
    if stopped || v["session_epoch"].as_u64() != Some(epoch) || state(&v)? == S::Ended {
        return Err(Error::Cancelled);
    }
    let (manifest, _) =
        publication::stored(conn, root, ws, &c.snapshot_id, "publication_snapshot")?;
    let docs = publication::documents(conn, root, fence, ws, &manifest["refs"], true)?;
    let (planned, mut current_start) = collect_plan(
        &docs,
        &v["cue_ids"],
        v["cursor"].as_u64().unwrap() as usize,
        op,
    )?;
    if v["sentence_mode"] == true {
        current_start = v["sentence_cursor"]
            .as_u64()
            .ok_or(Error::IntegrityFailed)? as usize;
    }
    let mut plan = LiveSpeechPlan {
        source: Some(c),
        run_id: id.into(),
        revision,
        epoch,
        sentences: planned,
        current_start: current_start as u32,
        prefetch_limit: PREFETCH_LIMIT as u32,
        max_sentence_chars: MAX_SENTENCE_CHARS as u32,
        ..Default::default()
    };
    plan.plan_sha256 = nai_contracts::live_speech_plan_hash(&plan);
    if let Some(b) = v.get("batch_approval") {
        let revoked = b["revoked"].as_bool().ok_or(Error::IntegrityFailed)?;
        let expires = b["expires_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?;
        plan.batch_approval = Some(LiveBatchApproval {
            approval_id: b["approval_id"]
                .as_str()
                .ok_or(Error::IntegrityFailed)?
                .into(),
            plan_sha256: b["plan_sha256"]
                .as_str()
                .ok_or(Error::IntegrityFailed)?
                .into(),
            expires_at_ms: expires as u64,
            approved: !revoked
                && now >= b["created_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?
                && now < expires
                && b["epoch"].as_u64() == Some(epoch)
                && b["plan_sha256"] == plan.plan_sha256,
            revoked,
        });
    }
    Ok(FoundationResult {
        resource_id: id.into(),
        revision,
        epoch,
        live_speech_plan: Some(plan),
        ..Default::default()
    })
}
fn collect_plan(
    docs: &[Value],
    ids: &Value,
    cursor: usize,
    op: &Operation,
) -> Result<(Vec<LiveSentence>, usize)> {
    use nai_runtime_core::narration::{MAX_SENTENCES, sentences};
    let mut planned = Vec::new();
    let mut current_start = 0;
    let mut text_bytes = 0usize;
    for (position, id) in ids
        .as_array()
        .ok_or(Error::IntegrityFailed)?
        .iter()
        .enumerate()
    {
        op.check()?;
        if position == cursor {
            current_start = planned.len();
        }
        let cue = docs[0]["cues"]
            .as_array()
            .ok_or(Error::IntegrityFailed)?
            .iter()
            .find(|cue| cue["id"] == *id)
            .ok_or(Error::IntegrityFailed)?;
        let track = docs[1]["talk_tracks"]
            .as_array()
            .ok_or(Error::IntegrityFailed)?
            .iter()
            .find(|track| track["id"] == cue["talk_track_id"])
            .ok_or(Error::IntegrityFailed)?;
        for (index, text) in sentences(track["text"].as_str().ok_or(Error::IntegrityFailed)?)?
            .into_iter()
            .enumerate()
        {
            text_bytes += text.len();
            if text_bytes > nai_runtime_core::narration::MAX_PLAN_BYTES {
                return Err(Error::ResourceExhausted);
            }
            planned.push(LiveSentence {
                cue_id: id.as_str().ok_or(Error::IntegrityFailed)?.into(),
                sentence_index: index as u32,
                text,
            });
            if planned.len() > MAX_SENTENCES {
                return Err(Error::ResourceExhausted);
            }
        }
    }
    if cursor == ids.as_array().unwrap().len() {
        current_start = planned.len();
    }
    Ok((planned, current_start))
}
pub(super) fn session(conn: &Connection, ws: &str, id: &str, actor: &str) -> Result<(u64, bool)> {
    let (owner, epoch, stopped): (String, u64, bool) = conn
        .query_row(
            "SELECT owner,epoch,stopped FROM sessions WHERE workspace_id=?1 AND id=?2",
            params![ws, id],
            |r| Ok((r.get(0)?, uint(r, 1)?, r.get(2)?)),
        )
        .wb()?;
    if owner != actor {
        return Err(Error::PermissionDenied);
    }
    Ok((epoch, stopped))
}
#[allow(clippy::too_many_arguments)]
fn source(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    v: &Value,
    now: i64,
) -> Result<AgentTaskContext> {
    op.check()?;
    fence.check(fence.generation())?;
    let (_, rev) = role(conn, ws, actor)?;
    fence.check_scope(&role_scope(ws, actor, rev))?;
    if Some(rev) != v["role_revision"].as_u64() {
        return Err(Error::PermissionDenied);
    }
    let c = nai_runtime_core::task_context::from_value(&v["context"])?;
    if c.workspace_id != ws {
        return Err(Error::IntegrityFailed);
    }
    let expected = model_context::from_snapshot(
        conn,
        root,
        fence,
        op,
        ws,
        &json!({"object_id":c.snapshot_id,"sha256":c.snapshot_sha256}),
        now,
    )?;
    model_context::verify(&v["context"], &expected)?;
    Ok(c)
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
    let (v, revision) = load(conn, root, ws, id, actor)?;
    let (epoch, stopped) = session(conn, ws, id, actor)?;
    let mut status = state(&v)?;
    let mut reason = "READY_FOR_EXPLICIT_PREPARATION";
    if status != S::Ended {
        if source(conn, root, fence, op, actor, ws, &v, now).is_err() {
            status = S::Unavailable;
            reason = "SOURCE_UNAVAILABLE";
        } else if stopped || v["session_epoch"].as_u64() != Some(epoch) {
            status = S::Interrupted;
            reason = "SESSION_RECOVERY_REQUIRED";
        } else {
            reason = match status {
                S::Dispatching => "REQUESTED_NOT_HARDWARE_ACK",
                S::PauseRequested => "PAUSE_AFTER_CURRENT_SUBMISSION",
                S::Paused => "PAUSED",
                S::Interrupted => "UNCERTAIN_SENTENCE_REQUIRES_SKIP",
                S::Completed => {
                    if v["skipped_count"].as_u64().unwrap_or(0) > 0 {
                        "QUEUE_FINISHED_WITH_SKIPS"
                    } else {
                        "QUEUE_SUBMISSION_COMPLETE"
                    }
                }
                _ => reason,
            };
        }
    }
    if status != S::Ended && v["sentence_mode"] != true {
        status = S::Unavailable;
        reason = "QUEUE_RECREATE_REQUIRED";
    }
    let ids: Vec<String> = v["cue_ids"]
        .as_array()
        .ok_or(Error::IntegrityFailed)?
        .iter()
        .map(|x| x.as_str().map(str::to_owned).ok_or(Error::IntegrityFailed))
        .collect::<Result<_>>()?;
    let cursor = v["cursor"].as_u64().ok_or(Error::IntegrityFailed)? as usize;
    Ok(FoundationResult {
        resource_id: id.into(),
        revision,
        epoch,
        live_context: Some(LiveContext {
            source: Some(nai_runtime_core::task_context::from_value(&v["context"])?),
            run_id: id.into(),
            revision,
            session_id: id.into(),
            epoch,
            controller_id: actor.into(),
            current_cue_id: ids.get(cursor).cloned().unwrap_or_default(),
            next_cue_id: ids.get(cursor + 1).cloned().unwrap_or_default(),
            cue_ids: ids,
            cursor: cursor as u32,
            state: status as i32,
            submitted_count: v["submitted_count"].as_u64().unwrap() as u32,
            skipped_count: v["skipped_count"].as_u64().unwrap() as u32,
            reason: reason.into(),
            sentence_mode: v["sentence_mode"] == true,
            sentence_cursor: v["sentence_cursor"].as_u64().unwrap_or(0) as u32,
            current_sentence_index: v["current_sentence_index"].as_u64().unwrap_or(0) as u32,
            submitted_sentences: v["submitted_sentences"].as_u64().unwrap_or(0) as u32,
            skipped_sentences: v["skipped_sentences"].as_u64().unwrap_or(0) as u32,
            sentence_total: v["sentence_total"].as_u64().unwrap_or(0) as u32,
            human_takeover: v["takeover"]["active"] == true
                && !stopped
                && v["session_epoch"].as_u64() == Some(epoch),
            takeover_id: v["takeover"]["takeover_id"].as_str().unwrap_or("").into(),
            partial_sentence: v["takeover"]["partial_sentence"] == true,
        }),
        ..Default::default()
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn create(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    revision: u64,
    x: &LiveRunCreate,
    now: i64,
) -> Result<FoundationResult> {
    uuid(&x.run_id)?;
    let scope = x.scope.as_ref().ok_or(Error::InvalidArgument)?;
    let c = model_context::from_snapshot(
        tx,
        root,
        fence,
        op,
        &scope.workspace_id,
        &json!({"object_id":scope.snapshot_id,"sha256":scope.snapshot_sha256}),
        now,
    )?;
    let (manifest, _) = publication::stored(
        tx,
        root,
        &scope.workspace_id,
        &scope.snapshot_id,
        "publication_snapshot",
    )?;
    let docs = publication::documents(
        tx,
        root,
        fence,
        &scope.workspace_id,
        &manifest["refs"],
        true,
    )?;
    let ids: Vec<Value> = docs[0]["cues"]
        .as_array()
        .ok_or(Error::IntegrityFailed)?
        .iter()
        .map(|x| x["id"].clone())
        .collect();
    if ids.is_empty() || ids.len() > 32 {
        return Err(Error::ResourceExhausted);
    }
    let (sentences, _) = collect_plan(&docs, &Value::Array(ids.clone()), 0, op)?;
    let count: u64 = tx
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE workspace_id=?1 AND owner=?2 AND stopped=0",
            params![scope.workspace_id, actor],
            |r| uint(r, 0),
        )
        .wb()?;
    if count >= 4 {
        return Err(Error::ResourceExhausted);
    }
    let name = format!("讲品队列 · {}", docs[0]["name"].as_str().unwrap_or("本场"))
        .chars()
        .take(128)
        .collect::<String>();
    let v = json!({"schema_version":1,"kind":"publication_live_run","name":name,"created_by":actor,"role_revision":revision,"created_at_ms":now,"updated_at_ms":now,"context":nai_runtime_core::task_context::to_value(&c),"session_epoch":1,"cue_ids":ids,"cursor":0,"state":S::Ready as i32,"submitted_count":0,"skipped_count":0,"active_approval":"","active_bundle":"","last_approval":"","last_bundle":"","sentence_mode":true,"sentence_cursor":0,"current_sentence_index":0,"submitted_sentences":0,"skipped_sentences":0,"sentence_total":sentences.len()});
    publication::insert(tx, root, &scope.workspace_id, &x.run_id, &v, now)?;
    tx.execute(
        "INSERT INTO sessions VALUES(?1,?2,?3,1,0,1,?4,?4)",
        params![scope.workspace_id, x.run_id, actor, now],
    )
    .wb()?;
    read(
        tx,
        root,
        fence,
        op,
        actor,
        &scope.workspace_id,
        &x.run_id,
        now,
    )
}
#[allow(clippy::too_many_arguments)]
pub(super) fn control(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    x: &LiveRunControl,
    now: i64,
) -> Result<FoundationResult> {
    let ws = &x.workspace_id;
    let id = &x.run_id;
    let (mut v, r) = load(tx, root, ws, id, actor)?;
    let (epoch, stopped) = session(tx, ws, id, actor)?;
    if r != x.expected_revision || epoch != x.expected_epoch {
        return Err(Error::RevisionConflict);
    }
    let action = A::try_from(x.action).map_err(|_| Error::InvalidArgument)?;
    if v["takeover"]["active"] == true && !matches!(action, A::End | A::Recover) {
        return Err(Error::PermissionDenied);
    }
    if crate::live_question::active(&v) && action == A::Resume {
        return Err(Error::PermissionDenied);
    }
    if v["sentence_mode"] != true && action != A::End {
        return Err(Error::CapabilityUnavailable);
    }
    let old = state(&v)?;
    if old == S::Ended {
        return Err(Error::RevisionConflict);
    }
    if action != A::End {
        source(tx, root, fence, op, actor, ws, &v, now)?;
    }
    let next: S;
    match action {
        A::End => {
            if let Some(t) = v.get_mut("takeover") {
                t["active"] = false.into();
            }
            let next_epoch = epoch.checked_add(1).ok_or(Error::ResourceExhausted)?;
            tx.execute("UPDATE sessions SET stopped=1,epoch=?1,revision=revision+1 WHERE workspace_id=?2 AND id=?3",params![int(next_epoch)?,ws,id]).wb()?;
            v["session_epoch"] = next_epoch.into();
            next = S::Ended;
        }
        A::Recover => {
            if let Some(t) = v.get_mut("takeover") {
                t["active"] = false.into();
            }
            if !stopped {
                return Err(Error::RevisionConflict);
            }
            let next_epoch = epoch.checked_add(1).ok_or(Error::ResourceExhausted)?;
            tx.execute("UPDATE sessions SET stopped=0,epoch=?1,revision=revision+1 WHERE workspace_id=?2 AND id=?3",params![int(next_epoch)?,ws,id]).wb()?;
            v["session_epoch"] = next_epoch.into();
            next = if v["active_approval"] == "" {
                if old == S::Completed {
                    S::Completed
                } else {
                    S::Paused
                }
            } else {
                S::Interrupted
            };
        }
        _ => {
            if stopped || v["session_epoch"].as_u64() != Some(epoch) {
                return Err(Error::Cancelled);
            }
            if action == A::SkipInterrupted {
                if old != S::Interrupted {
                    return Err(Error::RevisionConflict);
                }
                let cursor = v["cursor"].as_u64().unwrap();
                let n = v["cue_ids"].as_array().unwrap().len() as u64;
                if cursor >= n {
                    return Err(Error::RevisionConflict);
                }
                if v["sentence_mode"] == true {
                    let p = speech_plan(tx, root, fence, op, actor, ws, id, now)?
                        .live_speech_plan
                        .ok_or(Error::IntegrityFailed)?;
                    let start = v["sentence_cursor"].as_u64().unwrap() as usize;
                    let cue = &v["cue_ids"][cursor as usize];
                    let end = (start..p.sentences.len())
                        .find(|&i| cue.as_str() != Some(p.sentences[i].cue_id.as_str()))
                        .unwrap_or(p.sentences.len());
                    v["sentence_cursor"] = (end as u64).into();
                    v["skipped_sentences"] =
                        (v["skipped_sentences"].as_u64().unwrap() + (end - start) as u64).into();
                    v["current_sentence_index"] = 0.into();
                }
                v["cursor"] = (cursor + 1).into();
                v["skipped_count"] = (v["skipped_count"].as_u64().unwrap() + 1).into();
                v["active_approval"] = "".into();
                v["active_bundle"] = "".into();
                next = if cursor + 1 == n {
                    S::Completed
                } else {
                    S::Ready
                };
            } else {
                next = nai_runtime_core::live::control(old, action)?;
            }
        }
    }
    v["state"] = (next as i32).into();
    v["updated_at_ms"] = now.into();
    llm::put(tx, root, ws, id, r, &v, now)?;
    read(tx, root, fence, op, actor, ws, id, now)
}
/// Called only after the keeper observation has been verified by the root.
#[allow(clippy::too_many_arguments)]
pub(super) fn output(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    b: &PublicMediaBundle,
    action: &str,
    now: i64,
) -> Result<()> {
    let exists: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM objects WHERE workspace_id=?1 AND id=?2)",
            params![b.workspace_id, b.session_id],
            |r| r.get(0),
        )
        .wb()?;
    if !exists {
        return Ok(());
    }
    let (_, sha) = object(tx, &b.workspace_id, &b.session_id, 0)?;
    let doc: Value =
        serde_json::from_str(&body(root, &sha)?).map_err(|_| Error::IntegrityFailed)?;
    if doc["kind"] != "publication_live_run" {
        return Ok(());
    }
    let (mut v, revision) = load(tx, root, &b.workspace_id, &b.session_id, actor)?;
    if v["sentence_mode"] != true && action != "public_execution_stopped" {
        return Err(Error::CapabilityUnavailable);
    }
    if action == "public_submission_complete"
        && v["last_approval"] == b.approval_id
        && v["last_bundle"] == b.approved_sha256
    {
        return Ok(());
    }
    if action == "public_execution_stopped" && v["active_approval"] == "" {
        return Ok(());
    }
    let (epoch, stopped) = session(tx, &b.workspace_id, &b.session_id, actor)?;
    if stopped || epoch != b.epoch || v["session_epoch"].as_u64() != Some(epoch) {
        return Err(Error::Cancelled);
    }
    let mut sentence_last = true;
    if action != "public_execution_stopped" {
        let c = source(tx, root, fence, op, actor, &b.workspace_id, &v, now)?;
        if c.snapshot_id != b.snapshot_id
            || c.snapshot_sha256 != b.snapshot_sha256
            || c.facts_sha256 != b.facts_sha256
        {
            return Err(Error::RevisionConflict);
        }
        let audio = b.audio.as_ref().ok_or(Error::IntegrityFailed)?;
        let (task, _, _) = llm::raw(
            tx,
            root,
            &b.workspace_id,
            &audio.artifact_id,
            0,
            "publication_tts_preview",
        )?;
        crate::live_auto::task_gate(
            tx,
            root,
            fence,
            op,
            actor,
            &b.workspace_id,
            &audio.artifact_id,
            &task,
            now,
        )?;
        let (card, _) = publication::stored(
            tx,
            root,
            &b.workspace_id,
            task["source"]["object_id"]
                .as_str()
                .ok_or(Error::IntegrityFailed)?,
            "publication_private_preview",
        )?;
        if card.get("live_question").is_some() {
            return crate::live_question::output(
                tx, root, fence, op, actor, b, action, v, revision, &card, now,
            );
        }
        let cursor = v["cursor"].as_u64().unwrap() as usize;
        if task["state"] != "completed"
            || task["audio_sha256"] != audio.pcm_sha256
            || task["sample_count"].as_u64() != Some(audio.sample_count)
            || task["created_by"] != actor
            || card["session_id"] != b.session_id
            || card["epoch"] != b.epoch
            || v["cue_ids"].as_array().unwrap().get(cursor) != Some(&card["cue_id"])
        {
            return Err(Error::RevisionConflict);
        }
        if v["sentence_mode"] == true {
            let p = speech_plan(
                tx,
                root,
                fence,
                op,
                actor,
                &b.workspace_id,
                &b.session_id,
                now,
            )?
            .live_speech_plan
            .ok_or(Error::IntegrityFailed)?;
            let index = v["sentence_cursor"]
                .as_u64()
                .ok_or(Error::IntegrityFailed)? as usize;
            let sentence = p.sentences.get(index).ok_or(Error::RevisionConflict)?;
            let identity = &card["live_sentence"];
            if identity["run_id"] != b.session_id
                || identity["plan_sha256"] != p.plan_sha256
                || identity["sentence_cursor"].as_u64() != Some(index as u64)
                || identity["sentence_index"].as_u64() != Some(sentence.sentence_index as u64)
                || card["text"] != sentence.text
            {
                return Err(Error::RevisionConflict);
            }
            sentence_last = p
                .sentences
                .get(index + 1)
                .is_none_or(|next| next.cue_id != sentence.cue_id);
        }
    }
    let old = state(&v)?;
    if action == "public_execution_requested" {
        if old == S::Dispatching
            && v["active_approval"] == b.approval_id
            && v["active_bundle"] == b.approved_sha256
        {
            return Ok(());
        }
        if old != S::Ready {
            return Err(Error::RevisionConflict);
        }
        v["state"] = (S::Dispatching as i32).into();
        v["active_approval"] = b.approval_id.clone().into();
        v["active_bundle"] = b.approved_sha256.clone().into();
    } else {
        if v["active_approval"] != b.approval_id || v["active_bundle"] != b.approved_sha256 {
            return Err(Error::RevisionConflict);
        }
        if action == "public_execution_stopped" {
            let q = &v["auto_grant"]["question"];
            if q["state"] == "playing"
                && q["task"]["task_id"].as_str() == b.audio.as_ref().map(|a| a.artifact_id.as_str())
            {
                review_questions::delivery(
                    tx,
                    root,
                    actor,
                    &b.workspace_id,
                    &b.session_id,
                    q["faq_task_id"].as_str().ok_or(Error::IntegrityFailed)?,
                    "stopped",
                    now,
                )?;
            }
            v["state"] = (S::Interrupted as i32).into();
        } else {
            let cursor = v["cursor"].as_u64().unwrap() as usize;
            let next = nai_runtime_core::live::submitted(
                old,
                sentence_last && cursor + 1 == v["cue_ids"].as_array().unwrap().len(),
            )?;
            v["state"] = (if crate::live_question::active(&v) {
                S::Paused
            } else {
                next
            } as i32)
                .into();
            if v["sentence_mode"] == true {
                v["sentence_cursor"] = (v["sentence_cursor"].as_u64().unwrap() + 1).into();
                v["submitted_sentences"] = (v["submitted_sentences"].as_u64().unwrap() + 1).into();
                v["current_sentence_index"] = if sentence_last {
                    0.into()
                } else {
                    (v["current_sentence_index"].as_u64().unwrap() + 1).into()
                };
            }
            if sentence_last {
                v["cursor"] = (cursor + 1).into();
                v["submitted_count"] = (v["submitted_count"].as_u64().unwrap() + 1).into();
            }
            v["last_approval"] = b.approval_id.clone().into();
            v["last_bundle"] = b.approved_sha256.clone().into();
            v["active_approval"] = "".into();
            v["active_bundle"] = "".into();
        }
    }
    v["updated_at_ms"] = now.into();
    llm::put(tx, root, &b.workspace_id, &b.session_id, revision, &v, now)?;
    Ok(())
}
