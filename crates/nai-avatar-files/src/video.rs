//! Bounded ISO BMFF header inspection. This is not decoding, face scoring or authorization.
use sha2::{Digest, Sha256};

pub const MAX_VIDEO_BYTES: usize = 32 * 1024 * 1024;
pub const INSPECT_VERSION: &str = "mp4-ftyp-mvhd-v1";
pub const MIN_DURATION_MS: u64 = 1000;
pub const MAX_DURATION_MS: u64 = 120_000;
const MAX_BOXES: usize = 4096;
const MAX_DEPTH: u32 = 8;
const CACHE_BYTES: usize = 4096;
const ALLOWED_BRANDS: [[u8; 4]; 4] = [*b"isom", *b"iso2", *b"mp41", *b"mp42"];
const ENCRYPTED: [[u8; 4]; 6] = [*b"encv", *b"enca", *b"sinf", *b"tenc", *b"senc", *b"schm"];
const CONTAINERS: [[u8; 4]; 7] = [
    *b"moov", *b"trak", *b"mdia", *b"minf", *b"stbl", *b"stsd", *b"edts",
];

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct VideoMetadata {
    pub sha256: String,
    pub duration_ms: u64,
    pub size_bytes: u64,
    pub inspect_version: &'static str,
}

fn hex_sha256(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn digest_name(digest: &str) -> Result<(), &'static str> {
    if digest.len() == 64
        && digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        Ok(())
    } else {
        Err("VIDEO_IDENTITY_REJECTED")
    }
}

fn u32_be(data: &[u8], at: usize) -> Option<u32> {
    data.get(at..at + 4)?
        .try_into()
        .ok()
        .map(u32::from_be_bytes)
}

fn u64_be(data: &[u8], at: usize) -> Option<u64> {
    data.get(at..at + 8)?
        .try_into()
        .ok()
        .map(u64::from_be_bytes)
}

fn fourcc(data: &[u8], at: usize) -> Option<[u8; 4]> {
    data.get(at..at + 4)?.try_into().ok()
}

fn duration_ms(timescale: u32, duration: u64) -> Result<u64, &'static str> {
    if timescale == 0 || duration == 0 {
        return Err("VIDEO_DURATION_UNAVAILABLE");
    }
    let ms = duration
        .checked_mul(1000)
        .map(|n| n / u64::from(timescale))
        .ok_or("VIDEO_DURATION_UNAVAILABLE")?;
    if !(MIN_DURATION_MS..=MAX_DURATION_MS).contains(&ms) {
        return Err("VIDEO_DURATION_REJECTED");
    }
    Ok(ms)
}

struct Scan {
    boxes: usize,
    brands: bool,
    duration_ms: Option<u64>,
}

