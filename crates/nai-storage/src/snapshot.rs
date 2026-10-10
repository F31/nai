use crate::{
    DataPaths, Operation, Result, Store, error, hash, id, migration,
    paths::no_links,
    store::{access, recover},
};
use nai_contracts::v1::ErrorCode;
use rusqlite::{
    Connection, OpenFlags,
    backup::{Backup, StepResult},
    params,
};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ARTIFACT: u64 = 16 * 1024 * 1024;
const CATALOG: &[u8] = include_bytes!("../../../schemas/catalog/capabilities.json");
fn checked_file(path: &Path, maximum: u64, op: &Operation) -> Result<Vec<u8>> {
    op.check()?;
    no_links(path)?;
    if !fs::metadata(path)?.is_file() || fs::metadata(path)?.len() > maximum {
        return Err(error(ErrorCode::ResourceExhausted, "snapshot file limit"));
    }
    let mut bytes = Vec::new();
    let mut input = File::open(path)?.take(maximum + 1);
    let mut block = [0u8; 32768];
    loop {
        op.check()?;
        let n = input.read(&mut block)?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&block[..n]);
        if bytes.len() as u64 > maximum {
            return Err(error(
                ErrorCode::ResourceExhausted,
                "snapshot file grew beyond limit",
            ));
        }
    }
    Ok(bytes)
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    no_links(path)?;
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| error(ErrorCode::InvalidArgument, "missing parent"))?,
    )?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
