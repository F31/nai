use nai_contracts::workbench::*;
use nai_workbench_host::foundation::{Owner, random_id};
use prost::Message;
use std::path::PathBuf;
fn engine() -> PathBuf {
    PathBuf::from(
        std::env::var_os("NAI_FOUNDATION_ENGINE")
            .expect("build foundation engine and set NAI_FOUNDATION_ENGINE"),
    )
}
fn call(owner: &mut Owner, operation: foundation_command::Operation) -> FoundationResult {
    let response = owner
        .call(
            &FoundationCommand {
                idempotency_key: random_id().unwrap(),
                operation: Some(operation),
            }
            .encode_to_vec(),
        )
        .unwrap();
    let response = FoundationResponse::decode(response.as_slice()).unwrap();
    let foundation_response::Outcome::Result(result) = response.outcome.unwrap() else {
        panic!("unexpected engine problem")
    };
    result
}
#[test]
fn workspace_purge_and_last_selection_cross_real_ipc_without_affecting_other_content() {
    use foundation_command::Operation as C;
    let t = tempfile::tempdir().unwrap();
    let identity = t.path().join("operator.json");
    let root = t.path().join("data");
    let mut owner = Owner::open_at(&engine(), &identity, &root).unwrap();
    let active = random_id().unwrap();
    let trash = random_id().unwrap();
    let mut objects = vec![];
    for (ws, name) in [(&active, "正在使用"), (&trash, "待清理")] {
        call(
            &mut owner,
            C::CreateWorkspace(FoundationWorkspace {
                workspace_id: ws.clone(),
                name: name.into(),
                budget_limit_minor: 0,
                currency: "CNY".into(),
            }),
        );
        let id = random_id().unwrap();
        call(&mut owner,C::PutPreparation(FoundationObject {workspace_id:ws.clone(),object_id:id.clone(),expected_revision:0,body:r#"{"schema_version":1,"kind":"host_draft","name":"主持","style":"friendly","preferences":""}"#.into()}));
        objects.push(id);
    }
    call(
        &mut owner,
        C::WorkspaceContext(FoundationLlmRead {
            workspace_id: active.clone(),
        }),
    );
    call(
        &mut owner,
        C::WorkspaceLifecycle(FoundationWorkspaceLifecycle {
            workspace_id: trash.clone(),
            expected_revision: 1,
            action: WorkspaceAction::Recycle as i32,
        }),
    );
    let p = call(
        &mut owner,
        C::PreviewWorkspacePurge(WorkspacePurgeRead {
            workspace_id: trash.clone(),
            expected_revision: 2,
        }),
    )
    .workspace_purge_plan
    .unwrap();
    assert!(p.blockers.is_empty());
    let status = call(
        &mut owner,
        C::PurgeWorkspace(WorkspacePurgeCommand {
            workspace_id: trash.clone(),
            expected_revision: 2,
            plan_sha256: p.plan_sha256,
            acknowledge: true,
        }),
    )
    .workspace_purge_status
    .unwrap();
    assert!(status.deleted);
    assert_eq!(status.pending_files, 0);
    drop(owner);
    let mut owner = Owner::open_at(&engine(), &identity, &root).unwrap();
    assert_eq!(
        call(
            &mut owner,
            C::WorkspaceContext(FoundationLlmRead::default())
        )
        .resource_id,
        active
    );
    let content = call(
        &mut owner,
        C::ReadObject(FoundationReadObject {
            workspace_id: active,
            object_id: objects[0].clone(),
            revision: 1,
        }),
    );
    assert!(content.body.contains("主持"));
    let recycled = call(
        &mut owner,
        C::ListWorkspaces(FoundationBrowse {
            kind: "recycled".into(),
            limit: 32,
            ..Default::default()
        }),
    );
    assert!(recycled.resources.is_empty());
    assert!(
        call(
            &mut owner,
            C::ReadWorkspacePurge(WorkspacePurgeRead {
                workspace_id: trash,
                expected_revision: 2
            })
        )
        .workspace_purge_status
        .unwrap()
        .deleted
    );
}
#[test]
fn shared_catalog_migration_crosses_real_ipc_and_persists_across_owner_restart() {
    use foundation_command::Operation as C;
    let t = tempfile::tempdir().unwrap();
    let identity = t.path().join("operator.json");
    let root = t.path().join("data");
    let mut owner = Owner::open_at(&engine(), &identity, &root).unwrap();
    let ws = random_id().unwrap();
    call(
        &mut owner,
        C::CreateWorkspace(FoundationWorkspace {
            workspace_id: ws.clone(),
            name: "迁移来源".into(),
            budget_limit_minor: 10,
            currency: "CNY".into(),
        }),
    );
    call(
        &mut owner,
        C::SaveLlmModel(FoundationLlmModel {
            workspace_id: ws.clone(),
            model_id: random_id().unwrap(),
            name: "元数据模型".into(),
            base_url: "https://example.invalid/v1".into(),
            model: "test".into(),
            enabled: true,
            prices_confirmed: true,
            input_minor_per_million: 1,
            output_minor_per_million: 1,
            ..Default::default()
        }),
    );
    let p = call(
        &mut owner,
        C::PreviewSharedModels(FoundationLlmRead::default()),
    )
    .shared_model_preview
    .unwrap();
    let r = SharedModelMigration {
        migration_id: random_id().unwrap(),
        plan_sha256: p.plan_sha256,
        entries: p
            .rows
            .into_iter()
            .map(|x| {
                let mut e = x.source.unwrap();
                e.catalog_model_id = random_id().unwrap();
                e
            })
            .collect(),
    };
    call(&mut owner, C::MigrateSharedModels(r.clone()));
    drop(owner);
    let mut owner = Owner::open_at(&engine(), &identity, &root).unwrap();
    let state = call(
        &mut owner,
        C::ReadSharedMigration(FoundationLlmRead::default()),
    )
    .shared_model_migration
    .unwrap();
    assert_eq!(state.request, Some(r.clone()));
    let mut committed = false;
    for _ in 0..5 {
        if call(&mut owner, C::MigrateSharedModels(r.clone())).shared_catalog_enabled {
            committed = true;
            break;
        }
    }
    assert!(committed);
    let other = random_id().unwrap();
    call(
        &mut owner,
        C::CreateWorkspace(FoundationWorkspace {
            workspace_id: other.clone(),
            name: "另一个工作区".into(),
            budget_limit_minor: 25,
            currency: "CNY".into(),
        }),
    );
    let a = call(
        &mut owner,
        C::ReadLlmSettings(FoundationLlmRead { workspace_id: ws }),
    );
    let b = call(
        &mut owner,
        C::ReadLlmSettings(FoundationLlmRead {
            workspace_id: other,
        }),
    );
    assert!(a.shared_catalog_enabled);
    assert_eq!(a.llm_models, b.llm_models);
    assert_eq!(a.llm_models.len(), 1);
    assert!(!a.llm_models[0].credential_saved);
    assert_ne!(
        a.llm_budget.unwrap().limit_minor,
        b.llm_budget.unwrap().limit_minor
    );
}
#[test]
fn model_metadata_and_budget_cross_the_real_host_without_starting_a_provider() {
    use foundation_command::Operation as C;
    let t = tempfile::tempdir().unwrap();
    let identity = t.path().join("operator.json");
    let root = t.path().join("data");
    let mut owner = Owner::open_at(&engine(), &identity, &root).unwrap();
    let ws = random_id().unwrap();
    let model = random_id().unwrap();
    call(
        &mut owner,
        C::CreateWorkspace(FoundationWorkspace {
            workspace_id: ws.clone(),
            name: "isolated model settings".into(),
            budget_limit_minor: 0,
            currency: "CNY".into(),
        }),
    );
    let saved = call(
        &mut owner,
        C::SaveLlmModel(FoundationLlmModel {
            workspace_id: ws.clone(),
            model_id: model.clone(),
            expected_revision: 0,
            name: "unconfigured test model".into(),
            base_url: "https://example.invalid/v1".into(),
            model: "fixture-only".into(),
            input_minor_per_million: 100,
            output_minor_per_million: 200,
            enabled: true,
            api_key: Vec::new(),
            prices_confirmed: true,
        }),
    );
    assert_eq!(saved.llm_models.len(), 1);
    assert!(!saved.llm_models[0].credential_saved);
    assert!(saved.body.is_empty());
    let read = call(
        &mut owner,
        C::ReadLlmSettings(FoundationLlmRead {
            workspace_id: ws.clone(),
        }),
    );
    let b = read.llm_budget.unwrap();
    assert_eq!(b.used_minor, 0);
    assert_eq!(b.limit_minor, 0);
    let b = call(
        &mut owner,
        C::SetLlmBudget(FoundationLlmBudget {
            workspace_id: ws.clone(),
            expected_revision: b.revision,
            limit_minor: 100,
        }),
    )
    .llm_budget
    .unwrap();
    assert_eq!(b.limit_minor, 100);
    assert_eq!(b.used_minor, 0);
    drop(owner);
    let mut owner = Owner::open_at(&engine(), &identity, &root).unwrap();
    let read = call(
        &mut owner,
        C::ReadLlmSettings(FoundationLlmRead {
            workspace_id: ws.clone(),
        }),
    );
    assert_eq!(read.llm_models[0].id, model);
    assert!(!read.llm_models[0].credential_saved);
    assert_eq!(read.llm_budget.unwrap().used_minor, 0);
    let tts = random_id().unwrap();
    let saved = call(
        &mut owner,
        C::SaveTtsModel(FoundationTtsModel {
            protocol: "openai_speech".into(),
            language: String::new(),
            workspace_id: ws.clone(),
            model_id: tts.clone(),
            expected_revision: 0,
            name: "语音元数据".into(),
            base_url: "https://example.invalid/v1".into(),
            model: "test-speech".into(),
            voice: "test-voice".into(),
            enabled: true,
            ..Default::default()
        }),
    );
    assert_eq!(saved.tts_models.len(), 1);
    assert!(!saved.tts_models[0].credential_saved);
    let removed = call(
        &mut owner,
        C::DeleteTtsModel(FoundationDelete {
            workspace_id: ws.clone(),
            object_id: tts,
            expected_revision: 1,
        }),
    );
    assert!(removed.tts_models.is_empty());
    let removed = call(
        &mut owner,
        C::DeleteLlmModel(FoundationDelete {
            workspace_id: ws,
            object_id: model,
            expected_revision: 1,
        }),
    );
    assert!(removed.llm_models.is_empty());
}
#[test]
fn persistent_owner_survives_idle_and_old_request_limit_then_reopens_saved_data() {
    let t = tempfile::tempdir().unwrap();
    let identity = t.path().join("operator.json");
    let root = t.path().join("data");
    let mut owner = Owner::open_at(&engine(), &identity, &root).unwrap();
    let ws = random_id().unwrap();
    call(
        &mut owner,
        foundation_command::Operation::CreateWorkspace(FoundationWorkspace {
            workspace_id: ws.clone(),
            name: "直播间".into(),
            budget_limit_minor: 0,
            currency: "CNY".into(),
        }),
    );
    std::thread::sleep(std::time::Duration::from_millis(4200));
    for _ in 0..66 {
        let r = call(
            &mut owner,
            foundation_command::Operation::ListWorkspaces(FoundationBrowse {
                limit: 32,
                ..Default::default()
            }),
        );
        assert_eq!(r.resources[0].id, ws);
    }
    let bad = FoundationCommand {
        idempotency_key: random_id().unwrap(),
        operation: Some(foundation_command::Operation::PutObject(
            FoundationObject::default(),
        )),
    };
    assert_eq!(
        owner.call(&bad.encode_to_vec()).unwrap_err(),
        "WB_PERMISSION_DENIED"
    );
    drop(owner);
    let principal = std::fs::read(&identity).unwrap();
    let mut owner = Owner::open_at(&engine(), &identity, &root).unwrap();
    let r = call(
        &mut owner,
        foundation_command::Operation::ListWorkspaces(FoundationBrowse {
            limit: 32,
            ..Default::default()
        }),
    );
    assert_eq!(r.resources[0].name, "直播间");
    assert_eq!(std::fs::read(&identity).unwrap(), principal);
    drop(owner);
    std::fs::remove_file(&identity).unwrap();
    assert!(matches!(Owner::open_at(&engine(),&identity,&root),Err(e)if e=="WB_IDENTITY_MISSING"));
    assert!(!identity.exists());
}
#[test]
fn invalid_identity_does_not_create_database() {
    let t = tempfile::tempdir().unwrap();
    let identity = t.path().join("operator.json");
    let root = t.path().join("data");
    std::fs::write(&identity, b"{}").unwrap();
    assert!(matches!(Owner::open_at(&engine(),&identity,&root),Err(e)if e=="WB_IDENTITY_REJECTED"));
    assert!(!root.exists());
}

#[test]
fn workspace_settings_budget_mode_survives_real_owner_restart() {
    use nai_contracts::workbench::{
        WorkspaceBudgetMode, WorkspaceSettingsCommand, WorkspaceSettingsDraft,
        foundation_command::Operation as C,
    };
    let t = tempfile::tempdir().unwrap();
    let identity = t.path().join("operator.json");
    let root = t.path().join("data");
    let ws = random_id().unwrap();
    let settings = |revision, mode, limit| WorkspaceSettingsCommand {
        workspace_id: ws.clone(),
        settings: Some(WorkspaceSettingsDraft {
            name: "预算验证".into(),
            currency: "CNY".into(),
            budget_mode: mode as i32,
            budget_limit_minor: limit,
            expected_revision: revision,
        }),
    };
    let mut owner = Owner::open_at(&engine(), &identity, &root).unwrap();
    let r = call(
        &mut owner,
        C::CreateWorkspaceSettings(settings(0, WorkspaceBudgetMode::Unlimited, 0)),
    );
    assert_eq!(r.llm_budget.unwrap().mode, 3);
    drop(owner);
    let mut owner = Owner::open_at(&engine(), &identity, &root).unwrap();
    let r = call(
        &mut owner,
        C::ReadLlmSettings(FoundationLlmRead {
            workspace_id: ws.clone(),
        }),
    );
    assert_eq!(r.llm_budget.unwrap().mode, 3);
    let r = call(
        &mut owner,
        C::EditWorkspaceSettings(settings(1, WorkspaceBudgetMode::Limited, 100)),
    );
    let b = r.llm_budget.unwrap();
    assert_eq!(b.mode, 2);
    assert_eq!(b.limit_minor, 100);
}

#[test]
fn workspace_management_crosses_native_owner_and_survives_reopen() {
    use nai_contracts::workbench::{FoundationWorkspaceRename, foundation_command::Operation as C};
    let t = tempfile::tempdir().unwrap();
    let identity = t.path().join("operator.json");
    let root = t.path().join("data");
    let ws = random_id().unwrap();
    let mut owner = Owner::open_at(&engine(), &identity, &root).unwrap();
    call(
        &mut owner,
        C::CreateWorkspace(FoundationWorkspace {
            workspace_id: ws.clone(),
            name: "管理测试".into(),
            budget_limit_minor: 0,
            currency: "CNY".into(),
        }),
    );
    let browse = || {
        C::ListWorkspaces(FoundationBrowse {
            limit: 32,
            ..Default::default()
        })
    };
    let before = call(&mut owner, browse()).resources.remove(0);
    assert!(before.created_at_ms > 0);
    call(
        &mut owner,
        C::RenameWorkspace(FoundationWorkspaceRename {
            workspace_id: ws,
            name: "已重命名".into(),
            expected_revision: before.revision,
        }),
    );
    drop(owner);
    let mut owner = Owner::open_at(&engine(), &identity, &root).unwrap();
    let after = call(&mut owner, browse()).resources.remove(0);
    assert_eq!(after.name, "已重命名");
    assert_eq!(after.created_at_ms, before.created_at_ms);
    assert_eq!(after.revision, before.revision + 1);
}

#[test]
fn workspace_removal_crosses_owner_and_recycle_survives_reopen() {
    use nai_contracts::workbench::{
        FoundationWorkspaceLifecycle, WorkspaceAction, foundation_command::Operation as C,
    };
    let t = tempfile::tempdir().unwrap();
    let identity = t.path().join("operator.json");
    let root = t.path().join("data");
    let mut owner = Owner::open_at(&engine(), &identity, &root).unwrap();
    let empty = random_id().unwrap();
    let used = random_id().unwrap();
    for ws in [&empty, &used] {
        call(
            &mut owner,
            C::CreateWorkspace(FoundationWorkspace {
                workspace_id: ws.clone(),
                name: "删除验证".into(),
                budget_limit_minor: if ws == &used { 1 } else { 0 },
                currency: "CNY".into(),
            }),
        );
    }
    let change = |ws: &str, revision, action| {
        C::WorkspaceLifecycle(FoundationWorkspaceLifecycle {
            workspace_id: ws.into(),
            expected_revision: revision,
            action: action as i32,
        })
    };
    let inspected = call(&mut owner, change(&empty, 0, WorkspaceAction::Inspect));
    assert!(!inspected.resources[0].has_content);
    call(&mut owner, change(&empty, 1, WorkspaceAction::DeleteEmpty));
    assert!(call(&mut owner, change(&used, 0, WorkspaceAction::Inspect)).resources[0].has_content);
    call(&mut owner, change(&used, 1, WorkspaceAction::Recycle));
    drop(owner);
    let mut owner = Owner::open_at(&engine(), &identity, &root).unwrap();
    let rows = call(
        &mut owner,
        C::ListWorkspaces(FoundationBrowse {
            kind: "recycled".into(),
            limit: 32,
            ..Default::default()
        }),
    )
    .resources;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, used);
    assert!(rows[0].recycled);
    call(
        &mut owner,
        change(&used, rows[0].revision, WorkspaceAction::Restore),
    );
    let rows = call(
        &mut owner,
        C::ListWorkspaces(FoundationBrowse {
            limit: 32,
            ..Default::default()
        }),
    )
    .resources;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, used);
    assert!(!rows[0].recycled);
}

#[test]
fn trusted_owner_runs_real_grounded_faq_and_adopts_only_to_draft() {
    use foundation_command::Operation as C;
    use serde_json::{Value, json};
    let t = tempfile::tempdir().unwrap();
    let mut owner = Owner::open_at(
        &engine(),
        &t.path().join("operator.json"),
        &t.path().join("data"),
    )
    .unwrap();
    let ws = random_id().unwrap();
    call(
        &mut owner,
        C::CreateWorkspace(FoundationWorkspace {
            workspace_id: ws.clone(),
            name: "FAQ测试".into(),
            currency: "CNY".into(),
            budget_limit_minor: 0,
        }),
    );
    let mut refs = Vec::new();
    let product_id = random_id().unwrap();
    for (id, doc) in [
        (
            product_id.clone(),
            serde_json::from_str::<Value>(include_str!(
                "../../../tests/fixtures/contracts/preparation.json"
            ))
            .unwrap(),
        ),
        (
            random_id().unwrap(),
            json!({"schema_version":1,"kind":"host_draft","name":"主持","style":"friendly","preferences":""}),
        ),
        (
            random_id().unwrap(),
            json!({"schema_version":1,"kind":"series_draft","name":"系列","goal":"介绍用途"}),
        ),
    ] {
        let r = call(
            &mut owner,
            C::PutPreparation(FoundationObject {
                workspace_id: ws.clone(),
                object_id: id.clone(),
                expected_revision: 0,
                body: doc.to_string(),
            }),
        );
        refs.push(json!({"object_id":id,"revision":r.revision,"sha256":r.sha256}));
    }
    let show_id = random_id().unwrap();
    let show=call(&mut owner,C::PutPreparation(FoundationObject{workspace_id:ws.clone(),object_id:show_id.clone(),expected_revision:0,body:json!({"schema_version":1,"kind":"show_draft","name":"场次","product":refs[0],"host":refs[1],"series":refs[2],"cues":[{"id":"intro","title":"介绍","talk_track_id":"opening","mandatory_fact_ids":["capacity"],"allow_questions":true}]}).to_string()}));
    let report = call(
        &mut owner,
        C::PublicationImpact(FoundationImpact {
            workspace_id: ws.clone(),
            report_id: random_id().unwrap(),
            show_id,
            show_revision: show.revision,
            previous_snapshot_id: String::new(),
        }),
    );
    let snapshot = call(
        &mut owner,
        C::Publish(FoundationPublish {
            workspace_id: ws.clone(),
            snapshot_id: random_id().unwrap(),
            report_id: report.resource_id,
            report_sha256: report.sha256,
            acknowledge_unknown: true,
        }),
    );
    // Pool commands cross the same page allowlist and authenticated owner IPC.
    let pool_scope = QuestionPoolRead {
        workspace_id: ws.clone(),
        snapshot_id: snapshot.resource_id.clone(),
        snapshot_sha256: snapshot.sha256.clone(),
    };
    let queued = call(
        &mut owner,
        C::EnqueueQuestion(QuestionPoolEnqueue {
            scope: Some(pool_scope.clone()),
            question_id: random_id().unwrap(),
            question: "容量多大？".into(),
        }),
    );
    let q = queued.question_pool.unwrap().entries.remove(0);
    let ready = call(
        &mut owner,
        C::RunQuestion(QuestionPoolRun {
            workspace_id: ws.clone(),
            question_id: q.id.clone(),
            expected_revision: q.revision,
            task_id: random_id().unwrap(),
        }),
    );
    let q = ready.question_pool.unwrap().entries.remove(0);
    assert_eq!(q.state, AgentTaskState::WaitingHuman as i32);
    let candidate = call(
        &mut owner,
        C::ReadFaq(FoundationFaqRead {
            workspace_id: ws.clone(),
            task_id: q.task_id.clone(),
        }),
    );
    assert!(candidate.body.contains("parent_task"));
    call(
        &mut owner,
        C::CancelQuestion(QuestionPoolCancel {
            workspace_id: ws.clone(),
            question_id: q.id,
            expected_revision: q.revision,
        }),
    );
    let rows = call(&mut owner, C::ReadQuestionPool(pool_scope));
    assert_eq!(
        rows.question_pool.unwrap().entries[0].state,
        AgentTaskState::Cancelled as i32
    );
    let cancelled = call(
        &mut owner,
        C::ReadFaq(FoundationFaqRead {
            workspace_id: ws.clone(),
            task_id: q.task_id,
        }),
    );
    assert!(cancelled.body.is_empty());
    assert_eq!(cancelled.faq_reason, FaqReason::Cancelled as i32);
    let live = call(
        &mut owner,
        C::CreateLiveRun(LiveRunCreate {
            scope: Some(QuestionPoolRead {
                workspace_id: ws.clone(),
                snapshot_id: snapshot.resource_id.clone(),
                snapshot_sha256: snapshot.sha256.clone(),
            }),
            run_id: random_id().unwrap(),
        }),
    )
    .live_context
    .unwrap();
    assert_eq!(live.cursor, 0);
    let plan = call(
        &mut owner,
        C::ReadLiveSpeechPlan(LiveRunRead {
            workspace_id: ws.clone(),
            run_id: live.run_id.clone(),
        }),
    )
    .live_speech_plan
    .unwrap();
    assert_eq!(plan.prefetch_limit, 2);
    assert_eq!(plan.run_id, live.run_id);
    assert!(!plan.sentences.is_empty());
    assert_eq!(plan.revision, live.revision);
    let paused = call(
        &mut owner,
        C::ControlLiveRun(LiveRunControl {
            workspace_id: ws.clone(),
            run_id: live.run_id.clone(),
            expected_revision: live.revision,
            expected_epoch: live.epoch,
            action: LiveRunAction::Pause as i32,
        }),
    )
    .live_context
    .unwrap();
    assert_eq!(paused.state, LiveRunState::Paused as i32);
    let loaded = call(
        &mut owner,
        C::ReadLiveRun(LiveRunRead {
            workspace_id: ws.clone(),
            run_id: live.run_id.clone(),
        }),
    )
    .live_context
    .unwrap();
    assert_eq!(loaded.submitted_count, 0);
    let scope = LiveRunRead {
        workspace_id: ws.clone(),
        run_id: live.run_id.clone(),
    };
    let approval_id = random_id().unwrap();
    let approved = call(
        &mut owner,
        C::ControlLiveBatch(LiveBatchControl {
            run: Some(scope.clone()),
            expected_revision: loaded.revision,
            expected_epoch: loaded.epoch,
            approval_id: approval_id.clone(),
            plan_sha256: plan.plan_sha256.clone(),
            acknowledge_rights: true,
            revoke: false,
        }),
    )
    .live_context
    .unwrap();
    assert_eq!(approved.sentence_cursor, 0);
    let p = call(&mut owner, C::ReadLiveSpeechPlan(scope.clone()))
        .live_speech_plan
        .unwrap();
    assert!(p.batch_approval.unwrap().approved);
    let card = call(
        &mut owner,
        C::PreviewLiveSentence(LiveSentencePreview {
            run: Some(scope.clone()),
            preview_id: random_id().unwrap(),
            sentence_cursor: 0,
        }),
    );
    let doc: Value = serde_json::from_str(&card.body).unwrap();
    assert_eq!(doc["live_sentence"]["plan_sha256"], plan.plan_sha256);
    let loaded = call(
        &mut owner,
        C::ControlLiveBatch(LiveBatchControl {
            run: Some(scope),
            expected_revision: approved.revision,
            expected_epoch: approved.epoch,
            approval_id,
            plan_sha256: plan.plan_sha256.clone(),
            acknowledge_rights: false,
            revoke: true,
        }),
    )
    .live_context
    .unwrap();

    call(
        &mut owner,
        C::ControlLiveRun(LiveRunControl {
            workspace_id: ws.clone(),
            run_id: live.run_id,
            expected_revision: loaded.revision,
            expected_epoch: loaded.epoch,
            action: LiveRunAction::End as i32,
        }),
    );
    let model_id = random_id().unwrap();
    call(
        &mut owner,
        C::SaveLlmModel(FoundationLlmModel {
            workspace_id: ws.clone(),
            model_id: model_id.clone(),
            expected_revision: 0,
            name: "metadata-only preflight".into(),
            base_url: "https://example.invalid/v1".into(),
            model: "test".into(),
            input_minor_per_million: 100,
            output_minor_per_million: 100,
            enabled: true,
            api_key: Vec::new(),
            prices_confirmed: true,
        }),
    );
    let checked = call(
        &mut owner,
        C::CheckLlm(FoundationLlmPlan {
            workspace_id: ws.clone(),
            snapshot_id: snapshot.resource_id.clone(),
            snapshot_sha256: snapshot.sha256.clone(),
            cue_id: "intro".into(),
            model_id,
            model_revision: 1,
            max_cost_minor: 1,
        }),
    );
    assert_eq!(
        checked.llm_check.unwrap().reason,
        LlmLocalReason::CredentialMissing as i32
    );
    assert!(checked.body.is_empty());
    let task = call(
        &mut owner,
        C::AskFaq(FoundationFaqRequest {
            workspace_id: ws.clone(),
            task_id: random_id().unwrap(),
            snapshot_id: snapshot.resource_id,
            snapshot_sha256: snapshot.sha256,
            question: "这个杯子的容量有多大？".into(),
        }),
    );
    assert_eq!(task.faq_reason, FaqReason::Retrieved as i32);
    assert!(task.body.contains("matched_question"));
    assert!(task.body.contains("产品包装规格页"));
    let accepted = call(
        &mut owner,
        C::ResolveFaq(FoundationFaqResolve {
            workspace_id: ws,
            task_id: task.resource_id,
            task_sha256: task.sha256,
            decision: FaqDecision::Accept as i32,
            ..Default::default()
        }),
    );
    assert_eq!(accepted.adopted_object_id, product_id);
    assert_eq!(accepted.adopted_revision, 2);
    assert_eq!(accepted.suggestion_state, SuggestionState::Accepted as i32);
}

#[test]
fn zero_key_manual_preparation_crosses_real_ipc_and_restores_only_records() {
    use foundation_command::Operation as C;
    use serde_json::{Value, json};
    let t = tempfile::tempdir().unwrap();
    let identity = t.path().join("operator.json");
    let root = t.path().join("data");
    let mut owner = Owner::open_at(&engine(), &identity, &root).unwrap();
    let ws = random_id().unwrap();
    call(
        &mut owner,
        C::CreateWorkspace(FoundationWorkspace {
            workspace_id: ws.clone(),
            name: "自己的内容零API测试".into(),
            budget_limit_minor: 0,
            currency: "CNY".into(),
        }),
    );
    let ids: Vec<_> = (0..7).map(|_| random_id().unwrap()).collect();
    let mut refs = vec![];
    for (i,doc) in [serde_json::from_str::<Value>(include_str!("../../../tests/fixtures/contracts/preparation.json")).unwrap(),json!({"schema_version":1,"kind":"host_draft","name":"主持","style":"friendly","preferences":""}),json!({"schema_version":1,"kind":"series_draft","name":"本场","goal":"说明实际产品"})].into_iter().enumerate(){
        let r=call(&mut owner,C::PutPreparation(FoundationObject{workspace_id:ws.clone(),object_id:ids[i].clone(),expected_revision:0,body:doc.to_string()}));
        refs.push(json!({"object_id":ids[i],"revision":r.revision,"sha256":r.sha256}));
    }
    let show = json!({"schema_version":1,"kind":"show_draft","name":"手动场次","product":refs[0],"host":refs[1],"series":refs[2],"cues":[{"id":"intro","title":"产品介绍","talk_track_id":"opening","mandatory_fact_ids":["capacity"],"allow_questions":true},{"id":"details","title":"规格复核","talk_track_id":"opening","mandatory_fact_ids":["capacity"],"allow_questions":false}]});
    let r = call(
        &mut owner,
        C::PutPreparation(FoundationObject {
            workspace_id: ws.clone(),
            object_id: ids[3].clone(),
            expected_revision: 0,
            body: show.to_string(),
        }),
    );
    let show_ref = json!({"object_id":ids[3],"revision":r.revision,"sha256":r.sha256});
    let impact = call(
        &mut owner,
        C::PublicationImpact(FoundationImpact {
            workspace_id: ws.clone(),
            report_id: random_id().unwrap(),
            show_id: ids[3].clone(),
            show_revision: 1,
            previous_snapshot_id: String::new(),
        }),
    );
    let published = call(
        &mut owner,
        C::Publish(FoundationPublish {
            workspace_id: ws.clone(),
            snapshot_id: ids[4].clone(),
            report_id: impact.resource_id,
            report_sha256: impact.sha256,
            acknowledge_unknown: true,
        }),
    );
    let mut doc = json!({"schema_version":1,"kind":"manual_run_draft","name":"本场手动记录","show":show_ref,"snapshot":{"object_id":ids[4],"revision":published.revision,"sha256":published.sha256},"cue_ids":["intro","details"],"cursor":0,"status":"open","marks":[],"mode":"private_manual","evidence":"self_reported"});
    call(
        &mut owner,
        C::PutPreparation(FoundationObject {
            workspace_id: ws.clone(),
            object_id: ids[5].clone(),
            expected_revision: 0,
            body: doc.to_string(),
        }),
    );
    let card = call(
        &mut owner,
        C::Preview(FoundationPreview {
            workspace_id: ws.clone(),
            preview_id: ids[6].clone(),
            snapshot_id: ids[4].clone(),
            snapshot_sha256: published.sha256.clone(),
            cue_id: "intro".into(),
            session_id: ids[5].clone(),
            expected_epoch: 0,
        }),
    );
    assert_eq!(card.state, DeliveryState::Delivering as i32);
    assert!(!card.body.is_empty());
    call(
        &mut owner,
        C::StopPreview(FoundationPreviewStop {
            workspace_id: ws.clone(),
            session_id: ids[5].clone(),
            expected_epoch: card.epoch,
        }),
    );
    doc["cursor"] = json!(1);
    call(
        &mut owner,
        C::PutPreparation(FoundationObject {
            workspace_id: ws.clone(),
            object_id: ids[5].clone(),
            expected_revision: 1,
            body: doc.to_string(),
        }),
    );
    let second_session = random_id().unwrap();
    let second = call(
        &mut owner,
        C::Preview(FoundationPreview {
            workspace_id: ws.clone(),
            preview_id: random_id().unwrap(),
            snapshot_id: ids[4].clone(),
            snapshot_sha256: published.sha256.clone(),
            cue_id: "details".into(),
            session_id: second_session.clone(),
            expected_epoch: 0,
        }),
    );
    assert_eq!(second.state, DeliveryState::Delivering as i32);
    call(
        &mut owner,
        C::StopPreview(FoundationPreviewStop {
            workspace_id: ws.clone(),
            session_id: second_session,
            expected_epoch: second.epoch,
        }),
    );
    doc["status"] = "ended".into();
    doc["marks"] = json!([{"id":random_id().unwrap(),"cue_id":"intro","note":"下场补充说明"}]);
    call(
        &mut owner,
        C::PutPreparation(FoundationObject {
            workspace_id: ws.clone(),
            object_id: ids[5].clone(),
            expected_revision: 2,
            body: doc.to_string(),
        }),
    );
    let settings = call(
        &mut owner,
        C::ReadLlmSettings(FoundationLlmRead {
            workspace_id: ws.clone(),
        }),
    );
    assert!(settings.llm_models.is_empty() && settings.tts_models.is_empty());
    drop(owner);
    let mut owner = Owner::open_at(&engine(), &identity, &root).unwrap();
    let restored = call(
        &mut owner,
        C::ReadObject(FoundationReadObject {
            workspace_id: ws.clone(),
            object_id: ids[5].clone(),
            revision: 0,
        }),
    );
    assert_eq!(serde_json::from_str::<Value>(&restored.body).unwrap(), doc);
    let stopped = call(
        &mut owner,
        C::ReadPreview(FoundationPreviewRead {
            workspace_id: ws,
            preview_id: ids[6].clone(),
        }),
    );
    assert!(stopped.body.is_empty());
    drop(owner);
}
