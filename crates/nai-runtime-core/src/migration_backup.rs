use nai_contracts::workbench::WorkbenchError as Error;
use serde_json::Value;
use std::sync::OnceLock;
pub fn manifest(bytes: &[u8]) -> Result<Value, Error> {
    if bytes.len() > 1024 * 1024 {
        return Err(Error::FrameLimit);
    }
    static SCHEMA: OnceLock<Result<jsonschema::Validator, Error>> = OnceLock::new();
    let v: Value = serde_json::from_slice(bytes).map_err(|_| Error::IntegrityFailed)?;
    let validator = SCHEMA
        .get_or_init(|| {
            let source = serde_json::from_str(include_str!(
                "../../../schemas/config/model-migration-backup.schema.json"
            ))
            .map_err(|_| Error::IntegrityFailed)?;
            jsonschema::validator_for(&source).map_err(|_| Error::IntegrityFailed)
        })
        .as_ref()
        .map_err(|e| *e)?;
    if !validator.is_valid(&v) {
        return Err(Error::IntegrityFailed);
    }
    Ok(v)
}

#[cfg(test)]
mod backup_path_tests {
    use super::*;
    use serde_json::json;
    fn backup(extra: Option<&str>) -> Vec<u8> {
        let entry = json!({"sha256":"b".repeat(64),"bytes":1});
        let mut files = serde_json::Map::new();
        for name in [
            "foundation.sqlite",
            "foundation-root.json",
            "initialized.ok",
        ] {
            files.insert(name.into(), entry.clone());
        }
        if let Some(name) = extra {
            files.insert(name.into(), entry);
        }
        serde_json::to_vec(&json!({"schema_version":1,"backup_id":"00000000-0000-0000-0000-000000000001","plan_sha256":"a".repeat(64),"credential_keys":"external_keyring_not_exported","files":files})).unwrap()
    }
    #[test]
    fn manifest_accepts_fixed_avatar_files_and_rejects_escaped_paths() {
        assert!(manifest(&backup(None)).is_ok());
        for name in [
            "manifest.json",
            "config.json",
            "LICENSE.txt",
            "asset-ids.json",
            "asset-0.rgba",
            "asset-1.rgba",
            "asset-2.rgba",
            "preview.png",
        ] {
            assert!(
                manifest(&backup(Some(&format!(
                    "avatar-packages/{}/{name}",
                    "a".repeat(64)
                ))))
                .is_ok()
            );
        }
        for name in [
            "../manifest.json",
            "asset-3.rgba",
            "key.txt",
            "preview.png/extra",
        ] {
            assert!(
                manifest(&backup(Some(&format!(
                    "avatar-packages/{}/{name}",
                    "a".repeat(64)
                ))))
                .is_err()
            );
        }
        assert!(manifest(&backup(Some("avatar-packages/INVALID/manifest.json"))).is_err());
        let digest = "a".repeat(64);
        assert!(manifest(&backup(Some(&format!("avatar-photos/{digest}.jpg")))).is_ok());
        assert!(manifest(&backup(Some(&format!("avatar-videos/{digest}.mp4")))).is_ok());
        assert!(manifest(&backup(Some(&format!("avatar-video-cache/{digest}.json")))).is_err());
        assert!(manifest(&backup(Some(&format!("avatar-videos/{digest}.jpg")))).is_err());
    }
}
