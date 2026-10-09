use crate::{Result, error};
use nai_contracts::v1::ErrorCode;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct DataPaths {
    pub(crate) root: PathBuf,
}
impl DataPaths {
    pub fn for_current_user() -> Result<Self> {
        let directories = directories::ProjectDirs::from("", "F31", "nai").ok_or_else(|| {
            error(
                ErrorCode::CapabilityNotReady,
                "user data directory unavailable",
            )
        })?;
        Self::at(directories.data_local_dir())
    }
    pub fn at(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        if !root.is_absolute() || root.parent().is_none() {
            return Err(error(
                ErrorCode::InvalidArgument,
                "data root must be an absolute application directory",
            ));
        }
        if root
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(error(ErrorCode::InvalidArgument, "data root traversal"));
        }
        #[cfg(windows)]
        if root.components().any(|component|matches!(component,std::path::Component::Prefix(prefix) if matches!(prefix.kind(), std::path::Prefix::UNC(..)|std::path::Prefix::VerbatimUNC(..)))) { return Err(error(ErrorCode::InvalidArgument,"SQLite requires a local path")); }
        no_links(root)?;
        Ok(Self { root: root.into() })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub(crate) fn initialize(&self) -> Result<()> {
        no_links(&self.root)?;
        let new_root = !self.root.exists();
        fs::create_dir_all(&self.root)?;
        #[cfg(unix)]
        if new_root {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700))?;
        }
        #[cfg(not(unix))]
        let _ = new_root;
        for child in [
            "config",
            "db",
            "knowledge",
            "indexes",
            "assets",
            "models",
            "cache",
            "recordings",
            "logs",
            "backups",
            "upgrade-staging",
        ] {
            let path = self.root.join(child);
            no_links(&path)?;
            let new_child = !path.exists();
            fs::create_dir_all(&path)?;
            #[cfg(unix)]
            if new_child {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
            }
            #[cfg(not(unix))]
            let _ = new_child;
        }
        Ok(())
    }
    pub(crate) fn database(&self) -> PathBuf {
        self.root.join("db/state.sqlite3")
    }
}
pub(crate) fn no_links(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                let link = metadata.file_type().is_symlink();
                #[cfg(windows)]
                let link = {
                    use std::os::windows::fs::MetadataExt;
                    link || metadata.file_attributes() & 0x400 != 0
                };
                if link {
                    return Err(error(
                        ErrorCode::PermissionDenied,
                        "linked data path rejected",
                    ));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