fn unique() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}
/// Flushes a file this store just wrote and proves the bytes are durable.
///
/// Flushing needs a writable handle: Windows `FlushFileBuffers` refuses a
/// read-only one, so open for writing even though nothing is written. Without
/// this the manifests would be hashing bytes that were never confirmed on disk,
/// and every backup and restore would fail on Windows.
fn flush_durable(path: &Path) -> Result<()> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?
        .sync_all()?;
    Ok(())
}
fn validate_manifest(value: &serde_json::Value) -> Result<()> {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../schemas/config/backup.schema.json"))
            .map_err(|_| error(ErrorCode::Internal, "invalid backup schema"))?;
    if !jsonschema::validator_for(&schema)
        .map_err(|_| error(ErrorCode::Internal, "backup validator unavailable"))?
        .is_valid(value)
    {
        return Err(error(ErrorCode::InvalidArgument, "invalid backup manifest"));
    }
    if value["runtime_version"].as_str() != Some(env!("CARGO_PKG_VERSION"))
        || value["contract_catalog_sha256"].as_str() != Some(hash(CATALOG).as_str())
    {
        return Err(error(
            ErrorCode::VersionIncompatible,
            "snapshot runtime/contract combination unsupported",
        ));
    }
    Ok(())
}
fn database_integrity(conn: &Connection) -> Result<()> {
    if conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))? != "ok" {
        return Err(error(ErrorCode::Conflict, "database integrity failed"));
    }
    if conn
        .prepare("PRAGMA foreign_key_check")?
        .query([])?
        .next()?
        .is_some()
    {
        return Err(error(ErrorCode::Conflict, "snapshot foreign keys invalid"));
    }
    if migration::verify(conn)? != migration::VERSION {
        return Err(error(
            ErrorCode::VersionIncompatible,
            "snapshot schema incompatible",
        ));
    }
    Ok(())
}
fn artifacts(conn: &Connection) -> Result<Vec<(String, String, u64)>> {
    let mut statement = conn.prepare(
        "SELECT DISTINCT path,sha256,size_bytes FROM managed_artifacts ORDER BY path LIMIT 513",
    )?;
    let values = statement
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get::<_, i64>(2)? as u64))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if values.len() > 512 {
        return Err(error(
            ErrorCode::ResourceExhausted,
            "snapshot artifact count limit",
        ));
    }
    Ok(values)
}
impl Store {
    pub fn publish_artifact(
        &mut self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        artifact_id: &str,
        kind: &str,
        bytes: &[u8],
    ) -> Result<String> {
        self.start(op)?;
        id(artifact_id)?;
        access(&self.conn, workspace, actor, "knowledge.publish", "*")?;
        if !["knowledge", "assets", "models"].contains(&kind) || bytes.len() as u64 > MAX_ARTIFACT {
            return Err(error(
                ErrorCode::InvalidArgument,
                "artifact type/size invalid",
            ));
        }
        let digest = hash(bytes);
        let relative = format!("{kind}/{digest}");
        let path = self.paths.root.join(&relative);
        no_links(&path)?;
        if path.exists() {
            if hash(&checked_file(&path, MAX_ARTIFACT, op)?) != digest {
                return Err(error(ErrorCode::Conflict, "immutable artifact changed"));
            }
        } else {
            let temporary = self
                .paths
                .root
                .join(format!("{kind}/.pending-{}", unique()));
            write_new(&temporary, bytes)?;
            op.check()?;
            fs::rename(&temporary, &path)?;
        }
        op.check()?;
        self.conn.execute(
            "INSERT INTO managed_artifacts VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                workspace,
                artifact_id,
                relative,
                digest,
                bytes.len() as i64,
                kind
            ],
        )?;
        Ok(relative)
    }
    pub fn backup(&self, op: &Operation) -> Result<PathBuf> {
        self.start(op)?;
        let name = unique();
        let staging = self.paths.root.join(format!("backups/.pending-{name}"));
        let destination = self.paths.root.join(format!("backups/{name}"));
        no_links(&staging)?;
        fs::create_dir(&staging)?;
        let result = (|| {
            let path = staging.join("state.sqlite3");
            let mut snapshot = Connection::open(&path)?;
            snapshot.busy_timeout(std::time::Duration::from_millis(50))?;
            {
                let backup = Backup::new(&self.conn, &mut snapshot)?;
                loop {
                    op.check()?;
                    match backup.step(16)? {
                        StepResult::Done => break,
                        StepResult::More => {}
                        StepResult::Busy | StepResult::Locked => {
                            std::thread::sleep(std::time::Duration::from_millis(5))
                        }
                        _ => return Err(error(ErrorCode::Internal, "unexpected backup state")),
                    }
                }
            }
            snapshot.pragma_update(None, "journal_mode", "DELETE")?;
            database_integrity(&snapshot)?;
            let mut entries = Vec::new();
            let mut total = fs::metadata(&path)?.len();
            for (relative, digest, size) in artifacts(&snapshot)? {
                // Validate paths before joining them, even if the database was tampered with.
                let entry = serde_json::json!({"path":relative,"sha256":digest,"bytes":size});
                entries.push(entry);
            }
            let manifest = serde_json::json!({"schema_version":1,"db_schema_version":migration::VERSION,"created_at_utc_ms":crate::utc_ms()?.to_string(),"runtime_version":env!("CARGO_PKG_VERSION"),"contract_catalog_sha256":hash(CATALOG),"sensitive_scope":"configuration-facts-audit-registered-content;credential-references-only","database":{"path":"state.sqlite3","sha256":"0".repeat(64),"bytes":total},"artifacts":entries});
            validate_manifest(&manifest)?;
            for entry in &entries {
                let relative = entry["path"]
                    .as_str()
                    .ok_or_else(|| error(ErrorCode::InvalidArgument, "artifact path absent"))?;
                let bytes = checked_file(&self.paths.root.join(relative), MAX_ARTIFACT, op)?;
                if hash(&bytes) != entry["sha256"].as_str().unwrap_or_default()
                    || bytes.len() as u64 != entry["bytes"].as_u64().unwrap_or(u64::MAX)
                {
                    return Err(error(ErrorCode::Conflict, "artifact hash/size changed"));
                }
                total = total
                    .checked_add(bytes.len() as u64)
                    .ok_or_else(|| error(ErrorCode::ResourceExhausted, "snapshot size overflow"))?;
                if total > MAX_BYTES {
                    return Err(error(
                        ErrorCode::ResourceExhausted,
                        "snapshot total size limit",
                    ));
                }
                write_new(&staging.join(relative), &bytes)?;
            }
            drop(snapshot);
            // Confirm the durable bytes before the manifest hashes them, so the
            // manifest never describes a write the store did not land.
            flush_durable(&path)?;
            let database = checked_file(&path, MAX_BYTES, op)?;
            let mut manifest = manifest;
            manifest["database"]["sha256"] = hash(&database).into();
            manifest["database"]["bytes"] = (database.len() as u64).into();
            validate_manifest(&manifest)?;
            write_new(
                &staging.join("manifest.json"),
                &serde_json::to_vec_pretty(&manifest)
                    .map_err(|_| error(ErrorCode::Internal, "manifest serialization"))?,
            )?;
            op.check()?;
            fs::rename(&staging, &destination)?;
            Ok(destination.clone())
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        result
    }
    /// Restores into an entirely new directory, never replacing current data.
    pub fn restore(backup: &Path, destination: DataPaths, op: &Operation) -> Result<Self> {
        op.check()?;
        no_links(backup)?;
        no_links(&destination.root)?;
        if destination.root.exists() {
            return Err(error(
                ErrorCode::Conflict,
                "restore destination must not exist",
            ));
        }
        let manifest: serde_json::Value = serde_json::from_slice(&checked_file(
            &backup.join("manifest.json"),
            1024 * 1024,
            op,
        )?)
        .map_err(|_| error(ErrorCode::InvalidArgument, "invalid manifest JSON"))?;
        validate_manifest(&manifest)?;
        let database = checked_file(&backup.join("state.sqlite3"), MAX_BYTES, op)?;
        if hash(&database) != manifest["database"]["sha256"].as_str().unwrap_or_default()
            || database.len() as u64 != manifest["database"]["bytes"].as_u64().unwrap_or_default()
        {
            return Err(error(ErrorCode::Conflict, "database hash/size mismatch"));
        }
        // External WAL/journal files cannot add data outside the approved DB hash.
        for suffix in ["-wal", "-shm", "-journal"] {
            if backup.join(format!("state.sqlite3{suffix}")).exists() {
                return Err(error(
                    ErrorCode::Conflict,
                    "snapshot contains mutable journal",
                ));
            }
        }
        let source = Connection::open_with_flags(
            backup.join("state.sqlite3"),
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        database_integrity(&source)?;
        let expected:Vec<_>=artifacts(&source)?.into_iter().map(|(path,sha256,bytes)|serde_json::json!({"path":path,"sha256":sha256,"bytes":bytes})).collect();
        if manifest["artifacts"].as_array() != Some(&expected) {
            return Err(error(
                ErrorCode::Conflict,
                "artifact manifest does not match database",
            ));
        }
        let parent = destination
            .root
            .parent()
            .ok_or_else(|| error(ErrorCode::InvalidArgument, "restore parent absent"))?;
        no_links(parent)?;
        fs::create_dir_all(parent)?;
        let staging = parent.join(format!(".nai-restore-{}", unique()));
        let paths = DataPaths::at(&staging)?;
        paths.initialize()?;
        let result = (|| {
            write_new(&paths.database(), &database)?;
            let mut total = database.len() as u64;
            for entry in &expected {
                let relative = entry["path"]
                    .as_str()
                    .ok_or_else(|| error(ErrorCode::InvalidArgument, "artifact path absent"))?;
                let bytes = checked_file(&backup.join(relative), MAX_ARTIFACT, op)?;
                total += bytes.len() as u64;
                if total > MAX_BYTES
                    || hash(&bytes) != entry["sha256"].as_str().unwrap_or_default()
                    || bytes.len() as u64 != entry["bytes"].as_u64().unwrap_or_default()
                {
                    return Err(error(ErrorCode::Conflict, "restore artifact invalid"));
                }
                write_new(&paths.root.join(relative), &bytes)?;
            }
            let mut restored = Connection::open(paths.database())?;
            restored.pragma_update(None, "foreign_keys", true)?;
            let call = op.clone();
            restored.progress_handler(1000, Some(move || call.stopped()))?;
            recover(&mut restored, op, true)?;
            database_integrity(&restored)?;
            drop(restored);
            flush_durable(&paths.database())?;
            op.check()?;
            if destination.root.exists() {
                return Err(error(ErrorCode::Conflict, "restore destination appeared"));
            }
            fs::rename(&paths.root, &destination.root)?;
            Self::open(destination, op)
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        result
    }
}
