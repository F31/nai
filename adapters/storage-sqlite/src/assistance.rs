//! RF04 bounded rule assistance; all mutation stays in this owner's transaction.
use super::*;
use serde_json::{Value, json};
fn terminal_id(task: &str) -> String {
    let s = hash(format!("nai-faq-terminal:{task}").as_bytes());
    format!(
        "{}-{}-{}-{}-{}",
        &s[..8],
        &s[8..12],
        &s[12..16],
        &s[16..20],
        &s[20..32]
    )
}
fn terminal(conn: &Connection, root: &Path, ws: &str, task: &str) -> Result<Option<Value>> {
    let id = terminal_id(task);
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM objects WHERE workspace_id=?1 AND id=?2)",
            params![ws, id],
            |r| r.get(0),
        )
        .wb()?;
    if !exists {
        return Ok(None);
    }
    let (v, _) = publication::stored(conn, root, ws, &id, "publication_faq_resolution")?;
    if v["task"]["object_id"] != task {
        return Err(Error::IntegrityFailed);
    }
    Ok(Some(v))
}
fn task(
    conn: &Connection,
    root: &Path,
    actor: &str,
    ws: &str,
    id: &str,
) -> Result<(Value, String)> {
    uuid(id)?;
    let (v, sha) = publication::stored(conn, root, ws, id, "publication_faq_task")?;
    if v["created_by"] != actor {
        return Err(Error::PermissionDenied);
    }
    Ok((v, sha))
}
#[allow(clippy::too_many_arguments)]
fn context(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    v: &Value,
    now: i64,
) -> Result<[Value; 4]> {
    op.check()?;
    if v.get("parent_task").is_some() {
        crate::question_pool::check_parent(conn, root, fence, op, actor, ws, v, now)?;
    }
    fence.check(fence.generation())?;
    let (_, rev) = role(conn, ws, actor)?;
    if v["role_revision"].as_u64() != Some(rev) {
        return Err(Error::PermissionDenied);
    }
    fence.check_scope(&role_scope(ws, actor, rev))?;
    if now < v["created_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?
        || now >= v["expires_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?
    {
        return Err(Error::DeadlineExceeded);
    }
    let snapshot = publication::read(
        conn,
        root,
        fence,
        op,
        ws,
        v["snapshot"]["object_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?,
        now,
    )?;
    if snapshot.publication_status != PublicationStatus::PrivateContent as i32
        || v["snapshot"]["sha256"] != snapshot.sha256
    {
        return Err(Error::CapabilityUnavailable);
    }
    let docs = publication::documents(conn, root, fence, ws, &v["refs"], true)?;
    nai_runtime_core::publication::closure(op, [&docs[0], &docs[1], &docs[2], &docs[3]], now)?;
    Ok(docs)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn ask(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    role_rev: u64,
    now: i64,
    x: &FoundationFaqRequest,
) -> Result<FoundationResult> {
    ask_inner(tx, root, fence, op, actor, role_rev, now, x, None)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn ask_for_pool(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    role_rev: u64,
    now: i64,
    x: &FoundationFaqRequest,
    parent: &str,
    context: &AgentTaskContext,
) -> Result<FoundationResult> {
    ask_inner(
        tx,
        root,
        fence,
        op,
        actor,
        role_rev,
        now,
        x,
        Some((parent, context)),
    )
}
#[allow(clippy::too_many_arguments)]
fn ask_inner(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    role_rev: u64,
    now: i64,
    x: &FoundationFaqRequest,
    parent: Option<(&str, &AgentTaskContext)>,
) -> Result<FoundationResult> {
    uuid(&x.task_id)?;
    let count:u32=tx.query_row("SELECT COUNT(*) FROM audit_events WHERE workspace_id=?1 AND principal_id=?2 AND action='faq_lookup' AND created_at>=?3",params![x.workspace_id,actor,now-60000],|r|r.get(0)).wb()?;
    if count >= 12 {
        return Err(Error::ResourceExhausted);
    }
    let snapshot = publication::read(tx, root, fence, op, &x.workspace_id, &x.snapshot_id, now)?;
    if snapshot.publication_status != PublicationStatus::PrivateContent as i32 {
        return Err(Error::CapabilityUnavailable);
    }
    if snapshot.sha256 != x.snapshot_sha256 {
        return Err(Error::RevisionConflict);
    }
    let manifest: Value =
        serde_json::from_str(&snapshot.body).map_err(|_| Error::IntegrityFailed)?;
    let docs = publication::documents(tx, root, fence, &x.workspace_id, &manifest["refs"], true)?;
    let closure =
        nai_runtime_core::publication::closure(op, [&docs[0], &docs[1], &docs[2], &docs[3]], now)?;
    let found = nai_runtime_core::assistance::lookup(op, &docs[1], &x.question)?;
    let mut value = json!({"schema_version":1,"kind":"publication_faq_task","created_at_ms":now,"created_by":actor,"role_revision":role_rev,"snapshot":{"object_id":x.snapshot_id,"revision":1,"sha256":snapshot.sha256},"refs":manifest["refs"],"question":x.question,"expires_at_ms":closure.valid_until_ms.unwrap_or(now+300000).min(now+300000),"provider_calls":0,"api_cost_minor":0,"currency":"CNY","target":"private_workbench"});
    for (key, value2) in found.as_object().ok_or(Error::IntegrityFailed)? {
        value[key] = value2.clone();
    }
    if let Some((parent, context)) = parent {
        value["parent_task"] = json!({"parent_id":parent,"child_id":x.task_id,"context_sha256":context.context_sha256});
        let until = crate::question_pool::check_parent(
            tx,
            root,
            fence,
            op,
            actor,
            &x.workspace_id,
            &value,
            now,
        )?;
        value["expires_at_ms"] = value["expires_at_ms"]
            .as_i64()
            .ok_or(Error::IntegrityFailed)?
            .min(until)
            .into();
    }
    publication::insert(tx, root, &x.workspace_id, &x.task_id, &value, now)?;
    read(tx, root, fence, op, actor, &x.workspace_id, &x.task_id, now)
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
    let (v, sha) = task(conn, root, actor, ws, id)?;
    let mut result = FoundationResult {
        resource_id: id.into(),
        revision: 1,
        sha256: sha,
        task_state: AgentTaskState::Completed as i32,
        ..Default::default()
    };
    if let Some(t) = terminal(conn, root, ws, id)? {
        if t["task"]["sha256"] != result.sha256 || t["created_by"] != actor {
            return Err(Error::IntegrityFailed);
        }
        result.faq_reason = FaqReason::Resolved as i32;
        let decision = t["decision"].as_str().ok_or(Error::IntegrityFailed)?;
        result.suggestion_state = match decision {
            "accept" => SuggestionState::Accepted,
            "edit" => SuggestionState::Edited,
            "ignore" | "cancel" => SuggestionState::Rejected,
            _ => return Err(Error::IntegrityFailed),
        } as i32;
        if decision == "cancel" {
            result.faq_reason = FaqReason::Cancelled as i32;
        }
        result.adopted_object_id = t["adopted_object_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?
            .into();
        result.adopted_revision = t["adopted_revision"]
            .as_u64()
            .ok_or(Error::IntegrityFailed)?;
        return Ok(result);
    }
    match context(conn, root, fence, op, actor, ws, &v, now) {
        Ok(_) => (),
        Err(e) => {
            result.suggestion_state = SuggestionState::Expired as i32;
            result.faq_reason = match e {
                Error::DeadlineExceeded => FaqReason::Expired,
                Error::RevisionConflict => FaqReason::StaleContext,
                Error::PermissionDenied
                | Error::CapabilityUnavailable
                | Error::NotFound
                | Error::Cancelled => FaqReason::Revoked,
                _ => return Err(e),
            } as i32;
            return Ok(result);
        }
    }
    let reason = match v["result"].as_str() {
        Some("exact") => FaqReason::Exact,
        Some("retrieved") => FaqReason::Retrieved,
        Some("no_match") => FaqReason::NoMatch,
        Some("ambiguous") => FaqReason::Ambiguous,
        _ => return Err(Error::IntegrityFailed),
    };
    result.faq_reason = reason as i32;
    if matches!(reason, FaqReason::Exact | FaqReason::Retrieved) {
        result.suggestion_state = SuggestionState::Proposed as i32;
    }
    result.body = v.to_string();
    Ok(result)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    now: i64,
    x: &FoundationFaqResolve,
) -> Result<FoundationResult> {
    if matches!(
        FaqDecision::try_from(x.decision),
        Ok(FaqDecision::Accept | FaqDecision::Edit)
    ) {
        let (v, _, _) = llm::raw(
            tx,
            root,
            &x.workspace_id,
            &x.task_id,
            0,
            "publication_faq_task",
        )?;
        if let Some(parent) = v["parent_task"]["parent_id"].as_str() {
            review_questions::check_dispatch(
                tx,
                root,
                fence,
                op,
                actor,
                &x.workspace_id,
                parent,
                now,
            )?;
        }
    }

    let (v, sha) = task(tx, root, actor, &x.workspace_id, &x.task_id)?;
    if sha != x.task_sha256 {
        return Err(Error::RevisionConflict);
    }
    if terminal(tx, root, &x.workspace_id, &x.task_id)?.is_some() {
        return Err(Error::RevisionConflict);
    }
    let decision = FaqDecision::try_from(x.decision).map_err(|_| Error::InvalidArgument)?;
    let name = match decision {
        FaqDecision::Accept => "accept",
        FaqDecision::Edit => "edit",
        FaqDecision::Ignore => "ignore",
        FaqDecision::Cancel => "cancel",
        _ => return Err(Error::InvalidArgument),
    };
    if decision != FaqDecision::Edit && (!x.edited_text.is_empty() || x.acknowledge_edit) {
        return Err(Error::InvalidArgument);
    }
    let (mut adopted_id, mut adopted_revision) = (String::new(), 0);
    if matches!(decision, FaqDecision::Accept | FaqDecision::Edit) {
        let docs = context(tx, root, fence, op, actor, &x.workspace_id, &v, now)?;
        if v["result"] != "exact" && v["result"] != "retrieved" {
            return Err(Error::CapabilityUnavailable);
        }
        let text = if decision == FaqDecision::Edit {
            if !x.acknowledge_edit || x.edited_text.chars().count() > 2048 {
                return Err(Error::InvalidArgument);
            }
            manual_guard(&x.edited_text)?;
            x.edited_text.as_str()
        } else {
            v["text"].as_str().ok_or(Error::IntegrityFailed)?
        };
        let mut product = docs[1].clone();
        let tracks = product["talk_tracks"]
            .as_array_mut()
            .ok_or(Error::IntegrityFailed)?;
        if tracks.len() >= 32 {
            return Err(Error::ResourceExhausted);
        }
        let track_id = format!("faq_{}", x.task_id.replace('-', ""));
        if tracks.iter().any(|t| t["id"] == track_id) {
            return Err(Error::RevisionConflict);
        }
        tracks.push(json!({"id":track_id,"text":text,"fact_ids":v["fact_ids"]}));
        let bytes = product.to_string();
        nai_runtime_core::preparation::validate(op, &bytes)?;
        adopted_id = v["refs"]["product"]["object_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?
            .into();
        let prior = v["refs"]["product"]["revision"]
            .as_u64()
            .ok_or(Error::IntegrityFailed)?;
        if prior >= 1024 {
            return Err(Error::ResourceExhausted);
        }
        adopted_revision = prior + 1;
        let sha = store_body(root, bytes.as_bytes())?;
        let count=tx.execute("UPDATE objects SET revision=revision+1,updated_at=?1 WHERE workspace_id=?2 AND id=?3 AND revision=?4 AND deleted=0",params![now,x.workspace_id,adopted_id,int(prior)?]).wb()?;
        if count != 1 {
            return Err(Error::RevisionConflict);
        }
        tx.execute(
            "INSERT INTO object_versions VALUES(?1,?2,?3,?4,?5,?5)",
            params![x.workspace_id, adopted_id, int(adopted_revision)?, sha, now],
        )
        .wb()?;
    }
    let value = json!({"schema_version":1,"kind":"publication_faq_resolution","created_at_ms":now,"created_by":actor,"task":{"object_id":x.task_id,"revision":1,"sha256":sha},"decision":name,"adopted_object_id":adopted_id,"adopted_revision":adopted_revision});
    publication::insert(
        tx,
        root,
        &x.workspace_id,
        &terminal_id(&x.task_id),
        &value,
        now,
    )?;
    read(tx, root, fence, op, actor, &x.workspace_id, &x.task_id, now)
}
/// Recheck before commit without requiring the pre-adoption product head to stay unchanged.
#[allow(clippy::too_many_arguments)]
pub(super) fn commit_gate(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    id: &str,
    now: i64,
) -> Result<()> {
    fence.check(fence.generation())?;
    let (v, _) = task(conn, root, actor, ws, id)?;
    if v.get("parent_task").is_some() {
        crate::question_pool::check_parent_after_adoption(
            conn, root, fence, op, actor, ws, &v, now,
        )?;
    }
    let (_, rev) = role(conn, ws, actor)?;
    if v["role_revision"].as_u64() != Some(rev) {
        return Err(Error::PermissionDenied);
    }
    fence.check_scope(&role_scope(ws, actor, rev))?;
    if now >= v["expires_at_ms"].as_i64().ok_or(Error::IntegrityFailed)? {
        return Err(Error::DeadlineExceeded);
    }
    let snap = publication::read(
        conn,
        root,
        fence,
        op,
        ws,
        v["snapshot"]["object_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?,
        now,
    )?;
    if snap.publication_status != PublicationStatus::PrivateContent as i32 {
        return Err(Error::CapabilityUnavailable);
    }
    op.check()
}
