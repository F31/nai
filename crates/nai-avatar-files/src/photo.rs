//! Bounded JPEG header inspection. This is not decoding or authorization.
use sha2::{Digest, Sha256};

pub const MAX_PHOTO_BYTES: usize = 8 * 1024 * 1024;
const MAX_DIMENSION: u32 = 4096;
const MAX_PIXELS: u64 = 16 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub struct PhotoMetadata {
    pub width: u32,
    pub height: u32,
    pub sha256: String,
}

/// Checks bounded JPEG framing and dimensions only; a decoder must still
/// validate all pixels in an isolated, resource-limited context.
pub fn inspect_jpeg(data: &[u8]) -> Result<PhotoMetadata, &'static str> {
    if data.len() < 8 || data.len() > MAX_PHOTO_BYTES || !data.starts_with(&[0xff, 0xd8]) {
        return Err("PHOTO_INVALID");
    }
    // Some legitimate JPEGs carry trailing metadata. Bound it, and never
    // interpret it as an authorization or decoded image.
    let end = data
        .windows(2)
        .rposition(|w| w == [0xff, 0xd9])
        .and_then(|n| n.checked_add(2))
        .ok_or("PHOTO_INVALID")?;
    if data.len() - end > 4096 || end < 8 {
        return Err("PHOTO_INVALID");
    }
    let mut i = 2usize;
    let mut dimensions = None;
    let mut found_scan = false;
    while i < end - 2 {
        if data[i] != 0xff {
            return Err("PHOTO_INVALID");
        }
        while i < end - 2 && data[i] == 0xff {
            i += 1;
        }
        if i >= end - 2 {
            return Err("PHOTO_INVALID");
        }
        let marker = data[i];
        i += 1;
        if marker == 0xda {
            if i.checked_add(2).filter(|n| *n <= end - 2).is_none() {
                return Err("PHOTO_INVALID");
            }
            let scan_length = u16::from_be_bytes([data[i], data[i + 1]]) as usize;
            if scan_length < 6
                || i.checked_add(scan_length)
                    .filter(|n| *n < end - 2)
                    .is_none()
            {
                return Err("PHOTO_INVALID");
            }
            found_scan = true;
            break;
        }
        if marker == 0x01 {
            continue;
        }
        if marker == 0 || marker == 0xd8 || marker == 0xd9 || (0xd0..=0xd7).contains(&marker) {
            return Err("PHOTO_INVALID");
        }
        if i.checked_add(2).filter(|n| *n <= end - 2).is_none() {
            return Err("PHOTO_INVALID");
        }
        let length = u16::from_be_bytes([data[i], data[i + 1]]) as usize;
        if length < 2 || i.checked_add(length).filter(|n| *n <= end - 2).is_none() {
            return Err("PHOTO_INVALID");
        }
        if matches!(marker,0xc0..=0xc3|0xc5..=0xc7|0xc9..=0xcb|0xcd..=0xcf) {
            if dimensions.is_some() || length < 8 {
                return Err("PHOTO_INVALID");
            }
            let precision = data[i + 2];
            let height = u16::from_be_bytes([data[i + 3], data[i + 4]]) as u32;
            let width = u16::from_be_bytes([data[i + 5], data[i + 6]]) as u32;
            let components = data[i + 7] as usize;
            if precision != 8
                || components == 0
                || components > 4
                || length != 8 + 3 * components
                || width == 0
                || height == 0
                || width > MAX_DIMENSION
                || height > MAX_DIMENSION
                || u64::from(width) * u64::from(height) > MAX_PIXELS
            {
                return Err("PHOTO_DIMENSIONS_REJECTED");
            }
            dimensions = Some((width, height));
        }
        i += length;
    }
    let (width, height) = dimensions.ok_or("PHOTO_INVALID")?;
    if !found_scan {
        return Err("PHOTO_INVALID");
    }
    Ok(PhotoMetadata {
        width,
        height,
        sha256: Sha256::digest(data)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    })
}

/// Read a content-addressed stored photo under a held root handle.
/// Header validation is not full image decoding or an authorization.
pub fn read_stored_photo(
    root: &std::path::Path,
    digest: &str,
) -> Result<(Vec<u8>, PhotoMetadata), String> {
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("PHOTO_IDENTITY_REJECTED".into());
    }
    let bytes = crate::SelectedDirectory::open(root)?
        .child("avatar-photos")?
        .read(&format!("{digest}.jpg"), MAX_PHOTO_BYTES)?;
    let meta = inspect_jpeg(&bytes).map_err(str::to_owned)?;
    if meta.sha256 != digest {
        return Err("PHOTO_INTEGRITY_FAILED".into());
    }
    Ok((bytes, meta))
}

