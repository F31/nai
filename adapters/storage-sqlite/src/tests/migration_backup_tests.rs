use super::*;
#[test]
fn cancelled_snapshot_does_not_copy_database() {
    let source = Connection::open_in_memory().unwrap();
    source
        .execute_batch(
            "CREATE TABLE source_marker(value TEXT); INSERT INTO source_marker VALUES('source')",
        )
        .unwrap();
    let mut destination = Connection::open_in_memory().unwrap();
    let cancelled = Operation::new(4000).unwrap();
    cancelled.cancel();
    assert_eq!(
        migration_backup::snapshot_database(&source, &mut destination, &cancelled),
        Err(Error::Cancelled)
    );
    let exists: bool = destination
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='source_marker')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!exists);
}
fn plan(s: &mut Store) -> String {
    exec(
        s,
        1,
        C::PreviewSharedModels(FoundationLlmRead {
            workspace_id: String::new(),
        }),
    )
    .unwrap()
    .shared_model_preview
    .unwrap()
    .plan_sha256
}
fn drained(s: &mut Store) {
    exec(
        s,
        1,
        C::Stop(FoundationStop {
            workspace_id: id(10),
            session_id: id(30),
        }),
    )
    .unwrap();
}
fn op() -> Operation {
    Operation::new(4000).unwrap()
}
#[test]
fn backup_restore_verification_preserves_history_and_does_not_touch_live_root() {
    let (t, mut s, _) = setup();
    exec(
        &mut s,
        1,
        C::SaveLlmModel(FoundationLlmModel {
            workspace_id: id(10),
            model_id: id(710),
            expected_revision: 0,
            name: "备份文本".into(),
            base_url: "https://example.invalid/v1".into(),
            model: "test".into(),
            enabled: true,
            api_key: vec![],
            prices_confirmed: true,
            input_minor_per_million: 1,
            output_minor_per_million: 1,
        }),
    )
    .unwrap();
    exec(
        &mut s,
        1,
        C::SaveTtsModel(FoundationTtsModel {
            protocol: "openai_speech".into(),
            language: String::new(),
            workspace_id: id(10),
            model_id: id(711),
            expected_revision: 0,
            name: "备份语音".into(),
            base_url: "https://example.invalid/v1".into(),
            model: "test-speech".into(),
            voice: "test-voice".into(),
            enabled: true,
            api_key: vec![],
            price_known: false,
            character_minor_per_million: 0,
        }),
    )
    .unwrap();
    drained(&mut s);
    root::revoke(t.path(), &s.fence, &object_scope(&id(10), &id(99))).unwrap();
    let hash = plan(&mut s);
    let receipt = s
        .prepare_model_migration_backup(&op(), &id(1), &id(700), &hash)
        .unwrap();
    let backup = s
        .verify_model_migration_backup(&op(), &id(1), &receipt)
        .unwrap();
    assert_eq!(backup["credential_keys"], "external_keyring_not_exported");
    let live = std::fs::read(t.path().join("foundation.sqlite")).unwrap();
    let restored = s
        .restore_model_backup_for_verification(&op(), &id(1), &receipt)
        .unwrap();
    let mut verification = Store::open(&restored, &id(1), false, Arc::default()).unwrap();
    let settings = exec(
        &mut verification,
        1,
        C::ReadLlmSettings(FoundationLlmRead {
            workspace_id: id(10),
        }),
    )
    .unwrap();
    assert_eq!(settings.llm_models[0].name, "备份文本");
    assert_eq!(settings.tts_models[0].name, "备份语音");
    assert!(
        verification
            .fence
            .check_scope(&object_scope(&id(10), &id(99)))
            .is_err()
    );
    let object = exec(
        &mut verification,
        1,
        C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(20),
            revision: 1,
        }),
    )
    .unwrap();
    assert_eq!(object.body, "人工编写，仅本地私有展示");
    assert_eq!(
        std::fs::read(t.path().join("foundation.sqlite")).unwrap(),
        live
    );
    assert_eq!(
        s.restore_model_backup_for_verification(&op(), &id(1), &receipt)
            .unwrap_err(),
        Error::RevisionConflict
    );
    drop(verification);
    drop(s);
    let reopened = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    reopened
        .verify_model_migration_backup(&op(), &id(1), &receipt)
        .unwrap();
}
#[test]
fn backup_rejects_active_changed_plan_unauthorized_and_reused_paths() {
    let (_t, mut s, _) = setup();
    let hash = plan(&mut s);
    assert_eq!(
        s.prepare_model_migration_backup(&op(), &id(2), &id(701), &hash)
            .unwrap_err(),
        Error::PermissionDenied
    );
    assert_eq!(
        s.prepare_model_migration_backup(&op(), &id(1), &id(701), &hash)
            .unwrap_err(),
        Error::Busy
    );
    drained(&mut s);
    assert_eq!(
        s.prepare_model_migration_backup(&op(), &id(1), &id(701), &"f".repeat(64))
            .unwrap_err(),
        Error::RevisionConflict
    );
    assert!(
        s.prepare_model_migration_backup(&op(), &id(1), "../outside", &hash)
            .is_err()
    );
    s.prepare_model_migration_backup(&op(), &id(1), &id(701), &hash)
        .unwrap();
    assert_eq!(
        s.prepare_model_migration_backup(&op(), &id(1), &id(701), &hash)
            .unwrap_err(),
        Error::RevisionConflict
    );
}
#[test]
fn damaged_or_incomplete_backup_is_not_usable() {
    let (t, mut s, _) = setup();
    drained(&mut s);
    let hash = plan(&mut s);
    let r = s
        .prepare_model_migration_backup(&op(), &id(1), &id(702), &hash)
        .unwrap();
    let base = t.path().join("model-migration-backups").join(&r.backup_id);
    std::fs::write(base.join("foundation.sqlite"), b"damaged").unwrap();
    assert!(s.verify_model_migration_backup(&op(), &id(1), &r).is_err());
    assert!(
        s.restore_model_backup_for_verification(&op(), &id(1), &r)
            .is_err()
    );
    std::fs::remove_file(base.join("manifest.json")).unwrap();
    assert!(s.verify_model_migration_backup(&op(), &id(1), &r).is_err());
    assert!(!t.path().join("model-migration-verification").exists());
}

