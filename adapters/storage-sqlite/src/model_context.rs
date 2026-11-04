//! Shared immutable source identity. No model, payment or delivery authority.
use super::*;
use serde_json::Value;
pub(super) fn from_manifest(
    ws: &str,
    snapshot: &str,
    sha: &str,
    manifest: &Value,
) -> Result<AgentTaskContext> {
    nai_runtime_core::task_context::source(
        ws,
        snapshot,
        sha,
        &hash(manifest["refs"].to_string().as_bytes()),
    )
}
pub(super) fn verify(stored: &Value, expected: &AgentTaskContext) -> Result<()> {
    if stored.is_null() {
        return Err(Error::CapabilityUnavailable);
    }
    if nai_runtime_core::task_context::from_value(stored)? != *expected {
        return Err(Error::RevisionConflict);
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub(super) fn from_snapshot(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    ws: &str,
    pointer: &Value,
    now: i64,
) -> Result<AgentTaskContext> {
    let id = pointer["object_id"]
        .as_str()
        .ok_or(Error::IntegrityFailed)?;
    let snapshot = publication::read(conn, root, fence, op, ws, id, now)?;
    if snapshot.publication_status != PublicationStatus::PrivateContent as i32
        || snapshot.sha256 != pointer["sha256"]
    {
        return Err(Error::RevisionConflict);
    }
    let manifest: Value =
        serde_json::from_str(&snapshot.body).map_err(|_| Error::IntegrityFailed)?;
    let docs = publication::documents(conn, root, fence, ws, &manifest["refs"], true)?;
    nai_runtime_core::publication::closure(op, [&docs[0], &docs[1], &docs[2], &docs[3]], now)?;
    from_manifest(ws, id, &snapshot.sha256, &manifest)
}
