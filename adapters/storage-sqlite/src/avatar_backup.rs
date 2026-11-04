//! Candidate backup closure. Reads only; it is not a migration or media grant.
use super::*;
use std::collections::BTreeMap;

const MAX_OBJECT_VERSIONS: usize = 3840;
const MAX_PACKAGES: usize = 1024;

fn valid_digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn add(result: &mut BTreeMap<String, String>, package: &str, rights: &str) -> Result<()> {
    if !valid_digest(package) || !valid_digest(rights) {
        return Err(Error::IntegrityFailed);
    }
    if let Some(previous) = result.get(package) {
        if previous != rights {
            return Err(Error::IntegrityFailed);
        }
    } else {
        if result.len() >= MAX_PACKAGES {
            return Err(Error::ResourceExhausted);
        }
        result.insert(package.to_owned(), rights.to_owned());
    }
    Ok(())
}

/// The caller must use a consistent database snapshot and separately verify
/// and copy each returned package from an owned source to an isolated backup.
pub(super) fn inventory(
    c: &Connection,
    root: &Path,
    op: &Operation,
) -> Result<BTreeMap<String, String>> {
    op.check()?;
    let has_authority = workspace_purge::avatar_schema_ready(c)?;
    let mut result = BTreeMap::new();
    let mut query = c.prepare("SELECT DISTINCT v.sha256 FROM object_versions v JOIN objects o ON o.workspace_id=v.workspace_id AND o.id=v.object_id WHERE o.deleted=0 ORDER BY v.sha256 LIMIT 3841").wb()?;
    let hashes = query
        .query_map([], |r| r.get::<_, String>(0))
        .wb()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .wb()?;
    if hashes.len() > MAX_OBJECT_VERSIONS {
        return Err(Error::ResourceExhausted);
    }
    for hash in hashes {
        op.check()?;
        let text = body(root, &hash)?;
        let Ok(doc) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        if doc["kind"] == "avatar_profile_draft" {
            let package = doc["package_sha256"]
                .as_str()
                .ok_or(Error::IntegrityFailed)?;
            let rights = doc["license_sha256"]
                .as_str()
                .ok_or(Error::IntegrityFailed)?;
            add(&mut result, package, rights)?;
        }
    }
    if has_authority {
        for table in ["avatar_bindings", "avatar_rights_revocations"] {
            let mut q = c.prepare(&format!("SELECT package_sha256,rights_sha256 FROM {table} ORDER BY package_sha256,rights_sha256 LIMIT 1025")).wb()?;
            let rows = q
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                .wb()?
                .collect::<rusqlite::Result<Vec<_>>>()
                .wb()?;
            if rows.len() > MAX_PACKAGES {
                return Err(Error::ResourceExhausted);
            }
            for (package, rights) in rows {
                op.check()?;
                add(&mut result, &package, &rights)?;
            }
        }
    }
    op.check()?;
    Ok(result)
}