#[test]
fn model_backup_rejects_avatar_history_before_creating_backup() {
    let (t, mut s, _) = setup();
    let avatar = serde_json::json!({"schema_version":1,"kind":"avatar_profile_draft","name":"avatar","avatar_id":"avatar","package_version":"1","package_sha256":"a".repeat(64),"license_sha256":"b".repeat(64),"width":2,"height":1,"selected":false,"revoked":false}).to_string();
    exec(
        &mut s,
        1,
        C::PutPreparation(FoundationObject {
            workspace_id: id(10),
            object_id: id(70),
            expected_revision: 0,
            body: avatar,
        }),
    )
    .unwrap();
    drained(&mut s);
    let hash = plan(&mut s);
    assert_eq!(
        s.prepare_model_migration_backup(&op(), &id(1), &id(700), &hash),
        Err(Error::IntegrityFailed)
    );
    assert!(
        !t.path()
            .join("model-migration-backups")
            .join(id(700))
            .exists()
    );
    assert!(
        exec(
            &mut s,
            1,
            C::ReadObject(FoundationReadObject {
                workspace_id: id(10),
                object_id: id(70),
                revision: 0
            })
        )
        .is_ok()
    );
}

#[test]
fn old_model_backup_cannot_be_reused_after_avatar_is_added() {
    let (_temp, mut s, _) = setup();
    drained(&mut s);
    let hash = plan(&mut s);
    let receipt = s
        .prepare_model_migration_backup(&op(), &id(1), &id(700), &hash)
        .unwrap();
    let avatar = serde_json::json!({"schema_version":1,"kind":"avatar_profile_draft","name":"avatar","avatar_id":"avatar","package_version":"1","package_sha256":"a".repeat(64),"license_sha256":"b".repeat(64),"width":2,"height":1,"selected":false,"revoked":false}).to_string();
    exec(
        &mut s,
        1,
        C::PutPreparation(FoundationObject {
            workspace_id: id(10),
            object_id: id(70),
            expected_revision: 0,
            body: avatar,
        }),
    )
    .unwrap();
    assert_eq!(
        s.verify_model_migration_backup(&op(), &id(1), &receipt),
        Err(Error::RevisionConflict)
    );
}

#[test]
fn backup_object_closure_uses_snapshot_and_excludes_deleted_objects() {
    let source = Connection::open_in_memory().unwrap();
    source.execute_batch("CREATE TABLE object_versions(workspace_id TEXT,object_id TEXT,sha256 TEXT); CREATE TABLE objects(workspace_id TEXT,id TEXT,deleted INTEGER); INSERT INTO objects VALUES('w','active',0),('w','deleted',1); INSERT INTO object_versions VALUES('w','active','old'),('w','deleted','removed');").unwrap();
    let mut snapshot = Connection::open_in_memory().unwrap();
    migration_backup::snapshot_database(&source, &mut snapshot, &op()).unwrap();
    source.execute_batch("DELETE FROM object_versions; INSERT INTO object_versions VALUES('w','active','later');").unwrap();
    assert_eq!(
        migration_backup::snapshot_object_digests(&snapshot, &op()).unwrap(),
        vec!["old"]
    );
    let cancelled = op();
    cancelled.cancel();
    assert_eq!(
        migration_backup::snapshot_object_digests(&snapshot, &cancelled),
        Err(Error::Cancelled)
    );
}

