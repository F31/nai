use super::*;
use std::fs;
const BUSINESS: &[&str] = &[
    "public_media_approvals",
    "approvals",
    "private_cards",
    "deliveries",
    "outbox",
    "budget_reservations",
    "object_versions",
    "objects",
    "sessions",
    "roles",
];
const RETAIN: &[&str] = &[
    "audit_events",
    "workspace_selection",
    "workspace_retained_budget",
    "workspace_purges",
    "workspace_purge_files",
];
const AVATAR_TABLES: &[&str] = &["avatar_bindings", "avatar_rights_revocations"];

// An optional future schema is recognized only as the exact admitted pair.
// A partially applied migration or a same-name foreign table fails closed.
pub(super) fn avatar_schema_ready(c: &Connection) -> Result<bool> {
    let expected = include_str!("../../../schemas/storage/avatar-authority.sql")
        .lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n");
    let statements: Vec<String> = expected
        .split(';')
        .map(|s| s.split_whitespace().collect::<String>())
        .filter(|s| !s.is_empty())
        .collect();
    if statements.len() != AVATAR_TABLES.len() {
        return Err(Error::IntegrityFailed);
    }
    let mut present = 0;
    for (name, expected) in AVATAR_TABLES.iter().zip(statements) {
        let sql: Option<String> = c
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
                [name],
                |r| r.get(0),
            )
            .optional()
            .wb()?;
        if let Some(sql) = sql {
            if sql.split_whitespace().collect::<String>() != expected {
                return Err(Error::IntegrityFailed);
            }
            present += 1;
        }
    }
    match present {
        0 => Ok(false),
        2 => Ok(true),
        _ => Err(Error::IntegrityFailed),
    }
}

