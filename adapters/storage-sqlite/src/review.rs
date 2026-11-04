//! Review persistence/authority adapter; pure draft transformation lives in nai-live.
use super::*;
use nai_live::review as policy;
use serde_json::{Value, json};
struct Source {
    proof: Value,
    name: String,
    product: Value,
    show: Value,
    published: Value,
    product_ref: ReviewObjectRef,
    show_ref: ReviewObjectRef,
    facts_unchanged: bool,
    session_epoch: u64,
}
fn pointer(r: &ReviewObjectRef) -> Value {
    json!({"object_id":r.object_id,"revision":r.revision,"sha256":r.sha256})
}
fn ref_of(v: &Value) -> Result<ReviewObjectRef> {
    Ok(ReviewObjectRef {
        object_id: v["object_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?
            .into(),
        revision: v["revision"].as_u64().ok_or(Error::IntegrityFailed)?,
        sha256: v["sha256"].as_str().ok_or(Error::IntegrityFailed)?.into(),
    })
}
fn kind(s: &ReviewSource) -> Result<&'static str> {
    match ReviewRunKind::try_from(s.run_kind) {
        Ok(ReviewRunKind::Manual) => Ok("manual"),
        Ok(ReviewRunKind::Live) => Ok("live"),
        _ => Err(Error::InvalidArgument),
    }
}
fn source_from(ws: &str, v: &Value) -> Result<ReviewSource> {
    Ok(ReviewSource {
        workspace_id: ws.into(),
        run_id: v["run_id"].as_str().ok_or(Error::IntegrityFailed)?.into(),
        run_kind: match v["run_kind"].as_str() {
            Some("manual") => ReviewRunKind::Manual as i32,
            Some("live") => ReviewRunKind::Live as i32,
            _ => return Err(Error::IntegrityFailed),
        },
    })
}
fn latest(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    ws: &str,
    id: &str,
) -> Result<(Value, ReviewObjectRef)> {
    uuid(id)?;
    fence.check_scope(&object_scope(ws, id))?;
    let (rev, sha) = object(conn, ws, id, 0)?;
    fence.check_scope(&publication::version_scope(ws, id, rev))?;
    Ok((
        serde_json::from_str(&body(root, &sha)?).map_err(|_| Error::IntegrityFailed)?,
        ReviewObjectRef {
            object_id: id.into(),
            revision: rev,
            sha256: sha,
        },
    ))
}
#[allow(clippy::too_many_arguments)]
fn source_checked(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    s: &ReviewSource,
    now: i64,
    ended: bool,
) -> Result<Source> {
    op.check()?;
    uuid(&s.run_id)?;
    let run_kind = kind(s)?;
    let (v, run_ref) = latest(conn, root, fence, &s.workspace_id, &s.run_id)?;
    let (snapshot_id, snapshot_sha) = if run_kind == "manual" {
        if v["kind"] != "manual_run_draft" || v["status"] != if ended { "ended" } else { "open" } {
            return Err(Error::CapabilityUnavailable);
        }
        (
            v["snapshot"]["object_id"].as_str(),
            v["snapshot"]["sha256"].as_str(),
        )
    } else {
        if v["kind"] != "publication_live_run" || v["created_by"] != actor {
            return Err(Error::PermissionDenied);
        }
        if matches!(v["state"].as_i64(), Some(5 | 7)) != ended {
            return Err(Error::CapabilityUnavailable);
        }
        (
            v["context"]["snapshot_id"].as_str(),
            v["context"]["snapshot_sha256"].as_str(),
        )
    };
    let snapshot_id = snapshot_id.ok_or(Error::IntegrityFailed)?;
    let snapshot_sha = snapshot_sha.ok_or(Error::IntegrityFailed)?;
    let r = publication::read(conn, root, fence, op, &s.workspace_id, snapshot_id, now)?;
    if r.sha256 != snapshot_sha {
        return Err(Error::RevisionConflict);
    }
    if r.publication_status != PublicationStatus::PrivateContent as i32 {
        return Err(Error::CapabilityUnavailable);
    }
    let manifest: Value = serde_json::from_str(&r.body).map_err(|_| Error::IntegrityFailed)?;
    let docs =
        publication::documents(conn, root, fence, &s.workspace_id, &manifest["refs"], false)?;
    let (p, pr) = latest(
        conn,
        root,
        fence,
        &s.workspace_id,
        manifest["refs"]["product"]["object_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?,
    )?;
    let (sh, sr) = latest(
        conn,
        root,
        fence,
        &s.workspace_id,
        manifest["refs"]["show"]["object_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?,
    )?;
    let facts_unchanged = p["facts"] == docs[1]["facts"]
        && p["prohibitions"] == docs[1]["prohibitions"]
        && sh["product"]["object_id"] == pr.object_id;
    Ok(Source {
        proof: json!({"run_id":s.run_id,"run_kind":run_kind,"run_revision":run_ref.revision,"snapshot_id":snapshot_id,"snapshot_sha256":snapshot_sha}),
        name: v["name"].as_str().ok_or(Error::IntegrityFailed)?.into(),
        product: p,
        show: sh,
        published: docs[1].clone(),
        product_ref: pr,
        show_ref: sr,
        facts_unchanged,
        session_epoch: if run_kind == "live" {
            v["session_epoch"].as_u64().ok_or(Error::IntegrityFailed)?
        } else {
            0
        },
    })
}
#[allow(clippy::too_many_arguments)]
fn source(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    s: &ReviewSource,
    now: i64,
) -> Result<Source> {
    source_checked(conn, root, fence, op, actor, s, now, true)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn active_question_source(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    s: &ReviewSource,
    epoch: u64,
    now: i64,
) -> Result<Value> {
    let c = source_checked(conn, root, fence, op, actor, s, now, false)?;
    if c.session_epoch != epoch || !c.facts_unchanged {
        return Err(Error::RevisionConflict);
    }
    if s.run_kind == ReviewRunKind::Live as i32 {
        let (actual, stopped) = live::session(conn, &s.workspace_id, &s.run_id, actor)?;
        if stopped || actual != epoch {
            return Err(Error::Cancelled);
        }
    }
    Ok(c.proof)
}
fn index_id(s: &ReviewSource, actor: &str) -> String {
    let h = hash(
        format!(
            "review:{}:{}:{}:{actor}",
            s.workspace_id, s.run_id, s.run_kind
        )
        .as_bytes(),
    );
    format!(
        "{}-{}-4{}-a{}-{}",
        &h[..8],
        &h[8..12],
        &h[13..16],
        &h[17..20],
        &h[20..32]
    )
}
fn index(
    conn: &Connection,
    root: &Path,
    s: &ReviewSource,
    actor: &str,
    now: i64,
) -> Result<(String, Value, u64)> {
    let id = index_id(s, actor);
    match llm::raw(
        conn,
        root,
        &s.workspace_id,
        &id,
        0,
        "publication_review_index",
    ) {
        Ok((v, r, _)) => {
            if v["created_by"] != actor
                || v["source_run_id"] != s.run_id
                || v["source_run_kind"] != kind(s)?
            {
                return Err(Error::PermissionDenied);
            }
            Ok((id, v, r))
        }
        Err(Error::NotFound) => Ok((
            id,
            json!({"schema_version":1,"kind":"publication_review_index","created_by":actor,"created_at_ms":now,"updated_at_ms":now,"source_run_id":s.run_id,"source_run_kind":kind(s)?,"notes":[],"jobs":[]}),
            0,
        )),
        Err(e) => Err(e),
    }
}
fn owned(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    ws: &str,
    id: &str,
    actor: &str,
    kind: &str,
) -> Result<(Value, u64, String)> {
    uuid(id)?;
    fence.check_scope(&object_scope(ws, id))?;
    let (v, r, sha) = llm::raw(conn, root, ws, id, 0, kind)?;
    if v["created_by"] != actor {
        return Err(Error::PermissionDenied);
    }
    Ok((v, r, sha))
}
#[allow(clippy::too_many_arguments)]
pub(super) fn read(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    s: &ReviewSource,
    now: i64,
) -> Result<FoundationResult> {
    let c = source(conn, root, fence, op, actor, s, now)?;
    let (_, idx, _) = index(conn, root, s, actor, now)?;
    let mut notes = vec![];
    for id in idx["notes"].as_array().ok_or(Error::IntegrityFailed)? {
        op.check()?;
        let id = id.as_str().ok_or(Error::IntegrityFailed)?;
        let (v, r, _) = owned(
            conn,
            root,
            fence,
            &s.workspace_id,
            id,
            actor,
            "publication_review_note",
        )?;
        if let Some(q) = v.get("question_source") {
            let r = ref_of(q)?;
            review_questions::view(
                conn,
                root,
                fence,
                &s.workspace_id,
                actor,
                s,
                &r.object_id,
                now,
            )?;
        }
        notes.push(ReviewNoteView {
            note_id: id.into(),
            revision: r,
            title: v["title"].as_str().ok_or(Error::IntegrityFailed)?.into(),
            proposal: Some(policy::faq(&v["proposal"])?),
            created_at_ms: v["created_at_ms"].as_u64().ok_or(Error::IntegrityFailed)?,
            evidence: "self_reported".into(),
            question_source: v.get("question_source").map(ref_of).transpose()?,
        });
    }
    let questions = review_questions::list(conn, root, fence, op, actor, s, now)?;
    let faqs = |v: &Value| -> Result<Vec<ReviewFaq>> {
        v["faq"]
            .as_array()
            .ok_or(Error::IntegrityFailed)?
            .iter()
            .map(policy::faq)
            .collect()
    };
    let facts = c.product["facts"]
        .as_array()
        .ok_or(Error::IntegrityFailed)?
        .iter()
        .map(|f| {
            Ok(ReviewFact {
                id: f["id"].as_str().ok_or(Error::IntegrityFailed)?.into(),
                label: f["label"].as_str().ok_or(Error::IntegrityFailed)?.into(),
                value: format!(
                    "{}{}",
                    f["value"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| f["value"].to_string()),
                    f["unit"].as_str().unwrap_or("")
                ),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(FoundationResult {
        resource_id: s.run_id.clone(),
        review_context: Some(ReviewContext {
            source: Some(s.clone()),
            run_name: c.name,
            run_revision: c.proof["run_revision"].as_u64().unwrap(),
            snapshot_id: c.proof["snapshot_id"].as_str().unwrap().into(),
            snapshot_sha256: c.proof["snapshot_sha256"].as_str().unwrap().into(),
            product: Some(c.product_ref),
            show: Some(c.show_ref),
            published_faq: faqs(&c.published)?,
            current_faq: faqs(&c.product)?,
            facts,
            notes,
            questions,
            question_capacity: review_questions::CAPACITY as u32,
            facts_unchanged: c.facts_unchanged,
            reason: if c.facts_unchanged {
                "READY"
            } else {
                "FACTS_CHANGED"
            }
            .into(),
        }),
        ..Default::default()
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn save_note(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    x: &ReviewNoteSave,
    now: i64,
) -> Result<FoundationResult> {
    let s = x.source.as_ref().ok_or(Error::InvalidArgument)?;
    let c = source(tx, root, fence, op, actor, s, now)?;
    if let Some(r) = &x.question_source {
        review_questions::check_reference(tx, root, fence, &s.workspace_id, actor, s, r, now)?;
    }
    uuid(&x.note_id)?;
    if x.title.trim().is_empty()
        || x.title.chars().count() > 120
        || x.title.chars().any(char::is_control)
    {
        return Err(Error::InvalidArgument);
    }
    let mut p = x.proposal.clone().ok_or(Error::InvalidArgument)?;
    if p.id.is_empty() {
        p.id = format!("review_{}", x.note_id.replace('-', ""));
    }
    policy::plan(&c.product, &c.show, &p)?;
    let (idx_id, mut idx, idx_rev) = index(tx, root, s, actor, now)?;
    if let Some(q) = &x.question_source {
        for id in idx["notes"].as_array().unwrap() {
            if id != &x.note_id {
                let (v, _, _) = owned(
                    tx,
                    root,
                    fence,
                    &s.workspace_id,
                    id.as_str().ok_or(Error::IntegrityFailed)?,
                    actor,
                    "publication_review_note",
                )?;
                if v["question_source"]["object_id"] == q.object_id {
                    return Err(Error::AlreadyExists);
                }
            }
        }
    }
    let exists = idx["notes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n == &x.note_id);
    let old = if exists {
        Some(owned(
            tx,
            root,
            fence,
            &s.workspace_id,
            &x.note_id,
            actor,
            "publication_review_note",
        )?)
    } else {
        None
    };
    if let Some((old, r, _)) = &old {
        if old["source"]["run_id"] != s.run_id {
            return Err(Error::PermissionDenied);
        }
        if x.expected_revision == 0
            && old["title"] == x.title
            && old["proposal"] == policy::proposal_value(&p)
            && old["source"] == c.proof
            && old.get("question_source").map(ref_of).transpose()? == x.question_source
        {
            return read(tx, root, fence, op, actor, s, now);
        }
        if *r != x.expected_revision {
            return Err(Error::RevisionConflict);
        }
    } else {
        if x.expected_revision != 0 {
            return Err(Error::RevisionConflict);
        }
        if idx["notes"].as_array().unwrap().len() >= 3 {
            return Err(Error::ResourceExhausted);
        }
        idx["notes"]
            .as_array_mut()
            .unwrap()
            .push(x.note_id.clone().into());
    }
    let created = old
        .as_ref()
        .map_or(now, |(v, _, _)| v["created_at_ms"].as_i64().unwrap());
    let mut v = json!({"schema_version":1,"kind":"publication_review_note","created_by":actor,"created_at_ms":created,"updated_at_ms":now,"source":c.proof,"title":x.title.trim(),"proposal":policy::proposal_value(&p)});
    if let Some(q) = &x.question_source {
        v["question_source"] = pointer(q);
    }
    llm::put(
        tx,
        root,
        &s.workspace_id,
        &x.note_id,
        x.expected_revision,
        &v,
        now,
    )?;
    idx["updated_at_ms"] = now.into();
    llm::put(tx, root, &s.workspace_id, &idx_id, idx_rev, &idx, now)?;
    read(tx, root, fence, op, actor, s, now)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn read_job(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    id: &str,
    now: i64,
) -> Result<FoundationResult> {
    let (v, r, sha) = owned(
        conn,
        root,
        fence,
        ws,
        id,
        actor,
        "publication_review_draft_job",
    )?;
    let s = source_from(ws, &v["source"])?;
    let c = source(conn, root, fence, op, actor, &s, now)?;
    let note = ref_of(&v["note"])?;
    let (note_value, nr, _) = owned(
        conn,
        root,
        fence,
        ws,
        &note.object_id,
        actor,
        "publication_review_note",
    )?;
    let question_changed = if let Some(q) = note_value.get("question_source") {
        let r = ref_of(q)?;
        review_questions::view(conn, root, fence, ws, actor, &s, &r.object_id, now)?
            .record
            .as_ref()
            != Some(&r)
    } else {
        false
    };
    let reason = if question_changed {
        "QUESTION_CHANGED"
    } else if !c.facts_unchanged {
        "FACTS_CHANGED"
    } else if c.proof != v["source"] {
        "SOURCE_RUN_CHANGED"
    } else if nr != note.revision {
        "NOTE_CHANGED"
    } else if pointer(&c.product_ref) != v["base_product"] || pointer(&c.show_ref) != v["base_show"]
    {
        "DRAFT_CHANGED"
    } else {
        "READY"
    };
    let proposal = policy::faq(&v["proposal"])?;
    let saved = if v["saved"].is_null() {
        None
    } else {
        Some((
            ref_of(&v["saved"]["product"])?,
            ref_of(&v["saved"]["show"])?,
        ))
    };
    Ok(FoundationResult {
        resource_id: id.into(),
        revision: r,
        sha256: sha.clone(),
        review_draft: Some(ReviewDraftView {
            source: Some(s),
            job_id: id.into(),
            revision: r,
            sha256: sha,
            state: v["state"].as_str().ok_or(Error::IntegrityFailed)?.into(),
            reason: reason.into(),
            note_id: note.object_id,
            base_product: Some(ref_of(&v["base_product"])?),
            base_show: Some(ref_of(&v["base_show"])?),
            before: Some(policy::faq(&v["before"])?),
            current: Some(policy::before(&c.product, &proposal.id)?),
            proposal: Some(proposal),
            target_product_id: v["targets"]["product_id"]
                .as_str()
                .ok_or(Error::IntegrityFailed)?
                .into(),
            target_show_id: v["targets"]["show_id"]
                .as_str()
                .ok_or(Error::IntegrityFailed)?
                .into(),
            saved_product: saved.as_ref().map(|v| v.0.clone()),
            saved_show: saved.map(|v| v.1),
        }),
        ..Default::default()
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn plan(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    x: &ReviewDraftPlan,
    now: i64,
) -> Result<FoundationResult> {
    let s = x.source.as_ref().ok_or(Error::InvalidArgument)?;
    let c = source(tx, root, fence, op, actor, s, now)?;
    for id in [&x.job_id, &x.target_product_id, &x.target_show_id] {
        uuid(id)?;
    }
    if x.target_product_id == x.target_show_id
        || [&c.product_ref.object_id, &c.show_ref.object_id].contains(&&x.target_product_id)
        || [&c.product_ref.object_id, &c.show_ref.object_id].contains(&&x.target_show_id)
    {
        return Err(Error::InvalidArgument);
    }
    if !c.facts_unchanged {
        return Err(Error::CapabilityUnavailable);
    }
    if c.product_ref.revision != x.expected_product_revision
        || c.show_ref.revision != x.expected_show_revision
    {
        return Err(Error::RevisionConflict);
    }
    let (n, nr, nh) = owned(
        tx,
        root,
        fence,
        &s.workspace_id,
        &x.note_id,
        actor,
        "publication_review_note",
    )?;
    if nr != x.note_revision || n["source"] != c.proof {
        return Err(Error::RevisionConflict);
    }
    if let Some(q) = n.get("question_source") {
        review_questions::check_reference(
            tx,
            root,
            fence,
            &s.workspace_id,
            actor,
            s,
            &ref_of(q)?,
            now,
        )?;
    }
    let p = policy::faq(&n["proposal"])?;
    let change = policy::plan(&c.product, &c.show, &p)?;
    let note = ReviewObjectRef {
        object_id: x.note_id.clone(),
        revision: nr,
        sha256: nh,
    };
    let binding=hash(json!({"source":c.proof,"note":pointer(&note),"product":pointer(&c.product_ref),"show":pointer(&c.show_ref)}).to_string().as_bytes());
    let (idx_id, mut idx, idx_rev) = index(tx, root, s, actor, now)?;
    if idx["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|id| id == &x.job_id)
    {
        let (v, _, _) = owned(
            tx,
            root,
            fence,
            &s.workspace_id,
            &x.job_id,
            actor,
            "publication_review_draft_job",
        )?;
        if v["binding_sha256"] != binding
            || v["targets"]["product_id"] != x.target_product_id
            || v["targets"]["show_id"] != x.target_show_id
        {
            return Err(Error::RevisionConflict);
        }
        return read_job(tx, root, fence, op, actor, &s.workspace_id, &x.job_id, now);
    }
    for id in idx["jobs"].as_array().ok_or(Error::IntegrityFailed)? {
        op.check()?;
        let id = id.as_str().ok_or(Error::IntegrityFailed)?;
        let (v, _, _) = owned(
            tx,
            root,
            fence,
            &s.workspace_id,
            id,
            actor,
            "publication_review_draft_job",
        )?;
        if id == x.job_id
            && (v["binding_sha256"] != binding
                || v["targets"]["product_id"] != x.target_product_id
                || v["targets"]["show_id"] != x.target_show_id)
        {
            return Err(Error::RevisionConflict);
        }
        if v["binding_sha256"] == binding {
            return read_job(tx, root, fence, op, actor, &s.workspace_id, id, now);
        }
    }
    if idx["jobs"].as_array().unwrap().len() >= 32 {
        return Err(Error::ResourceExhausted);
    }
    let v = json!({"schema_version":1,"kind":"publication_review_draft_job","created_by":actor,"created_at_ms":now,"updated_at_ms":now,"source":c.proof,"note":pointer(&note),"base_product":pointer(&c.product_ref),"base_show":pointer(&c.show_ref),"targets":{"product_id":x.target_product_id,"show_id":x.target_show_id},"proposal":policy::proposal_value(&p),"before":policy::proposal_value(&change.before),"state":"preview","binding_sha256":binding,"saved":null});
    llm::put(tx, root, &s.workspace_id, &x.job_id, 0, &v, now)?;
    idx["jobs"]
        .as_array_mut()
        .unwrap()
        .push(x.job_id.clone().into());
    idx["updated_at_ms"] = now.into();
    llm::put(tx, root, &s.workspace_id, &idx_id, idx_rev, &idx, now)?;
    read_job(tx, root, fence, op, actor, &s.workspace_id, &x.job_id, now)
}
fn insert_draft(
    tx: &Transaction<'_>,
    root: &Path,
    op: &Operation,
    ws: &str,
    id: &str,
    v: &Value,
    now: i64,
) -> Result<ReviewObjectRef> {
    uuid(id)?;
    let bytes = v.to_string();
    nai_runtime_core::preparation::validate(op, &bytes)?;
    let exists: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM objects WHERE workspace_id=?1 AND id=?2)",
            params![ws, id],
            |r| r.get(0),
        )
        .wb()?;
    if exists {
        return Err(Error::AlreadyExists);
    }
    let sha = store_body(root, bytes.as_bytes())?;
    tx.execute(
        "INSERT INTO objects VALUES(?1,?2,1,0,?3,?3)",
        params![ws, id, now],
    )
    .wb()?;
    tx.execute(
        "INSERT INTO object_versions VALUES(?1,?2,1,?3,?4,?4)",
        params![ws, id, sha, now],
    )
    .wb()?;
    Ok(ReviewObjectRef {
        object_id: id.into(),
        revision: 1,
        sha256: sha,
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn apply(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    x: &ReviewDraftApply,
    now: i64,
) -> Result<FoundationResult> {
    if !x.acknowledge {
        return Err(Error::PermissionDenied);
    }
    let (mut v, r, sha) = owned(
        tx,
        root,
        fence,
        &x.workspace_id,
        &x.job_id,
        actor,
        "publication_review_draft_job",
    )?;
    let view = read_job(tx, root, fence, op, actor, &x.workspace_id, &x.job_id, now)?;
    if v["state"] == "applied" {
        return Ok(view);
    }
    if r != x.expected_revision || sha != x.expected_sha256 {
        return Err(Error::RevisionConflict);
    }
    let reason = &view.review_draft.as_ref().unwrap().reason;
    if reason != "READY" || v["state"] != "preview" {
        if v["state"] != "conflict" {
            v["state"] = "conflict".into();
            v["updated_at_ms"] = now.into();
            llm::put(tx, root, &x.workspace_id, &x.job_id, r, &v, now)?;
        }
        return read_job(tx, root, fence, op, actor, &x.workspace_id, &x.job_id, now);
    }
    let s = source_from(&x.workspace_id, &v["source"])?;
    let c = source(tx, root, fence, op, actor, &s, now)?;
    let change = policy::plan(&c.product, &c.show, &policy::faq(&v["proposal"])?)?;
    let product = insert_draft(
        tx,
        root,
        op,
        &x.workspace_id,
        v["targets"]["product_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?,
        &change.product,
        now,
    )?;
    let show = policy::show_draft(&c.show, pointer(&product))?;
    let show = insert_draft(
        tx,
        root,
        op,
        &x.workspace_id,
        v["targets"]["show_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?,
        &show,
        now,
    )?;
    v["state"] = "applied".into();
    v["saved"] = json!({"product":pointer(&product),"show":pointer(&show)});
    v["updated_at_ms"] = now.into();
    llm::put(tx, root, &x.workspace_id, &x.job_id, r, &v, now)?;
    read_job(tx, root, fence, op, actor, &x.workspace_id, &x.job_id, now)
}
