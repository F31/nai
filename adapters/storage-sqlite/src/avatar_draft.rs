//! Atomic private avatar-draft selection; never a public media authorization.
use super::*;

pub(super) fn prepare(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    x: &FoundationObject,
    now: i64,
) -> Result<()> {
    let doc: serde_json::Value =
        serde_json::from_str(&x.body).map_err(|_| Error::InvalidArgument)?;
    let selected = doc["selected"].as_bool().ok_or(Error::InvalidArgument)?;
    let revoked = doc["revoked"].as_bool().ok_or(Error::InvalidArgument)?;
    if selected && revoked {
        return Err(Error::InvalidArgument);
    }
    if x.expected_revision > 0 {
        let (_, old_hash) = object(tx, &x.workspace_id, &x.object_id, x.expected_revision)?;
        let old: serde_json::Value =
            serde_json::from_str(&body(root, &old_hash)?).map_err(|_| Error::IntegrityFailed)?;
        if old["revoked"] == true && !revoked {
            return Err(Error::PermissionDenied);
        }
        for key in [
            "avatar_id",
            "package_version",
            "package_sha256",
            "license_sha256",
            "width",
            "height",
        ] {
            if old[key] != doc[key] {
                return Err(Error::RevisionConflict);
            }
        }
    }
    if !selected {
        return Ok(());
    }
    let mut query = tx.prepare("SELECT id,revision FROM objects WHERE workspace_id=?1 AND deleted=0 AND id<>?2 ORDER BY id LIMIT 257").wb()?;
    let rows = query
        .query_map(params![x.workspace_id, x.object_id], |r| {
            Ok((r.get::<_, String>(0)?, uint(r, 1)?))
        })
        .wb()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .wb()?;
    if rows.len() > 256 {
        return Err(Error::ResourceExhausted);
    }
    drop(query);
    for (id, revision) in rows {
        op.check()?;
        let (_, hash) = object(tx, &x.workspace_id, &id, revision)?;
        let old = body(root, &hash)?;
        let Ok(mut other) = serde_json::from_str::<serde_json::Value>(&old) else {
            continue;
        };
        if other["kind"] != "avatar_profile_draft" || other["selected"] != true {
            continue;
        }
        fence.check_scope(&object_scope(&x.workspace_id, &id))?;
        if revision >= 1024 {
            return Err(Error::ResourceExhausted);
        }
        other["selected"] = false.into();
        let bytes = serde_json::to_vec(&other).map_err(|_| Error::IntegrityFailed)?;
        let hash = store_body(root, &bytes)?;
        let next = revision + 1;
        tx.execute("UPDATE objects SET revision=?1,updated_at=?2 WHERE workspace_id=?3 AND id=?4 AND revision=?5",params![int(next)?,now,x.workspace_id,id,int(revision)?]).wb()?;
        tx.execute(
            "INSERT INTO object_versions VALUES(?1,?2,?3,?4,?5,?5)",
            params![x.workspace_id, id, int(next)?, hash, now],
        )
        .wb()?;
        audit(
            tx,
            &x.workspace_id,
            actor,
            "avatar_draft_deselected",
            &id,
            now,
        )?;
    }
    Ok(())
}