fn walk(
    data: &[u8],
    start: usize,
    end: usize,
    depth: u32,
    scan: &mut Scan,
) -> Result<(), &'static str> {
    if depth > MAX_DEPTH {
        return Err("VIDEO_FORMAT_UNSUPPORTED");
    }
    let mut at = start;
    let mut first = true;
    while at + 8 <= end {
        scan.boxes += 1;
        if scan.boxes > MAX_BOXES {
            return Err("VIDEO_FORMAT_UNSUPPORTED");
        }
        let size = u32_be(data, at).ok_or("VIDEO_FORMAT_UNSUPPORTED")?;
        let kind = fourcc(data, at + 4).ok_or("VIDEO_FORMAT_UNSUPPORTED")?;
        let (header, total) = if size == 1 {
            let large = u64_be(data, at + 8).ok_or("VIDEO_FORMAT_UNSUPPORTED")?;
            if large < 16 || large > (end - at) as u64 {
                return Err("VIDEO_FORMAT_UNSUPPORTED");
            }
            (16usize, large as usize)
        } else if size == 0 {
            (8usize, end - at)
        } else {
            let n = size as usize;
            if n < 8 || at.checked_add(n).filter(|x| *x <= end).is_none() {
                return Err("VIDEO_FORMAT_UNSUPPORTED");
            }
            (8usize, n)
        };
        if depth == 0 && first && kind != *b"ftyp" {
            return Err("VIDEO_FORMAT_UNSUPPORTED");
        }
        first = false;
        if ENCRYPTED.contains(&kind) {
            return Err("VIDEO_FORMAT_UNSUPPORTED");
        }
        let payload = at + header;
        let stop = at + total;
        if kind == *b"ftyp" {
            if payload + 8 > stop {
                return Err("VIDEO_FORMAT_UNSUPPORTED");
            }
            let mut brand_ok =
                ALLOWED_BRANDS.contains(&fourcc(data, payload).ok_or("VIDEO_FORMAT_UNSUPPORTED")?);
            let mut brand = payload + 8;
            while brand + 4 <= stop {
                brand_ok |= ALLOWED_BRANDS
                    .contains(&fourcc(data, brand).ok_or("VIDEO_FORMAT_UNSUPPORTED")?);
                brand += 4;
            }
            if !(stop - payload - 8).is_multiple_of(4) {
                return Err("VIDEO_FORMAT_UNSUPPORTED");
            }
            scan.brands = brand_ok;
        } else if kind == *b"mvhd" {
            if scan.duration_ms.is_some() || payload >= stop {
                return Err("VIDEO_FORMAT_UNSUPPORTED");
            }
            let version = data[payload];
            let (timescale, duration) = if version == 0 {
                (
                    u32_be(data, payload + 12).ok_or("VIDEO_DURATION_UNAVAILABLE")?,
                    u64::from(u32_be(data, payload + 16).ok_or("VIDEO_DURATION_UNAVAILABLE")?),
                )
            } else if version == 1 {
                (
                    u32_be(data, payload + 20).ok_or("VIDEO_DURATION_UNAVAILABLE")?,
                    u64_be(data, payload + 24).ok_or("VIDEO_DURATION_UNAVAILABLE")?,
                )
            } else {
                return Err("VIDEO_FORMAT_UNSUPPORTED");
            };
            scan.duration_ms = Some(duration_ms(timescale, duration)?);
        } else if CONTAINERS.contains(&kind) {
            walk(data, payload, stop, depth + 1, scan)?;
        }
        at = stop;
    }
    if at != end {
        return Err("VIDEO_FORMAT_UNSUPPORTED");
    }
    Ok(())
}

/// Checks bounded MP4 framing, brands and mvhd duration only; a decoder must
/// still validate samples in an isolated, resource-limited context.
pub fn inspect_mp4(data: &[u8]) -> Result<VideoMetadata, &'static str> {
    if data.is_empty() || data.len() > MAX_VIDEO_BYTES {
        return Err("VIDEO_SIZE_REJECTED");
    }
    if data.len() < 16 {
        return Err("VIDEO_FORMAT_UNSUPPORTED");
    }
    let mut scan = Scan {
        boxes: 0,
        brands: false,
        duration_ms: None,
    };
    walk(data, 0, data.len(), 0, &mut scan)?;
    if !scan.brands {
        return Err("VIDEO_FORMAT_UNSUPPORTED");
    }
    let duration_ms = scan.duration_ms.ok_or("VIDEO_DURATION_UNAVAILABLE")?;
    Ok(VideoMetadata {
        sha256: hex_sha256(data),
        duration_ms,
        size_bytes: data.len() as u64,
        inspect_version: INSPECT_VERSION,
    })
}

fn cache_matches(raw: &[u8], meta: &VideoMetadata) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(raw) else {
        return false;
    };
    value["inspect_version"] == INSPECT_VERSION
        && value["sha256"] == meta.sha256
        && value["size_bytes"] == meta.size_bytes
        && value["duration_ms"] == meta.duration_ms
}

fn cache_dir(root: &std::path::Path) -> Result<crate::SelectedDirectory, String> {
    let base = crate::SelectedDirectory::open(root)?;
    match base.child("avatar-video-cache") {
        Ok(d) => Ok(d),
        Err(_) => base.create_child("avatar-video-cache"),
    }
}

fn write_inspect_cache(root: &std::path::Path, meta: &VideoMetadata) -> Result<(), String> {
    let body = serde_json::json!({
        "inspect_version": meta.inspect_version,
        "sha256": meta.sha256,
        "size_bytes": meta.size_bytes,
        "duration_ms": meta.duration_ms,
    })
    .to_string();
    let cache = cache_dir(root)?;
    let name = format!("{}.json", meta.sha256);
    if cache.write_new(&name, body.as_bytes()).is_err() {
        if let Ok(existing) = cache.read(&name, CACHE_BYTES)
            && cache_matches(&existing, meta)
        {
            return Ok(());
        }
        cache.remove_file(&name)?;
        cache.write_new(&name, body.as_bytes())?;
    }
    Ok(())
}

