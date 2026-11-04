use crate::{Error, Result};
use nai_runtime_core::foundation::uuid;
use rusqlite::Connection;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path},
    time::Duration,
};
pub fn no_links(path: &Path) -> Result<()> {
    if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(Error::DataRootRejected);
    }
    let mut prefix = std::path::PathBuf::new();
    for c in path.components() {
        prefix.push(c);
        match fs::symlink_metadata(&prefix) {
            Ok(m) => {
                if m.file_type().is_symlink() {
                    return Err(Error::DataRootRejected);
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if m.file_attributes() & 0x400 != 0 {
                        return Err(Error::DataRootRejected);
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => return Err(Error::DataRootRejected),
        }
    }
    Ok(())
}
pub fn read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    no_links(path)?;
    let mut bytes = vec![];
    File::open(path)
        .map_err(|_| Error::NotFound)?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::IoFailed)?;
    if bytes.len() as u64 > limit {
        return Err(Error::IntegrityFailed);
    }
    Ok(bytes)
}
fn manifest(marker: &Path) -> Result<String> {
    let m: serde_json::Value =
        serde_json::from_slice(&read(marker, 4096)?).map_err(|_| Error::DataRootRejected)?;
    let id = m["owner"].as_str().ok_or(Error::DataRootRejected)?;
    uuid(id).map_err(|_| Error::DataRootRejected)?;
    if m != serde_json::json!({"kind":"nai-foundation-v2","schema_version":2,"owner":id}) {
        return Err(Error::DataRootRejected);
    }
    Ok(id.to_owned())
}
pub fn open(root: &Path, principal: &str, initialize: bool) -> Result<(Connection, File, String)> {
    uuid(principal)?;
    no_links(root)?;
    let marker = root.join("foundation-root.json");
    if initialize {
        if root.exists()
            && fs::read_dir(root)
                .map_err(|_| Error::DataRootRejected)?
                .next()
                .is_some()
        {
            return Err(Error::DataRootRejected);
        }
        fs::create_dir_all(root).map_err(|_| Error::IoFailed)?;
    } else if !marker.is_file() {
        return Err(Error::DataRootRejected);
    } else {
        // Reject an older format before creating even a lock/objects directory in that root.
        manifest(&marker)?;
    }
    let lock = root.join("owner.lock");
    no_links(&lock)?;
    let owner = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock)
        .map_err(|_| Error::IoFailed)?;
    owner.try_lock().map_err(|_| Error::OwnerBusy)?;
    let root_owner = if initialize {
        let bytes = serde_json::to_vec(
            &serde_json::json!({"kind":"nai-foundation-v2","schema_version":2,"owner":principal}),
        )
        .map_err(|_| Error::IoFailed)?;
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&marker)
            .map_err(|_| Error::DataRootRejected)?;
        f.write_all(&bytes).map_err(|_| Error::IoFailed)?;
        f.sync_all().map_err(|_| Error::IoFailed)?;
        principal.to_owned()
    } else {
        manifest(&marker)?
    };
    let objects = root.join("objects");
    no_links(&objects)?;
    fs::create_dir_all(&objects).map_err(|_| Error::IoFailed)?;
    let db = root.join("foundation.sqlite");
    no_links(&db)?;
    let ready = root.join("initialized.ok");
    let pending = root.join("initialized.next");
    no_links(&ready)?;
    no_links(&pending)?;
    if pending.exists() && !b"nai-foundation-schema-2".starts_with(&read(&pending, 64)?) {
        return Err(Error::IntegrityFailed);
    }
    let committed = ready.exists();
    if committed && read(&ready, 64)? != b"nai-foundation-schema-2" {
        return Err(Error::IntegrityFailed);
    }
    if committed && !db.is_file() {
        return Err(Error::IntegrityFailed);
    }
    for suffix in [
        "foundation.sqlite-journal",
        "foundation.sqlite-wal",
        "foundation.sqlite-shm",
    ] {
        no_links(&root.join(suffix))?;
    }
    let mut conn = Connection::open(db).map_err(|_| Error::IoFailed)?;
    conn.busy_timeout(Duration::from_millis(50))
        .map_err(|_| Error::IoFailed)?;
    conn.execute_batch(
        "PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL; PRAGMA locking_mode=EXCLUSIVE;",
    )
    .map_err(|_| Error::IoFailed)?;
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(|_| Error::IntegrityFailed)?;
    let app: i64 = conn
        .pragma_query_value(None, "application_id", |r| r.get(0))
        .map_err(|_| Error::IntegrityFailed)?;
    if !committed && version == 0 && app == 0 {
        let tables:i64=conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",[],|r|r.get(0)).map_err(|_|Error::IntegrityFailed)?;
        if tables != 0 {
            return Err(Error::DataRootRejected);
        }
        let tx = conn.transaction().map_err(|_| Error::IoFailed)?;
        tx.execute_batch(include_str!("../../../schemas/storage/foundation-v2.sql"))
            .map_err(|_| Error::IoFailed)?;
        tx.commit().map_err(|_| Error::IoFailed)?;
    } else if version != 2 || app != 1312901428 {
        return Err(Error::DataRootRejected);
    }
    if !committed {
        // The immutable root marker is the initialization journal; ready is published only after FULL commit.
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&pending)
            .map_err(|_| Error::IoFailed)?;
        f.write_all(b"nai-foundation-schema-2")
            .map_err(|_| Error::IoFailed)?;
        f.sync_all().map_err(|_| Error::IoFailed)?;
        drop(f);
        fs::rename(&pending, &ready).map_err(|_| Error::IoFailed)?;
    }
    Ok((conn, owner, root_owner))
}

