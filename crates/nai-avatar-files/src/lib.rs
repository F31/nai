use std::{fs, path::Path};

/// Reads a regular file under a selected directory without following a declared
/// path through a symlink/reparse point. This is preparation only, not a grant.
pub struct SelectedDirectory(cap_std::fs::Dir);

impl SelectedDirectory {
    pub fn open(root: &Path) -> Result<Self, String> {
        use cap_std::fs::Dir;
        #[cfg(windows)]
        let directory = {
            use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
            let file = fs::OpenOptions::new()
                .read(true)
                .share_mode(0x1 | 0x2) // No FILE_SHARE_DELETE: pin the selected root.
                .custom_flags(0x0200_0000 | 0x0020_0000) // directory, open reparse itself
                .open(root)
                .map_err(|_| "AVATAR_NOT_FOUND")?;
            let metadata = file.metadata().map_err(|_| "AVATAR_NOT_FOUND")?;
            if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
                return Err("AVATAR_PATH_REJECTED".into());
            }
            Dir::from_std_file(file)
        };
        #[cfg(not(windows))]
        let directory = {
            let metadata = fs::symlink_metadata(root).map_err(|_| "AVATAR_NOT_FOUND")?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err("AVATAR_PATH_REJECTED".into());
            }
            Dir::open_ambient_dir(root, cap_std::ambient_authority())
                .map_err(|_| "AVATAR_NOT_FOUND")?
        };
        Ok(Self(directory))
    }

    /// Open one child directory without following a symlink or reparse point.
    pub fn child(&self, name: &str) -> Result<Self, String> {
        use cap_fs_ext::DirExt;
        if name.is_empty()
            || name == "."
            || name == ".."
            || name.contains('/')
            || name.contains('\\')
            || name.contains(':')
        {
            return Err("AVATAR_PATH_REJECTED".into());
        }
        self.0
            .open_dir_nofollow(name)
            .map(Self)
            .map_err(|_| "AVATAR_PATH_REJECTED".into())
    }

    fn create_child(&self, name: &str) -> Result<Self, String> {
        use cap_fs_ext::DirExt;
        if name.is_empty()
            || name == "."
            || name == ".."
            || name.contains('/')
            || name.contains('\\')
            || name.contains(':')
        {
            return Err("AVATAR_PATH_REJECTED".into());
        }
        self.0.create_dir(name).map_err(|_| "AVATAR_STORE_FAILED")?;
        self.0
            .open_dir_nofollow(name)
            .map(Self)
            .map_err(|_| "AVATAR_PATH_REJECTED".into())
    }
    fn write_new(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
        use std::io::Write;
        if name.is_empty()
            || name == "."
            || name == ".."
            || name.contains('/')
            || name.contains('\\')
            || name.contains(':')
        {
            return Err("AVATAR_PATH_REJECTED".into());
        }
        let mut opts = cap_std::fs::OpenOptions::new();
        opts.write(true).create_new(true).follow(FollowSymlinks::No);
        let mut file = self
            .0
            .open_with(name, &opts)
            .map_err(|_| "AVATAR_STORE_FAILED")?
            .into_std();
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| "AVATAR_STORE_FAILED".to_owned())
    }

    fn remove_file(&self, name: &str) -> Result<(), String> {
        if name.is_empty()
            || name == "."
            || name == ".."
            || name.contains('/')
            || name.contains('\\')
            || name.contains(':')
        {
            return Err("AVATAR_PATH_REJECTED".into());
        }
        self.0
            .remove_file(name)
            .map_err(|_| "AVATAR_STORE_FAILED".into())
    }

    pub fn read(&self, relative: &str, limit: usize) -> Result<Vec<u8>, String> {
        use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
        use cap_std::fs::OpenOptions;
        use std::io::Read;
        if relative.is_empty() || relative.contains('\\') || relative.contains(':') {
            return Err("AVATAR_PATH_REJECTED".into());
        }
        if relative
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err("AVATAR_PATH_REJECTED".into());
        }
        let mut directory = self.0.try_clone().map_err(|_| "AVATAR_READ_FAILED")?;
        let mut parts = relative.split('/').peekable();
        while let Some(part) = parts.next() {
            if parts.peek().is_none() {
                let mut options = OpenOptions::new();
                options.read(true).follow(FollowSymlinks::No);
                let file = directory
                    .open_with(part, &options)
                    .map_err(|_| "AVATAR_PATH_REJECTED")?
                    .into_std();
                let opened = file.metadata().map_err(|_| "AVATAR_NOT_FOUND")?;
                if !opened.is_file() {
                    return Err("AVATAR_PATH_REJECTED".into());
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if opened.file_attributes() & 0x400 != 0 {
                        return Err("AVATAR_PATH_REJECTED".into());
                    }
                }
                let mut bytes = Vec::new();
                file.take(limit.saturating_add(1) as u64)
                    .read_to_end(&mut bytes)
                    .map_err(|_| "AVATAR_READ_FAILED")?;
                if bytes.is_empty() || bytes.len() > limit {
                    return Err("AVATAR_SIZE_REJECTED".into());
                }
                return Ok(bytes);
            }
            directory = directory
                .open_dir_nofollow(part)
                .map_err(|_| "AVATAR_PATH_REJECTED")?;
        }
        Err("AVATAR_PATH_REJECTED".into())
    }
}