#[test]
fn deleted_object_does_not_block_real_backup_and_isolated_restore() {
    let (_temp, mut s, _) = setup();
    let object_id = id(770);
    let stored = exec(
        &mut s,
        1,
        C::PutObject(FoundationObject {
            workspace_id: id(10),
            object_id: object_id.clone(),
            expected_revision: 0,
            body: "deleted object".into(),
        }),
    )
    .unwrap();
    exec(
        &mut s,
        1,
        C::DeleteObject(FoundationDelete {
            workspace_id: id(10),
            object_id,
            expected_revision: stored.revision,
        }),
    )
    .unwrap();
    assert_eq!(body(&s.root, &stored.sha256), Err(Error::NotFound));
    drained(&mut s);
    let hash = plan(&mut s);
    let receipt = s
        .prepare_model_migration_backup(&op(), &id(1), &id(771), &hash)
        .unwrap();
    let restored = s
        .restore_model_backup_for_verification(&op(), &id(1), &receipt)
        .unwrap();
    assert_eq!(body(&restored, &stored.sha256), Err(Error::NotFound));
}

#[test]
fn avatar_packages_are_backed_up_verified_and_restored_with_snapshot() {
    let (t, mut s, _) = setup();
    let ids = ["base", "closed", "open"];
    let assets: [&[u8]; 3] = [b"base", b"closed", b"open"];
    let manifest = b"test-manifest";
    let config = b"test-config";
    let license = b"test-license";
    let mut h = Sha256::new();
    h.update(manifest);
    h.update(config);
    h.update(license);
    for (id, bytes) in ids.iter().zip(assets) {
        h.update(id.as_bytes());
        h.update(bytes);
    }
    let package = h
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let rights = hash(license);
    let dir = t.path().join("avatar-packages").join(&package);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, bytes) in [
        ("manifest.json", manifest.as_slice()),
        ("config.json", config.as_slice()),
        ("LICENSE.txt", license.as_slice()),
        ("preview.png", b"preview".as_slice()),
    ] {
        std::fs::write(dir.join(name), bytes).unwrap();
    }
    std::fs::write(
        dir.join("asset-ids.json"),
        serde_json::to_vec(&ids).unwrap(),
    )
    .unwrap();
    for (index, bytes) in assets.iter().enumerate() {
        std::fs::write(dir.join(format!("asset-{index}.rgba")), bytes).unwrap();
    }
    let avatar = serde_json::json!({"schema_version":1,"kind":"avatar_profile_draft","name":"avatar","avatar_id":"avatar","package_version":"1","package_sha256":package,"license_sha256":rights,"width":2,"height":1,"selected":false,"revoked":false}).to_string();
    exec(
        &mut s,
        1,
        C::PutPreparation(FoundationObject {
            workspace_id: id(10),
            object_id: id(70),
            expected_revision: 0,
            body: avatar,
        }),
    )
    .unwrap();
    drained(&mut s);
    let plan_hash = plan(&mut s);
    let receipt = s
        .prepare_model_migration_backup(&op(), &id(1), &id(771), &plan_hash)
        .unwrap();
    let backup = s
        .verify_model_migration_backup(&op(), &id(1), &receipt)
        .unwrap();
    assert!(
        backup["files"]
            .get(format!("avatar-packages/{package}/preview.png"))
            .is_some()
    );
    let restored = s
        .restore_model_backup_for_verification(&op(), &id(1), &receipt)
        .unwrap();
    assert!(nai_avatar_files::verify_installed(&restored, &package, &rights).is_ok());
    assert_eq!(
        std::fs::read(
            restored
                .join("avatar-packages")
                .join(&package)
                .join("preview.png")
        )
        .unwrap(),
        b"preview"
    );
    let backup_dir = t
        .path()
        .join("model-migration-backups")
        .join(id(771))
        .join("avatar-packages")
        .join(&package);
    std::fs::write(backup_dir.join("preview.png"), b"corrupt-preview").unwrap();
    assert!(
        s.verify_model_migration_backup(&op(), &id(1), &receipt)
            .is_err()
    );
    std::fs::write(backup_dir.join("preview.png"), b"preview").unwrap();
    std::fs::remove_file(backup_dir.join("asset-1.rgba")).unwrap();
    assert!(
        s.verify_model_migration_backup(&op(), &id(1), &receipt)
            .is_err()
    );
    std::fs::write(backup_dir.join("asset-1.rgba"), b"corrupt").unwrap();
    assert!(
        s.verify_model_migration_backup(&op(), &id(1), &receipt)
            .is_err()
    );
    assert!(nai_avatar_files::verify_installed(t.path(), &package, &rights).is_ok());
}

