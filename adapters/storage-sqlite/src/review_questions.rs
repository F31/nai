//! Independent review-purpose input/receipts. Candidate answer bodies are never copied.
use super::*;
use serde_json::{Value, json};
pub(super) const CAPACITY: usize = 64;
fn stable_id(key: &str) -> String {
    let h = hash(key.as_bytes());
    format!(
        "{}-{}-4{}-a{}-{}",
        &h[..8],
        &h[8..12],
        &h[13..16],
        &h[17..20],
        &h[20..32]
    )
}
fn record_id(ws: &str, actor: &str, question: &str) -> String {
    stable_id(&format!("review-question:{ws}:{actor}:{question}"))
}
fn index_id(s: &ReviewSource, actor: &str) -> String {
    stable_id(&format!(
        "review-question-index:{}:{}:{}:{actor}",
        s.workspace_id, s.run_id, s.run_kind
    ))
}
fn run_kind(s: &ReviewSource) -> Result<&'static str> {
    match ReviewRunKind::try_from(s.run_kind) {
        Ok(ReviewRunKind::Manual) => Ok("manual"),
        Ok(ReviewRunKind::Live) => Ok("live"),
        _ => Err(Error::InvalidArgument),
    }
}
fn same_run(v: &Value, s: &ReviewSource) -> bool {
    v["source"]["run_id"] == s.run_id && v["source"]["run_kind"].as_str() == run_kind(s).ok()
}
fn record(
    conn: &Connection,
    root: &Path,
    ws: &str,
    actor: &str,
    question: &str,
) -> Result<Option<(Value, u64, String)>> {
    match llm::raw(
        conn,
        root,
        ws,
        &record_id(ws, actor, question),
        0,
        "publication_review_question",
    ) {
        Ok((v, r, sha)) if v["created_by"] == actor => Ok(Some((v, r, sha))),
        Ok(_) => Err(Error::PermissionDenied),
        Err(Error::NotFound) => Ok(None),
        Err(e) => Err(e),
    }
}
pub(super) fn belongs(
    conn: &Connection,
    root: &Path,
    ws: &str,
    actor: &str,
    question: &str,
    source: Option<&ReviewSource>,
) -> Result<bool> {
    Ok(match (record(conn, root, ws, actor, question)?, source) {
        (None, None) => true,
        (Some((v, _, _)), Some(s)) => {
            if !same_run(&v, s) {
                false
            } else if s.run_kind == ReviewRunKind::Live as i32 {
                let (run, _, _) = llm::raw(conn, root, ws, &s.run_id, 0, "publication_live_run")?;
                v["session_epoch"] == run["session_epoch"]
            } else {
                v["session_epoch"] == 0
            }
        }
        _ => false,
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn active(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    s: &ReviewSource,
    scope: &QuestionPoolRead,
    epoch: u64,
    now: i64,
) -> Result<Value> {
    if s.workspace_id != scope.workspace_id {
        return Err(Error::PermissionDenied);
    }
    let proof = review::active_question_source(conn, root, fence, op, actor, s, epoch, now)?;
    if proof["snapshot_id"] != scope.snapshot_id
        || proof["snapshot_sha256"] != scope.snapshot_sha256
    {
        return Err(Error::RevisionConflict);
    }
    Ok(proof)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn remember(
    tx: &Transaction<'_>,
    root: &Path,
    actor: &str,
    s: &ReviewSource,
    proof: &Value,
    epoch: u64,
    e: &QuestionPoolEntry,
    now: i64,
) -> Result<()> {
    let id = record_id(&s.workspace_id, actor, &e.id);
    if let Some((mut v, r, _)) = record(tx, root, &s.workspace_id, actor, &e.id)? {
        if !same_run(&v, s) {
            return Err(Error::RevisionConflict);
        }
        v["occurrences"] = e.occurrences.into();
        v["updated_at_ms"] = now.into();
        llm::put(tx, root, &s.workspace_id, &id, r, &v, now)?;
        return Ok(());
    }
    let ix = index_id(s, actor);
    let (mut idx, rev) = match llm::raw(
        tx,
        root,
        &s.workspace_id,
        &ix,
        0,
        "publication_review_question_index",
    ) {
        Ok((v, r, _)) if v["created_by"] == actor => (v, r),
        Ok(_) => return Err(Error::PermissionDenied),
        Err(Error::NotFound) => (
            json!({"schema_version":1,"kind":"publication_review_question_index","created_by":actor,"created_at_ms":now,"updated_at_ms":now,"source_run_id":s.run_id,"source_run_kind":run_kind(s)?,"questions":[]}),
            0,
        ),
        Err(e) => return Err(e),
    };
    let ids = idx["questions"]
        .as_array_mut()
        .ok_or(Error::IntegrityFailed)?;
    if ids.len() >= CAPACITY {
        return Err(Error::ResourceExhausted);
    }
    ids.push(id.clone().into());
    idx["updated_at_ms"] = now.into();
    let v = json!({"schema_version":1,"kind":"publication_review_question","created_by":actor,"created_at_ms":now,"updated_at_ms":now,"source":proof,"session_epoch":epoch,"question_id":e.id,"question":e.question,"occurrences":e.occurrences,"expires_at_ms":e.expires_at_ms,"processing":"queued","delivery":"unconfirmed","candidate_id":"","candidate_sha256":""});
    llm::put(tx, root, &s.workspace_id, &id, 0, &v, now)?;
    llm::put(tx, root, &s.workspace_id, &ix, rev, &idx, now)?;
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub(super) fn progress(
    tx: &Transaction<'_>,
    root: &Path,
    actor: &str,
    ws: &str,
    entry: &QuestionPoolEntry,
    now: i64,
) -> Result<()> {
    let Some((mut v, rev, _)) = record(tx, root, ws, actor, &entry.id)? else {
        return Ok(());
    };
    v["occurrences"] = entry.occurrences.into();
    v["candidate_id"] = entry.task_id.clone().into();
    v["candidate_sha256"] = entry.task_sha256.clone().into();
    v["processing"] = match entry.reason.as_str() {
        "ADOPTED_TO_DRAFT_ONLY" => {
            if entry.suggestion_state == SuggestionState::Edited as i32 {
                "edited"
            } else {
                "adopted"
            }
        }
        "QUESTION_CANCELLED" => "cancelled",
        "CANDIDATE_IGNORED" => "ignored",
        "NO_ADOPTABLE_CANDIDATE" => "no_candidate",
        "CANDIDATE_WAITING_HUMAN" => "waiting_human",
        _ => return Ok(()),
    }
    .into();
    v["updated_at_ms"] = now.into();
    llm::put(tx, root, ws, &record_id(ws, actor, &entry.id), rev, &v, now)?;
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub(super) fn resolution(
    tx: &Transaction<'_>,
    root: &Path,
    actor: &str,
    x: &FoundationFaqResolve,
    now: i64,
) -> Result<()> {
    let (child, _, _) = llm::raw(
        tx,
        root,
        &x.workspace_id,
        &x.task_id,
        0,
        "publication_faq_task",
    )?;
    let Some(q) = child["parent_task"]["parent_id"].as_str() else {
        return Ok(());
    };
    let Some((mut v, rev, _)) = record(tx, root, &x.workspace_id, actor, q)? else {
        return Ok(());
    };
    v["processing"] = match FaqDecision::try_from(x.decision) {
        Ok(FaqDecision::Accept) => "adopted",
        Ok(FaqDecision::Edit) => "edited",
        Ok(FaqDecision::Ignore) => "ignored",
        Ok(FaqDecision::Cancel) => "cancelled",
        _ => return Err(Error::InvalidArgument),
    }
    .into();
    v["updated_at_ms"] = now.into();
    llm::put(
        tx,
        root,
        &x.workspace_id,
        &record_id(&x.workspace_id, actor, q),
        rev,
        &v,
        now,
    )?;
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub(super) fn delivery(
    tx: &Transaction<'_>,
    root: &Path,
    actor: &str,
    ws: &str,
    run: &str,
    faq: &str,
    state: &str,
    now: i64,
) -> Result<()> {
    let (child, _, _) = llm::raw(tx, root, ws, faq, 0, "publication_faq_task")?;
    let Some(q) = child["parent_task"]["parent_id"].as_str() else {
        return Ok(());
    };
    let Some((mut v, rev, _)) = record(tx, root, ws, actor, q)? else {
        return Ok(());
    };
    if v["source"]["run_id"] != run {
        return Err(Error::RevisionConflict);
    }
    if v["delivery"] == state {
        return Ok(());
    }
    v["delivery"] = state.into();
    v["updated_at_ms"] = now.into();
    llm::put(tx, root, ws, &record_id(ws, actor, q), rev, &v, now)?;
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub(super) fn view(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    ws: &str,
    actor: &str,
    s: &ReviewSource,
    id: &str,
    now: i64,
) -> Result<ReviewQuestion> {
    fence.check_scope(&object_scope(ws, id))?;
    let (v, rev, sha) = llm::raw(conn, root, ws, id, 0, "publication_review_question")?;
    if v["created_by"] != actor || !same_run(&v, s) {
        return Err(Error::PermissionDenied);
    }
    fence.check_scope(&publication::version_scope(ws, id, rev))?;
    let (_, run_sha) = object(conn, ws, &s.run_id, 0)?;
    let run: Value =
        serde_json::from_str(&body(root, &run_sha)?).map_err(|_| Error::IntegrityFailed)?;
    let (snapshot_id, snapshot_sha) = if s.run_kind == ReviewRunKind::Manual as i32 {
        (&run["snapshot"]["object_id"], &run["snapshot"]["sha256"])
    } else {
        (
            &run["context"]["snapshot_id"],
            &run["context"]["snapshot_sha256"],
        )
    };
    if snapshot_id != &v["source"]["snapshot_id"] || snapshot_sha != &v["source"]["snapshot_sha256"]
    {
        return Err(Error::RevisionConflict);
    }
    let qid = v["question_id"].as_str().ok_or(Error::IntegrityFailed)?;
    fence.check_scope(&object_scope(ws, qid))?;
    let (parent_rev, _) = object(conn, ws, qid, 0)?;
    fence.check_scope(&publication::version_scope(ws, qid, parent_rev))?;
    let (parent, _, _) = llm::raw(conn, root, ws, qid, 0, "publication_question_task")?;
    if parent["created_by"] != actor
        || parent["question"] != v["question"]
        || parent["context"]["snapshot_id"] != v["source"]["snapshot_id"]
        || parent["context"]["snapshot_sha256"] != v["source"]["snapshot_sha256"]
    {
        return Err(Error::IntegrityFailed);
    }
    let string = |key: &str| {
        v[key]
            .as_str()
            .map(str::to_owned)
            .ok_or(Error::IntegrityFailed)
    };
    let expires = v["expires_at_ms"].as_u64().ok_or(Error::IntegrityFailed)?;
    Ok(ReviewQuestion {
        record: Some(ReviewObjectRef {
            object_id: id.into(),
            revision: rev,
            sha256: sha,
        }),
        question_id: qid.into(),
        question: string("question")?,
        occurrences: v["occurrences"].as_u64().ok_or(Error::IntegrityFailed)? as u32,
        processing: string("processing")?,
        delivery: string("delivery")?,
        candidate_id: string("candidate_id")?,
        candidate_sha256: string("candidate_sha256")?,
        runtime_expired: now < v["created_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?
            || now as u64 >= expires,
        expires_at_ms: expires,
        updated_at_ms: v["updated_at_ms"].as_u64().ok_or(Error::IntegrityFailed)?,
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn list(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    s: &ReviewSource,
    now: i64,
) -> Result<Vec<ReviewQuestion>> {
    let (idx, _, _) = match llm::raw(
        conn,
        root,
        &s.workspace_id,
        &index_id(s, actor),
        0,
        "publication_review_question_index",
    ) {
        Ok(v) => v,
        Err(Error::NotFound) => return Ok(vec![]),
        Err(e) => return Err(e),
    };
    if idx["created_by"] != actor {
        return Err(Error::PermissionDenied);
    }
    let mut out = vec![];
    for id in idx["questions"].as_array().ok_or(Error::IntegrityFailed)? {
        op.check()?;
        out.push(view(
            conn,
            root,
            fence,
            &s.workspace_id,
            actor,
            s,
            id.as_str().ok_or(Error::IntegrityFailed)?,
            now,
        )?);
    }
    Ok(out)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn check_reference(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    ws: &str,
    actor: &str,
    s: &ReviewSource,
    r: &ReviewObjectRef,
    now: i64,
) -> Result<()> {
    let v = view(conn, root, fence, ws, actor, s, &r.object_id, now)?;
    if v.record.as_ref() != Some(r) {
        return Err(Error::RevisionConflict);
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub(super) fn check_dispatch(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    question: &str,
    now: i64,
) -> Result<()> {
    let Some((v, _, _)) = record(conn, root, ws, actor, question)? else {
        return Ok(());
    };
    let source = ReviewSource {
        workspace_id: ws.into(),
        run_id: v["source"]["run_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?
            .into(),
        run_kind: if v["source"]["run_kind"] == "live" {
            ReviewRunKind::Live as i32
        } else {
            ReviewRunKind::Manual as i32
        },
    };
    review::active_question_source(
        conn,
        root,
        fence,
        op,
        actor,
        &source,
        v["session_epoch"].as_u64().ok_or(Error::IntegrityFailed)?,
        now,
    )?;
    Ok(())
}
pub(super) fn check_answer_run(
    conn: &Connection,
    root: &Path,
    actor: &str,
    ws: &str,
    run: &str,
    faq: &Value,
) -> Result<()> {
    if let Some(parent) = faq["parent_task"]["parent_id"].as_str()
        && let Some((v, _, _)) = record(conn, root, ws, actor, parent)?
        && (v["source"]["run_id"] != run || v["source"]["run_kind"] != "live")
    {
        return Err(Error::RevisionConflict);
    }
    Ok(())
}