/// A content-verified installed package, not a legal rights grant.
pub struct InstalledAsset {
    pub id: String,
    pub bytes: Vec<u8>,
}
pub struct InstalledPackage {
    pub manifest: String,
    pub config: String,
    pub license: String,
    pub assets: Vec<InstalledAsset>,
}

pub fn read_installed(root: &Path, digest: &str) -> Result<InstalledPackage, String> {
    use sha2::{Digest, Sha256};
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("AVATAR_IDENTITY_REJECTED".into());
    }
    let dir = SelectedDirectory::open(root)
        .and_then(|root| root.child("avatar-packages"))
        .and_then(|packages| packages.child(digest))
        .map_err(|_| "AVATAR_STORE_INTEGRITY")?;
    let read = |name: &str, limit: usize| {
        dir.read(name, limit)
            .map_err(|_| "AVATAR_STORE_INTEGRITY".to_owned())
    };
    let manifest =
        String::from_utf8(read("manifest.json", 65536)?).map_err(|_| "AVATAR_STORE_INTEGRITY")?;
    let config =
        String::from_utf8(read("config.json", 65536)?).map_err(|_| "AVATAR_STORE_INTEGRITY")?;
    let license =
        String::from_utf8(read("LICENSE.txt", 65536)?).map_err(|_| "AVATAR_STORE_INTEGRITY")?;
    let ids: Vec<String> = serde_json::from_slice(&read("asset-ids.json", 1024)?)
        .map_err(|_| "AVATAR_STORE_INTEGRITY")?;
    if ids.len() != 3 {
        return Err("AVATAR_STORE_INTEGRITY".into());
    }
    let mut hash = Sha256::new();
    hash.update(manifest.as_bytes());
    hash.update(config.as_bytes());
    hash.update(license.as_bytes());
    let mut total = 0usize;
    let mut assets = Vec::new();
    for (index, id) in ids.into_iter().enumerate() {
        let bytes = read(
            &format!("asset-{index}.rgba"),
            (8 * 1024 * 1024).min((16 * 1024 * 1024usize).saturating_sub(total)),
        )?;
        total = total
            .checked_add(bytes.len())
            .ok_or("AVATAR_STORE_INTEGRITY")?;
        hash.update(id.as_bytes());
        hash.update(&bytes);
        assets.push(InstalledAsset { id, bytes });
    }
    if hash
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
        != digest
    {
        return Err("AVATAR_STORE_INTEGRITY".into());
    }
    Ok(InstalledPackage {
        manifest,
        config,
        license,
        assets,
    })
}

pub fn verify_installed(root: &Path, digest: &str, rights: &str) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    if rights.len() != 64
        || !rights
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("AVATAR_IDENTITY_REJECTED".into());
    }
    let package = read_installed(root, digest)?;
    if Sha256::digest(package.license.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
        != rights
    {
        return Err("AVATAR_STORE_INTEGRITY".into());
    }
    Ok(())
}

