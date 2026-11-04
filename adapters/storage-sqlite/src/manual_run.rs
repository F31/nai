//! User-declared checkpoints, never playback or publication authority.
use super::*;
use serde_json::Value;

pub(super) fn validate(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    x: &FoundationObject,
    now: i64,
) -> Result<()> {
    let value: Value = serde_json::from_str(&x.body).map_err(|_| Error::InvalidArgument)?;
    if x.expected_revision > 0 {
        let (_, digest) = object(conn, &x.workspace_id, &x.object_id, 0)?;
        let old: Value =
            serde_json::from_str(&body(root, &digest)?).map_err(|_| Error::IntegrityFailed)?;
        if ["snapshot", "show", "cue_ids"]
            .iter()
            .any(|key| old[key] != value[key])
            || old["status"] == "ended" && value["status"] != "ended"
        {
            return Err(Error::RevisionConflict);
        }
        return Ok(());
    }
    let snapshot_id = value["snapshot"]["object_id"]
        .as_str()
        .ok_or(Error::InvalidArgument)?;
    let snapshot = publication::read(conn, root, fence, op, &x.workspace_id, snapshot_id, now)?;
    if snapshot.publication_status != PublicationStatus::PrivateContent as i32 {
        return Err(Error::CapabilityUnavailable);
    }
    if Some(snapshot.sha256.as_str()) != value["snapshot"]["sha256"].as_str()
        || Some(snapshot.revision) != value["snapshot"]["revision"].as_u64()
    {
        return Err(Error::RevisionConflict);
    }
    let manifest: Value =
        serde_json::from_str(&snapshot.body).map_err(|_| Error::IntegrityFailed)?;
    if value["show"] != manifest["refs"]["show"] {
        return Err(Error::RevisionConflict);
    }
    let docs =
        publication::documents(conn, root, fence, &x.workspace_id, &manifest["refs"], false)?;
    let cues: Vec<Value> = docs[0]["cues"]
        .as_array()
        .ok_or(Error::IntegrityFailed)?
        .iter()
        .map(|cue| cue["id"].clone())
        .collect();
    if value["cue_ids"] != Value::Array(cues) {
        return Err(Error::InvalidArgument);
    }
    Ok(())
}
