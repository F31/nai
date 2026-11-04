use super::*;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelMigrationBackup {
    pub backup_id: String,
    pub plan_sha256: String,
    pub manifest_sha256: String,
}
fn directory(path: &Path) -> Result<()> {
    root::no_links(path)?;
    fs::create_dir(path).map_err(|_| Error::RevisionConflict)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|_| Error::IoFailed)?;
    }
    Ok(())
}
fn parent(path: &Path) -> Result<()> {
    root::no_links(path)?;
    if !path.exists() {
        directory(path)?;
    }
    Ok(())
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    root::no_links(path)?;
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| Error::RevisionConflict)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        f.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| Error::IoFailed)?;
    }
    f.write_all(bytes).map_err(|_| Error::IoFailed)?;
    f.sync_all().map_err(|_| Error::IoFailed)
}
fn limit(name: &str) -> u64 {
    if name == "foundation.sqlite" {
        64 * 1024 * 1024
    } else if name.starts_with("objects/") {
        8192
    } else if name.starts_with("avatar-packages/") || name.starts_with("avatar-photos/") {
        16 * 1024 * 1024
    } else if name.starts_with("avatar-videos/") {
        32 * 1024 * 1024
    } else {
        4096
    }
}
fn copy(
    source: &Path,
    dest: &Path,
    name: &str,
    files: &mut BTreeMap<String, Value>,
    op: &Operation,
) -> Result<()> {
    op.check()?;
    let bytes = root::read(&source.join(name), limit(name))?;
    if let Some((folder, _)) = name.rsplit_once('/') {
        let mut path = dest.to_path_buf();
        for part in folder.split('/') {
            path.push(part);
            parent(&path)?;
        }
    }
    write_new(&dest.join(name), &bytes)?;
    files.insert(
        name.into(),
        json!({"sha256":hash(&bytes),"bytes":bytes.len()}),
    );
    Ok(())
}

/// Copy an already-open SQLite owner to an isolated destination. This only
/// snapshots the database; callers must separately back up referenced files,
/// verify the manifest and journal activation/recovery before migration.
pub(super) fn snapshot_database(
    source: &Connection,
    dest: &mut Connection,
    op: &Operation,
) -> Result<()> {
    op.check()?;
    let check = op.clone();
    dest.progress_handler(1000, Some(move || check.check().is_err()))
        .wb()?;
    {
        let backup = rusqlite::backup::Backup::new(source, dest).wb()?;
        loop {
            op.check()?;
            match backup.step(128).wb()? {
                rusqlite::backup::StepResult::Done => break,
                rusqlite::backup::StepResult::More => (),
                _ => return Err(Error::Busy),
            }
        }
    }
    let integrity: String = dest
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .wb()?;
    if integrity != "ok" {
        return Err(Error::IntegrityFailed);
    }
    op.check()
}
pub(super) fn snapshot_object_digests(
    snapshot: &Connection,
    op: &Operation,
) -> Result<Vec<String>> {
    op.check()?;
    let mut query = snapshot.prepare("SELECT DISTINCT v.sha256 FROM object_versions v JOIN objects o ON o.workspace_id=v.workspace_id AND o.id=v.object_id WHERE o.deleted=0 ORDER BY v.sha256 LIMIT 3841").wb()?;
    let digests = query
        .query_map([], |row| row.get::<_, String>(0))
        .wb()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .wb()?;
    if digests.len() > 3840 {
        return Err(Error::ResourceExhausted);
    }
    op.check()?;
    Ok(digests)
}

/// Preserve only the journal-referenced incomplete attempt. The owner never
/// deletes backup content; missing source after a completed move is idempotent.
pub(super) fn quarantine_incomplete(
    root_path: &Path,
    backup_id: &str,
    op: &Operation,
) -> Result<()> {
    op.check()?;
    uuid(backup_id)?;
    let source = root_path.join("model-migration-backups").join(backup_id);
    let quarantine = root_path.join("model-migration-incomplete");
    let destination = quarantine.join(backup_id);
    root::no_links(&source)?;
    root::no_links(&destination)?;
    if !source.exists() {
        return Ok(());
    }
    if !source.is_dir() || source.join("manifest.json").exists() {
        return Err(Error::RevisionConflict);
    }
    parent(&quarantine)?;
    if destination.exists() {
        return Err(Error::RevisionConflict);
    }
    if fs::read_dir(&quarantine)
        .map_err(|_| Error::IoFailed)?
        .take(65)
        .count()
        >= 64
    {
        return Err(Error::ResourceExhausted);
    }
    op.check()?;
    fs::rename(&source, &destination).map_err(|_| Error::IoFailed)?;
    op.check()
}