/// Enumerate live photo draft versions from the same snapshot, including
/// historical revisions. This is metadata only, not an ownership grant.
pub(super) fn photo_inventory(
    c: &Connection,
    root: &Path,
    op: &Operation,
) -> Result<BTreeMap<String, (u32, u32)>> {
    op.check()?;
    let mut result = BTreeMap::new();
    let mut q=c.prepare("SELECT DISTINCT v.sha256 FROM object_versions v JOIN objects o ON o.workspace_id=v.workspace_id AND o.id=v.object_id WHERE o.deleted=0 ORDER BY v.sha256 LIMIT 3841").wb()?;
    let rows = q
        .query_map([], |r| r.get::<_, String>(0))
        .wb()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .wb()?;
    if rows.len() > MAX_OBJECT_VERSIONS {
        return Err(Error::ResourceExhausted);
    }
    for hash in rows {
        op.check()?;
        let text = body(root, &hash)?;
        let Ok(doc) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        if doc["kind"] == "avatar_photo_draft" {
            let digest = doc["photo_sha256"].as_str().ok_or(Error::IntegrityFailed)?;
            let width = u32::try_from(doc["width"].as_u64().ok_or(Error::IntegrityFailed)?)
                .map_err(|_| Error::IntegrityFailed)?;
            let height = u32::try_from(doc["height"].as_u64().ok_or(Error::IntegrityFailed)?)
                .map_err(|_| Error::IntegrityFailed)?;
            if !valid_digest(digest) || width == 0 || height == 0 || width > 4096 || height > 4096 {
                return Err(Error::IntegrityFailed);
            }
            if let Some(old) = result.insert(digest.to_owned(), (width, height))
                && old != (width, height)
            {
                return Err(Error::IntegrityFailed);
            }
            if result.len() > MAX_PACKAGES {
                return Err(Error::ResourceExhausted);
            }
        }
    }
    op.check()?;
    Ok(result)
}
pub(super) fn verify_photos(
    root: &Path,
    photos: &BTreeMap<String, (u32, u32)>,
    op: &Operation,
) -> Result<()> {
    for (digest, (width, height)) in photos {
        op.check()?;
        let (_, m) = nai_avatar_files::photo::read_stored_photo(root, digest)
            .map_err(|_| Error::IntegrityFailed)?;
        if (m.width, m.height) != (*width, *height) {
            return Err(Error::IntegrityFailed);
        }
        op.check()?;
    }
    op.check()
}
pub(super) fn copy_photos(
    source: &Path,
    dest: &Path,
    photos: &BTreeMap<String, (u32, u32)>,
    op: &Operation,
) -> Result<()> {
    verify_photos(source, photos, op)?;
    let digests = photos.keys().cloned().collect::<Vec<_>>();
    let result = nai_avatar_files::photo::copy_stored_photos(source, dest, &digests, || {
        op.check().map_err(|_| "cancelled".to_owned())
    });
    op.check()?;
    result.map_err(|_| Error::IntegrityFailed)?;
    verify_photos(dest, photos, op)
}

/// Enumerate live video draft versions from the same snapshot, including
/// historical revisions. This is metadata only, not an ownership grant.
pub(super) fn video_inventory(
    c: &Connection,
    root: &Path,
    op: &Operation,
) -> Result<BTreeMap<String, u64>> {
    op.check()?;
    let mut result = BTreeMap::new();
    let mut q=c.prepare("SELECT DISTINCT v.sha256 FROM object_versions v JOIN objects o ON o.workspace_id=v.workspace_id AND o.id=v.object_id WHERE o.deleted=0 ORDER BY v.sha256 LIMIT 3841").wb()?;
    let rows = q
        .query_map([], |r| r.get::<_, String>(0))
        .wb()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .wb()?;
    if rows.len() > MAX_OBJECT_VERSIONS {
        return Err(Error::ResourceExhausted);
    }
    for hash in rows {
        op.check()?;
        let text = body(root, &hash)?;
        let Ok(doc) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        if doc["kind"] == "avatar_video_draft" {
            let digest = doc["video_sha256"].as_str().ok_or(Error::IntegrityFailed)?;
            let duration = doc["duration_ms"].as_u64().ok_or(Error::IntegrityFailed)?;
            if !valid_digest(digest)
                || !(nai_avatar_files::video::MIN_DURATION_MS
                    ..=nai_avatar_files::video::MAX_DURATION_MS)
                    .contains(&duration)
            {
                return Err(Error::IntegrityFailed);
            }
            if let Some(old) = result.insert(digest.to_owned(), duration)
                && old != duration
            {
                return Err(Error::IntegrityFailed);
            }
            if result.len() > MAX_PACKAGES {
                return Err(Error::ResourceExhausted);
            }
        }
    }
    op.check()?;
    Ok(result)
}
pub(super) fn verify_videos(
    root: &Path,
    videos: &BTreeMap<String, u64>,
    op: &Operation,
) -> Result<()> {
    for (digest, duration) in videos {
        op.check()?;
        let (_, m) = nai_avatar_files::video::read_stored_video(root, digest)
            .map_err(|_| Error::IntegrityFailed)?;
        if m.duration_ms != *duration {
            return Err(Error::IntegrityFailed);
        }
        op.check()?;
    }
    op.check()
}
pub(super) fn copy_videos(
    source: &Path,
    dest: &Path,
    videos: &BTreeMap<String, u64>,
    op: &Operation,
) -> Result<()> {
    verify_videos(source, videos, op)?;
    let digests = videos.keys().cloned().collect::<Vec<_>>();
    let result = nai_avatar_files::video::copy_stored_videos(source, dest, &digests, || {
        op.check().map_err(|_| "cancelled".to_owned())
    });
    op.check()?;
    result.map_err(|_| Error::IntegrityFailed)?;
    verify_videos(dest, videos, op)
}

