//! Fixed-root, readonly preparation input. No playback or device authority.
use nai_contracts::workbench::{PublicWindowPreparation, WorkbenchError as Error};
use std::{
    fs::File,
    io::Write,
    path::{Component, Path},
};
fn no_links(path: &Path) -> Result<(), Error> {
    if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(Error::DataRootRejected);
    }
    let mut p = std::path::PathBuf::new();
    for c in path.components() {
        p.push(c);
        let m = std::fs::symlink_metadata(&p).map_err(|_| Error::DataRootRejected)?;
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
    Ok(())
}
pub fn load(
    root: &Path,
    id: &str,
    x: &PublicWindowPreparation,
    check: &dyn Fn() -> Result<(), Error>,
) -> Result<crate::public_pipe::Consumed, Error> {
    check()?;
    let p = Path::new(&x.data_path);
    let name = p
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or(Error::DataRootRejected)?;
    if x.output_id != id
        || p.parent() != Some(root)
        || !name.starts_with("public-preload-")
        || !name.ends_with(".pcm")
        || name.len() > 128
        || x.bundle_sha256.len() != 64
        || !x.bundle_sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(Error::DataRootRejected);
    }
    no_links(p)?;
    let mut f = File::open(p).map_err(|_| Error::IoFailed)?;
    if !f.metadata().map_err(|_| Error::IoFailed)?.is_file()
        || f.metadata().map_err(|_| Error::IoFailed)?.len() > 960000
    {
        return Err(Error::IntegrityFailed);
    }
    if f.write_all(&[0]).is_ok() {
        return Err(Error::PermissionDenied);
    }
    crate::public_pipe::read_pcm(
        &mut f,
        x.prepared_descriptor
            .as_ref()
            .ok_or(Error::IntegrityFailed)?,
        id,
        check,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readonly_actual_file_and_source_identity_are_verified() {
        let t = tempfile::tempdir().unwrap();
        let (d, b) = crate::public_pipe::tests::fixture();
        let mut file = tempfile::Builder::new()
            .prefix("public-preload-")
            .suffix(".pcm")
            .tempfile_in(t.path())
            .unwrap();
        file.write_all(&b).unwrap();
        let x = PublicWindowPreparation {
            prepared_descriptor: Some(d),
            data_path: file.path().to_str().unwrap().into(),
            output_id: "00000000-0000-4000-8000-000000000003".into(),
            bundle_sha256: "a".repeat(64),
            // This test covers read-only loading, not execution admission.
            ..Default::default()
        };
        let result = load(t.path(), &x.output_id, &x, &|| Ok(())).unwrap();
        assert_eq!(result.samples.len(), 1601);
        assert_eq!(result.receipt.consumer_instance_id, x.output_id);
        for case in 0..4 {
            let mut bad = x.clone();
            match case {
                0 => bad.output_id = "other".into(),
                1 => bad.data_path = t.path().join("../escape.pcm").to_str().unwrap().into(),
                2 => {
                    bad.prepared_descriptor
                        .as_mut()
                        .unwrap()
                        .audio
                        .as_mut()
                        .unwrap()
                        .pcm_sha256 = "f".repeat(64)
                }
                _ => bad.data_path = t.path().join("unknown.pcm").to_str().unwrap().into(),
            };
            assert!(load(t.path(), &x.output_id, &bad, &|| Ok(())).is_err());
        }
        assert_eq!(
            load(t.path(), &x.output_id, &x, &|| Err(Error::Cancelled)).err(),
            Some(Error::Cancelled)
        );
    }
    #[cfg(unix)]
    #[test]
    fn symlink_data_is_rejected_even_when_pcm_matches() {
        use std::os::unix::fs::symlink;
        let t = tempfile::tempdir().unwrap();
        let (d, b) = crate::public_pipe::tests::fixture();
        std::fs::write(t.path().join("source"), b).unwrap();
        let p = t.path().join("public-preload-link.pcm");
        symlink(t.path().join("source"), &p).unwrap();
        let x = PublicWindowPreparation {
            prepared_descriptor: Some(d),
            data_path: p.to_str().unwrap().into(),
            output_id: "00000000-0000-4000-8000-000000000003".into(),
            bundle_sha256: "a".repeat(64),
        };
        assert!(load(t.path(), &x.output_id, &x, &|| Ok(())).is_err());
    }
}
