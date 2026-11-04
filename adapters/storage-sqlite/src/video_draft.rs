//! Private video metadata; never a public output grant.
use super::*;
pub(super) fn prepare(
    tx: &Transaction<'_>,
    root: &Path,
    op: &Operation,
    x: &FoundationObject,
) -> Result<()> {
    op.check()?;
    let doc: serde_json::Value =
        serde_json::from_str(&x.body).map_err(|_| Error::InvalidArgument)?;
    let digest = doc["video_sha256"].as_str().ok_or(Error::InvalidArgument)?;
    if x.expected_revision > 0 {
        let (_, old_hash) = object(tx, &x.workspace_id, &x.object_id, x.expected_revision)?;
        let old: serde_json::Value =
            serde_json::from_str(&body(root, &old_hash)?).map_err(|_| Error::IntegrityFailed)?;
        for key in [
            "video_sha256",
            "duration_ms",
            "inspect_version",
            "realistic_drive",
        ] {
            if old[key] != doc[key] {
                return Err(Error::RevisionConflict);
            }
        }
        if old["revoked"] == true && doc["revoked"] != true {
            return Err(Error::PermissionDenied);
        }
        // A revoked video must remain revocable even if its source is missing.
        if doc["revoked"] == true {
            nai_avatar_files::video::revoke_inspect_cache(root, digest)
                .map_err(|_| Error::IntegrityFailed)?;
            return op.check();
        }
    }
    if doc["revoked"] == true {
        nai_avatar_files::video::revoke_inspect_cache(root, digest)
            .map_err(|_| Error::IntegrityFailed)?;
        return op.check();
    }
    let (_, meta) = nai_avatar_files::video::read_stored_video(root, digest)
        .map_err(|_| Error::IntegrityFailed)?;
    if doc["duration_ms"].as_u64() != Some(meta.duration_ms)
        || doc["inspect_version"].as_str() != Some(nai_avatar_files::video::INSPECT_VERSION)
        || doc["realistic_drive"].as_str() != Some("model_missing")
    {
        return Err(Error::IntegrityFailed);
    }
    op.check()
}
