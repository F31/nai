use super::*;
use serde_json::Value;
use std::collections::BTreeSet;
pub(super) const SCOPE: &str = "00000000-0000-4000-8000-000000004802";
pub(super) fn enabled(c: &Connection) -> Result<bool> {
    c.query_row(
        "SELECT EXISTS(SELECT 1 FROM shared_model_migration WHERE stage='committed')",
        [],
        |r| r.get(0),
    )
    .wb()
}
pub(super) fn pending(c: &Connection) -> Result<bool> {
    c.query_row(
        "SELECT EXISTS(SELECT 1 FROM shared_model_migration WHERE stage IN ('preparing','rewrapping'))",
        [],
        |r| r.get(0),
    )
    .wb()
}
pub(super) fn credential_scope<'a>(c: &Connection, ws: &'a str, id: &str) -> Result<&'a str> {
    let found: bool = c
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM shared_models WHERE id=?1)",
            [id],
            |r| r.get(0),
        )
        .wb()?;
    Ok(if found { SCOPE } else { ws })
}
pub(super) fn model_kind(kind: &str) -> bool {
    matches!(kind, "publication_llm_config" | "publication_tts_config")
}
pub(super) fn raw(
    c: &Connection,
    id: &str,
    revision: u64,
    kind: &str,
) -> Result<Option<(Value, u64, String)>> {
    let row: Option<(String,u64,String,String)> = c.query_row("SELECT m.kind,v.revision,v.sha256,v.body FROM shared_models m JOIN shared_model_versions v ON v.model_id=m.id AND v.revision=CASE WHEN ?2=0 THEN m.revision ELSE ?2 END WHERE m.id=?1",params![id,int(revision)?],|r|Ok((r.get(0)?,uint(r,1)?,r.get(2)?,r.get(3)?))).optional().wb()?;
    row.map(|(k, r, s, b)| {
        if k != kind || hash(b.as_bytes()) != s {
            return Err(Error::IntegrityFailed);
        }
        Ok((nai_runtime_core::publication::artifact(&b)?, r, s))
    })
    .transpose()
}
pub(super) fn models(c: &Connection, kind: &str) -> Result<Vec<(String, Value, u64)>> {
    let mut q = c
        .prepare("SELECT id FROM shared_models WHERE kind=?1 ORDER BY id LIMIT 65")
        .wb()?;
    let ids = q
        .query_map([kind], |r| r.get::<_, String>(0))
        .wb()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .wb()?;
    if ids.len() > 64 {
        return Err(Error::ResourceExhausted);
    }
    let mut out = vec![];
    for id in ids {
        let (v, r, _) = raw(c, &id, 0, kind)?.ok_or(Error::IntegrityFailed)?;
        if v["removed"] != true {
            out.push((id, v, r));
        }
    }
    Ok(out)
}
pub(super) fn put(
    tx: &Transaction<'_>,
    id: &str,
    expected: u64,
    v: &Value,
) -> Result<(u64, String)> {
    let kind = v["kind"].as_str().ok_or(Error::InvalidArgument)?;
    let current: Option<u64> = tx
        .query_row(
            "SELECT revision FROM shared_models WHERE id=?1",
            [id],
            |r| uint(r, 0),
        )
        .optional()
        .wb()?;
    if current.unwrap_or(0) != expected {
        return Err(Error::RevisionConflict);
    }
    if current.is_none() {
        let legacy: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM objects WHERE id=?1)",
                [id],
                |r| r.get(0),
            )
            .wb()?;
        if legacy {
            return Err(Error::RevisionConflict);
        }
    }
    if let Some((_, _, _)) = raw(tx, id, 0, kind)? {
    } else if expected > 0 {
        return Err(Error::NotFound);
    }
    let b = v.to_string();
    nai_runtime_core::publication::artifact(&b)?;
    if b.len() > 8192 {
        return Err(Error::ResourceExhausted);
    }
    let sha = hash(b.as_bytes());
    tx.execute("INSERT INTO shared_models VALUES(?1,1,?2) ON CONFLICT(id) DO UPDATE SET revision=revision+1",params![id,kind]).wb()?;
    tx.execute(
        "INSERT INTO shared_model_versions VALUES(?1,?2,?3,?4)",
        params![id, int(expected + 1)?, sha, b],
    )
    .wb()?;
    Ok((expected + 1, sha))
}
fn drained(c: &Connection) -> Result<()> {
    let busy:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE stopped=0) OR EXISTS(SELECT 1 FROM outbox WHERE state IN('pending','dispatching'))",[],|r|r.get(0)).wb()?;
    if busy { Err(Error::Busy) } else { Ok(()) }
}
impl Store {
    pub(super) fn cancel_shared(
        &mut self,
        op: &Operation,
        actor: &str,
        x: &FoundationLlmTaskRead,
    ) -> Result<FoundationResult> {
        if actor != self.root_owner {
            return Err(Error::PermissionDenied);
        }
        if !x.workspace_id.is_empty() {
            return Err(Error::InvalidArgument);
        }
        uuid(&x.task_id)?;
        let tx = self.conn.transaction().wb()?;
        let stage: String = tx
            .query_row(
                "SELECT stage FROM shared_model_migration WHERE id=?1",
                [&x.task_id],
                |r| r.get(0),
            )
            .optional()
            .wb()?
            .ok_or(Error::NotFound)?;
        if stage == "committed" {
            return Err(Error::RevisionConflict);
        }
        if stage != "aborted" {
            tx.execute(
                "UPDATE shared_model_migration SET stage='aborted' WHERE id=?1",
                [&x.task_id],
            )
            .wb()?;
            tx.execute(
                "UPDATE shared_model_staging SET body=NULL WHERE migration_id=?1",
                [&x.task_id],
            )
            .wb()?;
            audit(&tx, "", actor, "shared_migration_abort", &x.task_id, utc()?)?;
        }
        op.check()?;
        tx.commit().wb()?;
        self.shared_status()
    }
    pub(super) fn shared_status(&self) -> Result<FoundationResult> {
        let status:Option<SharedModelMigrationStatus>=self.conn.query_row("SELECT id,stage,backup_id,backup_manifest_sha256,(SELECT count(*) FROM shared_model_staging WHERE migration_id=m.id AND body IS NOT NULL),(SELECT count(*) FROM shared_model_staging WHERE migration_id=m.id)  ,request FROM shared_model_migration m WHERE stage!='aborted' LIMIT 1",[],|r|Ok(SharedModelMigrationStatus {migration_id:r.get(0)?,stage:r.get(1)?,backup_id:r.get(2)?,backup_manifest_sha256:r.get(3)?,completed:r.get(4)?,total:r.get(5)?,request:Some(SharedModelMigration::decode(r.get::<_,Vec<u8>>(6)?.as_slice()).map_err(|_|rusqlite::Error::InvalidQuery)?)})).optional().wb()?;
        Ok(FoundationResult {
            shared_catalog_enabled: enabled(&self.conn)?,
            shared_model_migration: status,
            ..Default::default()
        })
    }
    pub(super) fn migrate_shared(
        &mut self,
        op: &Operation,
        actor: &str,
        request: &SharedModelMigration,
    ) -> Result<FoundationResult> {
        if actor != self.root_owner {
            return Err(Error::PermissionDenied);
        }
        uuid(&request.migration_id)?;

        let bytes = request.encode_to_vec();
        let existing:Option<(String,Vec<u8>,String,String,u32)>=self.conn.query_row("SELECT id,request,stage,backup_id,backup_attempts FROM shared_model_migration WHERE stage!='aborted' LIMIT 1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional().wb()?;
        if let Some((id, b, stage, _, _)) = &existing {
            if id != &request.migration_id || b != &bytes {
                return Err(Error::RevisionConflict);
            }
            if stage == "committed" {
                return self.shared_status();
            }
        }
        drained(&self.conn)?;
        let preview = shared_model_preview::preview(&self.conn, &self.root, op)?
            .shared_model_preview
            .ok_or(Error::IntegrityFailed)?;
        if preview.plan_sha256 != request.plan_sha256 || preview.rows.len() != request.entries.len()
        {
            return Err(Error::RevisionConflict);
        }
        let mut ids = BTreeSet::new();
        let mut counts = [0; 2];
        for entry in &request.entries {
            uuid(&entry.catalog_model_id)?;
            if !ids.insert(&entry.catalog_model_id) {
                return Err(Error::InvalidArgument);
            }
            let source = preview
                .rows
                .iter()
                .filter_map(|r| r.source.as_ref())
                .find(|s| {
                    s.source_workspace_id == entry.source_workspace_id
                        && s.source_model_id == entry.source_model_id
                })
                .ok_or(Error::InvalidArgument)?;
            let mut expected = source.clone();
            expected.catalog_model_id = entry.catalog_model_id.clone();
            if &expected != entry {
                return Err(Error::RevisionConflict);
            }
            let collision: bool = self
                .conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM objects WHERE id=?1)",
                    [&entry.catalog_model_id],
                    |r| r.get(0),
                )
                .wb()?;
            if collision {
                return Err(Error::InvalidArgument);
            }
            counts[usize::from(entry.capability == "tts")] += 1;
        }
        if counts.iter().any(|n| *n > 8) {
            return Err(Error::ResourceExhausted);
        }
        // Complete source coverage: duplicate source mappings cannot replace another row.
        let sources = request
            .entries
            .iter()
            .map(|e| (&e.source_workspace_id, &e.source_model_id))
            .collect::<BTreeSet<_>>();
        if sources.len() != preview.rows.len() {
            return Err(Error::InvalidArgument);
        }
        if existing.is_none() {
            let used: bool = self
                .conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM shared_model_migration WHERE id=?1)",
                    [&request.migration_id],
                    |r| r.get(0),
                )
                .wb()?;
            if used {
                return Err(Error::RevisionConflict);
            }
            let tx = self.conn.transaction().wb()?;
            tx.execute("INSERT INTO shared_model_migration(id,request,plan_sha256,stage) VALUES(?1,?2,?3,'preparing')",params![request.migration_id,bytes,request.plan_sha256]).wb()?;
            for e in &request.entries {
                tx.execute("INSERT INTO shared_model_staging(migration_id,model_id,capability) VALUES(?1,?2,?3)",params![request.migration_id,e.catalog_model_id,e.capability]).wb()?;
            }
            audit(
                &tx,
                "",
                actor,
                "shared_migration_prepare",
                &request.migration_id,
                utc()?,
            )?;
            op.check()?;
            tx.commit().wb()?;
            return self.shared_status();
        }
        let (_, _, stage, backup_id, attempts) = existing.ok_or(Error::IntegrityFailed)?;
        if stage == "preparing" {
            let path = self
                .root
                .join("model-migration-backups")
                .join(&backup_id)
                .join("manifest.json");
            root::no_links(&path)?;
            let receipt = if !backup_id.is_empty() && path.exists() {
                let manifest = root::read(&path, 1024 * 1024)?;
                ModelMigrationBackup {
                    backup_id,
                    plan_sha256: request.plan_sha256.clone(),
                    manifest_sha256: hash(&manifest),
                }
            } else {
                if attempts >= 8 {
                    return Err(Error::ResourceExhausted);
                }
                if !backup_id.is_empty() {
                    migration_backup::quarantine_incomplete(&self.root, &backup_id, op)?;
                }
                let h = hash(format!("{}:{}", request.migration_id, attempts).as_bytes());
                let id = format!(
                    "{}-{}-4{}-8{}-{}",
                    &h[..8],
                    &h[8..12],
                    &h[13..16],
                    &h[17..20],
                    &h[20..32]
                );
                self.conn.execute("UPDATE shared_model_migration SET backup_id=?1,backup_attempts=backup_attempts+1 WHERE id=?2",params![id,request.migration_id]).wb()?;
                self.prepare_model_migration_backup(op, actor, &id, &request.plan_sha256)?
            };
            self.verify_model_migration_backup(op, actor, &receipt)?;
            op.check()?;
            self.conn.execute("UPDATE shared_model_migration SET stage='rewrapping',backup_manifest_sha256=?1 WHERE id=?2",params![receipt.manifest_sha256,request.migration_id]).wb()?;
            return self.shared_status();
        }
        let next:Option<(String,u32)>=self.conn.query_row("SELECT model_id,attempts FROM shared_model_staging WHERE migration_id=?1 AND body IS NULL ORDER BY model_id LIMIT 1",[&request.migration_id],|r|Ok((r.get(0)?,r.get(1)?))).optional().wb()?;
        if let Some((id, attempts)) = next {
            if attempts >= 3 {
                return Err(Error::ResourceExhausted);
            }
            let e = request
                .entries
                .iter()
                .find(|e| e.catalog_model_id == id)
                .ok_or(Error::IntegrityFailed)?;
            let kind = if e.capability == "llm" {
                "publication_llm_config"
            } else {
                "publication_tts_config"
            };
            let (mut v, _, _) = llm::raw(
                &self.conn,
                &self.root,
                &e.source_workspace_id,
                &e.source_model_id,
                e.source_revision,
                kind,
            )?;
            self.conn.execute("UPDATE shared_model_staging SET attempts=attempts+1 WHERE migration_id=?1 AND model_id=?2",params![request.migration_id,id]).wb()?;
            if !v["secret"].is_null() {
                let vault = if e.capability == "llm" {
                    self.llm_vault.as_deref()
                } else {
                    self.tts_vault.as_deref()
                }
                .ok_or(Error::CapabilityUnavailable)?;
                let url = v["base_url"].as_str().ok_or(Error::IntegrityFailed)?;
                use nai_runtime_core::credential_rewrap::{Identity, rewrap};
                v["secret"] = rewrap(
                    op,
                    vault,
                    vault,
                    Identity {
                        workspace: &e.source_workspace_id,
                        model: &e.source_model_id,
                        endpoint: url,
                    },
                    Identity {
                        workspace: SCOPE,
                        model: &id,
                        endpoint: url,
                    },
                    &v["secret"],
                )?;
            }
            nai_runtime_core::publication::artifact(&v.to_string())?;
            op.check()?;
            self.conn
                .execute(
                    "UPDATE shared_model_staging SET body=?1 WHERE migration_id=?2 AND model_id=?3",
                    params![v.to_string(), request.migration_id, id],
                )
                .wb()?;
            return self.shared_status();
        }
        let receipt:ModelMigrationBackup=self.conn.query_row("SELECT backup_id,plan_sha256,backup_manifest_sha256 FROM shared_model_migration WHERE id=?1",[&request.migration_id],|r|Ok(ModelMigrationBackup {backup_id:r.get(0)?,plan_sha256:r.get(1)?,manifest_sha256:r.get(2)?})).wb()?;
        self.verify_model_migration_backup(op, actor, &receipt)?;
        let tx = self.conn.transaction().wb()?;
        for e in &request.entries {
            let b: String = tx
                .query_row(
                    "SELECT body FROM shared_model_staging WHERE migration_id=?1 AND model_id=?2",
                    params![request.migration_id, e.catalog_model_id],
                    |r| r.get(0),
                )
                .wb()?;
            let v = nai_runtime_core::publication::artifact(&b)?;
            put(&tx, &e.catalog_model_id, 0, &v)?;
        }
        tx.execute(
            "UPDATE shared_model_migration SET stage='committed' WHERE id=?1",
            [&request.migration_id],
        )
        .wb()?;
        audit(
            &tx,
            "",
            actor,
            "shared_migration_commit",
            &request.migration_id,
            utc()?,
        )?;
        op.check()?;
        tx.commit().wb()?;
        self.shared_status()
    }
}
