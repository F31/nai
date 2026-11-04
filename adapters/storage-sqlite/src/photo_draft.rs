//! Private photo metadata; never a public output grant.
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
    let digest = doc["photo_sha256"].as_str().ok_or(Error::InvalidArgument)?;
    if x.expected_revision > 0 {
        let (_, old_hash) = object(tx, &x.workspace_id, &x.object_id, x.expected_revision)?;
        let old: serde_json::Value =
            serde_json::from_str(&body(root, &old_hash)?).map_err(|_| Error::IntegrityFailed)?;
        for key in ["photo_sha256", "width", "height"] {
            if old[key] != doc[key] {
                return Err(Error::RevisionConflict);
            }
        }
        if old["revoked"] == true && doc["revoked"] != true {
            return Err(Error::PermissionDenied);
        }
        // A revoked photo must remain revocable even if its source is missing.
        if doc["revoked"] == true {
            return op.check();
        }
    }
    let (_, meta) = nai_avatar_files::photo::read_stored_photo(root, digest)
        .map_err(|_| Error::IntegrityFailed)?;
    if doc["width"].as_u64() != Some(u64::from(meta.width))
        || doc["height"].as_u64() != Some(u64::from(meta.height))
    {
        return Err(Error::IntegrityFailed);
    }
    op.check()
}
