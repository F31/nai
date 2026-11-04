//! Persistent private question parents and grounded FAQ children, under the existing owner.
use super::*;
use nai_runtime_core::task_context::{POOL_CAPACITY, POOL_TTL_MS};
use serde_json::{Value, json};

fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key].as_str().ok_or(Error::IntegrityFailed)
}
fn stored_context(v: &Value) -> Result<AgentTaskContext> {
    nai_runtime_core::task_context::from_value(&v["context"])
}
#[allow(clippy::too_many_arguments)]
fn source(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    x: &QuestionPoolRead,
    now: i64,
) -> Result<(AgentTaskContext, i64, u64)> {
    source_checked(conn, root, fence, op, actor, x, now, true)
}
#[allow(clippy::too_many_arguments)]
fn source_checked(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    x: &QuestionPoolRead,
    now: i64,
    latest: bool,
) -> Result<(AgentTaskContext, i64, u64)> {
    op.check()?;
    fence.check(fence.generation())?;
    let (_, revision) = role(conn, &x.workspace_id, actor)?;
    fence.check_scope(&role_scope(&x.workspace_id, actor, revision))?;
    let snapshot = publication::read(conn, root, fence, op, &x.workspace_id, &x.snapshot_id, now)?;
    if snapshot.sha256 != x.snapshot_sha256 {
        return Err(Error::RevisionConflict);
    }
    if snapshot.publication_status != PublicationStatus::PrivateContent as i32 {
        return Err(Error::CapabilityUnavailable);
    }
    let manifest: Value =
        serde_json::from_str(&snapshot.body).map_err(|_| Error::IntegrityFailed)?;
    let docs = publication::documents(
        conn,
        root,
        fence,
        &x.workspace_id,
        &manifest["refs"],
        latest,
    )?;
    let closure =
        nai_runtime_core::publication::closure(op, [&docs[0], &docs[1], &docs[2], &docs[3]], now)?;
    let context = nai_runtime_core::task_context::source(
        &x.workspace_id,
        &x.snapshot_id,
        &x.snapshot_sha256,
        &hash(manifest["refs"].to_string().as_bytes()),
    )?;
    Ok((
        context,
        closure
            .valid_until_ms
            .unwrap_or(now + POOL_TTL_MS)
            .min(now + POOL_TTL_MS),
        revision,
    ))
}
fn scope(c: &AgentTaskContext) -> QuestionPoolRead {
    QuestionPoolRead {
        workspace_id: c.workspace_id.clone(),
        snapshot_id: c.snapshot_id.clone(),
        snapshot_sha256: c.snapshot_sha256.clone(),
    }
}
fn load(
    conn: &Connection,
    root: &Path,
    ws: &str,
    id: &str,
    actor: &str,
) -> Result<(Value, u64, String)> {
    uuid(id)?;
    let (revision, sha) = object(conn, ws, id, 0)?;
    let v = nai_runtime_core::publication::artifact(&body(root, &sha)?)?;
    if v["kind"] != "publication_question_task" {
        return Err(Error::InvalidArgument);
    }
    if v["created_by"] != actor {
        return Err(Error::PermissionDenied);
    }
    let c = stored_context(&v)?;
    if c.workspace_id != ws {
        return Err(Error::IntegrityFailed);
    }
    Ok((v, revision, sha))
}
fn update(
    tx: &Transaction<'_>,
    root: &Path,
    ws: &str,
    id: &str,
    revision: u64,
    v: &Value,
    now: i64,
) -> Result<u64> {
    if revision >= 1024 {
        return Err(Error::ResourceExhausted);
    }
    let bytes = v.to_string();
    nai_runtime_core::publication::artifact(&bytes)?;
    if bytes.len() > 8192 {
        return Err(Error::ResourceExhausted);
    }
    let sha = store_body(root, bytes.as_bytes())?;
    let n=tx.execute("UPDATE objects SET revision=revision+1,updated_at=?1 WHERE workspace_id=?2 AND id=?3 AND revision=?4 AND deleted=0",params![now,ws,id,int(revision)?]).wb()?;
    if n != 1 {
        return Err(Error::RevisionConflict);
    }
    let next = revision + 1;
    tx.execute(
        "INSERT INTO object_versions VALUES(?1,?2,?3,?4,?5,?5)",
        params![ws, id, int(next)?, sha, now],
    )
    .wb()?;
    Ok(next)
}
fn action(c: &AgentTaskContext) -> String {
    format!("question_enqueue:{}", c.context_sha256)
}
fn index_id(c: &AgentTaskContext, actor: &str, bucket: i64) -> String {
    let h = hash(
        format!(
            "nai-question-index-v1:{}:{actor}:{bucket}",
            c.context_sha256
        )
        .as_bytes(),
    );
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}
fn index(
    conn: &Connection,
    root: &Path,
    ws: &str,
    actor: &str,
    c: &AgentTaskContext,
    bucket: i64,
) -> Result<Option<(Value, u64)>> {
    let id = index_id(c, actor, bucket);
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
    let (revision, sha) = object(conn, ws, &id, 0)?;
    let v = nai_runtime_core::publication::artifact(&body(root, &sha)?)?;
    if v["kind"] != "publication_question_index"
        || v["created_by"] != actor
        || v["context_sha256"] != c.context_sha256
        || v["bucket"] != bucket
    {
        return Err(Error::IntegrityFailed);
    }
    Ok(Some((v, revision)))
}
fn recent(
    conn: &Connection,
    root: &Path,
    ws: &str,
    actor: &str,
    c: &AgentTaskContext,
    now: i64,
) -> Result<Vec<String>> {
    let bucket = now / POOL_TTL_MS;
    let mut ids = Vec::new();
    for b in (bucket.saturating_sub(1)..=bucket).rev() {
        if let Some((v, _)) = index(conn, root, ws, actor, c, b)? {
            for value in v["entries"]
                .as_array()
                .ok_or(Error::IntegrityFailed)?
                .iter()
                .rev()
            {
                let id = value.as_str().ok_or(Error::IntegrityFailed)?;
                let (question, _, _) = load(conn, root, ws, id, actor)?;
                if question["created_at_ms"]
                    .as_i64()
                    .ok_or(Error::IntegrityFailed)?
                    > now - POOL_TTL_MS
                {
                    ids.push(id.to_owned());
                }
            }
        }
    }
    if ids.len() > POOL_CAPACITY {
        return Err(Error::ResourceExhausted);
    }
    Ok(ids)
}
fn append_index(
    tx: &Transaction<'_>,
    root: &Path,
    ws: &str,
    actor: &str,
    c: &AgentTaskContext,
    id: &str,
    now: i64,
) -> Result<()> {
    let bucket = now / POOL_TTL_MS;
    let index_id = index_id(c, actor, bucket);
    if let Some((mut v, revision)) = index(tx, root, ws, actor, c, bucket)? {
        let entries = v["entries"].as_array_mut().ok_or(Error::IntegrityFailed)?;
        if entries.len() >= POOL_CAPACITY {
            return Err(Error::ResourceExhausted);
        }
        entries.push(id.into());
        update(tx, root, ws, &index_id, revision, &v, now)?;
    } else {
        publication::insert(
            tx,
            root,
            ws,
            &index_id,
            &json!({"schema_version":1,"kind":"publication_question_index","created_by":actor,"context_sha256":c.context_sha256,"bucket":bucket,"entries":[id]}),
            now,
        )?;
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn view(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    id: &str,
    v: &Value,
    revision: u64,
    now: i64,
) -> Result<QuestionPoolEntry> {
    op.check()?;
    let mut e = QuestionPoolEntry {
        id: id.into(),
        revision,
        question: text(v, "question")?.into(),
        created_at_ms: v["created_at_ms"].as_u64().ok_or(Error::IntegrityFailed)?,
        expires_at_ms: v["expires_at_ms"].as_u64().ok_or(Error::IntegrityFailed)?,
        occurrences: v["occurrences"].as_u64().ok_or(Error::IntegrityFailed)? as u32,
        task_id: text(v, "child_task_id")?.into(),
        task_sha256: text(v, "child_task_sha256")?.into(),
        context_sha256: stored_context(v)?.context_sha256,
        ..Default::default()
    };
    let (_, current_role) = role(conn, ws, actor)?;
    if v["state"] == "cancelled" {
        e.state = AgentTaskState::Cancelled as i32;
        e.reason = "QUESTION_CANCELLED".into();
    } else if now < v["created_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?
        || now >= v["expires_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?
        || v["role_revision"].as_u64() != Some(current_role)
    {
        e.state = AgentTaskState::Expired as i32;
        e.reason = "QUESTION_EXPIRED_OR_ROLE_CHANGED".into();
    } else if e.task_id.is_empty() {
        e.state = AgentTaskState::Queued as i32;
        e.reason = "QUESTION_QUEUED".into();
    } else {
        let child = assistance::read(conn, root, fence, op, actor, ws, &e.task_id, now)?;
        if child.sha256 != e.task_sha256 {
            return Err(Error::IntegrityFailed);
        }
        e.suggestion_state = child.suggestion_state;
        if child.suggestion_state == SuggestionState::Proposed as i32 {
            e.state = AgentTaskState::WaitingHuman as i32;
            e.reason = "CANDIDATE_WAITING_HUMAN".into();
        } else if matches!(
            SuggestionState::try_from(child.suggestion_state),
            Ok(SuggestionState::Accepted | SuggestionState::Edited)
        ) {
            e.state = AgentTaskState::Completed as i32;
            e.reason = "ADOPTED_TO_DRAFT_ONLY".into();
        } else if child.suggestion_state == SuggestionState::Expired as i32 {
            e.state = AgentTaskState::Expired as i32;
            e.reason = "CHILD_CONTEXT_EXPIRED".into();
        } else {
            e.state = AgentTaskState::Completed as i32;
            e.reason = if child.faq_reason == FaqReason::Resolved as i32 {
                "CANDIDATE_IGNORED"
            } else {
                "NO_ADOPTABLE_CANDIDATE"
            }
            .into();
        }
    }
    Ok(e)
}
fn result(context: AgentTaskContext, entries: Vec<QuestionPoolEntry>) -> FoundationResult {
    FoundationResult {
        resource_id: entries.first().map(|e| e.id.clone()).unwrap_or_default(),
        revision: entries.first().map_or(0, |e| e.revision),
        question_pool: Some(QuestionPoolView {
            context: Some(context),
            entries,
            capacity: POOL_CAPACITY as u32,
            ttl_ms: POOL_TTL_MS as u32,
        }),
        ..Default::default()
    }
}
#[allow(clippy::too_many_arguments)]
fn read_scoped(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    x: &QuestionPoolRead,
    now: i64,
    run: Option<&ReviewSource>,
) -> Result<FoundationResult> {
    let (c, _, _) = source(conn, root, fence, op, actor, x, now)?;
    let mut entries = Vec::new();
    for id in recent(conn, root, &x.workspace_id, actor, &c, now)? {
        if !review_questions::belongs(conn, root, &x.workspace_id, actor, &id, run)? {
            continue;
        }
        let (v, revision, _) = load(conn, root, &x.workspace_id, &id, actor)?;
        if stored_context(&v)? != c {
            return Err(Error::IntegrityFailed);
        }
        entries.push(view(
            conn,
            root,
            fence,
            op,
            actor,
            &x.workspace_id,
            &id,
            &v,
            revision,
            now,
        )?);
    }
    Ok(result(c, entries))
}
#[allow(clippy::too_many_arguments)]
fn enqueue_scoped(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    x: &QuestionPoolEnqueue,
    now: i64,
    run: Option<&ReviewSource>,
) -> Result<FoundationResult> {
    uuid(&x.question_id)?;
    let xscope = x.scope.as_ref().ok_or(Error::InvalidArgument)?;
    let (c, expires, role_rev) = source(tx, root, fence, op, actor, xscope, now)?;
    let key = nai_runtime_core::task_context::question_key(&x.question)?;
    let ids = recent(tx, root, &xscope.workspace_id, actor, &c, now)?;
    for id in &ids {
        if !review_questions::belongs(tx, root, &xscope.workspace_id, actor, id, run)? {
            continue;
        }
        let (mut v, revision, _) = load(tx, root, &xscope.workspace_id, id, actor)?;
        if v["dedup_key"] == key
            && v["expires_at_ms"].as_i64().is_some_and(|until| now < until)
            && (v["state"] == "queued" || v["state"] == "waiting_human")
        {
            let entry = view(
                tx,
                root,
                fence,
                op,
                actor,
                &xscope.workspace_id,
                id,
                &v,
                revision,
                now,
            )?;
            if !matches!(
                AgentTaskState::try_from(entry.state),
                Ok(AgentTaskState::Queued | AgentTaskState::WaitingHuman)
            ) {
                continue;
            }
            let n = v["occurrences"].as_u64().ok_or(Error::IntegrityFailed)?;
            if n >= 99 {
                return Err(Error::ResourceExhausted);
            }
            v["occurrences"] = (n + 1).into();
            v["updated_at_ms"] = now.into();
            let next = update(tx, root, &xscope.workspace_id, id, revision, &v, now)?;
            return Ok(result(
                c,
                vec![view(
                    tx,
                    root,
                    fence,
                    op,
                    actor,
                    &xscope.workspace_id,
                    id,
                    &v,
                    next,
                    now,
                )?],
            ));
        }
    }
    if ids.len() >= POOL_CAPACITY {
        return Err(Error::ResourceExhausted);
    }
    let v = json!({"schema_version":1,"kind":"publication_question_task","created_by":actor,"role_revision":role_rev,"created_at_ms":now,"updated_at_ms":now,"expires_at_ms":expires,"context":nai_runtime_core::task_context::to_value(&c),"question":x.question.trim(),"dedup_key":key,"state":"queued","occurrences":1,"child_task_id":"","child_task_sha256":""});
    publication::insert(tx, root, &xscope.workspace_id, &x.question_id, &v, now)?;
    append_index(
        tx,
        root,
        &xscope.workspace_id,
        actor,
        &c,
        &x.question_id,
        now,
    )?;
    audit(
        tx,
        &xscope.workspace_id,
        actor,
        &action(&c),
        &x.question_id,
        now,
    )?;
    Ok(result(
        c,
        vec![view(
            tx,
            root,
            fence,
            op,
            actor,
            &xscope.workspace_id,
            &x.question_id,
            &v,
            1,
            now,
        )?],
    ))
}
#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    x: &QuestionPoolRun,
    now: i64,
) -> Result<FoundationResult> {
    uuid(&x.task_id)?;
    review_questions::check_dispatch(
        tx,
        root,
        fence,
        op,
        actor,
        &x.workspace_id,
        &x.question_id,
        now,
    )?;
    let (mut v, revision, _) = load(tx, root, &x.workspace_id, &x.question_id, actor)?;
    if revision != x.expected_revision {
        return Err(Error::RevisionConflict);
    }
    let c = stored_context(&v)?;
    let (actual, _, role_rev) = source(tx, root, fence, op, actor, &scope(&c), now)?;
    if actual != c || v["role_revision"].as_u64() != Some(role_rev) {
        return Err(Error::RevisionConflict);
    }
    if now < v["created_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?
        || now >= v["expires_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?
    {
        return Err(Error::DeadlineExceeded);
    }
    if v["state"] != "queued" {
        return Err(Error::RevisionConflict);
    }
    let remaining =
        (v["expires_at_ms"].as_i64().ok_or(Error::IntegrityFailed)? - now).min(4000) as u32;
    let child_op = op.child(remaining)?;
    v["state"] = "running".into();
    v["child_task_id"] = x.task_id.clone().into();
    v["updated_at_ms"] = now.into();
    let running = update(tx, root, &x.workspace_id, &x.question_id, revision, &v, now)?;
    let child = assistance::ask_for_pool(
        tx,
        root,
        fence,
        &child_op,
        actor,
        role_rev,
        now,
        &FoundationFaqRequest {
            workspace_id: x.workspace_id.clone(),
            task_id: x.task_id.clone(),
            snapshot_id: c.snapshot_id.clone(),
            snapshot_sha256: c.snapshot_sha256.clone(),
            question: text(&v, "question")?.into(),
        },
        &x.question_id,
        &c,
    )?;
    child_op.check()?;
    v["state"] = "waiting_human".into();
    v["child_task_sha256"] = child.sha256.into();
    let ready = update(tx, root, &x.workspace_id, &x.question_id, running, &v, now)?;
    audit(tx, &x.workspace_id, actor, "faq_lookup", &x.task_id, now)?;
    assistance::commit_gate(
        tx,
        root,
        fence,
        &child_op,
        actor,
        &x.workspace_id,
        &x.task_id,
        utc()?,
    )?;
    Ok(result(
        c,
        vec![view(
            tx,
            root,
            fence,
            op,
            actor,
            &x.workspace_id,
            &x.question_id,
            &v,
            ready,
            now,
        )?],
    ))
}
#[allow(clippy::too_many_arguments)]
pub(super) fn cancel(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    x: &QuestionPoolCancel,
    now: i64,
) -> Result<FoundationResult> {
    let (mut v, revision, _) = load(tx, root, &x.workspace_id, &x.question_id, actor)?;
    if revision != x.expected_revision {
        return Err(Error::RevisionConflict);
    }
    let c = stored_context(&v)?;
    if v["state"] == "cancelled" {
        return Ok(result(
            c,
            vec![view(
                tx,
                root,
                fence,
                op,
                actor,
                &x.workspace_id,
                &x.question_id,
                &v,
                revision,
                now,
            )?],
        ));
    }
    if !text(&v, "child_task_id")?.is_empty() {
        let child = assistance::read(
            tx,
            root,
            fence,
            op,
            actor,
            &x.workspace_id,
            text(&v, "child_task_id")?,
            now,
        )?;
        if !matches!(
            FaqReason::try_from(child.faq_reason),
            Ok(FaqReason::Resolved | FaqReason::Cancelled)
        ) {
            assistance::resolve(
                tx,
                root,
                fence,
                op,
                actor,
                now,
                &FoundationFaqResolve {
                    workspace_id: x.workspace_id.clone(),
                    task_id: text(&v, "child_task_id")?.into(),
                    task_sha256: text(&v, "child_task_sha256")?.into(),
                    decision: FaqDecision::Cancel as i32,
                    edited_text: String::new(),
                    acknowledge_edit: false,
                },
            )?;
        }
    }
    v["state"] = "cancelled".into();
    v["updated_at_ms"] = now.into();
    let next = update(tx, root, &x.workspace_id, &x.question_id, revision, &v, now)?;
    Ok(result(
        c,
        vec![view(
            tx,
            root,
            fence,
            op,
            actor,
            &x.workspace_id,
            &x.question_id,
            &v,
            next,
            now,
        )?],
    ))
}
#[allow(clippy::too_many_arguments)]
pub(super) fn check_parent(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    task: &Value,
    now: i64,
) -> Result<i64> {
    check_parent_checked(conn, root, fence, op, actor, ws, task, now, true)
}
// Adoption has already changed the product head in this transaction. Keep checking
// the pinned source, parent, role and expiry without rejecting that own write.
#[allow(clippy::too_many_arguments)]
pub(super) fn check_parent_after_adoption(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    task: &Value,
    now: i64,
) -> Result<i64> {
    check_parent_checked(conn, root, fence, op, actor, ws, task, now, false)
}
#[allow(clippy::too_many_arguments)]
fn check_parent_checked(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    task: &Value,
    now: i64,
    latest: bool,
) -> Result<i64> {
    let link = &task["parent_task"];
    super::review_questions::check_dispatch(
        conn,
        root,
        fence,
        op,
        actor,
        ws,
        text(link, "parent_id")?,
        now,
    )?;
    let (v, _, _) = load(conn, root, ws, text(link, "parent_id")?, actor)?;
    if v["state"] != "running" && v["state"] != "waiting_human" {
        return Err(Error::Cancelled);
    }
    if v["child_task_id"] != link["child_id"] {
        return Err(Error::IntegrityFailed);
    }
    let until = v["expires_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?;
    if now < v["created_at_ms"].as_i64().ok_or(Error::IntegrityFailed)? || now >= until {
        return Err(Error::DeadlineExceeded);
    }
    let c = stored_context(&v)?;
    let (actual, _, role_rev) =
        source_checked(conn, root, fence, op, actor, &scope(&c), now, latest)?;
    if actual != c
        || link["context_sha256"] != c.context_sha256
        || v["role_revision"].as_u64() != Some(role_rev)
        || task["snapshot"]["object_id"] != c.snapshot_id
        || task["snapshot"]["sha256"] != c.snapshot_sha256
        || hash(task["refs"].to_string().as_bytes()) != c.facts_sha256
    {
        return Err(Error::RevisionConflict);
    }
    Ok(until)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn refresh(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    id: &str,
    now: i64,
) -> Result<FoundationResult> {
    let (v, rev, _) = load(conn, root, ws, id, actor)?;
    let c = stored_context(&v)?;
    source(conn, root, fence, op, actor, &scope(&c), now)?;
    review_questions::check_dispatch(conn, root, fence, op, actor, ws, id, now)?;
    Ok(result(
        c,
        vec![view(conn, root, fence, op, actor, ws, id, &v, rev, now)?],
    ))
}
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
    let (v, _, _) = load(conn, root, ws, id, actor)?;
    if v["state"] == "cancelled" {
        return op.check();
    }
    review_questions::check_dispatch(conn, root, fence, op, actor, ws, id, now)?;
    if now < v["created_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?
        || now >= v["expires_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?
    {
        return Err(Error::DeadlineExceeded);
    }
    let c = stored_context(&v)?;
    let (actual, _, revision) = source(conn, root, fence, op, actor, &scope(&c), now)?;
    if c != actual || v["role_revision"].as_u64() != Some(revision) {
        return Err(Error::RevisionConflict);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn read(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    x: &QuestionPoolRead,
    now: i64,
) -> Result<FoundationResult> {
    read_scoped(conn, root, fence, op, actor, x, now, None)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn read_run(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    x: &RunQuestionPoolRead,
    now: i64,
) -> Result<FoundationResult> {
    let source = x.source.as_ref().ok_or(Error::InvalidArgument)?;
    let scope = x.scope.as_ref().ok_or(Error::InvalidArgument)?;
    review_questions::active(
        conn,
        root,
        fence,
        op,
        actor,
        source,
        scope,
        x.expected_epoch,
        now,
    )?;
    read_scoped(conn, root, fence, op, actor, scope, now, Some(source))
}
#[allow(clippy::too_many_arguments)]
pub(super) fn enqueue(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    x: &QuestionPoolEnqueue,
    now: i64,
) -> Result<FoundationResult> {
    enqueue_scoped(tx, root, fence, op, actor, x, now, None)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn enqueue_run(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    x: &ReviewRunQuestion,
    now: i64,
) -> Result<FoundationResult> {
    let source = x.source.as_ref().ok_or(Error::InvalidArgument)?;
    let question = x.question.as_ref().ok_or(Error::InvalidArgument)?;
    let proof = review_questions::active(
        tx,
        root,
        fence,
        op,
        actor,
        source,
        question.scope.as_ref().ok_or(Error::InvalidArgument)?,
        x.expected_epoch,
        now,
    )?;
    let result = enqueue_scoped(tx, root, fence, op, actor, question, now, Some(source))?;
    let entry = result
        .question_pool
        .as_ref()
        .and_then(|p| p.entries.first())
        .ok_or(Error::IntegrityFailed)?;
    review_questions::remember(
        tx,
        root,
        actor,
        source,
        &proof,
        x.expected_epoch,
        entry,
        now,
    )?;
    Ok(result)
}
