//! Publication manifests share the RF02 immutable object owner; no new database/migration.
use super::*;
use nai_runtime_core::publication as policy;
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(super) fn version_scope(ws: &str, id: &str, revision: u64) -> String {
    format!("v:{ws}:{id}:{revision}")
}
fn pointer(id: &str, revision: u64, sha: &str) -> Value {
    json!({"object_id":id,"revision":revision,"sha256":sha})
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key].as_str().ok_or(Error::IntegrityFailed)
}
fn revision(value: &Value) -> Result<u64> {
    value["revision"].as_u64().ok_or(Error::IntegrityFailed)
}
fn resolve(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    ws: &str,
    r: &Value,
    latest: bool,
) -> Result<Value> {
    let id = text(r, "object_id")?;
    let rev = revision(r)?;
    fence.check_scope(&object_scope(ws, id))?;
    fence.check_scope(&version_scope(ws, id, rev))?;
    let (current, sha) = object(conn, ws, id, rev)?;
    if (latest && current != rev) || sha != text(r, "sha256")? {
        return Err(Error::RevisionConflict);
    }
    serde_json::from_str(&body(root, &sha)?).map_err(|_| Error::InvalidArgument)
}
pub(super) fn documents(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    ws: &str,
    refs: &Value,
    latest: bool,
) -> Result<[Value; 4]> {
    Ok([
        resolve(conn, root, fence, ws, &refs["show"], latest)?,
        resolve(conn, root, fence, ws, &refs["product"], latest)?,
        resolve(conn, root, fence, ws, &refs["host"], latest)?,
        resolve(conn, root, fence, ws, &refs["series"], latest)?,
    ])
}
pub(super) fn stored(
    conn: &Connection,
    root: &Path,
    ws: &str,
    id: &str,
    kind: &str,
) -> Result<(Value, String)> {
    let (rev, sha) = object(conn, ws, id, 0)?;
    if rev != 1 {
        return Err(Error::IntegrityFailed);
    }
    let value = policy::artifact(&body(root, &sha)?)?;
    if value["kind"] != kind {
        return Err(Error::InvalidArgument);
    }
    Ok((value, sha))
}
pub(super) fn insert(
    tx: &Transaction<'_>,
    root: &Path,
    ws: &str,
    id: &str,
    value: &Value,
    now: i64,
) -> Result<FoundationResult> {
    uuid(id)?;
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
    let bytes = value.to_string();
    if bytes.len() > 8192 {
        return Err(Error::ResourceExhausted);
    }
    policy::artifact(&bytes)?;
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
    Ok(FoundationResult {
        resource_id: id.into(),
        revision: 1,
        sha256: sha,
        body: bytes,
        ..Default::default()
    })
}
struct Graph {
    sha: String,
    snapshots: Vec<(String, Value)>,
    sessions: Vec<String>,
    object_count: usize,
}
fn graph(conn: &Connection, root: &Path, fence: &StopFence, ws: &str) -> Result<Graph> {
    let mut statement = conn
        .prepare(
            "SELECT id,revision,deleted FROM objects WHERE workspace_id=?1 ORDER BY id LIMIT 257",
        )
        .wb()?;
    let rows = statement
        .query_map(params![ws], |r| {
            Ok((r.get::<_, String>(0)?, uint(r, 1)?, r.get::<_, bool>(2)?))
        })
        .wb()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .wb()?;
    if rows.len() > 256 {
        return Err(Error::ResourceExhausted);
    }
    let object_count = rows.len();
    let mut heads = vec![];
    let mut snapshots = vec![];
    for (id, rev, deleted) in rows {
        if deleted {
            heads.push(json!([id, rev, true]));
            continue;
        }
        let (_, sha) = object(conn, ws, &id, rev)?;
        let raw = body(root, &sha)?;
        let doc = serde_json::from_str::<Value>(&raw).unwrap_or(Value::Null);
        if doc["kind"] == "publication_impact" {
            continue;
        }
        let denied = fence.check_scope(&object_scope(ws, &id)).is_err()
            || fence.check_scope(&version_scope(ws, &id, rev)).is_err();
        heads.push(json!([id, rev, sha, denied]));
        if doc["kind"] == "publication_snapshot" {
            policy::artifact(&raw)?;
            snapshots.push((id, doc));
        }
    }
    let mut statement=conn.prepare("SELECT id,epoch,stopped,revision,owner FROM sessions WHERE workspace_id=?1 ORDER BY id LIMIT 65").wb()?;
    let rows = statement
        .query_map(params![ws], |r| {
            Ok((
                r.get::<_, String>(0)?,
                uint(r, 1)?,
                r.get::<_, bool>(2)?,
                uint(r, 3)?,
                r.get::<_, String>(4)?,
            ))
        })
        .wb()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .wb()?;
    if rows.len() > 64 {
        return Err(Error::ResourceExhausted);
    }
    let sessions = rows.iter().filter(|r| !r.2).map(|r| r.0.clone()).collect();
    let mut statement=conn.prepare("SELECT principal_id,role,revision FROM roles WHERE workspace_id=?1 ORDER BY principal_id LIMIT 65").wb()?;
    let roles = statement
        .query_map(params![ws], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, uint(r, 2)?))
        })
        .wb()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .wb()?;
    if roles.len() > 64 {
        return Err(Error::ResourceExhausted);
    }
    let mut statement=conn.prepare("SELECT id,revision,state,evidence FROM deliveries WHERE workspace_id=?1 ORDER BY id LIMIT 257").wb()?;
    let deliveries = statement
        .query_map(params![ws], |r| {
            Ok((
                r.get::<_, String>(0)?,
                uint(r, 1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })
        .wb()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .wb()?;
    if deliveries.len() > 256 {
        return Err(Error::ResourceExhausted);
    }
    let value = json!([
        ws,
        heads,
        rows,
        roles,
        deliveries,
        fence.generation(),
        fence.revocation_scopes()?
    ]);
    Ok(Graph {
        sha: hash(value.to_string().as_bytes()),
        snapshots,
        sessions,
        object_count,
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn impact(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    role_revision: u64,
    now: i64,
    x: &FoundationImpact,
) -> Result<FoundationResult> {
    let ws = &x.workspace_id;
    let (current, sha) = object(tx, ws, &x.show_id, 0)?;
    if current != x.show_revision {
        return Err(Error::RevisionConflict);
    }
    let show: Value =
        serde_json::from_str(&body(root, &sha)?).map_err(|_| Error::InvalidArgument)?;
    if nai_runtime_core::preparation::validate(op, &show.to_string())? != "show_draft" {
        return Err(Error::InvalidArgument);
    }
    let refs = json!({"show":pointer(&x.show_id,current,&sha),"product":show["product"],"host":show["host"],"series":show["series"]});
    let docs = documents(tx, root, fence, ws, &refs, true)?;
    let checked = policy::closure(op, [&docs[0], &docs[1], &docs[2], &docs[3]], now)?;
    let graph = graph(tx, root, fence, ws)?;
    if graph.object_count >= 255 {
        return Err(Error::ResourceExhausted);
    }
    let mut changed = BTreeSet::new();
    let mut unknown = vec!["free_text_semantics_requires_human_review".to_owned()];
    let previous = if x.previous_snapshot_id.is_empty() {
        changed.extend(["show", "product", "host", "series"].map(str::to_owned));
        Value::Null
    } else {
        let (prior, prior_sha) = stored(
            tx,
            root,
            ws,
            &x.previous_snapshot_id,
            "publication_snapshot",
        )?;
        if prior["refs"]["show"]["object_id"] != x.show_id {
            return Err(Error::InvalidArgument);
        }
        match documents(tx, root, fence, ws, &prior["refs"], false) {
            Ok(old) => {
                for (i, name) in ["show", "product", "host", "series"].iter().enumerate() {
                    policy::diff(&old[i], &docs[i], name, &mut changed)?;
                }
            }
            Err(Error::NotFound | Error::PermissionDenied | Error::Cancelled) => {
                changed.insert("unknown_previous_content".into());
                unknown.push("previous_content_unavailable_expand_review".into());
            }
            Err(e) => return Err(e),
        }
        pointer(&x.previous_snapshot_id, 1, &prior_sha)
    };
    // No session→snapshot binding exists yet: every active session is possible, never proven unaffected.
    if !graph.sessions.is_empty() {
        unknown.push("session_snapshot_binding_unavailable_all_active_sessions_possible".into());
    }
    let affected = graph
        .snapshots
        .iter()
        .filter(|(_, s)| {
            ["show", "product", "host", "series"].iter().any(|name| {
                s["refs"][name]["object_id"] == refs[name]["object_id"]
                    && s["refs"][name] != refs[name]
            })
        })
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    if affected.len() > 64 {
        return Err(Error::ResourceExhausted);
    }
    let value = json!({"schema_version":1,"kind":"publication_impact","created_at_ms":now,"created_by":actor,"role_revision":role_revision,"refs":refs,"previous_snapshot":previous,"graph_sha256":graph.sha,"expires_at_ms":now+30000,"changed_fields":changed,"explicit_dependencies":checked.dependencies,"affected_sessions":graph.sessions,"affected_snapshots":affected,"unknown_items":unknown});
    insert(tx, root, ws, &x.report_id, &value, now)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn publish(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    role_revision: u64,
    now: i64,
    x: &FoundationPublish,
) -> Result<FoundationResult> {
    let ws = &x.workspace_id;
    let (report, sha) = stored(tx, root, ws, &x.report_id, "publication_impact")?;
    if sha != x.report_sha256 {
        return Err(Error::RevisionConflict);
    }
    if report["created_by"] != actor || report["role_revision"].as_u64() != Some(role_revision) {
        return Err(Error::PermissionDenied);
    }
    if now
        < report["created_at_ms"]
            .as_i64()
            .ok_or(Error::IntegrityFailed)?
    {
        return Err(Error::DeadlineExceeded);
    }
    if report["expires_at_ms"]
        .as_i64()
        .ok_or(Error::IntegrityFailed)?
        <= now
    {
        return Err(Error::DeadlineExceeded);
    }
    if !x.acknowledge_unknown {
        return Err(Error::InvalidArgument);
    }
    let current_graph = graph(tx, root, fence, ws)?;
    if current_graph.object_count >= 256 {
        return Err(Error::ResourceExhausted);
    }
    if current_graph.sha != text(&report, "graph_sha256")? {
        return Err(Error::RevisionConflict);
    }
    let docs = documents(tx, root, fence, ws, &report["refs"], true)?;
    let checked = policy::closure(op, [&docs[0], &docs[1], &docs[2], &docs[3]], now)?;
    let snapshot = json!({"schema_version":1,"kind":"publication_snapshot","created_at_ms":now,"created_by":actor,"refs":report["refs"],"previous_snapshot":report["previous_snapshot"],"report":pointer(&x.report_id,1,&sha),"scope":"private","presentation":"content_only","valid_until_ms":checked.valid_until_ms,"explicit_dependencies":checked.dependencies});
    let mut result = insert(tx, root, ws, &x.snapshot_id, &snapshot, now)?;
    tx.execute(
        "INSERT INTO outbox VALUES(?1,?2,?3,'applied',0,1,?4,?4)",
        params![x.snapshot_id, ws, x.snapshot_id, now],
    )
    .wb()?;
    result.publication_status = PublicationStatus::PrivateContent as i32;
    Ok(result)
}
pub(super) fn report_time(
    conn: &Connection,
    root: &Path,
    ws: &str,
    id: &str,
    now: i64,
) -> Result<()> {
    let (report, _) = stored(conn, root, ws, id, "publication_impact")?;
    if now
        < report["created_at_ms"]
            .as_i64()
            .ok_or(Error::IntegrityFailed)?
        || now
            >= report["expires_at_ms"]
                .as_i64()
                .ok_or(Error::IntegrityFailed)?
    {
        return Err(Error::DeadlineExceeded);
    }
    Ok(())
}
pub(super) fn read(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    ws: &str,
    id: &str,
    now: i64,
) -> Result<FoundationResult> {
    let (snapshot, sha) = stored(conn, root, ws, id, "publication_snapshot")?;
    let check = (|| {
        if now
            < snapshot["created_at_ms"]
                .as_i64()
                .ok_or(Error::IntegrityFailed)?
        {
            return Err(Error::CapabilityUnavailable);
        }
        fence.check_scope(&object_scope(ws, id))?;
        let docs = documents(conn, root, fence, ws, &snapshot["refs"], false)?;
        policy::closure(op, [&docs[0], &docs[1], &docs[2], &docs[3]], now)?;
        Ok::<_, Error>(())
    })();
    let status = match check {
        Ok(()) => PublicationStatus::PrivateContent,
        Err(
            Error::NotFound
            | Error::PermissionDenied
            | Error::Cancelled
            | Error::CapabilityUnavailable,
        ) => PublicationStatus::Blocked,
        Err(e) => return Err(e),
    };
    Ok(FoundationResult {
        resource_id: id.into(),
        revision: 1,
        sha256: sha,
        body: snapshot.to_string(),
        publication_status: status as i32,
        ..Default::default()
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn revoke(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    actor: &str,
    now: i64,
    x: &FoundationPublicationRevoke,
) -> Result<FoundationResult> {
    manual_guard(&x.reason)?;
    if x.reason.len() > 512 {
        return Err(Error::InvalidArgument);
    }
    uuid(&x.revocation_id)?;
    let exists: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM objects WHERE workspace_id=?1 AND id=?2)",
            params![x.workspace_id, x.revocation_id],
            |r| r.get(0),
        )
        .wb()?;
    if exists {
        return Err(Error::AlreadyExists);
    }
    let (snapshot, sha) = stored(
        tx,
        root,
        &x.workspace_id,
        &x.snapshot_id,
        "publication_snapshot",
    )?;
    if sha != x.snapshot_sha256 {
        return Err(Error::RevisionConflict);
    }
    let product = &snapshot["refs"]["product"];
    // The invalid fact version is denied for every snapshot referencing it; corrected revisions remain usable.
    root::revoke(
        root,
        fence,
        &version_scope(
            &x.workspace_id,
            text(product, "object_id")?,
            revision(product)?,
        ),
    )?;
    let value = json!({"schema_version":1,"kind":"publication_revocation","created_at_ms":now,"created_by":actor,"snapshot":pointer(&x.snapshot_id,1,&sha),"reason":x.reason});
    let mut result = insert(tx, root, &x.workspace_id, &x.revocation_id, &value, now)?;
    tx.execute(
        "INSERT INTO outbox VALUES(?1,?2,?3,'applied',0,1,?4,?4)",
        params![x.revocation_id, x.workspace_id, x.snapshot_id, now],
    )
    .wb()?;
    result.publication_status = PublicationStatus::Blocked as i32;
    Ok(result)
}