/// Copy a bounded inventory to a fresh caller-owned root. On failure the
/// destination remains incomplete and must not be treated as a backup.
pub fn copy_verified_batch<F: Fn() -> Result<(), String>>(
    source: &Path,
    destination: &Path,
    entries: &[(String, String)],
    check: F,
) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    use std::collections::BTreeSet;
    check()?;
    if entries.len() > 1024 {
        return Err("AVATAR_SIZE_REJECTED".into());
    }
    let src = fs::canonicalize(source).map_err(|_| "AVATAR_PATH_REJECTED")?;
    let dst = fs::canonicalize(destination).map_err(|_| "AVATAR_PATH_REJECTED")?;
    if src == dst || src.starts_with(&dst) {
        return Err("AVATAR_PATH_REJECTED".into());
    }
    if !entries.is_empty() {
        let asset_root =
            fs::canonicalize(source.join("avatar-packages")).map_err(|_| "AVATAR_PATH_REJECTED")?;
        if asset_root.starts_with(&dst) || dst.starts_with(&asset_root) {
            return Err("AVATAR_PATH_REJECTED".into());
        }
    }
    let mut seen = BTreeSet::new();
    for (digest, rights) in entries {
        check()?;
        if !seen.insert(digest) {
            return Err("AVATAR_IDENTITY_REJECTED".into());
        }
        verify_installed(source, digest, rights)?;
    }
    if entries.is_empty() {
        return check();
    }
    let parent = SelectedDirectory::open(destination)?.create_child("avatar-packages")?;
    for (digest, rights) in entries {
        check()?;
        let package = read_installed(source, digest)?;
        if Sha256::digest(package.license.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
            != *rights
        {
            return Err("AVATAR_STORE_INTEGRITY".into());
        }
        // Preview is derived, untrusted data. Preserve it so the desktop can
        // regenerate and compare it when this package is later reopened.
        let preview = SelectedDirectory::open(source)?
            .child("avatar-packages")?
            .child(digest)?
            .read("preview.png", 16 * 1024 * 1024)?;
        let target = parent.create_child(digest)?;
        let ids: Vec<_> = package.assets.iter().map(|a| &a.id).collect();
        let ids = serde_json::to_vec(&ids).map_err(|_| "AVATAR_STORE_FAILED")?;
        for (name, bytes) in [
            ("manifest.json", package.manifest.as_bytes()),
            ("config.json", package.config.as_bytes()),
            ("LICENSE.txt", package.license.as_bytes()),
            ("asset-ids.json", ids.as_slice()),
            ("preview.png", preview.as_slice()),
        ] {
            check()?;
            target.write_new(name, bytes)?;
        }
        for (index, asset) in package.assets.iter().enumerate() {
            check()?;
            target.write_new(&format!("asset-{index}.rgba"), &asset.bytes)?;
        }
        check()?;
        verify_installed(destination, digest, rights)?;
    }
    check()
}

