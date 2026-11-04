use super::*;
fn parameters(x: &FoundationBrowse) -> Result<usize> {
    if !(1..=32).contains(&x.limit) {
        return Err(Error::InvalidArgument);
    }
    if !x.after_id.is_empty() {
        uuid(&x.after_id)?;
    }
    Ok(x.limit as usize)
}
pub(super) fn workspaces(
    conn: &Connection,
    fence: &StopFence,
    actor: &str,
    x: &FoundationBrowse,
) -> Result<FoundationResult> {
    let limit = parameters(x)?;
    if !x.workspace_id.is_empty() || !matches!(x.kind.as_str(), "" | "recycled") {
        return Err(Error::InvalidArgument);
    }
    let mut q=conn.prepare("SELECT w.id,w.name,w.revision,w.updated_at,r.revision,w.created_at FROM workspaces w JOIN roles r ON r.workspace_id=w.id WHERE r.principal_id=?1 AND r.role IN(1,2,3) AND w.id>?2 ORDER BY w.id LIMIT 257").wb()?;
    let rows = q
        .query_map(params![actor, x.after_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                uint(r, 2)?,
                uint(r, 3)?,
                uint(r, 4)?,
                uint(r, 5)?,
            ))
        })
        .wb()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .wb()?;
    if rows.len() > 256 {
        return Err(Error::ResourceExhausted);
    }
    let mut list = vec![];
    for (id, name, revision, updated, role_revision, created) in rows {
        let recycled = workspace_lifecycle::state(conn, &id)? == "workspace_recycle";
        if recycled != (x.kind == "recycled") {
            continue;
        }
        if fence
            .check_scope(&role_scope(&id, actor, role_revision))
            .is_ok()
        {
            list.push(FoundationResource {
                id,
                name,
                kind: "workspace".into(),
                revision,
                updated_at_ms: updated,
                created_at_ms: created,
                recycled,
                ..Default::default()
            });
        }
    }
    page(list, limit)
}
pub(super) fn preparation(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    x: &FoundationBrowse,
) -> Result<FoundationResult> {
    let limit = parameters(x)?;
    let allowed = [
        "product_draft",
        "host_draft",
        "series_draft",
        "show_draft",
        "publication_snapshot",
        "manual_run_draft",
        "avatar_profile_draft",
        "avatar_photo_draft",
        "avatar_video_draft",
    ];
    if !x.kind.is_empty() && !allowed.contains(&x.kind.as_str()) {
        return Err(Error::InvalidArgument);
    }
    let mut q=conn.prepare("SELECT id,revision,updated_at FROM objects WHERE workspace_id=?1 AND deleted=0 AND id>?2 ORDER BY id LIMIT 257").wb()?;
    let rows = q
        .query_map(params![x.workspace_id, x.after_id], |r| {
            Ok((r.get::<_, String>(0)?, uint(r, 1)?, uint(r, 2)?))
        })
        .wb()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .wb()?;
    if rows.len() > 256 {
        return Err(Error::ResourceExhausted);
    }
    let mut list = vec![];
    for (id, revision, updated) in rows {
        if fence
            .check_scope(&object_scope(&x.workspace_id, &id))
            .is_err()
        {
            continue;
        }
        let (_, sha) = object(conn, &x.workspace_id, &id, revision)?;
        let Ok(doc) = serde_json::from_str::<serde_json::Value>(&body(root, &sha)?) else {
            continue;
        };
        let Some(kind) = doc["kind"].as_str() else {
            continue;
        };
        if !allowed.contains(&kind) || !x.kind.is_empty() && kind != x.kind {
            continue;
        }
        let name = doc["name"].as_str().unwrap_or("已发布场次").to_owned();
        let parent = doc["refs"]["show"]["object_id"]
            .as_str()
            .or_else(|| doc["show"]["object_id"].as_str())
            .unwrap_or_default()
            .to_owned();
        list.push(FoundationResource {
            id,
            name,
            kind: kind.into(),
            revision,
            sha256: sha,
            updated_at_ms: updated,
            parent_id: parent,
            created_at_ms: 0,
            ..Default::default()
        });
    }
    page(list, limit)
}
fn page(mut list: Vec<FoundationResource>, limit: usize) -> Result<FoundationResult> {
    let next_cursor = if list.len() > limit {
        list[limit - 1].id.clone()
    } else {
        String::new()
    };
    list.truncate(limit);
    Ok(FoundationResult {
        resources: list,
        next_cursor,
        ..Default::default()
    })
}
