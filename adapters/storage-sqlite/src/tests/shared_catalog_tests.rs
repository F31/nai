use super::*;
use nai_runtime_core::llm::CredentialVault;
use serde_json::{Value, json};
use std::sync::{Mutex, atomic::AtomicBool};
#[derive(Default)]
struct Vault {
    keys: Mutex<TestKeys>,
    seals: AtomicU64,
    fail: AtomicBool,
}
type TestKeys = std::collections::BTreeMap<String, (String, String, String, Vec<u8>)>;
impl CredentialVault for Vault {
    fn seal(&self, ws: &str, id: &str, url: &str, key: &[u8]) -> Result<Value> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(Error::PermissionDenied);
        }
        let n = self.seals.fetch_add(1, Ordering::SeqCst) + 1000;
        let reference = super::id(n);
        self.keys.lock().unwrap().insert(
            reference.clone(),
            (ws.into(), id.into(), url.into(), key.into()),
        );
        Ok(json!({"reference":reference,"nonce":"00".repeat(12),"ciphertext":"aa".repeat(32)}))
    }
    fn open(
        &self,
        ws: &str,
        id: &str,
        url: &str,
        v: &Value,
    ) -> Result<zeroize::Zeroizing<Vec<u8>>> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(Error::PermissionDenied);
        }
        let keys = self.keys.lock().unwrap();
        let (a, b, c, k) = keys
            .get(v["reference"].as_str().ok_or(Error::InvalidArgument)?)
            .ok_or(Error::NotFound)?;
        if (a.as_str(), b.as_str(), c.as_str()) != (ws, id, url) {
            return Err(Error::PermissionDenied);
        }
        Ok(zeroize::Zeroizing::new(k.clone()))
    }
}
fn text(ws: u64) -> C {
    C::SaveLlmModel(FoundationLlmModel {
        workspace_id: id(ws),
        model_id: id(100),
        name: "同名文本".into(),
        base_url: "https://example.invalid/v1".into(),
        model: "test".into(),
        enabled: true,
        api_key: b"synthetic-key".to_vec(),
        prices_confirmed: true,
        input_minor_per_million: 100,
        output_minor_per_million: 200,
        ..Default::default()
    })
}
fn speech() -> C {
    C::SaveTtsModel(FoundationTtsModel {
        protocol: "openai_speech".into(),
        language: String::new(),
        workspace_id: id(10),
        model_id: id(101),
        name: "语音".into(),
        base_url: "https://example.invalid/v1".into(),
        model: "speech".into(),
        voice: "alex".into(),
        enabled: true,
        api_key: b"synthetic-key".to_vec(),
        price_known: true,
        character_minor_per_million: 100,
        ..Default::default()
    })
}
fn stop(s: &mut Store) {
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
fn request(s: &mut Store) -> SharedModelMigration {
    let p = exec(s, 1, C::PreviewSharedModels(FoundationLlmRead::default()))
        .unwrap()
        .shared_model_preview
        .unwrap();
    SharedModelMigration {
        migration_id: id(900),
        plan_sha256: p.plan_sha256,
        entries: p
            .rows
            .into_iter()
            .enumerate()
            .map(|(i, r)| {
                let mut e = r.source.unwrap();
                e.catalog_model_id = id(910 + i as u64);
                e
            })
            .collect(),
    }
}
fn step(s: &mut Store, r: &SharedModelMigration) -> Result<FoundationResult> {
    exec(s, 1, C::MigrateSharedModels(r.clone()))
}
fn finish(s: &mut Store, r: &SharedModelMigration) {
    for _ in 0..r.entries.len() + 4 {
        if step(s, r).unwrap().shared_catalog_enabled {
            return;
        }
    }
    panic!("migration did not commit");
}
fn settings(s: &mut Store, ws: u64) -> FoundationResult {
    exec(
        s,
        1,
        C::ReadLlmSettings(FoundationLlmRead {
            workspace_id: id(ws),
        }),
    )
    .unwrap()
}
#[test]
fn migration_resumes_across_restart_preserves_sources_and_shares_crud_and_budget() {
    let (t, mut s, _) = setup();
    let vault = Arc::new(Vault::default());
    s.attach_llm_vault(vault.clone());
    s.attach_tts_vault(vault.clone());
    exec(&mut s, 1, text(10)).unwrap();
    exec(&mut s, 1, speech()).unwrap();
    exec(
        &mut s,
        1,
        C::CreateWorkspace(FoundationWorkspace {
            workspace_id: id(20),
            name: "回收来源".into(),
            budget_limit_minor: 7,
            currency: "CNY".into(),
        }),
    )
    .unwrap();
    exec(&mut s, 1, text(20)).unwrap();
    exec(
        &mut s,
        1,
        C::WorkspaceLifecycle(FoundationWorkspaceLifecycle {
            workspace_id: id(20),
            expected_revision: 1,
            action: WorkspaceAction::Recycle as i32,
        }),
    )
    .unwrap();
    stop(&mut s);
    let r = request(&mut s);
    let source = llm::raw(
        &s.conn,
        &s.root,
        &id(10),
        &id(100),
        1,
        "publication_llm_config",
    )
    .unwrap();
    assert_eq!(
        step(&mut s, &r)
            .unwrap()
            .shared_model_migration
            .unwrap()
            .stage,
        "preparing"
    );
    assert_eq!(exec(&mut s, 1, text(10)), Err(Error::Busy));
    step(&mut s, &r).unwrap(); // verified backup
    step(&mut s, &r).unwrap(); // one item durably rewrapped
    assert!(!shared_catalog::enabled(&s.conn).unwrap());
    let seals = vault.seals.load(Ordering::SeqCst);
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    s.attach_llm_vault(vault.clone());
    s.attach_tts_vault(vault.clone());
    let resumed = exec(
        &mut s,
        1,
        C::ReadSharedMigration(FoundationLlmRead::default()),
    )
    .unwrap()
    .shared_model_migration
    .unwrap();
    assert_eq!(resumed.completed, 1);
    assert_eq!(resumed.request, Some(r.clone()));
    finish(&mut s, &r);
    assert_eq!(vault.seals.load(Ordering::SeqCst), seals + 2);
    assert_eq!(
        llm::raw(
            &s.conn,
            &s.root,
            &id(10),
            &id(100),
            1,
            "publication_llm_config"
        )
        .unwrap(),
        source
    );
    exec(
        &mut s,
        1,
        C::CreateWorkspace(FoundationWorkspace {
            workspace_id: id(21),
            name: "新工作区".into(),
            budget_limit_minor: 55,
            currency: "CNY".into(),
        }),
    )
    .unwrap();
    let a = settings(&mut s, 10);
    let b = settings(&mut s, 21);
    assert!(a.shared_catalog_enabled);
    assert_eq!(a.llm_models, b.llm_models);
    assert_eq!(a.tts_models, b.tts_models);
    assert_eq!(a.llm_models.len(), 2);
    assert_eq!(a.tts_models.len(), 1);
    assert_ne!(
        a.llm_budget.unwrap().limit_minor,
        b.llm_budget.unwrap().limit_minor
    );
    let e = r.entries.iter().find(|e| e.capability == "llm").unwrap();
    let (v, _, _) = llm::raw(
        &s.conn,
        &s.root,
        &id(21),
        &e.catalog_model_id,
        0,
        "publication_llm_config",
    )
    .unwrap();
    assert_eq!(
        &*vault
            .open(
                shared_catalog::SCOPE,
                &e.catalog_model_id,
                "https://example.invalid/v1",
                &v["secret"]
            )
            .unwrap(),
        b"synthetic-key"
    );
    assert!(
        vault
            .open(
                &id(21),
                &e.catalog_model_id,
                "https://example.invalid/v1",
                &v["secret"]
            )
            .is_err()
    );
    let before = vault.seals.load(Ordering::SeqCst);
    assert!(step(&mut s, &r).unwrap().shared_catalog_enabled);
    assert_eq!(before, vault.seals.load(Ordering::SeqCst));
    let mut edit = match text(21) {
        C::SaveLlmModel(x) => x,
        _ => unreachable!(),
    };
    edit.model_id = e.catalog_model_id.clone();
    edit.expected_revision = 1;
    edit.api_key.clear();
    edit.name = "共享编辑".into();
    exec(&mut s, 1, C::SaveLlmModel(edit)).unwrap();
    assert!(
        settings(&mut s, 10)
            .llm_models
            .iter()
            .any(|m| m.name == "共享编辑")
    );
    exec(
        &mut s,
        1,
        C::DeleteLlmModel(FoundationDelete {
            workspace_id: id(21),
            object_id: e.catalog_model_id.clone(),
            expected_revision: 2,
        }),
    )
    .unwrap();
    assert_eq!(settings(&mut s, 10).llm_models.len(), 1);
    let mut edit_speech = match speech() {
        C::SaveTtsModel(x) => x,
        _ => unreachable!(),
    };
    edit_speech.workspace_id = id(21);
    edit_speech.model_id = a.tts_models[0].id.clone();
    edit_speech.expected_revision = 1;
    edit_speech.api_key.clear();
    edit_speech.voice = "new-voice".into();
    exec(&mut s, 1, C::SaveTtsModel(edit_speech)).unwrap();
    assert_eq!(settings(&mut s, 10).tts_models[0].voice, "new-voice");
    exec(
        &mut s,
        1,
        C::Grant(FoundationGrant {
            workspace_id: id(21),
            principal_id: id(2),
            role: FoundationRole::Admin as i32,
            expected_revision: 0,
        }),
    )
    .unwrap();
    let mut forbidden = match text(21) {
        C::SaveLlmModel(x) => x,
        _ => unreachable!(),
    };
    forbidden.model_id = id(999);
    forbidden.api_key.clear();
    assert_eq!(
        exec(&mut s, 2, C::SaveLlmModel(forbidden)),
        Err(Error::PermissionDenied)
    );
    assert_eq!(
        llm::raw(
            &s.conn,
            &s.root,
            &id(10),
            &id(100),
            1,
            "publication_llm_config"
        )
        .unwrap(),
        source
    );
}
#[test]
fn migration_rejects_bad_mapping_busy_unauthorized_and_damaged_backup() {
    let (_t, mut s, _) = setup();
    exec(&mut s, 1, text_without_key()).unwrap();
    let r = request(&mut s);
    assert_eq!(step(&mut s, &r), Err(Error::Busy));
    stop(&mut s);
    assert_eq!(
        exec(&mut s, 2, C::MigrateSharedModels(r.clone())),
        Err(Error::PermissionDenied)
    );
    let mut bad = r.clone();
    bad.entries[0].source_sha256 = "00".repeat(32);
    assert_eq!(step(&mut s, &bad), Err(Error::RevisionConflict));
    step(&mut s, &r).unwrap();
    let result = step(&mut s, &r).unwrap();
    let backup = result.shared_model_migration.unwrap().backup_id;
    step(&mut s, &r).unwrap();
    std::fs::write(
        s.root
            .join("model-migration-backups")
            .join(backup)
            .join("foundation.sqlite"),
        b"damaged",
    )
    .unwrap();
    assert!(step(&mut s, &r).is_err());
    assert!(!shared_catalog::enabled(&s.conn).unwrap());
    assert_eq!(settings(&mut s, 10).llm_models.len(), 1);
}
fn text_without_key() -> C {
    let C::SaveLlmModel(mut x) = text(10) else {
        unreachable!()
    };
    x.api_key.clear();
    C::SaveLlmModel(x)
}
#[test]
fn credential_failure_is_bounded_and_never_publishes_partial_catalogue() {
    let (_t, mut s, _) = setup();
    let vault = Arc::new(Vault::default());
    s.attach_llm_vault(vault.clone());
    exec(&mut s, 1, text(10)).unwrap();
    stop(&mut s);
    let r = request(&mut s);
    step(&mut s, &r).unwrap();
    step(&mut s, &r).unwrap();
    vault.fail.store(true, Ordering::SeqCst);
    for _ in 0..3 {
        assert_eq!(step(&mut s, &r), Err(Error::PermissionDenied));
    }
    assert_eq!(step(&mut s, &r), Err(Error::ResourceExhausted));
    assert!(!shared_catalog::enabled(&s.conn).unwrap());
    assert_eq!(
        s.conn
            .query_row("SELECT count(*) FROM shared_models", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    exec(
        &mut s,
        1,
        C::CancelSharedMigration(FoundationLlmTaskRead {
            workspace_id: String::new(),
            task_id: r.migration_id.clone(),
        }),
    )
    .unwrap();
    assert!(!shared_catalog::pending(&s.conn).unwrap());
    assert!(exec(&mut s, 1, text_without_key()).is_err()); // existing ID remains, not erased
    assert_eq!(settings(&mut s, 10).llm_models.len(), 1);
}

#[test]
fn shared_models_feed_both_worker_paths_without_changing_budget_ownership() {
    let (t, mut s, _) = setup();
    let vault = Arc::new(Vault::default());
    s.attach_llm_vault(vault.clone());
    s.attach_tts_vault(vault.clone());
    exec(&mut s, 1, text(10)).unwrap();
    exec(&mut s, 1, speech()).unwrap();
    stop(&mut s);
    let r = request(&mut s);
    finish(&mut s, &r);
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    s.attach_llm_vault(vault.clone());
    s.attach_tts_vault(vault.clone());
    exec(
        &mut s,
        1,
        C::CreateWorkspace(FoundationWorkspace {
            workspace_id: id(21),
            name: "另一个预算".into(),
            budget_limit_minor: 55,
            currency: "CNY".into(),
        }),
    )
    .unwrap();
    let settings = settings(&mut s, 21);
    let text = &settings.llm_models[0];
    let speech = &settings.tts_models[0];
    assert_eq!(
        exec(
            &mut s,
            1,
            C::TestLlm(FoundationModelTest {
                workspace_id: id(21),
                task_id: id(950),
                model_id: id(100),
                model_revision: 1,
            })
        ),
        Err(Error::NotFound)
    );
    exec(
        &mut s,
        1,
        C::TestLlm(FoundationModelTest {
            workspace_id: id(21),
            task_id: id(951),
            model_id: text.id.clone(),
            model_revision: 1,
        }),
    )
    .unwrap();
    let worker = s
        .llm_request(&Operation::new(4000).unwrap(), &id(1), &id(21), &id(951))
        .unwrap();
    assert_eq!(&*worker.api_key, b"synthetic-key");
    exec(
        &mut s,
        1,
        C::CancelLlm(FoundationLlmTaskRead {
            workspace_id: id(21),
            task_id: id(951),
        }),
    )
    .unwrap();
    exec(
        &mut s,
        1,
        C::TestTts(FoundationTtsTest {
            workspace_id: id(21),
            task_id: id(952),
            model_id: speech.id.clone(),
            model_revision: 1,
        }),
    )
    .unwrap();
    let worker = s
        .tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(21), &id(952))
        .unwrap();
    assert_eq!(&*worker.api_key, b"synthetic-key");
    let own: u32 = s
        .conn
        .query_row(
            "SELECT count(*) FROM budget_reservations WHERE workspace_id=?1",
            [id(21)],
            |r| r.get(0),
        )
        .unwrap();
    let other: u32 = s
        .conn
        .query_row(
            "SELECT count(*) FROM budget_reservations WHERE workspace_id=?1",
            [id(10)],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!((own, other), (0, 0));
}

#[test]
fn empty_catalogue_can_be_enabled_and_new_models_are_shared_immediately() {
    let (_t, mut s, _) = setup();
    stop(&mut s);
    let r = request(&mut s);
    assert!(r.entries.is_empty());
    finish(&mut s, &r);
    exec(&mut s, 1, text_without_key()).unwrap();
    assert!(settings(&mut s, 10).shared_catalog_enabled);
    assert_eq!(settings(&mut s, 10).llm_models.len(), 1);
    assert_eq!(
        exec(
            &mut s,
            1,
            C::CancelSharedMigration(FoundationLlmTaskRead {
                workspace_id: String::new(),
                task_id: r.migration_id
            })
        ),
        Err(Error::RevisionConflict)
    );
}

#[test]
fn capacity_overflow_is_rejected_before_creating_migration_or_partial_catalogue() {
    let (_t, mut s, _) = setup();
    exec(
        &mut s,
        1,
        C::CreateWorkspace(FoundationWorkspace {
            workspace_id: id(21),
            name: "第二来源".into(),
            budget_limit_minor: 0,
            currency: "CNY".into(),
        }),
    )
    .unwrap();
    for ws in [10, 21] {
        for n in 0..5 {
            let C::SaveLlmModel(mut x) = text(ws) else {
                unreachable!()
            };
            x.model_id = id(100 + n);
            x.api_key.clear();
            exec(&mut s, 1, C::SaveLlmModel(x)).unwrap();
        }
    }
    stop(&mut s);
    let r = request(&mut s);
    assert_eq!(step(&mut s, &r), Err(Error::ResourceExhausted));
    assert!(!shared_catalog::pending(&s.conn).unwrap());
    assert_eq!(settings(&mut s, 10).llm_models.len(), 5);
}

#[test]
fn migration_with_avatar_resumes_after_restart_and_preserves_credentials_and_package() {
    let (t, mut s, _) = setup();
    let vault = Arc::new(Vault::default());
    s.attach_llm_vault(vault.clone());
    exec(&mut s, 1, text(10)).unwrap();
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
    let avatar = json!({"schema_version":1,"kind":"avatar_profile_draft","name":"avatar","avatar_id":"avatar","package_version":"1","package_sha256":package,"license_sha256":rights,"width":2,"height":1,"selected":false,"revoked":false}).to_string();
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
    stop(&mut s);
    let r = request(&mut s);
    assert_eq!(
        step(&mut s, &r)
            .unwrap()
            .shared_model_migration
            .unwrap()
            .stage,
        "preparing"
    );
    assert_eq!(
        step(&mut s, &r)
            .unwrap()
            .shared_model_migration
            .unwrap()
            .stage,
        "rewrapping"
    );
    step(&mut s, &r).unwrap();
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    s.attach_llm_vault(vault.clone());
    finish(&mut s, &r);
    assert!(shared_catalog::enabled(&s.conn).unwrap());
    assert!(nai_avatar_files::verify_installed(t.path(), &package, &rights).is_ok());
    let model = &r.entries[0];
    let (doc, _, _) = llm::raw(
        &s.conn,
        &s.root,
        &id(10),
        &model.catalog_model_id,
        0,
        "publication_llm_config",
    )
    .unwrap();
    assert_eq!(
        &*vault
            .open(
                shared_catalog::SCOPE,
                &model.catalog_model_id,
                "https://example.invalid/v1",
                &doc["secret"]
            )
            .unwrap(),
        b"synthetic-key"
    );
}

fn avatar_kill_recovery(corrupt_backup: bool, kill_during_copy: bool) {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    const FLAG: &str = "NAI_AVATAR_BACKUP_KILL_TEST_ROOT";
    if let Some(root) = std::env::var_os(FLAG) {
        let root = std::path::PathBuf::from(root);
        let mut s = Store::open(&root, &id(1), true, Arc::default()).unwrap();
        exec(
            &mut s,
            1,
            C::CreateWorkspace(FoundationWorkspace {
                workspace_id: id(10),
                name: "isolated".into(),
                budget_limit_minor: 0,
                currency: "CNY".into(),
            }),
        )
        .unwrap();
        let ids = ["base", "closed", "open"];
        let assets: [&[u8]; 3] = [b"base", b"closed", b"open"];
        let manifest = b"manifest";
        let config = b"config";
        let license = b"license";
        let mut h = Sha256::new();
        h.update(manifest);
        h.update(config);
        h.update(license);
        for (name, bytes) in ids.iter().zip(assets) {
            h.update(name.as_bytes());
            h.update(bytes);
        }
        let package = h
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let rights = hash(license);
        let dir = root.join("avatar-packages").join(&package);
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
        let avatar=json!({"schema_version":1,"kind":"avatar_profile_draft","name":"avatar","avatar_id":"avatar","package_version":"1","package_sha256":package,"license_sha256":rights,"width":2,"height":1,"selected":false,"revoked":false}).to_string();
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
        let r = request(&mut s);
        step(&mut s, &r).unwrap();
        assert_eq!(
            step(&mut s, &r)
                .unwrap()
                .shared_model_migration
                .unwrap()
                .stage,
            "rewrapping"
        );
        std::fs::write(root.join("kill-test-ready"), b"ready").unwrap();
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    let root = tempfile::tempdir().unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "tests::shared_catalog_tests::avatar_migration_recovers_after_process_is_killed",
            "--test-threads=1",
        ])
        .env(FLAG, root.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if kill_during_copy {
        command.env("NAI_AVATAR_MIGRATION_COPY_PAUSE", root.path());
    }
    let mut child = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    while !root.path().join("kill-test-ready").exists() && Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            panic!("child exited before backup");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let ready = root.path().join("kill-test-ready").exists();
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(ready, "child did not complete backup before deadline");
    let mut s = Store::open(root.path(), &id(1), false, Arc::default()).unwrap();
    let state = exec(
        &mut s,
        1,
        C::ReadSharedMigration(FoundationLlmRead::default()),
    )
    .unwrap()
    .shared_model_migration
    .unwrap();
    assert_eq!(
        state.stage,
        if kill_during_copy {
            "preparing"
        } else {
            "rewrapping"
        }
    );
    let r = state.request.unwrap();
    let interrupted_id: String = s
        .conn
        .query_row(
            "SELECT backup_id FROM shared_model_migration WHERE id=?1",
            [&r.migration_id],
            |row| row.get(0),
        )
        .unwrap();
    let result = exec(
        &mut s,
        1,
        C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(70),
            revision: 0,
        }),
    )
    .unwrap();
    let doc: Value = serde_json::from_str(&result.body).unwrap();
    let package = doc["package_sha256"].as_str().unwrap();
    let rights = doc["license_sha256"].as_str().unwrap();
    if corrupt_backup {
        let backup_id: String = s
            .conn
            .query_row(
                "SELECT backup_id FROM shared_model_migration WHERE id=?1",
                [&r.migration_id],
                |row| row.get(0),
            )
            .unwrap();
        std::fs::write(
            root.path()
                .join("model-migration-backups")
                .join(backup_id)
                .join("avatar-packages")
                .join(package)
                .join("asset-1.rgba"),
            b"corrupt",
        )
        .unwrap();
        assert!(step(&mut s, &r).is_err());
        assert!(!shared_catalog::enabled(&s.conn).unwrap());
    } else {
        finish(&mut s, &r);
        assert!(shared_catalog::enabled(&s.conn).unwrap());
        if kill_during_copy {
            let preserved = root
                .path()
                .join("model-migration-incomplete")
                .join(&interrupted_id);
            assert!(
                preserved
                    .join("avatar-packages")
                    .join(package)
                    .join("manifest.json")
                    .exists()
            );
            assert!(!preserved.join("manifest.json").exists());
            assert!(
                !root
                    .path()
                    .join("model-migration-backups")
                    .join(&interrupted_id)
                    .exists()
            );
        }
    }
    assert!(nai_avatar_files::verify_installed(root.path(), package, rights).is_ok());
}
#[test]
fn avatar_migration_recovers_after_process_is_killed() {
    avatar_kill_recovery(false, false);
}
#[test]
fn avatar_migration_rejects_corrupt_backup_after_process_is_killed() {
    avatar_kill_recovery(true, false);
}
#[test]
fn avatar_migration_recovers_after_kill_during_copy() {
    avatar_kill_recovery(false, true);
}