/// Write an exact snapshot photo set into a new destination-owned directory.
/// A failed destination is incomplete and must not be treated as a backup.
pub fn copy_stored_photos<F: Fn() -> Result<(), String>>(
    source: &std::path::Path,
    destination: &std::path::Path,
    digests: &[String],
    check: F,
) -> Result<(), String> {
    check()?;
    if digests.len() > 1024 {
        return Err("PHOTO_SIZE_REJECTED".into());
    }
    let src = std::fs::canonicalize(source).map_err(|_| "PHOTO_PATH_REJECTED")?;
    let dst = std::fs::canonicalize(destination).map_err(|_| "PHOTO_PATH_REJECTED")?;
    if src == dst || src.starts_with(&dst) {
        return Err("PHOTO_PATH_REJECTED".into());
    }
    let mut unique = std::collections::BTreeSet::new();
    for digest in digests {
        check()?;
        if !unique.insert(digest) {
            return Err("PHOTO_IDENTITY_REJECTED".into());
        }
        read_stored_photo(source, digest)?;
    }
    if digests.is_empty() {
        return check();
    }
    let photo_root =
        std::fs::canonicalize(source.join("avatar-photos")).map_err(|_| "PHOTO_PATH_REJECTED")?;
    if photo_root.starts_with(&dst) || dst.starts_with(&photo_root) {
        return Err("PHOTO_PATH_REJECTED".into());
    }
    let target = crate::SelectedDirectory::open(destination)?.create_child("avatar-photos")?;
    for digest in digests {
        check()?;
        let (bytes, _) = read_stored_photo(source, digest)?;
        target.write_new(&format!("{digest}.jpg"), &bytes)?;
        check()?;
        read_stored_photo(destination, digest)?;
    }
    check()
}

/// Install a private, content-addressed source photo. Existing bytes are never
/// overwritten; a failed write can leave a rejected partial file for inspection.
pub fn store_photo(root: &std::path::Path, bytes: &[u8]) -> Result<PhotoMetadata, String> {
    let meta = inspect_jpeg(bytes).map_err(str::to_owned)?;
    let base = crate::SelectedDirectory::open(root)?;
    let photos = match base.child("avatar-photos") {
        Ok(d) => d,
        Err(_) => base.create_child("avatar-photos")?,
    };
    let name = format!("{}.jpg", meta.sha256);
    if photos.write_new(&name, bytes).is_err() {
        let (existing, _) = read_stored_photo(root, &meta.sha256)?;
        if existing != bytes {
            return Err("PHOTO_INTEGRITY_FAILED".into());
        }
    }
    let (_, observed) = read_stored_photo(root, &meta.sha256)?;
    if observed != meta {
        return Err("PHOTO_INTEGRITY_FAILED".into());
    }
    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn jpeg(width: u16, height: u16) -> Vec<u8> {
        let mut v = vec![0xff, 0xd8, 0xff, 0xc0, 0, 11, 8];
        v.extend(height.to_be_bytes());
        v.extend(width.to_be_bytes());
        v.extend([
            1, 1, 0x11, 0, 0xff, 0xda, 0, 8, 1, 1, 0, 0, 63, 0, 0, 0xff, 0xd9,
        ]);
        v
    }
    #[test]
    fn jpeg_header_limits_and_corruption_are_rejected() {
        let good = jpeg(640, 480);
        let p = inspect_jpeg(&good).unwrap();
        assert_eq!((p.width, p.height), (640, 480));
        assert_eq!(p.sha256.len(), 64);
        assert!(inspect_jpeg(b"not jpeg").is_err());
        assert!(inspect_jpeg(&jpeg(4097, 20)).is_err());
        assert!(inspect_jpeg(&jpeg(0, 20)).is_err());
        let mut bad = good.clone();
        bad[5] = 255;
        assert!(inspect_jpeg(&bad).is_err());
        assert!(inspect_jpeg(&good[..good.len() - 2]).is_err());
        assert!(inspect_jpeg(&vec![0; MAX_PHOTO_BYTES + 1]).is_err());
        let mut trailing = good.clone();
        trailing.extend([0u8; 24]);
        assert!(inspect_jpeg(&trailing).is_ok());
        trailing.extend(vec![0u8; 4097]);
        assert!(inspect_jpeg(&trailing).is_err());
        let mut scan = good.clone();
        let at = scan.windows(2).position(|w| w == [0xff, 0xda]).unwrap();
        scan[at + 2] = 255;
        assert!(inspect_jpeg(&scan).is_err());
    }
    #[test]
    #[ignore = "explicit local user-provided photo only; header probe is not a decoder"]
    fn local_photo_header_probe() {
        let path = std::path::PathBuf::from(
            std::env::var_os("NAI_LOCAL_PHOTO_PROBE").expect("explicit path"),
        );
        let parent = path.parent().unwrap();
        let name = path.file_name().unwrap().to_str().unwrap();
        let bytes = crate::SelectedDirectory::open(parent)
            .unwrap()
            .read(name, MAX_PHOTO_BYTES)
            .unwrap();
        let meta = inspect_jpeg(&bytes).unwrap();
        assert!(meta.width > 0 && meta.height > 0);
        let isolated = tempfile::tempdir().unwrap();
        let stored = store_photo(isolated.path(), &bytes).unwrap();
        assert_eq!(stored, meta);
        assert_eq!(store_photo(isolated.path(), &bytes).unwrap(), meta);
        assert_eq!(
            read_stored_photo(isolated.path(), &meta.sha256).unwrap().0,
            bytes
        );
        let backup = tempfile::tempdir().unwrap();
        copy_stored_photos(
            isolated.path(),
            backup.path(),
            std::slice::from_ref(&meta.sha256),
            || Ok(()),
        )
        .unwrap();
        assert_eq!(
            read_stored_photo(backup.path(), &meta.sha256).unwrap().0,
            bytes
        );
        eprintln!(
            "local JPEG header and isolated storage/backup: {}x{}; {} bytes",
            meta.width,
            meta.height,
            bytes.len()
        );
    }
}
