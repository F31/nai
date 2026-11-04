use super::*;

// The append-only lifecycle audit is the durable state projection, not a UI flag.
pub(super) fn state(conn: &Connection, ws: &str) -> Result<String> {
    Ok(conn.query_row("SELECT action FROM audit_events WHERE workspace_id=?1 AND action IN('workspace_recycle','workspace_restore','workspace_delete') ORDER BY seq DESC LIMIT 1",[ws],|r|r.get(0)).optional().wb()?.unwrap_or_default())
}
pub(super) fn active(conn: &Connection, ws: &str) -> Result<()> {
    if matches!(
        state(conn, ws)?.as_str(),
        "workspace_recycle" | "workspace_delete"
    ) {
        return Err(Error::PermissionDenied);
    }
    Ok(())
}
fn content(conn: &Connection, ws: &str) -> Result<bool> {
    let limit: u64 = conn
        .query_row(
            "SELECT budget_limit FROM workspaces WHERE id=?1",
            [ws],
            |r| uint(r, 0),
        )
        .optional()
        .wb()?
        .ok_or(Error::NotFound)?;
    if limit != 0 {
        return Ok(true);
    }
    let mut q=conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name LIMIT 65").wb()?;
    let tables = q
        .query_map([], |r| r.get::<_, String>(0))
        .wb()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .wb()?;
    if tables.len() > 64 {
        return Err(Error::ResourceExhausted);
    }
    for table in tables {
        if matches!(
            table.as_str(),
            "workspaces" | "roles" | "audit_events" | "command_results"
        ) {
            continue;
        }
        if !table
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            return Err(Error::IntegrityFailed);
        }
        let mut p = conn.prepare(&format!("PRAGMA table_info({table})")).wb()?;
        let columns = p
            .query_map([], |r| r.get::<_, String>(1))
            .wb()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .wb()?;
        if columns.iter().any(|c| c == "workspace_id") {
            let found: bool = conn
                .query_row(
                    &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE workspace_id=?1)"),
                    [ws],
                    |r| r.get(0),
                )
                .wb()?;
            if found {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
pub(super) fn apply(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    actor: &str,
    now: i64,
    x: &FoundationWorkspaceLifecycle,
) -> Result<FoundationResult> {
    let action = WorkspaceAction::try_from(x.action).map_err(|_| Error::InvalidArgument)?;
    let ws = &x.workspace_id;
    let row: Option<(String, u64, u64, u64)> = tx
        .query_row(
            "SELECT name,revision,created_at,updated_at FROM workspaces WHERE id=?1",
            [ws],
            |r| Ok((r.get(0)?, uint(r, 1)?, uint(r, 2)?, uint(r, 3)?)),
        )
        .optional()
        .wb()?;
    let (name, revision, created, updated) = row.ok_or(Error::NotFound)?;
    let recycled = state(tx, ws)? == "workspace_recycle";
    let has_content = content(tx, ws)?;
    if action == WorkspaceAction::Inspect {
        return Ok(FoundationResult {
            resources: vec![FoundationResource {
                id: ws.clone(),
                name,
                kind: "workspace".into(),
                revision,
                created_at_ms: created,
                updated_at_ms: updated,
                recycled,
                has_content,
                ..Default::default()
            }],
            ..Default::default()
        });
    }
    if revision != x.expected_revision {
        return Err(Error::RevisionConflict);
    }
    if matches!(
        action,
        WorkspaceAction::DeleteEmpty | WorkspaceAction::Recycle
    ) {
        let selected: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM workspace_selection WHERE workspace_id=?1)",
                [ws],
                |r| r.get(0),
            )
            .wb()?;
        if selected {
            return Err(Error::Busy);
        }
    }
    let label = match action {
        WorkspaceAction::DeleteEmpty => {
            if recycled || has_content {
                return Err(Error::CapabilityUnavailable);
            }
            tx.execute("DELETE FROM roles WHERE workspace_id=?1", [ws])
                .wb()?;
            tx.execute(
                "DELETE FROM workspaces WHERE id=?1 AND revision=?2",
                params![ws, int(revision)?],
            )
            .wb()?;
            "workspace_delete"
        }
        WorkspaceAction::Recycle => {
            if recycled {
                return Err(Error::RevisionConflict);
            }
            // Durable scope revocations precede the transactional state change;
            // a failed commit may revoke extra authority but can never revive it.
            let mut q = tx
                .prepare("SELECT principal_id,revision FROM roles WHERE workspace_id=?1 LIMIT 257")
                .wb()?;
            let roles = q
                .query_map([ws], |r| Ok((r.get::<_, String>(0)?, uint(r, 1)?)))
                .wb()?
                .collect::<rusqlite::Result<Vec<_>>>()
                .wb()?;
            if roles.len() > 256 {
                return Err(Error::ResourceExhausted);
            }
            for (id, rev) in roles {
                root::revoke(root, fence, &role_scope(ws, &id, rev))?;
            }
            let mut q = tx
                .prepare("SELECT DISTINCT resource_id FROM outbox WHERE workspace_id=?1 LIMIT 257")
                .wb()?;
            let ids = q
                .query_map([ws], |r| r.get::<_, String>(0))
                .wb()?
                .collect::<rusqlite::Result<Vec<_>>>()
                .wb()?;
            if ids.len() > 256 {
                return Err(Error::ResourceExhausted);
            }
            for id in ids {
                root::revoke(root, fence, &object_scope(ws, &id))?;
            }
            tx.execute(
                "UPDATE roles SET revision=revision+1,updated_at=?1 WHERE workspace_id=?2",
                params![now, ws],
            )
            .wb()?;
            tx.execute("UPDATE sessions SET stopped=1,epoch=epoch+1,revision=revision+1,updated_at=?1 WHERE workspace_id=?2",params![now,ws]).wb()?;
            tx.execute("UPDATE deliveries SET state=CASE WHEN state=?1 THEN ?2 ELSE ?3 END,revision=revision+1,updated_at=?4 WHERE workspace_id=?5 AND state IN(?1,?6,?7,?8)",params![DeliveryState::Committed as i32,DeliveryState::OutcomeUnknown as i32,DeliveryState::Cancelled as i32,now,ws,DeliveryState::AwaitingApproval as i32,DeliveryState::Approved as i32,DeliveryState::Delivering as i32]).wb()?;
            tx.execute("DELETE FROM approvals WHERE workspace_id=?1", [ws])
                .wb()?;
            tx.execute("UPDATE public_media_approvals SET state='revoked',updated_at=?1 WHERE workspace_id=?2 AND state='approved'", params![now,ws]).wb()?;
            tx.execute("UPDATE budget_reservations SET status=CASE WHEN id IN(SELECT resource_id FROM outbox WHERE workspace_id=?1 AND state='pending') THEN 'released' ELSE 'unknown' END,revision=revision+1,updated_at=?2 WHERE workspace_id=?1 AND status='reserved'",params![ws,now]).wb()?;
            tx.execute("UPDATE outbox SET state=CASE WHEN state='pending' THEN 'cancelled' ELSE 'outcome_unknown' END,revision=revision+1,updated_at=?1 WHERE workspace_id=?2 AND state IN('pending','dispatching')",params![now,ws]).wb()?;
            tx.execute(
                "UPDATE workspaces SET revision=revision+1,updated_at=?1 WHERE id=?2",
                params![now, ws],
            )
            .wb()?;
            "workspace_recycle"
        }
        WorkspaceAction::Restore => {
            if !recycled {
                return Err(Error::RevisionConflict);
            }
            tx.execute(
                "UPDATE workspaces SET revision=revision+1,updated_at=?1 WHERE id=?2",
                params![now, ws],
            )
            .wb()?;
            "workspace_restore"
        }
        _ => return Err(Error::InvalidArgument),
    };
    audit(tx, ws, actor, label, ws, now)?;
    Ok(FoundationResult {
        resource_id: ws.clone(),
        revision: revision + 1,
        ..Default::default()
    })
}