impl Store {
    /// Internal maintenance primitive. No page-supplied paths, no active rollback.
    pub fn prepare_model_migration_backup(
        &mut self,
        op: &Operation,
        actor: &str,
        id: &str,
        plan: &str,
    ) -> Result<ModelMigrationBackup> {
        self.begin(op)?;
        if actor != self.root_owner {
            return Err(Error::PermissionDenied);
        }
        uuid(id)?;
        let p = shared_model_preview::preview(&self.conn, &self.root, op)?
            .shared_model_preview
            .ok_or(Error::IntegrityFailed)?;
        if p.plan_sha256 != plan {
            return Err(Error::RevisionConflict);
        }
        let busy:bool=self.conn.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE stopped=0) OR EXISTS(SELECT 1 FROM outbox WHERE state IN('pending','dispatching'))",[],|r|r.get(0)).wb()?;
        if busy {
            return Err(Error::Busy);
        }
        // Fail before allocating a backup directory when referenced source
        // packages are absent or damaged.
        let preflight_avatars = avatar_backup::inventory(&self.conn, &self.root, op)?;
        avatar_backup::verify_packages(&self.root, &preflight_avatars, op)?;
        let preflight_photos = avatar_backup::photo_inventory(&self.conn, &self.root, op)?;
        avatar_backup::verify_photos(&self.root, &preflight_photos, op)?;
        let preflight_videos = avatar_backup::video_inventory(&self.conn, &self.root, op)?;
        avatar_backup::verify_videos(&self.root, &preflight_videos, op)?;
        let base = self.root.join("model-migration-backups");
        parent(&base)?;
        if fs::read_dir(&base)
            .map_err(|_| Error::IoFailed)?
            .take(9)
            .count()
            >= 8
        {
            return Err(Error::ResourceExhausted);
        }
        let dest = base.join(id);
        directory(&dest)?;
        let db = dest.join("foundation.sqlite");
        let mut out = Connection::open(&db).wb()?;
        snapshot_database(&self.conn, &mut out, op)?;
        // Deleted object bodies are removed by the storage lifecycle. Copy
        // every live object version from the snapshot, never the live view.
        let digests = snapshot_object_digests(&out, op)?;
        let avatars = avatar_backup::inventory(&out, &self.root, op)?;
        if avatars != preflight_avatars {
            return Err(Error::RevisionConflict);
        }
        let photos = avatar_backup::photo_inventory(&out, &self.root, op)?;
        if photos != preflight_photos {
            return Err(Error::RevisionConflict);
        }
        let videos = avatar_backup::video_inventory(&out, &self.root, op)?;
        if videos != preflight_videos {
            return Err(Error::RevisionConflict);
        }
        drop(out);
        let f = OpenOptions::new()
            .write(true)
            .open(&db)
            .map_err(|_| Error::IoFailed)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            f.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|_| Error::IoFailed)?;
        }
        f.sync_all().map_err(|_| Error::IoFailed)?;
        let bytes = root::read(&db, 64 * 1024 * 1024)?;
        let mut files = BTreeMap::new();
        files.insert(
            "foundation.sqlite".into(),
            json!({"sha256":hash(&bytes),"bytes":bytes.len()}),
        );
        for name in ["foundation-root.json", "initialized.ok"] {
            copy(&self.root, &dest, name, &mut files, op)?;
        }
        for digest in digests {
            body(&self.root, &digest)?;
            copy(
                &self.root,
                &dest,
                &format!("objects/{digest}"),
                &mut files,
                op,
            )?;
        }
        let revocations = self.root.join("revocations");
        root::no_links(&revocations)?;
        for (i, entry) in fs::read_dir(&revocations)
            .map_err(|_| Error::IoFailed)?
            .enumerate()
        {
            if i >= 256 {
                return Err(Error::ResourceExhausted);
            }
            let entry = entry.map_err(|_| Error::IoFailed)?;
            let name = entry
                .file_name()
                .to_str()
                .ok_or(Error::IntegrityFailed)?
                .to_owned();
            if name.len() != 64 || !name.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err(Error::IntegrityFailed);
            }
            copy(
                &self.root,
                &dest,
                &format!("revocations/{name}"),
                &mut files,
                op,
            )?;
        }
        if !avatars.is_empty() {
            avatar_backup::copy_inventory(&self.root, &dest, &avatars, op)?;
            for package in avatars.keys() {
                for name in [
                    "manifest.json",
                    "config.json",
                    "LICENSE.txt",
                    "asset-ids.json",
                    "asset-0.rgba",
                    "asset-1.rgba",
                    "asset-2.rgba",
                    "preview.png",
                ] {
                    op.check()?;
                    let path = format!("avatar-packages/{package}/{name}");
                    let bytes = root::read(&dest.join(&path), limit(&path))?;
                    files.insert(path, json!({"sha256":hash(&bytes),"bytes":bytes.len()}));
                }
            }
        }
        if !photos.is_empty() {
            avatar_backup::copy_photos(&self.root, &dest, &photos, op)?;
            for digest in photos.keys() {
                op.check()?;
                let path = format!("avatar-photos/{digest}.jpg");
                let bytes = root::read(&dest.join(&path), limit(&path))?;
                files.insert(path, json!({"sha256":hash(&bytes),"bytes":bytes.len()}));
            }
        }
        if !videos.is_empty() {
            avatar_backup::copy_videos(&self.root, &dest, &videos, op)?;
            for digest in videos.keys() {
                op.check()?;
                let path = format!("avatar-videos/{digest}.mp4");
                let bytes = root::read(&dest.join(&path), limit(&path))?;
                files.insert(path, json!({"sha256":hash(&bytes),"bytes":bytes.len()}));
            }
        }
        let manifest=serde_json::to_vec(&json!({"schema_version":1,"backup_id":id,"plan_sha256":plan,"credential_keys":"external_keyring_not_exported","files":files})).map_err(|_|Error::IoFailed)?;
        nai_runtime_core::migration_backup::manifest(&manifest)?;
        op.check()?;
        write_new(&dest.join("manifest.json"), &manifest)?;
        let receipt = ModelMigrationBackup {
            backup_id: id.into(),
            plan_sha256: plan.into(),
            manifest_sha256: hash(&manifest),
        };
        self.verify_model_migration_backup(op, actor, &receipt)?;
        Ok(receipt)
    }
    pub fn verify_model_migration_backup(
        &self,
        op: &Operation,
        actor: &str,
        receipt: &ModelMigrationBackup,
    ) -> Result<Value> {
        op.check()?;
        if actor != self.root_owner {
            return Err(Error::PermissionDenied);
        }
        uuid(&receipt.backup_id)?;
        let dest = self
            .root
            .join("model-migration-backups")
            .join(&receipt.backup_id);
        let bytes = root::read(&dest.join("manifest.json"), 1024 * 1024)?;
        if hash(&bytes) != receipt.manifest_sha256 {
            return Err(Error::IntegrityFailed);
        }
        let manifest = nai_runtime_core::migration_backup::manifest(&bytes)?;
        if manifest["backup_id"] != receipt.backup_id
            || manifest["plan_sha256"] != receipt.plan_sha256
        {
            return Err(Error::IntegrityFailed);
        }
        for (name, entry) in manifest["files"]
            .as_object()
            .ok_or(Error::IntegrityFailed)?
        {
            op.check()?;
            let bytes = root::read(&dest.join(name), limit(name))?;
            if hash(&bytes) != entry["sha256"]
                || Some(bytes.len() as u64) != entry["bytes"].as_u64()
            {
                return Err(Error::IntegrityFailed);
            }
        }
        // Bind the package closure to this checked snapshot and refuse stale
        // receipts after the current set of avatar identities has changed.
        let db = dest.join("foundation.sqlite");
        root::no_links(&db)?;
        let snapshot =
            Connection::open_with_flags(&db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).wb()?;
        let avatars = avatar_backup::inventory(&snapshot, &dest, op)?;
        if avatars != avatar_backup::inventory(&self.conn, &self.root, op)? {
            return Err(Error::RevisionConflict);
        }
        let manifest_files = manifest["files"]
            .as_object()
            .ok_or(Error::IntegrityFailed)?;
        let avatar_count = manifest_files
            .keys()
            .filter(|name| name.starts_with("avatar-packages/"))
            .count();
        if avatar_count
            != avatars
                .len()
                .checked_mul(8)
                .ok_or(Error::ResourceExhausted)?
        {
            return Err(Error::IntegrityFailed);
        }
        for package in avatars.keys() {
            for name in [
                "manifest.json",
                "config.json",
                "LICENSE.txt",
                "asset-ids.json",
                "asset-0.rgba",
                "asset-1.rgba",
                "asset-2.rgba",
                "preview.png",
            ] {
                if !manifest_files.contains_key(&format!("avatar-packages/{package}/{name}")) {
                    return Err(Error::IntegrityFailed);
                }
            }
        }
        avatar_backup::verify_packages(&dest, &avatars, op)?;
        let photos = avatar_backup::photo_inventory(&snapshot, &dest, op)?;
        if photos != avatar_backup::photo_inventory(&self.conn, &self.root, op)? {
            return Err(Error::RevisionConflict);
        }
        if manifest_files
            .keys()
            .filter(|name| name.starts_with("avatar-photos/"))
            .count()
            != photos.len()
        {
            return Err(Error::IntegrityFailed);
        }
        for digest in photos.keys() {
            if !manifest_files.contains_key(&format!("avatar-photos/{digest}.jpg")) {
                return Err(Error::IntegrityFailed);
            }
        }
        avatar_backup::verify_photos(&dest, &photos, op)?;
        let videos = avatar_backup::video_inventory(&snapshot, &dest, op)?;
        if videos != avatar_backup::video_inventory(&self.conn, &self.root, op)? {
            return Err(Error::RevisionConflict);
        }
        if manifest_files
            .keys()
            .filter(|name| name.starts_with("avatar-videos/"))
            .count()
            != videos.len()
        {
            return Err(Error::IntegrityFailed);
        }
        for digest in videos.keys() {
            if !manifest_files.contains_key(&format!("avatar-videos/{digest}.mp4")) {
                return Err(Error::IntegrityFailed);
            }
        }
        avatar_backup::verify_videos(&dest, &videos, op)?;
        let verified = dest.join("verified.json");
        let marker = serde_json::to_vec(
            &json!({"manifest_sha256":receipt.manifest_sha256,"state":"verified"}),
        )
        .map_err(|_| Error::IoFailed)?;
        if verified.exists() {
            if root::read(&verified, 4096)? != marker {
                return Err(Error::IntegrityFailed);
            }
        } else {
            write_new(&verified, &marker)?;
        }
        Ok(manifest)
    }
    /// Restores only to a new owned verification root; never overwrites live data.
    pub fn restore_model_backup_for_verification(
        &self,
        op: &Operation,
        actor: &str,
        receipt: &ModelMigrationBackup,
    ) -> Result<PathBuf> {
        let manifest = self.verify_model_migration_backup(op, actor, receipt)?;
        let base = self.root.join("model-migration-verification");
        parent(&base)?;
        let dest = base.join(&receipt.backup_id);
        directory(&dest)?;
        let source = self
            .root
            .join("model-migration-backups")
            .join(&receipt.backup_id);
        let mut files = BTreeMap::new();
        for name in manifest["files"]
            .as_object()
            .ok_or(Error::IntegrityFailed)?
            .keys()
        {
            copy(&source, &dest, name, &mut files, op)?;
        }
        if serde_json::to_value(files).map_err(|_| Error::IoFailed)? != manifest["files"] {
            return Err(Error::IntegrityFailed);
        }
        op.check()?;
        Ok(dest)
    }
}