fn plan(
    c: &Connection,
    root: &Path,
    op: &Operation,
    ws: &str,
    expected: u64,
) -> Result<(WorkspacePurgePlan, Vec<String>)> {
    uuid(ws)?;
    op.check()?;
    let avatar_schema = avatar_schema_ready(c)?;
    let revision = c
        .query_row("SELECT revision FROM workspaces WHERE id=?1", [ws], |r| {
            uint(r, 0)
        })
        .optional()
        .wb()?
        .ok_or(Error::NotFound)?;
    if expected != revision {
        return Err(Error::RevisionConflict);
    }
    let mut p = WorkspacePurgePlan {
        workspace_id: ws.into(),
        revision,
        name: c
            .query_row("SELECT name FROM workspaces WHERE id=?1", [ws], |r| {
                r.get(0)
            })
            .wb()?,
        ..Default::default()
    };
    if workspace_lifecycle::state(c, ws)? != "workspace_recycle" {
        p.blockers.push("NOT_RECYCLED".into());
    }
    if c.query_row(
        "SELECT EXISTS(SELECT 1 FROM workspace_selection WHERE workspace_id=?1)",
        [ws],
        |r| r.get::<_, bool>(0),
    )
    .wb()?
    {
        p.blockers.push("WORKSPACE_IN_USE".into());
    }
    if shared_catalog::pending(c)? {
        p.blockers.push("MODEL_MIGRATION_PENDING".into());
    }
    let busy:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE workspace_id=?1 AND stopped=0) OR EXISTS(SELECT 1 FROM outbox WHERE workspace_id=?1 AND state IN('pending','dispatching')) OR EXISTS(SELECT 1 FROM budget_reservations WHERE workspace_id=?1 AND status='reserved')",[ws],|r|r.get(0)).wb()?;
    if busy {
        p.blockers.push("TASKS_NOT_DRAINED".into());
    }
    let mut names=c.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name LIMIT 65").wb()?;
    let names = names
        .query_map([], |r| r.get::<_, String>(0))
        .wb()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .wb()?;
    if names.len() > 64 {
        return Err(Error::ResourceExhausted);
    }
    let mut fingerprint = Sha256::new();
    for name in names {
        op.check()?;
        if !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return Err(Error::IntegrityFailed);
        }
        let mut columns = c.prepare(&format!("PRAGMA table_info({name})")).wb()?;
        let columns = columns
            .query_map([], |r| r.get::<_, String>(1))
            .wb()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .wb()?;
        if !columns.iter().any(|s| s == "workspace_id") {
            continue;
        }
        if !(BUSINESS.contains(&name.as_str())
            || RETAIN.contains(&name.as_str())
            || (avatar_schema && AVATAR_TABLES.contains(&name.as_str())))
        {
            return Err(Error::IntegrityFailed);
        }
        if RETAIN.contains(&name.as_str()) {
            continue;
        }
        let mut q = c
            .prepare(&format!(
                "SELECT * FROM {name} WHERE workspace_id=?1 ORDER BY rowid LIMIT 10001"
            ))
            .wb()?;
        let mut rows = q.query([ws]).wb()?;
        let mut count = 0;
        fingerprint.update(name.as_bytes());
        while let Some(row) = rows.next().wb()? {
            op.check()?;
            count += 1;
            if count > 10000 {
                return Err(Error::ResourceExhausted);
            }
            for col in 0..row.as_ref().column_count() {
                let value = format!("{:?}", row.get_ref(col).wb()?);
                fingerprint.update((value.len() as u64).to_le_bytes());
                fingerprint.update(value.as_bytes());
            }
        }
        p.counts.push(WorkspacePurgeCount { kind: name, count });
    }
    if avatar_schema && c.query_row("SELECT EXISTS(SELECT 1 FROM avatar_bindings WHERE workspace_id=?1) OR EXISTS(SELECT 1 FROM avatar_rights_revocations WHERE workspace_id=?1)",[ws], |r| r.get::<_,bool>(0)).wb()? {
        p.blockers.push("AVATAR_CLEANUP_UNAVAILABLE".into());
    }
    // Photo and video files are content-addressed and may be shared across
    // workspaces. Until ownership/refcount cleanup is wired, never report
    // full deletion. Both blockers can appear on one plan.
    let photo_directory = root.join("avatar-photos");
    super::root::no_links(&photo_directory)?;
    let video_directory = root.join("avatar-videos");
    super::root::no_links(&video_directory)?;
    if photo_directory.exists() || video_directory.exists() {
        let mut media_query=c.prepare("SELECT v.sha256 FROM objects o JOIN object_versions v ON v.workspace_id=o.workspace_id AND v.object_id=o.id AND v.revision=o.revision WHERE o.workspace_id=?1 AND o.deleted=0 ORDER BY o.id LIMIT 257").wb()?;
        let media_rows = media_query
            .query_map([ws], |r| r.get::<_, String>(0))
            .wb()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .wb()?;
        if media_rows.len() > 256 {
            return Err(Error::ResourceExhausted);
        }
        let mut photo_block = false;
        let mut video_block = false;
        for digest in media_rows {
            op.check()?;
            let text = body(root, &digest)?;
            match nai_runtime_core::preparation::kind(&text).as_deref() {
                Some("avatar_photo_draft") if photo_directory.exists() => photo_block = true,
                Some("avatar_video_draft") if video_directory.exists() => video_block = true,
                _ => {}
            }
            if photo_block && video_block {
                break;
            }
        }
        if photo_block {
            p.blockers.push("PHOTO_CLEANUP_UNAVAILABLE".into());
        }
        if video_block {
            p.blockers.push("VIDEO_CLEANUP_UNAVAILABLE".into());
        }
    }
    let mut q=c.prepare("SELECT DISTINCT sha256 FROM object_versions WHERE workspace_id=?1 ORDER BY sha256 LIMIT 4097").wb()?;
    let files = q
        .query_map([ws], |r| r.get::<_, String>(0))
        .wb()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .wb()?;
    if files.len() > 4096 {
        return Err(Error::ResourceExhausted);
    }
    let queued: u64 = c
        .query_row("SELECT count(*) FROM workspace_purge_files", [], |r| {
            uint(r, 0)
        })
        .wb()?;
    if queued + files.len() as u64 > 4096 {
        return Err(Error::ResourceExhausted);
    }
    p.file_count = files.len() as u32;
    if !shared_catalog::enabled(c)? {
        for kind in ["publication_llm_config", "publication_tts_config"] {
            if !llm::models_kind(c, root, ws, kind)?.is_empty() {
                p.blockers.push("UNSHARED_MODELS".into());
                break;
            }
        }
    }
    p.retained_audit_records = c
        .query_row(
            "SELECT count(*) FROM audit_events WHERE workspace_id=?1",
            [ws],
            |r| uint(r, 0),
        )
        .wb()?;
    p.retained_budget_records = c
        .query_row(
            "SELECT count(*) FROM budget_reservations WHERE workspace_id=?1",
            [ws],
            |r| uint(r, 0),
        )
        .wb()?;
    let mut bytes = p.encode_to_vec();
    bytes.extend_from_slice(&fingerprint.finalize());
    p.plan_sha256 = hash(&bytes);
    Ok((p, files))
}
impl Store {
    pub(super) fn pending_purges(
        &self,
        actor: &str,
        x: &FoundationBrowse,
    ) -> Result<FoundationResult> {
        if actor != self.root_owner {
            return Err(Error::PermissionDenied);
        }
        if !x.workspace_id.is_empty() || !(1..=32).contains(&x.limit) {
            return Err(Error::InvalidArgument);
        }
        if !x.after_id.is_empty() {
            uuid(&x.after_id)?;
        }
        let mut q=self.conn.prepare("SELECT workspace_id,plan,created_at FROM workspace_purges p WHERE workspace_id>?1 AND EXISTS(SELECT 1 FROM workspace_purge_files f WHERE f.workspace_id=p.workspace_id) ORDER BY workspace_id LIMIT ?2").wb()?;
        let rows = q
            .query_map(params![x.after_id, x.limit + 1], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?, uint(r, 2)?))
            })
            .wb()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .wb()?;
        let more = rows.len() > x.limit as usize;
        let resources = rows
            .into_iter()
            .take(x.limit as usize)
            .map(|(id, b, created)| {
                let p =
                    WorkspacePurgePlan::decode(b.as_slice()).map_err(|_| Error::IntegrityFailed)?;
                Ok(FoundationResource {
                    id,
                    name: p.name,
                    kind: "workspace_cleanup".into(),
                    revision: p.revision,
                    created_at_ms: created,
                    updated_at_ms: created,
                    recycled: true,
                    has_content: true,
                    ..Default::default()
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let next_cursor = if more {
            resources.last().ok_or(Error::IntegrityFailed)?.id.clone()
        } else {
            String::new()
        };
        Ok(FoundationResult {
            resources,
            next_cursor,
            ..Default::default()
        })
    }
    pub(super) fn workspace_context(
        &mut self,
        op: &Operation,
        actor: &str,
        x: &FoundationLlmRead,
    ) -> Result<FoundationResult> {
        if actor != self.root_owner {
            return Err(Error::PermissionDenied);
        }
        let old: Option<String> = self
            .conn
            .query_row(
                "SELECT workspace_id FROM workspace_selection WHERE principal_id=?1",
                [actor],
                |r| r.get(0),
            )
            .optional()
            .wb()?;
        let explicit = !x.workspace_id.is_empty();
        let wanted = if explicit {
            Some(x.workspace_id.clone())
        } else {
            old.clone()
        };
        let valid = |id: &str| -> Result<bool> {
            Ok(!matches!(workspace_lifecycle::state(&self.conn,id)?.as_str(),"workspace_recycle"|"workspace_delete")&&self.conn.query_row("SELECT EXISTS(SELECT 1 FROM workspaces w JOIN roles r ON r.workspace_id=w.id WHERE w.id=?1 AND r.principal_id=?2 AND r.role!=0)",params![id,actor],|r|r.get::<_,bool>(0)).wb()?)
        };
        let mut chosen = None;
        if let Some(id) = &wanted
            && valid(id)?
        {
            chosen = Some(id.clone());
        }
        if explicit && chosen.is_none() {
            return Err(Error::PermissionDenied);
        }
        let fallback = chosen.is_none() && old.is_some();
        if chosen.is_none() {
            let mut q=self.conn.prepare("SELECT w.id FROM workspaces w JOIN roles r ON r.workspace_id=w.id WHERE r.principal_id=?1 AND r.role!=0 ORDER BY w.created_at,w.id LIMIT 257").wb()?;
            let ids = q
                .query_map([actor], |r| r.get::<_, String>(0))
                .wb()?
                .collect::<rusqlite::Result<Vec<_>>>()
                .wb()?;
            if ids.len() > 256 {
                return Err(Error::ResourceExhausted);
            }
            for id in ids {
                op.check()?;
                if valid(&id)? {
                    chosen = Some(id);
                    break;
                }
            }
        }
        op.check()?;
        if let Some(id) = &chosen {
            self.conn.execute("INSERT INTO workspace_selection VALUES(?1,?2) ON CONFLICT(principal_id) DO UPDATE SET workspace_id=excluded.workspace_id",params![actor,id]).wb()?;
        } else {
            self.conn
                .execute(
                    "DELETE FROM workspace_selection WHERE principal_id=?1",
                    [actor],
                )
                .wb()?;
        }
        Ok(FoundationResult {
            resource_id: chosen.unwrap_or_default(),
            task_reason: if fallback {
                "WORKSPACE_SELECTION_FALLBACK"
            } else {
                "WORKSPACE_SELECTION_RESTORED"
            }
            .into(),
            ..Default::default()
        })
    }
    pub(super) fn preview_purge(
        &self,
        op: &Operation,
        actor: &str,
        x: &WorkspacePurgeRead,
    ) -> Result<FoundationResult> {
        if actor != self.root_owner {
            return Err(Error::PermissionDenied);
        }
        // A durable receipt also supplies the review screen after its workspace
        // has disappeared from the ordinary recycle-bin list.
        let saved: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT plan FROM workspace_purges WHERE workspace_id=?1",
                [&x.workspace_id],
                |r| r.get(0),
            )
            .optional()
            .wb()?;
        if let Some(bytes) = saved {
            let p =
                WorkspacePurgePlan::decode(bytes.as_slice()).map_err(|_| Error::IntegrityFailed)?;
            if p.revision != x.expected_revision {
                return Err(Error::RevisionConflict);
            }
            return Ok(FoundationResult {
                workspace_purge_plan: Some(p),
                ..Default::default()
            });
        }
        let (p, _) = plan(
            &self.conn,
            &self.root,
            op,
            &x.workspace_id,
            x.expected_revision,
        )?;
        Ok(FoundationResult {
            workspace_purge_plan: Some(p),
            ..Default::default()
        })
    }
    pub(super) fn cleanup_purge_files(&mut self, op: &Operation, ws: Option<&str>) -> Result<()> {
        let mut q=self.conn.prepare("SELECT workspace_id,sha256 FROM workspace_purge_files WHERE (?1 IS NULL OR workspace_id=?1) ORDER BY workspace_id,sha256 LIMIT 32").wb()?;
        let rows = q
            .query_map([ws], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .wb()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .wb()?;
        drop(q);
        for (ws, sha) in rows {
            op.check()?;
            if sha.len() != 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(Error::IntegrityFailed);
            }
            let used:bool=self.conn.query_row("SELECT EXISTS(SELECT 1 FROM object_versions v JOIN objects o ON o.workspace_id=v.workspace_id AND o.id=v.object_id WHERE v.sha256=?1 AND o.deleted=0)",[&sha],|r|r.get(0)).wb()?;
            if !used {
                let file = self.root.join("objects").join(&sha);
                let marker = self.root.join("object-ownership").join(&sha);
                root::no_links(&file)?;
                root::no_links(&marker)?;
                if marker.exists() {
                    if root::read(&marker, 64)? != b"nai-owned-text-v1" {
                        return Err(Error::IntegrityFailed);
                    }
                    if file.exists() {
                        if hash(&root::read(&file, 8192)?) != sha {
                            return Err(Error::IntegrityFailed);
                        }
                        fs::remove_file(&file).map_err(|_| Error::IoFailed)?;
                    }
                    op.check()?;
                    fs::remove_file(&marker).map_err(|_| Error::IoFailed)?;
                } else if file.exists() {
                    return Err(Error::IntegrityFailed);
                }
            }
            self.conn
                .execute(
                    "DELETE FROM workspace_purge_files WHERE workspace_id=?1 AND sha256=?2",
                    params![ws, sha],
                )
                .wb()?;
        }
        Ok(())
    }
    pub(super) fn read_purge(
        &mut self,
        op: &Operation,
        actor: &str,
        x: &WorkspacePurgeRead,
    ) -> Result<FoundationResult> {
        if actor != self.root_owner {
            return Err(Error::PermissionDenied);
        }
        uuid(&x.workspace_id)?;
        let revision = self
            .conn
            .query_row(
                "SELECT revision FROM workspace_purges WHERE workspace_id=?1",
                [&x.workspace_id],
                |r| uint(r, 0),
            )
            .optional()
            .wb()?
            .ok_or(Error::NotFound)?;
        if revision != x.expected_revision {
            return Err(Error::RevisionConflict);
        }
        let blocked = self.cleanup_purge_files(op, Some(&x.workspace_id)).is_err();
        let pending = self
            .conn
            .query_row(
                "SELECT count(*) FROM workspace_purge_files WHERE workspace_id=?1",
                [&x.workspace_id],
                |r| r.get(0),
            )
            .wb()?;
        Ok(FoundationResult {
            resource_id: x.workspace_id.clone(),
            workspace_purge_status: Some(WorkspacePurgeStatus {
                workspace_id: x.workspace_id.clone(),
                deleted: true,
                pending_files: pending,
                cleanup_blocked: blocked,
            }),
            ..Default::default()
        })
    }
    pub(super) fn purge_workspace(
        &mut self,
        op: &Operation,
        actor: &str,
        x: &WorkspacePurgeCommand,
    ) -> Result<FoundationResult> {
        if actor != self.root_owner {
            return Err(Error::PermissionDenied);
        }
        uuid(&x.workspace_id)?;
        if !x.acknowledge {
            return Err(Error::InvalidArgument);
        }
        let existing: Option<(u64, String)> = self
            .conn
            .query_row(
                "SELECT revision,plan_sha256 FROM workspace_purges WHERE workspace_id=?1",
                [&x.workspace_id],
                |r| Ok((uint(r, 0)?, r.get(1)?)),
            )
            .optional()
            .wb()?;
        if let Some((rev, sha)) = existing {
            if rev != x.expected_revision || sha != x.plan_sha256 {
                return Err(Error::RevisionConflict);
            }
            return self.read_purge(
                op,
                actor,
                &WorkspacePurgeRead {
                    workspace_id: x.workspace_id.clone(),
                    expected_revision: rev,
                },
            );
        }
        let tx = self.conn.transaction().wb()?;
        let (p, files) = plan(&tx, &self.root, op, &x.workspace_id, x.expected_revision)?;
        if p.plan_sha256 != x.plan_sha256 {
            return Err(Error::RevisionConflict);
        }
        if !p.blockers.is_empty() {
            return Err(Error::CapabilityUnavailable);
        }
        let now = utc()?;
        let ws = &x.workspace_id;
        tx.execute(
            "INSERT INTO workspace_purges VALUES(?1,?2,?3,?4,?5)",
            params![
                ws,
                int(x.expected_revision)?,
                p.plan_sha256,
                p.encode_to_vec(),
                now
            ],
        )
        .wb()?;
        for sha in files {
            op.check()?;
            tx.execute(
                "INSERT INTO workspace_purge_files VALUES(?1,?2)",
                params![ws, sha],
            )
            .wb()?;
        }
        tx.execute("INSERT INTO workspace_retained_budget SELECT * FROM budget_reservations WHERE workspace_id=?1",[ws]).wb()?;
        for table in BUSINESS {
            op.check()?;
            tx.execute(&format!("DELETE FROM {table} WHERE workspace_id=?1"), [ws])
                .wb()?;
        }
        if avatar_schema_ready(&tx)? {
            for table in AVATAR_TABLES {
                op.check()?;
                tx.execute(&format!("DELETE FROM {table} WHERE workspace_id=?1"), [ws])
                    .wb()?;
            }
        }
        tx.execute("DELETE FROM workspaces WHERE id=?1", [ws])
            .wb()?;
        audit(&tx, ws, actor, "workspace_delete", ws, now)?;
        audit(
            &tx,
            ws,
            actor,
            "workspace_purge_retaining_financial_facts",
            ws,
            now,
        )?;
        op.check()?;
        tx.commit().wb()?;
        // Once committed, cleanup/response failure must never masquerade as a
        // failed database deletion. The durable receipt supports safe resumption.
        Ok(self
            .read_purge(
                op,
                actor,
                &WorkspacePurgeRead {
                    workspace_id: ws.clone(),
                    expected_revision: x.expected_revision,
                },
            )
            .unwrap_or_else(|_| FoundationResult {
                resource_id: ws.clone(),
                workspace_purge_status: Some(WorkspacePurgeStatus {
                    workspace_id: ws.clone(),
                    deleted: true,
                    pending_files: p.file_count,
                    cleanup_blocked: true,
                }),
                ..Default::default()
            }))
    }
}