/// One-package convenience wrapper with the same isolated-root boundary.
pub fn copy_verified<F: Fn() -> Result<(), String>>(
    source: &Path,
    destination: &Path,
    digest: &str,
    rights: &str,
    check: F,
) -> Result<(), String> {
    copy_verified_batch(
        source,
        destination,
        &[(digest.to_owned(), rights.to_owned())],
        check,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    fn child_directory_rejects_traversal_and_accepts_real_child() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("valid")).unwrap();
        let root = SelectedDirectory::open(temp.path()).unwrap();
        assert!(root.child("valid").is_ok());
        for path in [
            "",
            ".",
            "..",
            "../valid",
            "valid/..",
            "valid\\..",
            "C:escape",
        ] {
            assert!(root.child(path).is_err(), "accepted {path}");
        }
    }
    #[test]
    fn batch_copy_requires_complete_unique_inventory() {
        let source = tempfile::tempdir().unwrap();
        let backup = tempfile::tempdir().unwrap();
        let hex = |v: &[u8]| v.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let ids = ["base", "closed", "open"];
        let mut entries = Vec::new();
        for marker in [b"first".as_slice(), b"second".as_slice()] {
            let mut h = Sha256::new();
            h.update(marker);
            h.update(b"config");
            h.update(b"license");
            for id in ids {
                h.update(id.as_bytes());
                h.update(marker);
            }
            let digest = hex(&h.finalize());
            let rights = hex(&Sha256::digest(b"license"));
            let dir = source.path().join("avatar-packages").join(&digest);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("manifest.json"), marker).unwrap();
            fs::write(dir.join("config.json"), b"config").unwrap();
            fs::write(dir.join("LICENSE.txt"), b"license").unwrap();
            fs::write(dir.join("preview.png"), b"preview").unwrap();
            fs::write(
                dir.join("asset-ids.json"),
                serde_json::to_vec(&ids).unwrap(),
            )
            .unwrap();
            for i in 0..3 {
                fs::write(dir.join(format!("asset-{i}.rgba")), marker).unwrap();
            }
            entries.push((digest, rights));
        }
        let duplicate = tempfile::tempdir().unwrap();
        assert!(
            copy_verified_batch(
                source.path(),
                duplicate.path(),
                &[entries[0].clone(), entries[0].clone()],
                || Ok(())
            )
            .is_err()
        );
        assert!(!duplicate.path().join("avatar-packages").exists());
        let missing = tempfile::tempdir().unwrap();
        let mut absent = entries.clone();
        absent.push(("a".repeat(64), "b".repeat(64)));
        assert!(copy_verified_batch(source.path(), missing.path(), &absent, || Ok(())).is_err());
        assert!(!missing.path().join("avatar-packages").exists());
        copy_verified_batch(source.path(), backup.path(), &entries, || Ok(())).unwrap();
        for (digest, rights) in &entries {
            assert!(verify_installed(backup.path(), digest, rights).is_ok());
            assert_eq!(
                fs::read(
                    backup
                        .path()
                        .join("avatar-packages")
                        .join(digest)
                        .join("preview.png")
                )
                .unwrap(),
                b"preview"
            );
        }
        let nested = source.path().join("model-migration-backups").join("backup");
        fs::create_dir_all(&nested).unwrap();
        copy_verified_batch(source.path(), &nested, &entries, || Ok(())).unwrap();
        for (digest, rights) in &entries {
            assert!(verify_installed(&nested, digest, rights).is_ok());
        }
        let unsafe_dest = source.path().join("avatar-packages").join("backup");
        fs::create_dir(&unsafe_dest).unwrap();
        assert!(copy_verified_batch(source.path(), &unsafe_dest, &entries, || Ok(())).is_err());
        assert!(!unsafe_dest.join("avatar-packages").exists());
        fs::write(
            backup
                .path()
                .join("avatar-packages")
                .join(&entries[1].0)
                .join("asset-2.rgba"),
            b"corrupt",
        )
        .unwrap();
        assert!(verify_installed(backup.path(), &entries[1].0, &entries[1].1).is_err());
    }
    #[test]
    fn interrupted_copy_never_verifies_as_complete() {
        use std::{
            process::{Command, Stdio},
            time::{Duration, Instant},
        };
        const FLAG: &str = "NAI_AVATAR_COPY_KILL_ROOT";
        if let Some(root) = std::env::var_os(FLAG) {
            let root = std::path::PathBuf::from(root);
            let digest = fs::read_to_string(root.join("digest")).unwrap();
            let rights = fs::read_to_string(root.join("rights")).unwrap();
            let calls = std::cell::Cell::new(0);
            let _ = copy_verified(
                &root.join("source"),
                &root.join("destination"),
                &digest,
                &rights,
                || {
                    calls.set(calls.get() + 1);
                    if calls.get() == 5 {
                        fs::write(root.join("ready"), b"ready").unwrap();
                        loop {
                            std::thread::sleep(Duration::from_secs(1));
                        }
                    }
                    Ok(())
                },
            );
            panic!("copy completed before forced termination");
        }
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&destination).unwrap();
        let ids = ["base", "closed", "open"];
        let assets: [&[u8]; 3] = [b"one", b"two", b"three"];
        let mut h = Sha256::new();
        h.update(b"manifest");
        h.update(b"config");
        h.update(b"license");
        for (id, bytes) in ids.iter().zip(assets) {
            h.update(id.as_bytes());
            h.update(bytes);
        }
        let hex = |v: &[u8]| v.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let digest = hex(&h.finalize());
        let rights = hex(&Sha256::digest(b"license"));
        let pkg = source.join("avatar-packages").join(&digest);
        fs::create_dir_all(&pkg).unwrap();
        for (name, bytes) in [
            ("manifest.json", b"manifest".as_slice()),
            ("config.json", b"config".as_slice()),
            ("LICENSE.txt", b"license".as_slice()),
            ("preview.png", b"preview".as_slice()),
        ] {
            fs::write(pkg.join(name), bytes).unwrap();
        }
        fs::write(
            pkg.join("asset-ids.json"),
            serde_json::to_vec(&ids).unwrap(),
        )
        .unwrap();
        for (n, bytes) in assets.iter().enumerate() {
            fs::write(pkg.join(format!("asset-{n}.rgba")), bytes).unwrap();
        }
        fs::write(root.path().join("digest"), &digest).unwrap();
        fs::write(root.path().join("rights"), &rights).unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::interrupted_copy_never_verifies_as_complete",
                "--test-threads=1",
            ])
            .env(FLAG, root.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while !root.path().join("ready").exists() && Instant::now() < deadline {
            if child.try_wait().unwrap().is_some() {
                panic!("copy worker exited before interruption");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let ready = root.path().join("ready").exists();
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(ready, "copy worker did not reach interruption point");
        assert!(
            destination
                .join("avatar-packages")
                .join(&digest)
                .join("manifest.json")
                .exists()
        );
        assert!(verify_installed(&destination, &digest, &rights).is_err());
        assert!(verify_installed(&source, &digest, &rights).is_ok());
    }
    #[cfg(windows)]
    #[test]
    fn package_parent_swap_never_reads_outside_root() {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        let outside = temp.path().join("outside");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&outside).unwrap();
        let active = root.join("avatar-packages");
        let safe = root.join("safe");
        let junction = root.join("junction");
        fs::create_dir(&active).unwrap();
        fs::write(active.join("marker"), b"inside").unwrap();
        fs::write(outside.join("marker"), b"outside").unwrap();
        assert!(
            std::process::Command::new("cmd.exe")
                .args(["/C", "mklink", "/J"])
                .arg(&junction)
                .arg(&outside)
                .status()
                .unwrap()
                .success()
        );
        let done = Arc::new(AtomicBool::new(false));
        let thread_done = done.clone();
        let worker_root = root.clone();
        let worker = std::thread::spawn(move || {
            let active = worker_root.join("avatar-packages");
            let safe = worker_root.join("safe");
            let junction = worker_root.join("junction");
            let mut swaps = 0;
            while !thread_done.load(Ordering::Relaxed) {
                if fs::rename(&active, &safe).is_err() {
                    std::thread::yield_now();
                    continue;
                }
                if fs::rename(&junction, &active).is_ok() {
                    swaps += 1;
                    while fs::rename(&active, &junction).is_err() {
                        std::thread::yield_now();
                    }
                }
                while fs::rename(&safe, &active).is_err() {
                    std::thread::yield_now();
                }
            }
            swaps
        });
        let root_handle = SelectedDirectory::open(&root).unwrap();
        for _ in 0..1000 {
            if let Ok(bytes) = root_handle
                .child("avatar-packages")
                .and_then(|dir| dir.read("marker", 32))
            {
                assert_eq!(bytes, b"inside");
            }
        }
        done.store(true, Ordering::Relaxed);
        let swaps = worker.join().unwrap();
        assert!(swaps > 0, "directory replacement must occur");
        fs::remove_dir(&junction).unwrap();
        assert!(!safe.exists());
    }
    #[cfg(windows)]
    #[test]
    fn destination_parent_swap_never_creates_outside_root() {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        let outside = temp.path().join("outside");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&outside).unwrap();
        let active = root.join("avatar-packages");
        let safe = root.join("safe");
        let junction = root.join("junction");
        fs::create_dir(&active).unwrap();
        assert!(
            std::process::Command::new("cmd.exe")
                .args(["/C", "mklink", "/J"])
                .arg(&junction)
                .arg(&outside)
                .status()
                .unwrap()
                .success()
        );
        let done = Arc::new(AtomicBool::new(false));
        let thread_done = done.clone();
        let worker_root = root.clone();
        let worker = std::thread::spawn(move || {
            let active = worker_root.join("avatar-packages");
            let safe = worker_root.join("safe");
            let junction = worker_root.join("junction");
            let mut swaps = 0;
            while !thread_done.load(Ordering::Relaxed) {
                if fs::rename(&active, &safe).is_err() {
                    std::thread::yield_now();
                    continue;
                }
                if fs::rename(&junction, &active).is_ok() {
                    swaps += 1;
                    while fs::rename(&active, &junction).is_err() {
                        std::thread::yield_now();
                    }
                }
                while fs::rename(&safe, &active).is_err() {
                    std::thread::yield_now();
                }
            }
            swaps
        });
        let root_handle = SelectedDirectory::open(&root).unwrap();
        for n in 0..500 {
            if let Ok(parent) = root_handle.child("avatar-packages") {
                let _ = parent.create_child(&format!("created-{n}"));
            }
        }
        done.store(true, Ordering::Relaxed);
        let swaps = worker.join().unwrap();
        assert!(swaps > 0, "directory replacement must occur");
        assert_eq!(
            fs::read_dir(&outside).unwrap().count(),
            0,
            "write escaped root"
        );
        fs::remove_dir(&junction).unwrap();
        assert!(!safe.exists());
    }
    #[cfg(windows)]
    #[test]
    fn installed_reader_rejects_junction_at_package_parent() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        let outside = temp.path().join("outside");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&outside).unwrap();
        let junction = root.join("avatar-packages");
        let status = std::process::Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&outside)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(read_installed(&root, &"a".repeat(64)).is_err());
        fs::remove_dir(&junction).unwrap();
    }
    #[test]
    fn installed_identity_rejects_wrong_rights_and_tampered_asset() {
        let temp = tempfile::tempdir().unwrap();
        let manifest = b"manifest";
        let config = b"config";
        let license = b"license";
        let ids = ["base", "closed", "open"];
        let assets: [&[u8]; 3] = [b"one", b"two", b"three"];
        let hex = |v: &[u8]| v.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let mut h = Sha256::new();
        h.update(manifest);
        h.update(config);
        h.update(license);
        for (id, bytes) in ids.iter().zip(assets) {
            h.update(id.as_bytes());
            h.update(bytes);
        }
        let digest = hex(&h.finalize());
        let rights = hex(&Sha256::digest(license));
        let dir = temp.path().join("avatar-packages").join(&digest);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("manifest.json"), manifest).unwrap();
        fs::write(dir.join("config.json"), config).unwrap();
        fs::write(dir.join("LICENSE.txt"), license).unwrap();
        fs::write(dir.join("preview.png"), b"preview").unwrap();
        fs::write(
            dir.join("asset-ids.json"),
            serde_json::to_vec(&ids).unwrap(),
        )
        .unwrap();
        for (i, bytes) in assets.iter().enumerate() {
            fs::write(dir.join(format!("asset-{i}.rgba")), bytes).unwrap();
        }
        assert!(verify_installed(temp.path(), &digest, &rights).is_ok());
        assert!(verify_installed(temp.path(), &digest, &"a".repeat(64)).is_err());
        let backup = tempfile::tempdir().unwrap();
        assert!(copy_verified(temp.path(), temp.path(), &digest, &rights, || Ok(())).is_err());
        assert!(copy_verified(temp.path(), backup.path(), &digest, &rights, || Ok(())).is_ok());
        assert!(verify_installed(backup.path(), &digest, &rights).is_ok());
        assert!(copy_verified(temp.path(), backup.path(), &digest, &rights, || Ok(())).is_err());
        fs::write(
            backup
                .path()
                .join("avatar-packages")
                .join(&digest)
                .join("asset-0.rgba"),
            b"modified",
        )
        .unwrap();
        assert!(verify_installed(backup.path(), &digest, &rights).is_err());
        let cancelled = tempfile::tempdir().unwrap();
        assert!(
            copy_verified(temp.path(), cancelled.path(), &digest, &rights, || Err(
                "cancelled".into()
            ))
            .is_err()
        );
        assert!(!cancelled.path().join("avatar-packages").exists());
        let interrupted = tempfile::tempdir().unwrap();
        let count = std::cell::Cell::new(0);
        assert!(
            copy_verified(temp.path(), interrupted.path(), &digest, &rights, || {
                count.set(count.get() + 1);
                if count.get() >= 4 {
                    Err("cancelled".into())
                } else {
                    Ok(())
                }
            })
            .is_err()
        );
        assert!(verify_installed(interrupted.path(), &digest, &rights).is_err());
        assert!(verify_installed(temp.path(), &digest, &rights).is_ok());
        fs::write(dir.join("asset-1.rgba"), b"tampered").unwrap();
        assert!(verify_installed(temp.path(), &digest, &rights).is_err());
    }
}

pub mod photo;
pub mod video;