/// Returns cached inspect metadata only when it agrees with a fresh inspect.
/// Tampered or stale cache entries are unused.
pub fn cached_metadata(root: &std::path::Path, bytes: &[u8]) -> Option<VideoMetadata> {
    let meta = inspect_mp4(bytes).ok()?;
    digest_name(&meta.sha256).ok()?;
    let raw = crate::SelectedDirectory::open(root)
        .ok()?
        .child("avatar-video-cache")
        .ok()?
        .read(&format!("{}.json", meta.sha256), CACHE_BYTES)
        .ok()?;
    cache_matches(&raw, &meta).then_some(meta)
}

/// Drop the inspect cache for one video identity. Missing cache is success.
/// Content-addressed source bytes are left in place.
pub fn revoke_inspect_cache(root: &std::path::Path, digest: &str) -> Result<(), String> {
    digest_name(digest).map_err(str::to_owned)?;
    let Ok(cache) =
        crate::SelectedDirectory::open(root).and_then(|d| d.child("avatar-video-cache"))
    else {
        return Ok(());
    };
    let name = format!("{digest}.json");
    if cache.remove_file(&name).is_err() {
        if cache.read(&name, CACHE_BYTES).is_err() {
            return Ok(());
        }
        return Err("VIDEO_CACHE_UNAVAILABLE".into());
    }
    Ok(())
}

/// Read a content-addressed stored video under a held root handle.
/// Header validation is not full decoding or an authorization.
pub fn read_stored_video(
    root: &std::path::Path,
    digest: &str,
) -> Result<(Vec<u8>, VideoMetadata), String> {
    digest_name(digest).map_err(str::to_owned)?;
    let bytes = crate::SelectedDirectory::open(root)?
        .child("avatar-videos")?
        .read(&format!("{digest}.mp4"), MAX_VIDEO_BYTES)?;
    let meta = inspect_mp4(&bytes).map_err(str::to_owned)?;
    if meta.sha256 != digest {
        return Err("VIDEO_INTEGRITY_FAILED".into());
    }
    Ok((bytes, meta))
}

/// Write an exact snapshot video set into a new destination-owned directory.
/// A failed destination is incomplete and must not be treated as a backup.
pub fn copy_stored_videos<F: Fn() -> Result<(), String>>(
    source: &std::path::Path,
    destination: &std::path::Path,
    digests: &[String],
    check: F,
) -> Result<(), String> {
    check()?;
    if digests.len() > 1024 {
        return Err("VIDEO_SIZE_REJECTED".into());
    }
    let src = std::fs::canonicalize(source).map_err(|_| "VIDEO_PATH_REJECTED")?;
    let dst = std::fs::canonicalize(destination).map_err(|_| "VIDEO_PATH_REJECTED")?;
    if src == dst || src.starts_with(&dst) {
        return Err("VIDEO_PATH_REJECTED".into());
    }
    let mut unique = std::collections::BTreeSet::new();
    for digest in digests {
        check()?;
        if !unique.insert(digest) {
            return Err("VIDEO_IDENTITY_REJECTED".into());
        }
        read_stored_video(source, digest)?;
    }
    if digests.is_empty() {
        return check();
    }
    let video_root =
        std::fs::canonicalize(source.join("avatar-videos")).map_err(|_| "VIDEO_PATH_REJECTED")?;
    if video_root.starts_with(&dst) || dst.starts_with(&video_root) {
        return Err("VIDEO_PATH_REJECTED".into());
    }
    let target = crate::SelectedDirectory::open(destination)?.create_child("avatar-videos")?;
    for digest in digests {
        check()?;
        let (bytes, _) = read_stored_video(source, digest)?;
        target.write_new(&format!("{digest}.mp4"), &bytes)?;
        check()?;
        read_stored_video(destination, digest)?;
    }
    check()
}

