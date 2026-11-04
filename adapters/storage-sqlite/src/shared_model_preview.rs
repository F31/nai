use super::*;
pub(super) fn preview(conn: &Connection, root: &Path, op: &Operation) -> Result<FoundationResult> {
    let mut q=conn.prepare("SELECT o.workspace_id,o.id,o.revision,v.sha256,w.name FROM objects o JOIN object_versions v ON v.workspace_id=o.workspace_id AND v.object_id=o.id AND v.revision=o.revision JOIN workspaces w ON w.id=o.workspace_id WHERE o.deleted=0 ORDER BY o.workspace_id,o.id LIMIT 2049").wb()?;
    let objects = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                uint(r, 2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })
        .wb()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .wb()?;
    if objects.len() > 2048 {
        return Err(Error::ResourceExhausted);
    }
    let mut rows = vec![];
    for (ws, id, revision, sha, workspace_name) in objects {
        op.check()?;
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&body(root, &sha)?) else {
            continue;
        };
        let capability = match v["kind"].as_str() {
            Some("publication_llm_config") => "llm",
            Some("publication_tts_config") => "tts",
            _ => continue,
        };
        if v["removed"] == true {
            continue;
        }
        nai_runtime_core::publication::artifact(&v.to_string())?;
        rows.push(SharedModelPreviewRow {
            source: Some(SharedModelMigrationEntry {
                catalog_model_id: String::new(),
                source_workspace_id: ws.clone(),
                source_model_id: id,
                source_revision: revision,
                source_sha256: sha,
                capability: capability.into(),
            }),
            name: v["name"].as_str().ok_or(Error::IntegrityFailed)?.into(),
            workspace_name,
            recycled: workspace_lifecycle::state(conn, &ws)? == "workspace_recycle",
            credential_saved: !v["secret"].is_null(),
            issues: vec![],
        });
        if rows.len() > 64 {
            return Err(Error::ResourceExhausted);
        }
    }
    let keys = rows
        .iter()
        .map(|r| {
            let s = r.source.as_ref().expect("constructed source");
            (
                s.source_model_id.clone(),
                format!("{}:{}", s.capability, r.name.trim().to_lowercase()),
            )
        })
        .collect::<Vec<_>>();
    for (i, row) in rows.iter_mut().enumerate() {
        if keys.iter().filter(|k| k.0 == keys[i].0).count() > 1 {
            row.issues.push("DUPLICATE_MODEL_ID".into());
        }
        if keys.iter().filter(|k| k.1 == keys[i].1).count() > 1 {
            row.issues.push("DUPLICATE_NAME".into());
        }
        if !row.credential_saved {
            row.issues.push("CREDENTIAL_MISSING".into());
        }
    }
    let mut preview = SharedModelPreview {
        rows,
        plan_sha256: String::new(),
        executable: false,
    };
    preview.plan_sha256 = hash(&preview.encode_to_vec());
    let result = FoundationResult {
        shared_model_preview: Some(preview),
        ..Default::default()
    };
    if result.encoded_len() > 60000 {
        return Err(Error::FrameLimit);
    }
    op.check()?;
    Ok(result)
}