#[test]
fn incomplete_backup_is_preserved_and_quarantine_is_idempotent() {
    let (t, _s, _) = setup();
    let base = t.path().join("model-migration-backups");
    std::fs::create_dir(&base).unwrap();
    let source = base.join(id(700));
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("partial"), b"retain").unwrap();
    migration_backup::quarantine_incomplete(t.path(), &id(700), &op()).unwrap();
    let preserved = t.path().join("model-migration-incomplete").join(id(700));
    assert_eq!(std::fs::read(preserved.join("partial")).unwrap(), b"retain");
    assert!(!source.exists());
    migration_backup::quarantine_incomplete(t.path(), &id(700), &op()).unwrap();
    let complete = base.join(id(701));
    std::fs::create_dir(&complete).unwrap();
    std::fs::write(complete.join("manifest.json"), b"keep").unwrap();
    assert_eq!(
        migration_backup::quarantine_incomplete(t.path(), &id(701), &op()),
        Err(Error::RevisionConflict)
    );
    assert!(complete.join("manifest.json").exists());
    let conflict = base.join(id(700));
    std::fs::create_dir(&conflict).unwrap();
    std::fs::write(conflict.join("other"), b"keep").unwrap();
    assert_eq!(
        migration_backup::quarantine_incomplete(t.path(), &id(700), &op()),
        Err(Error::RevisionConflict)
    );
    assert!(conflict.join("other").exists());
    assert!(preserved.join("partial").exists());
    let cancelled = op();
    cancelled.cancel();
    assert_eq!(
        migration_backup::quarantine_incomplete(t.path(), &id(700), &cancelled),
        Err(Error::Cancelled)
    );
}