/// Create a bounded SQLite snapshot and enumerate only that snapshot's closure.
/// The caller must own an isolated destination and verify/copy all files before
/// using this as a backup; neither this result nor the snapshot grants rights.
fn snapshot_inventory(
    source: &Connection,
    snapshot: &mut Connection,
    root: &Path,
    op: &Operation,
) -> Result<BTreeMap<String, String>> {
    migration_backup::snapshot_database(source, snapshot, op)?;
    inventory(snapshot, root, op)
}

/// Validate each installed package against the snapshot inventory. This
/// does not copy bytes or establish a backup or an authorization.
pub(super) fn verify_packages(
    root: &Path,
    packages: &BTreeMap<String, String>,
    op: &Operation,
) -> Result<()> {
    if packages.len() > MAX_PACKAGES {
        return Err(Error::ResourceExhausted);
    }
    for (package, rights) in packages {
        op.check()?;
        nai_avatar_files::verify_installed(root, package, rights)
            .map_err(|_| Error::IntegrityFailed)?;
        op.check()?;
    }
    op.check()
}

/// Copy the exact snapshot inventory to a fresh isolated root and verify it.
/// The caller must discard failed destinations; this is not activation.
pub(super) fn copy_inventory(
    source: &Path,
    destination: &Path,
    packages: &BTreeMap<String, String>,
    op: &Operation,
) -> Result<()> {
    op.check()?;
    let entries: Vec<_> = packages
        .iter()
        .map(|(a, b)| (a.clone(), b.clone()))
        .collect();
    #[cfg(test)]
    let checks = std::cell::Cell::new(0usize);
    let copied = nai_avatar_files::copy_verified_batch(source, destination, &entries, || {
        #[cfg(test)]
        if let Some(root) = std::env::var_os("NAI_AVATAR_MIGRATION_COPY_PAUSE") {
            checks.set(checks.get() + 1);
            if checks.get() == 5 {
                std::fs::write(Path::new(&root).join("kill-test-ready"), b"copying")
                    .map_err(|_| "fault-test-signal".to_owned())?;
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
        }
        op.check().map_err(|_| "cancelled".to_owned())
    });
    op.check()?;
    copied.map_err(|_| Error::IntegrityFailed)?;
    verify_packages(destination, packages, op)
}

/// Compare metadata from a verified database snapshot with metadata from
/// independently verified copies. This cannot verify the files or issue a grant.
fn compare_copies(
    expected: &BTreeMap<String, String>,
    database_sha256: &str,
    manifest: &AvatarBackupInventory,
    op: &Operation,
) -> Result<()> {
    op.check()?;
    if !valid_digest(database_sha256)
        || manifest.database_sha256 != database_sha256
        || manifest.packages.len() != expected.len()
        || manifest.packages.len() > MAX_PACKAGES
    {
        return Err(Error::IntegrityFailed);
    }
    let mut observed = BTreeMap::new();
    for item in &manifest.packages {
        op.check()?;
        if !valid_digest(&item.package_sha256)
            || !valid_digest(&item.rights_sha256)
            || observed
                .insert(item.package_sha256.clone(), item.rights_sha256.clone())
                .is_some()
        {
            return Err(Error::IntegrityFailed);
        }
    }
    if &observed != expected {
        return Err(Error::IntegrityFailed);
    }
    op.check()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_inventory_package_verification_rejects_tampering_and_cancellation() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("objects")).unwrap();
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE object_versions(sha256 TEXT,workspace_id TEXT DEFAULT 'w',object_id TEXT DEFAULT 'o'); CREATE TABLE objects(workspace_id TEXT,id TEXT,deleted INTEGER); INSERT INTO objects VALUES('w','o',0); CREATE TABLE workspaces(id TEXT PRIMARY KEY);").unwrap();
        let manifest = b"manifest";
        let config = b"config";
        let license = b"license";
        let ids = ["base", "closed", "open"];
        let assets: [&[u8]; 3] = [b"one", b"two", b"three"];
        let mut h = sha2::Sha256::new();
        h.update(manifest);
        h.update(config);
        h.update(license);
        for (id, bytes) in ids.iter().zip(assets) {
            h.update(id.as_bytes());
            h.update(bytes);
        }
        let package = h
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let rights = hash(license);
        let dir = temp.path().join("avatar-packages").join(&package);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, bytes) in [
            ("manifest.json", manifest.as_slice()),
            ("config.json", config.as_slice()),
            ("LICENSE.txt", license.as_slice()),
        ] {
            std::fs::write(dir.join(name), bytes).unwrap();
        }
        std::fs::write(dir.join("preview.png"), b"preview").unwrap();
        std::fs::write(
            dir.join("asset-ids.json"),
            serde_json::to_vec(&ids).unwrap(),
        )
        .unwrap();
        for (i, bytes) in assets.iter().enumerate() {
            std::fs::write(dir.join(format!("asset-{i}.rgba")), bytes).unwrap();
        }
        let doc = serde_json::json!({"kind":"avatar_profile_draft","package_sha256":package,"license_sha256":rights}).to_string();
        let object_hash = store_body(temp.path(), doc.as_bytes()).unwrap();
        c.execute(
            "INSERT INTO object_versions(sha256) VALUES(?1)",
            [object_hash],
        )
        .unwrap();
        let op = Operation::new(4000).unwrap();
        let items = inventory(&c, temp.path(), &op).unwrap();
        assert_eq!(verify_packages(temp.path(), &items, &op), Ok(()));
        let backup = tempfile::tempdir().unwrap();
        assert_eq!(
            copy_inventory(temp.path(), backup.path(), &items, &op),
            Ok(())
        );
        assert_eq!(verify_packages(backup.path(), &items, &op), Ok(()));
        std::fs::write(dir.join("asset-1.rgba"), b"modified").unwrap();
        assert_eq!(
            verify_packages(temp.path(), &items, &op),
            Err(Error::IntegrityFailed)
        );
        let cancelled = Operation::new(4000).unwrap();
        cancelled.cancel();
        assert_eq!(
            verify_packages(temp.path(), &items, &cancelled),
            Err(Error::Cancelled)
        );
    }

    #[test]
    fn snapshot_inventory_tracks_snapshot_not_later_database_changes() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("objects")).unwrap();
        let source = Connection::open_in_memory().unwrap();
        source.execute_batch("CREATE TABLE object_versions(sha256 TEXT,workspace_id TEXT DEFAULT 'w',object_id TEXT DEFAULT 'o'); CREATE TABLE objects(workspace_id TEXT,id TEXT,deleted INTEGER); INSERT INTO objects VALUES('w','o',0); CREATE TABLE workspaces(id TEXT PRIMARY KEY);").unwrap();
        let package = "a".repeat(64);
        let rights = "b".repeat(64);
        let doc = serde_json::json!({"kind":"avatar_profile_draft","package_sha256":package,"license_sha256":rights}).to_string();
        let digest = store_body(temp.path(), doc.as_bytes()).unwrap();
        source
            .execute("INSERT INTO object_versions(sha256) VALUES(?1)", [&digest])
            .unwrap();
        let mut snapshot = Connection::open_in_memory().unwrap();
        let op = Operation::new(4000).unwrap();
        let found = snapshot_inventory(&source, &mut snapshot, temp.path(), &op).unwrap();
        source.execute("DELETE FROM object_versions", []).unwrap();
        assert_eq!(found.get(&package), Some(&rights));
        assert_eq!(inventory(&snapshot, temp.path(), &op).unwrap(), found);
        assert!(inventory(&source, temp.path(), &op).unwrap().is_empty());
        std::fs::remove_file(temp.path().join("objects").join(digest)).unwrap();
        assert!(inventory(&snapshot, temp.path(), &op).is_err());
        let cancelled = Operation::new(4000).unwrap();
        cancelled.cancel();
        let mut other = Connection::open_in_memory().unwrap();
        assert_eq!(
            snapshot_inventory(&source, &mut other, temp.path(), &cancelled),
            Err(Error::Cancelled)
        );
    }

    #[test]
    fn inventory_includes_history_revocations_and_fails_on_missing_or_conflicting_data() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("objects")).unwrap();
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE object_versions(sha256 TEXT,workspace_id TEXT DEFAULT 'w',object_id TEXT DEFAULT 'o'); CREATE TABLE objects(workspace_id TEXT,id TEXT,deleted INTEGER); INSERT INTO objects VALUES('w','o',0); CREATE TABLE workspaces(id TEXT PRIMARY KEY); INSERT INTO workspaces VALUES('workspace');").unwrap();
        c.execute_batch(include_str!(
            "../../../schemas/storage/avatar-authority.sql"
        ))
        .unwrap();
        let package = "a".repeat(64);
        let rights = "b".repeat(64);
        let doc = serde_json::json!({"kind":"avatar_profile_draft","package_sha256":package,"license_sha256":rights}).to_string();
        let digest = store_body(temp.path(), doc.as_bytes()).unwrap();
        c.execute("INSERT INTO object_versions(sha256) VALUES(?1)", [&digest])
            .unwrap();
        let older_package = "d".repeat(64);
        let older = serde_json::json!({"kind":"avatar_profile_draft","package_sha256":older_package,"license_sha256":rights}).to_string();
        let older_digest = store_body(temp.path(), older.as_bytes()).unwrap();
        c.execute(
            "INSERT INTO object_versions(sha256) VALUES(?1)",
            [&older_digest],
        )
        .unwrap();
        c.execute(
            "INSERT INTO avatar_rights_revocations VALUES('workspace',?1,?2)",
            params![package, rights],
        )
        .unwrap();
        let op = Operation::new(4000).unwrap();
        let found = inventory(&c, temp.path(), &op).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found.get(&package), Some(&rights));
        assert_eq!(found.get(&older_package), Some(&rights));
        c.execute(
            "UPDATE avatar_rights_revocations SET rights_sha256=?1",
            ["c".repeat(64)],
        )
        .unwrap();
        assert_eq!(inventory(&c, temp.path(), &op), Err(Error::IntegrityFailed));
        c.execute("DELETE FROM avatar_rights_revocations", [])
            .unwrap();
        std::fs::remove_file(temp.path().join("objects").join(digest)).unwrap();
        assert!(inventory(&c, temp.path(), &op).is_err());
    }

    #[test]
    fn backup_copy_manifest_requires_exact_snapshot_and_unique_complete_identities() {
        let op = Operation::new(4000).unwrap();
        let package = "a".repeat(64);
        let rights = "b".repeat(64);
        let database = "c".repeat(64);
        let expected = BTreeMap::from([(package.clone(), rights.clone())]);
        let row = AvatarBackupPackageIdentity {
            package_sha256: package,
            rights_sha256: rights,
        };
        let manifest = AvatarBackupInventory {
            database_sha256: database.clone(),
            packages: vec![row.clone()],
        };
        assert_eq!(compare_copies(&expected, &database, &manifest, &op), Ok(()));
        let mut missing = manifest.clone();
        missing.packages.clear();
        assert_eq!(
            compare_copies(&expected, &database, &missing, &op),
            Err(Error::IntegrityFailed)
        );
        let mut wrong = manifest.clone();
        wrong.packages[0].rights_sha256 = "d".repeat(64);
        assert_eq!(
            compare_copies(&expected, &database, &wrong, &op),
            Err(Error::IntegrityFailed)
        );
        let mut duplicate = manifest.clone();
        duplicate.packages.push(row);
        let mut expected_two = expected.clone();
        expected_two.insert("e".repeat(64), "f".repeat(64));
        assert_eq!(
            compare_copies(&expected_two, &database, &duplicate, &op),
            Err(Error::IntegrityFailed)
        );
        assert_eq!(
            compare_copies(&expected, &"e".repeat(64), &manifest, &op),
            Err(Error::IntegrityFailed)
        );
        let cancelled = Operation::new(4000).unwrap();
        cancelled.cancel();
        assert_eq!(
            compare_copies(&expected, &database, &manifest, &cancelled),
            Err(Error::Cancelled)
        );
    }
}
