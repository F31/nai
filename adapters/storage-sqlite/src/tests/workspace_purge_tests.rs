use super::*;
fn space(s: &mut Store, n: u64) {
    exec(
        s,
        1,
        C::CreateWorkspace(FoundationWorkspace {
            workspace_id: id(n),
            name: format!("工作区{n}"),
            budget_limit_minor: 0,
            currency: "CNY".into(),
        }),
    )
    .unwrap();
}
fn recycle(s: &mut Store, n: u64) {
    exec(
        s,
        1,
        C::WorkspaceLifecycle(FoundationWorkspaceLifecycle {
            workspace_id: id(n),
            expected_revision: 1,
            action: WorkspaceAction::Recycle as i32,
        }),
    )
    .unwrap();
}
fn read(n: u64) -> WorkspacePurgeRead {
    WorkspacePurgeRead {
        workspace_id: id(n),
        expected_revision: 2,
    }
}
fn preview(s: &mut Store, n: u64) -> WorkspacePurgePlan {
    exec(s, 1, C::PreviewWorkspacePurge(read(n)))
        .unwrap()
        .workspace_purge_plan
        .unwrap()
}
fn purge(s: &mut Store, p: &WorkspacePurgePlan) -> Result<FoundationResult> {
    exec(
        s,
        1,
        C::PurgeWorkspace(WorkspacePurgeCommand {
            workspace_id: p.workspace_id.clone(),
            expected_revision: p.revision,
            plan_sha256: p.plan_sha256.clone(),
            acknowledge: true,
        }),
    )
}
fn object(s: &mut Store, ws: u64, n: u64, text: &str) -> String {
    exec(
        s,
        1,
        C::PutObject(FoundationObject {
            workspace_id: id(ws),
            object_id: id(n),
            body: text.into(),
            expected_revision: 0,
        }),
    )
    .unwrap()
    .sha256
}
#[test]
fn purge_is_atomic_retains_financial_facts_and_other_workspace_files_and_never_revives() {
    let (t, mut s, shared_sha) = setup();
    space(&mut s, 80);
    let shared = body(&s.root, &shared_sha).unwrap();
    object(&mut s, 80, 81, &shared);
    let unique = object(&mut s, 80, 82, "待清理的独立内容");
    s.conn
        .execute(
            "INSERT INTO budget_reservations VALUES(?1,?2,?3,7,NULL,'unknown','CNY',1,1,1)",
            params![id(80), id(83), id(1)],
        )
        .unwrap();
    recycle(&mut s, 80);
    let p = preview(&mut s, 80);
    assert!(p.blockers.is_empty());
    assert_eq!(p.retained_budget_records, 1);
    let audit = p.retained_audit_records;
    let result = purge(&mut s, &p).unwrap().workspace_purge_status.unwrap();
    assert!(result.deleted);
    assert_eq!(result.pending_files, 0);
    assert!(!t.path().join("objects").join(&unique).exists());
    assert!(t.path().join("objects").join(&shared_sha).exists());
    assert_eq!(
        s.conn
            .query_row(
                "SELECT reserved_minor FROM workspace_retained_budget WHERE workspace_id=?1",
                [id(80)],
                |r| uint(r, 0)
            )
            .unwrap(),
        7
    );
    assert!(
        s.conn
            .query_row(
                "SELECT count(*) FROM audit_events WHERE workspace_id=?1",
                [id(80)],
                |r| uint(r, 0)
            )
            .unwrap()
            > audit
    );
    assert!(
        purge(&mut s, &p)
            .unwrap()
            .workspace_purge_status
            .unwrap()
            .deleted
    );
    assert_eq!(
        exec(
            &mut s,
            1,
            C::WorkspaceLifecycle(FoundationWorkspaceLifecycle {
                workspace_id: id(80),
                expected_revision: 2,
                action: WorkspaceAction::Restore as i32
            })
        ),
        Err(Error::PermissionDenied)
    );
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    assert_eq!(
        workspace_lifecycle::state(&s.conn, &id(80)).unwrap(),
        "workspace_delete"
    );
    assert_eq!(
        exec(
            &mut s,
            1,
            C::CreateWorkspace(FoundationWorkspace {
                workspace_id: id(80),
                name: "禁止复活".into(),
                budget_limit_minor: 0,
                currency: "CNY".into()
            })
        ),
        Err(Error::PermissionDenied)
    );
    assert_eq!(body(&s.root, &shared_sha).unwrap(), shared);
}
#[test]
fn rollback_keeps_all_business_rows_and_does_not_delete_files_or_archive_partial_money() {
    let (_t, mut s, _) = setup();
    space(&mut s, 80);
    let sha = object(&mut s, 80, 82, "事务故障内容");
    recycle(&mut s, 80);
    let p = preview(&mut s, 80);
    s.conn.execute_batch("CREATE TEMP TRIGGER reject_purge BEFORE DELETE ON objects BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert!(purge(&mut s, &p).is_err());
    assert_eq!(
        workspace_lifecycle::state(&s.conn, &id(80)).unwrap(),
        "workspace_recycle"
    );
    assert!(s.root.join("objects").join(sha).exists());
    assert_eq!(
        s.conn
            .query_row("SELECT count(*) FROM workspace_purges", [], |r| uint(r, 0))
            .unwrap(),
        0
    );
    s.conn.execute_batch("DROP TRIGGER reject_purge").unwrap();
    assert!(
        purge(&mut s, &p)
            .unwrap()
            .workspace_purge_status
            .unwrap()
            .deleted
    );
}
#[test]
fn damaged_owned_file_reports_committed_deletion_and_resumes_after_restart() {
    let (t, mut s, _) = setup();
    exec(
        &mut s,
        1,
        C::Stop(FoundationStop {
            workspace_id: id(10),
            session_id: id(30),
        }),
    )
    .unwrap();
    let model_preview = exec(
        &mut s,
        1,
        C::PreviewSharedModels(FoundationLlmRead::default()),
    )
    .unwrap()
    .shared_model_preview
    .unwrap();
    let request = SharedModelMigration {
        migration_id: id(300),
        plan_sha256: model_preview.plan_sha256,
        entries: vec![],
    };
    for _ in 0..3 {
        exec(&mut s, 1, C::MigrateSharedModels(request.clone())).unwrap();
    }
    space(&mut s, 80);
    let text = "待验证文件";
    let sha = object(&mut s, 80, 82, text);
    recycle(&mut s, 80);
    let p = preview(&mut s, 80);
    std::fs::write(s.root.join("objects").join(&sha), b"tampered").unwrap();
    let status = purge(&mut s, &p).unwrap().workspace_purge_status.unwrap();
    assert!(status.deleted && status.cleanup_blocked);
    assert_eq!(status.pending_files, 1);
    drop(s);
    let s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    assert_eq!(
        workspace_lifecycle::state(&s.conn, &id(80)).unwrap(),
        "workspace_delete"
    );
    std::fs::write(t.path().join("objects").join(&sha), text.as_bytes()).unwrap();
    let marker = t.path().join("object-ownership").join(&sha);
    std::fs::write(&marker, b"invalid-owned-marker").unwrap();
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    let pending = exec(&mut s, 1, C::ReadWorkspacePurge(read(80)))
        .unwrap()
        .workspace_purge_status
        .unwrap();
    assert!(pending.deleted && pending.cleanup_blocked);
    assert_eq!(pending.pending_files, 1);
    let cleanup = exec(
        &mut s,
        1,
        C::ListWorkspaces(FoundationBrowse {
            kind: "cleanup".into(),
            limit: 32,
            ..Default::default()
        }),
    )
    .unwrap()
    .resources;
    assert_eq!(cleanup.len(), 1);
    assert_eq!(cleanup[0].id, id(80));
    assert_eq!(cleanup[0].name, "工作区80");
    assert_eq!(
        exec(&mut s, 1, C::PreviewWorkspacePurge(read(80)))
            .unwrap()
            .workspace_purge_plan
            .unwrap()
            .plan_sha256,
        p.plan_sha256
    );
    std::fs::write(&marker, b"nai-owned-text-v1").unwrap();
    let status = exec(&mut s, 1, C::ReadWorkspacePurge(read(80)))
        .unwrap()
        .workspace_purge_status
        .unwrap();
    assert_eq!(status.pending_files, 0);
    assert!(!status.cleanup_blocked);
    assert!(!t.path().join("objects").join(sha).exists());
    assert!(
        exec(
            &mut s,
            1,
            C::ListWorkspaces(FoundationBrowse {
                kind: "cleanup".into(),
                limit: 32,
                ..Default::default()
            })
        )
        .unwrap()
        .resources
        .is_empty()
    );
}
#[test]
fn preview_rejects_stale_confirmation_unknown_tables_permissions_and_missing_acknowledgement() {
    let (_t, mut s, _) = setup();
    space(&mut s, 80);
    recycle(&mut s, 80);
    let p = preview(&mut s, 80);
    assert_eq!(
        exec(&mut s, 2, C::PreviewWorkspacePurge(read(80))),
        Err(Error::PermissionDenied)
    );
    let x = WorkspacePurgeCommand {
        workspace_id: id(80),
        expected_revision: 2,
        plan_sha256: p.plan_sha256.clone(),
        acknowledge: false,
    };
    assert_eq!(
        exec(&mut s, 1, C::PurgeWorkspace(x)),
        Err(Error::InvalidArgument)
    );
    s.conn
        .execute(
            "UPDATE roles SET revision=revision+1 WHERE workspace_id=?1",
            [id(80)],
        )
        .unwrap();
    assert_eq!(purge(&mut s, &p), Err(Error::RevisionConflict));
    s.conn
        .execute_batch("CREATE TABLE future_workspace_data(workspace_id TEXT,content TEXT)")
        .unwrap();
    assert_eq!(
        exec(&mut s, 1, C::PreviewWorkspacePurge(read(80))),
        Err(Error::IntegrityFailed)
    );
}
#[test]
fn avatar_authority_tables_block_purge_when_rights_records_exist() {
    let (_temp, mut store, _) = setup();
    space(&mut store, 80);
    recycle(&mut store, 80);
    store
        .conn
        .execute_batch(include_str!(
            "../../../../schemas/storage/avatar-authority.sql"
        ))
        .unwrap();
    store
        .conn
        .execute(
            "INSERT INTO avatar_rights_revocations VALUES(?1,?2,?3)",
            params![id(80), "a".repeat(64), "b".repeat(64)],
        )
        .unwrap();
    let plan = preview(&mut store, 80);
    assert!(
        plan.blockers
            .iter()
            .any(|b| b == "AVATAR_CLEANUP_UNAVAILABLE")
    );
    assert_eq!(purge(&mut store, &plan), Err(Error::CapabilityUnavailable));
    assert_eq!(
        store
            .conn
            .query_row(
                "SELECT count(*) FROM workspaces WHERE id=?1",
                [id(80)],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
}

#[test]
fn empty_avatar_tables_allow_purge_and_partial_schema_fails_closed() {
    let (_temp, mut store, _) = setup();
    space(&mut store, 80);
    recycle(&mut store, 80);
    store
        .conn
        .execute_batch(include_str!(
            "../../../../schemas/storage/avatar-authority.sql"
        ))
        .unwrap();
    let plan = preview(&mut store, 80);
    assert!(plan.blockers.is_empty());
    assert!(
        purge(&mut store, &plan)
            .unwrap()
            .workspace_purge_status
            .unwrap()
            .deleted
    );
    space(&mut store, 90);
    recycle(&mut store, 90);
    store
        .conn
        .execute_batch("DROP TABLE avatar_rights_revocations")
        .unwrap();
    assert_eq!(
        exec(&mut store, 1, C::PreviewWorkspacePurge(read(90))),
        Err(Error::IntegrityFailed)
    );
    store.conn.execute_batch("CREATE TABLE avatar_rights_revocations(workspace_id TEXT, package_sha256 TEXT, rights_sha256 TEXT)").unwrap();
    assert_eq!(
        exec(&mut store, 1, C::PreviewWorkspacePurge(read(90))),
        Err(Error::IntegrityFailed)
    );
    assert_eq!(
        store
            .conn
            .query_row(
                "SELECT count(*) FROM workspaces WHERE id=?1",
                [id(90)],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
}

#[test]
fn last_workspace_persists_and_invalid_selection_explains_fallback_without_resuming_sessions() {
    let (t, mut s, _) = setup();
    space(&mut s, 80);
    let context = |ws: String| C::WorkspaceContext(FoundationLlmRead { workspace_id: ws });
    exec(&mut s, 1, context(id(80))).unwrap();
    assert_eq!(
        exec(
            &mut s,
            1,
            C::WorkspaceLifecycle(FoundationWorkspaceLifecycle {
                workspace_id: id(80),
                expected_revision: 1,
                action: WorkspaceAction::Recycle as i32
            })
        ),
        Err(Error::Busy)
    );
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    assert_eq!(
        exec(&mut s, 1, context(String::new())).unwrap().resource_id,
        id(80)
    );
    s.conn
        .execute("UPDATE roles SET role=0 WHERE workspace_id=?1", [id(80)])
        .unwrap();
    let fallback = exec(&mut s, 1, context(String::new())).unwrap();
    assert_eq!(fallback.resource_id, id(10));
    assert_eq!(fallback.task_reason, "WORKSPACE_SELECTION_FALLBACK");
    assert_eq!(
        s.conn
            .query_row("SELECT count(*) FROM sessions WHERE stopped=0", [], |r| {
                uint(r, 0)
            })
            .unwrap(),
        0
    );
}

#[test]
fn unshared_sources_are_blocked_and_migrated_shared_models_survive_source_purge() {
    let (_t, mut s, _) = setup();
    space(&mut s, 80);
    exec(
        &mut s,
        1,
        C::SaveLlmModel(FoundationLlmModel {
            workspace_id: id(80),
            model_id: id(81),
            name: "共享后保留".into(),
            base_url: "https://example.invalid/v1".into(),
            model: "test".into(),
            enabled: true,
            prices_confirmed: true,
            input_minor_per_million: 1,
            output_minor_per_million: 1,
            ..Default::default()
        }),
    )
    .unwrap();
    recycle(&mut s, 80);
    assert!(
        preview(&mut s, 80)
            .blockers
            .contains(&"UNSHARED_MODELS".into())
    );
    exec(
        &mut s,
        1,
        C::Stop(FoundationStop {
            workspace_id: id(10),
            session_id: id(30),
        }),
    )
    .unwrap();
    let p = exec(
        &mut s,
        1,
        C::PreviewSharedModels(FoundationLlmRead::default()),
    )
    .unwrap()
    .shared_model_preview
    .unwrap();
    let r = SharedModelMigration {
        migration_id: id(700),
        plan_sha256: p.plan_sha256,
        entries: p
            .rows
            .into_iter()
            .map(|r| {
                let mut e = r.source.unwrap();
                e.catalog_model_id = id(701);
                e
            })
            .collect(),
    };
    for _ in 0..4 {
        exec(&mut s, 1, C::MigrateSharedModels(r.clone())).unwrap();
    }
    let before = llm::settings(&s.conn, &s.root, &id(10)).unwrap().llm_models;
    assert_eq!(before.len(), 1);
    let p = preview(&mut s, 80);
    assert!(p.blockers.is_empty());
    purge(&mut s, &p).unwrap();
    assert_eq!(
        llm::settings(&s.conn, &s.root, &id(10)).unwrap().llm_models,
        before
    );
    assert!(s.root.join("model-migration-backups").exists());
}

#[test]
fn older_backup_restore_stays_isolated_and_cannot_replace_current_purge_tombstone() {
    let (_t, mut s, _) = setup();
    space(&mut s, 80);
    object(&mut s, 80, 81, "备份中的旧内容");
    recycle(&mut s, 80);
    exec(
        &mut s,
        1,
        C::Stop(FoundationStop {
            workspace_id: id(10),
            session_id: id(30),
        }),
    )
    .unwrap();
    let hash = exec(
        &mut s,
        1,
        C::PreviewSharedModels(FoundationLlmRead::default()),
    )
    .unwrap()
    .shared_model_preview
    .unwrap()
    .plan_sha256;
    let op = Operation::new(4000).unwrap();
    let backup = s
        .prepare_model_migration_backup(&op, &id(1), &id(800), &hash)
        .unwrap();
    let p = preview(&mut s, 80);
    purge(&mut s, &p).unwrap();
    let isolated = s
        .restore_model_backup_for_verification(&Operation::new(4000).unwrap(), &id(1), &backup)
        .unwrap();
    assert!(isolated.starts_with(s.root.join("model-migration-verification")));
    assert_eq!(
        workspace_lifecycle::state(&s.conn, &id(80)).unwrap(),
        "workspace_delete"
    );
    assert_eq!(
        s.conn
            .query_row(
                "SELECT count(*) FROM workspaces WHERE id=?1",
                [id(80)],
                |r| uint(r, 0)
            )
            .unwrap(),
        0
    );
}

#[test]
fn photo_reference_blocks_permanent_purge_without_deleting_source() {
    let (t, mut s, _) = setup();
    space(&mut s, 80);
    let jpeg = vec![
        0xff, 0xd8, 0xff, 0xc0, 0, 11, 8, 0, 2, 0, 3, 1, 1, 0x11, 0, 0xff, 0xda, 0, 8, 1, 1, 0, 0,
        63, 0, 0, 0xff, 0xd9,
    ];
    let meta = nai_avatar_files::photo::store_photo(t.path(), &jpeg).unwrap();
    let doc=serde_json::json!({"schema_version":1,"kind":"avatar_photo_draft","name":"photo","photo_sha256":meta.sha256,"width":meta.width,"height":meta.height,"revoked":false}).to_string();
    exec(
        &mut s,
        1,
        C::PutPreparation(FoundationObject {
            workspace_id: id(80),
            object_id: id(81),
            expected_revision: 0,
            body: doc,
        }),
    )
    .unwrap();
    assert_eq!(
        exec(
            &mut s,
            1,
            C::DeleteObject(FoundationDelete {
                workspace_id: id(80),
                object_id: id(81),
                expected_revision: 1
            })
        ),
        Err(Error::PermissionDenied)
    );
    recycle(&mut s, 80);
    let p = preview(&mut s, 80);
    assert!(p.blockers.iter().any(|b| b == "PHOTO_CLEANUP_UNAVAILABLE"));
    assert_eq!(purge(&mut s, &p), Err(Error::CapabilityUnavailable));
    assert_eq!(
        nai_avatar_files::photo::read_stored_photo(t.path(), &meta.sha256)
            .unwrap()
            .0,
        jpeg
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
fn video_reference_blocks_permanent_purge_without_deleting_source() {
    let (t, mut s, _) = setup();
    space(&mut s, 90);
    let mp4 = fixture_mp4(1000, 5000);
    let meta = nai_avatar_files::video::store_video(t.path(), &mp4).unwrap();
    let doc=serde_json::json!({"schema_version":1,"kind":"avatar_video_draft","name":"video","video_sha256":meta.sha256,"duration_ms":meta.duration_ms,"inspect_version":"mp4-ftyp-mvhd-v1","realistic_drive":"model_missing","revoked":false}).to_string();
    exec(
        &mut s,
        1,
        C::PutPreparation(FoundationObject {
            workspace_id: id(90),
            object_id: id(91),
            expected_revision: 0,
            body: doc,
        }),
    )
    .unwrap();
    assert_eq!(
        exec(
            &mut s,
            1,
            C::DeleteObject(FoundationDelete {
                workspace_id: id(90),
                object_id: id(91),
                expected_revision: 1
            })
        ),
        Err(Error::PermissionDenied)
    );
    recycle(&mut s, 90);
    let p = preview(&mut s, 90);
    assert!(p.blockers.iter().any(|b| b == "VIDEO_CLEANUP_UNAVAILABLE"));
    assert_eq!(purge(&mut s, &p), Err(Error::CapabilityUnavailable));
    assert_eq!(
        nai_avatar_files::video::read_stored_video(t.path(), &meta.sha256)
            .unwrap()
            .0,
        mp4
    );
}