pub fn revoke(
    root: &Path,
    fence: &nai_runtime_core::foundation::StopFence,
    key: &str,
) -> Result<()> {
    fence.revoke_scope(key)?;
    let directory = root.join("revocations");
    no_links(&directory)?;
    std::fs::create_dir_all(&directory).map_err(|_| Error::IoFailed)?;
    let file = directory.join(super::hash(key.as_bytes()));
    no_links(&file)?;
    match OpenOptions::new().write(true).create_new(true).open(&file) {
        Ok(mut f) => {
            f.write_all(key.as_bytes()).map_err(|_| Error::IoFailed)?;
            f.sync_all().map_err(|_| Error::IoFailed)?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            if read(&file, 128)? != key.as_bytes() {
                return Err(Error::IntegrityFailed);
            }
        }
        Err(_) => return Err(Error::IoFailed),
    }
    Ok(())
}
pub fn load_revocations(
    root: &Path,
    conn: &Connection,
    fence: &nai_runtime_core::foundation::StopFence,
) -> Result<()> {
    let directory = root.join("revocations");
    no_links(&directory)?;
    std::fs::create_dir_all(&directory).map_err(|_| Error::IoFailed)?;
    for (i, entry) in std::fs::read_dir(&directory)
        .map_err(|_| Error::IoFailed)?
        .enumerate()
    {
        if i >= 256 {
            return Err(Error::ResourceExhausted);
        }
        let entry = entry.map_err(|_| Error::IoFailed)?;
        let key =
            String::from_utf8(read(&entry.path(), 128)?).map_err(|_| Error::IntegrityFailed)?;
        if entry.file_name().to_str() != Some(super::hash(key.as_bytes()).as_str()) {
            return Err(Error::IntegrityFailed);
        }
        let fields: Vec<_> = key.split(':').collect();
        if fields.len() < 3 {
            return Err(Error::IntegrityFailed);
        }
        uuid(fields[1])?;
        uuid(fields[2])?;
        if fields[0] == "v" && fields.len() == 4 {
            let revision = fields[3]
                .parse::<u64>()
                .map_err(|_| Error::IntegrityFailed)?;
            if !(1..=1024).contains(&revision) {
                return Err(Error::IntegrityFailed);
            }
            fence.revoke_scope(&key)?;
            continue;
        }
        // Committed revocations are now enforced by the durable DB; only unresolved intents need a side journal.
        if fields[0] == "public" && fields.len() == 3 {
            let resolved: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM public_media_approvals WHERE workspace_id=?1 AND approval_id=?2 AND state='revoked')", rusqlite::params![fields[1],fields[2]], |r|r.get(0)).map_err(|_|Error::IoFailed)?;
            if resolved {
                std::fs::remove_file(entry.path()).map_err(|_| Error::IoFailed)?;
            } else {
                fence.revoke_scope(&key)?;
            }
            continue;
        }
        let resolved=match fields[0]{"r" if fields.len()==4=>{let revision=fields[3].parse::<i64>().map_err(|_|Error::IntegrityFailed)?;conn.query_row("SELECT EXISTS(SELECT 1 FROM roles WHERE workspace_id=?1 AND principal_id=?2 AND revision>?3)",rusqlite::params![fields[1],fields[2],revision],|r|r.get::<_,bool>(0)).map_err(|_|Error::IoFailed)?},"o" if fields.len()==3=>conn.query_row("SELECT EXISTS(SELECT 1 FROM objects WHERE workspace_id=?1 AND id=?2 AND deleted=1)",rusqlite::params![fields[1],fields[2]],|r|r.get::<_,bool>(0)).map_err(|_|Error::IoFailed)?,_=>return Err(Error::IntegrityFailed)};
        if resolved {
            std::fs::remove_file(entry.path()).map_err(|_| Error::IoFailed)?;
        } else {
            fence.revoke_scope(&key)?;
        }
    }
    Ok(())
}