#[test]
fn photo_draft_requires_matching_file_and_backup_preserves_it() {
    let (t, mut s, _) = setup();
    let mut jpeg = vec![
        0xff, 0xd8, 0xff, 0xc0, 0, 11, 8, 0, 2, 0, 3, 1, 1, 0x11, 0, 0xff, 0xda, 0, 8, 1, 1, 0, 0,
        63, 0, 0, 0xff, 0xd9,
    ];
    let digest = hash(&jpeg);
    let doc = |d: &str, w: u32, revoked: bool| {
        serde_json::json!({"schema_version":1,"kind":"avatar_photo_draft","name":"photo","photo_sha256":d,"width":w,"height":2,"revoked":revoked}).to_string()
    };
    let put = |rev, body| {
        C::PutPreparation(FoundationObject {
            workspace_id: id(10),
            object_id: id(770),
            expected_revision: rev,
            body,
        })
    };
    assert_eq!(
        exec(&mut s, 1, put(0, doc(&digest, 3, false))),
        Err(Error::IntegrityFailed)
    );
    let dir = t.path().join("avatar-photos");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join(format!("{digest}.jpg")), &jpeg).unwrap();
    assert_eq!(
        exec(&mut s, 1, put(0, doc(&digest, 4, false))),
        Err(Error::IntegrityFailed)
    );
    exec(&mut s, 1, put(0, doc(&digest, 3, false))).unwrap();
    let list = exec(
        &mut s,
        1,
        C::ListPreparation(FoundationBrowse {
            workspace_id: id(10),
            kind: "avatar_photo_draft".into(),
            limit: 32,
            ..Default::default()
        }),
    )
    .unwrap();
    assert!(
        list.resources
            .iter()
            .any(|r| r.id == id(770) && r.kind == "avatar_photo_draft")
    );
    assert_eq!(
        exec(&mut s, 1, put(1, doc(&digest, 4, false))),
        Err(Error::RevisionConflict)
    );
    drained(&mut s);
    let plan_hash = plan(&mut s);
    let receipt = s
        .prepare_model_migration_backup(&op(), &id(1), &id(771), &plan_hash)
        .unwrap();
    let backup = s
        .verify_model_migration_backup(&op(), &id(1), &receipt)
        .unwrap();
    assert!(
        backup["files"]
            .get(format!("avatar-photos/{digest}.jpg"))
            .is_some()
    );
    let restored = s
        .restore_model_backup_for_verification(&op(), &id(1), &receipt)
        .unwrap();
    assert_eq!(
        nai_avatar_files::photo::read_stored_photo(&restored, &digest)
            .unwrap()
            .0,
        jpeg
    );
    jpeg[5] = 255;
    std::fs::write(
        t.path()
            .join("model-migration-backups")
            .join(id(771))
            .join("avatar-photos")
            .join(format!("{digest}.jpg")),
        jpeg,
    )
    .unwrap();
    assert!(
        s.verify_model_migration_backup(&op(), &id(1), &receipt)
            .is_err()
    );
    assert!(nai_avatar_files::photo::read_stored_photo(t.path(), &digest).is_ok());
    std::fs::remove_file(dir.join(format!("{digest}.jpg"))).unwrap();
    exec(&mut s, 1, put(1, doc(&digest, 3, true))).unwrap();
    assert_eq!(
        exec(&mut s, 1, put(2, doc(&digest, 3, false))),
        Err(Error::PermissionDenied)
    );
}
fn fixture_mp4(timescale: u32, duration: u32) -> Vec<u8> {
    fn box_of(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut v = ((8 + payload.len()) as u32).to_be_bytes().to_vec();
        v.extend(kind);
        v.extend(payload);
        v
    }
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
fn video_draft_requires_matching_file_and_backup_preserves_it() {
    let (t, mut s, _) = setup();
    let mut mp4 = fixture_mp4(1000, 5000);
    let digest = nai_avatar_files::video::inspect_mp4(&mp4).unwrap().sha256;
    let doc = |d: &str, duration: u64, revoked: bool| {
        serde_json::json!({"schema_version":1,"kind":"avatar_video_draft","name":"video","video_sha256":d,"duration_ms":duration,"inspect_version":"mp4-ftyp-mvhd-v1","realistic_drive":"model_missing","revoked":revoked}).to_string()
    };
    let put = |rev, body| {
        C::PutPreparation(FoundationObject {
            workspace_id: id(10),
            object_id: id(780),
            expected_revision: rev,
            body,
        })
    };
    assert_eq!(
        exec(&mut s, 1, put(0, doc(&digest, 5000, false))),
        Err(Error::IntegrityFailed)
    );
    nai_avatar_files::video::store_video(t.path(), &mp4).unwrap();
    assert_eq!(
        exec(&mut s, 1, put(0, doc(&digest, 4000, false))),
        Err(Error::IntegrityFailed)
    );
    exec(&mut s, 1, put(0, doc(&digest, 5000, false))).unwrap();
    let list = exec(
        &mut s,
        1,
        C::ListPreparation(FoundationBrowse {
            workspace_id: id(10),
            kind: "avatar_video_draft".into(),
            limit: 32,
            ..Default::default()
        }),
    )
    .unwrap();
    assert!(
        list.resources
            .iter()
            .any(|r| r.id == id(780) && r.kind == "avatar_video_draft")
    );
    assert_eq!(
        exec(&mut s, 1, put(1, doc(&digest, 4000, false))),
        Err(Error::RevisionConflict)
    );
    drained(&mut s);
    let plan_hash = plan(&mut s);
    let receipt = s
        .prepare_model_migration_backup(&op(), &id(1), &id(781), &plan_hash)
        .unwrap();
    let backup = s
        .verify_model_migration_backup(&op(), &id(1), &receipt)
        .unwrap();
    assert!(
        backup["files"]
            .get(format!("avatar-videos/{digest}.mp4"))
            .is_some()
    );
    assert!(
        backup["files"]
            .get(format!("avatar-video-cache/{digest}.json"))
            .is_none()
    );
    let restored = s
        .restore_model_backup_for_verification(&op(), &id(1), &receipt)
        .unwrap();
    assert_eq!(
        nai_avatar_files::video::read_stored_video(&restored, &digest)
            .unwrap()
            .0,
        mp4
    );
    mp4[8] = b'x';
    std::fs::write(
        t.path()
            .join("model-migration-backups")
            .join(id(781))
            .join("avatar-videos")
            .join(format!("{digest}.mp4")),
        &mp4,
    )
    .unwrap();
    assert!(
        s.verify_model_migration_backup(&op(), &id(1), &receipt)
            .is_err()
    );
    assert!(nai_avatar_files::video::read_stored_video(t.path(), &digest).is_ok());
    std::fs::remove_file(
        t.path()
            .join("avatar-videos")
            .join(format!("{digest}.mp4")),
    )
    .unwrap();
    exec(&mut s, 1, put(1, doc(&digest, 5000, true))).unwrap();
    assert_eq!(
        exec(&mut s, 1, put(2, doc(&digest, 5000, false))),
        Err(Error::PermissionDenied)
    );
}