/// Install a private, content-addressed source video. Existing bytes are never
/// overwritten; a failed write can leave a rejected partial file for inspection.
pub fn store_video(root: &std::path::Path, bytes: &[u8]) -> Result<VideoMetadata, String> {
    let meta = inspect_mp4(bytes).map_err(str::to_owned)?;
    let base = crate::SelectedDirectory::open(root)?;
    let videos = match base.child("avatar-videos") {
        Ok(d) => d,
        Err(_) => base.create_child("avatar-videos")?,
    };
    let name = format!("{}.mp4", meta.sha256);
    if videos.write_new(&name, bytes).is_err() {
        let (existing, _) = read_stored_video(root, &meta.sha256)?;
        if existing != bytes {
            return Err("VIDEO_INTEGRITY_FAILED".into());
        }
    }
    let (_, observed) = read_stored_video(root, &meta.sha256)?;
    if observed != meta {
        return Err("VIDEO_INTEGRITY_FAILED".into());
    }
    write_inspect_cache(root, &meta)?;
    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box_of(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut v = ((8 + payload.len()) as u32).to_be_bytes().to_vec();
        v.extend(kind);
        v.extend(payload);
        v
    }

    fn fixture_mp4(timescale: u32, duration: u32) -> Vec<u8> {
        let mut ftyp = Vec::from(*b"isom");
        ftyp.extend(0u32.to_be_bytes());
        ftyp.extend(b"isomiso2mp41");
        let mut mvhd = vec![0; 4];
        mvhd.extend([0u8; 8]);
        mvhd.extend(timescale.to_be_bytes());
        mvhd.extend(duration.to_be_bytes());
        let mut v = box_of(b"ftyp", &ftyp);
        v.extend(box_of(b"moov", &box_of(b"mvhd", &mvhd)));
        v.extend(box_of(b"mdat", b"x"));
        v
    }

    #[test]
    fn mp4_header_limits_and_corruption_are_rejected() {
        let good = fixture_mp4(1000, 5000);
        let meta = inspect_mp4(&good).unwrap();
        assert_eq!(meta.duration_ms, 5000);
        assert_eq!(meta.inspect_version, INSPECT_VERSION);
        assert_eq!(meta.sha256.len(), 64);
        assert_eq!(
            inspect_mp4(b"not mp4").unwrap_err(),
            "VIDEO_FORMAT_UNSUPPORTED"
        );
        assert_eq!(inspect_mp4(&[]).unwrap_err(), "VIDEO_SIZE_REJECTED");
        assert_eq!(
            inspect_mp4(&vec![0; MAX_VIDEO_BYTES + 1]).unwrap_err(),
            "VIDEO_SIZE_REJECTED"
        );
        assert_eq!(
            inspect_mp4(&fixture_mp4(1000, 500)).unwrap_err(),
            "VIDEO_DURATION_REJECTED"
        );
        assert_eq!(
            inspect_mp4(&fixture_mp4(1000, 121_000)).unwrap_err(),
            "VIDEO_DURATION_REJECTED"
        );
        assert_eq!(
            inspect_mp4(&fixture_mp4(1000, 0)).unwrap_err(),
            "VIDEO_DURATION_UNAVAILABLE"
        );
        let mut encrypted = fixture_mp4(1000, 5000);
        encrypted.extend(box_of(b"encv", b"x"));
        assert_eq!(
            inspect_mp4(&encrypted).unwrap_err(),
            "VIDEO_FORMAT_UNSUPPORTED"
        );
        let mut qt = fixture_mp4(1000, 5000);
        qt.splice(8..12, *b"qt  ");
        qt.splice(16..28, *b"qt  qt  qt  ");
        assert_eq!(inspect_mp4(&qt).unwrap_err(), "VIDEO_FORMAT_UNSUPPORTED");
    }

    #[test]
    fn inspect_cache_agrees_with_bytes_and_drops_on_revoke() {
        let isolated = tempfile::tempdir().unwrap();
        let bytes = fixture_mp4(1000, 4000);
        let meta = store_video(isolated.path(), &bytes).unwrap();
        assert_eq!(meta.duration_ms, 4000);
        assert_eq!(store_video(isolated.path(), &bytes).unwrap(), meta);
        assert_eq!(
            read_stored_video(isolated.path(), &meta.sha256).unwrap().0,
            bytes
        );
        assert_eq!(
            cached_metadata(isolated.path(), &bytes)
                .unwrap()
                .duration_ms,
            4000
        );
        let cache = isolated
            .path()
            .join("avatar-video-cache")
            .join(format!("{}.json", meta.sha256));
        std::fs::write(&cache, r#"{"inspect_version":"mp4-ftyp-mvhd-v1","sha256":"00","size_bytes":1,"duration_ms":4000}"#).unwrap();
        assert!(cached_metadata(isolated.path(), &bytes).is_none());
        revoke_inspect_cache(isolated.path(), &meta.sha256).unwrap();
        assert!(cached_metadata(isolated.path(), &bytes).is_none());
        assert_eq!(
            read_stored_video(isolated.path(), &meta.sha256).unwrap().0,
            bytes
        );
        let backup = tempfile::tempdir().unwrap();
        copy_stored_videos(
            isolated.path(),
            backup.path(),
            std::slice::from_ref(&meta.sha256),
            || Ok(()),
        )
        .unwrap();
        assert_eq!(
            read_stored_video(backup.path(), &meta.sha256).unwrap().0,
            bytes
        );
    }
}
