use super::*;
use nai_runtime_core::llm::CredentialVault;
use serde_json::{Value, json};
struct TestVault;
impl CredentialVault for TestVault {
    fn seal(&self, _: &str, _: &str, _: &str, _: &[u8]) -> Result<Value> {
        Ok(json!({"reference":id(600),"nonce":"00".repeat(12),"ciphertext":"aa".repeat(32)}))
    }
    fn open(&self, _: &str, _: &str, _: &str, _: &Value) -> Result<zeroize::Zeroizing<Vec<u8>>> {
        Ok(zeroize::Zeroizing::new(b"synthetic-key".to_vec()))
    }
}
fn speech(n: u64, rev: u64) -> FoundationTtsModel {
    FoundationTtsModel {
        protocol: "openai_speech".into(),
        language: String::new(),
        workspace_id: id(10),
        model_id: id(n),
        expected_revision: rev,
        name: format!("测试语音{n}"),
        base_url: "https://example.invalid/v1".into(),
        model: "test-speech".into(),
        voice: "test-voice".into(),
        enabled: true,
        api_key: if rev == 0 {
            b"synthetic-key".to_vec()
        } else {
            vec![]
        },
        price_known: false,
        character_minor_per_million: 0,
    }
}
fn text() -> FoundationLlmModel {
    FoundationLlmModel {
        workspace_id: id(10),
        model_id: id(100),
        expected_revision: 0,
        name: "text".into(),
        base_url: "https://example.invalid/v1".into(),
        model: "test".into(),
        enabled: true,
        api_key: b"synthetic-key".to_vec(),
        prices_confirmed: true,
        input_minor_per_million: 100,
        output_minor_per_million: 200,
    }
}
fn settings(s: &mut Store) -> FoundationResult {
    exec(
        s,
        1,
        C::ReadLlmSettings(FoundationLlmRead {
            workspace_id: id(10),
        }),
    )
    .unwrap()
}
fn removal(n: u64, revision: u64) -> FoundationDelete {
    FoundationDelete {
        workspace_id: id(10),
        object_id: id(n),
        expected_revision: revision,
    }
}
#[test]
fn speech_crud_preserves_key_unknown_price_and_rejects_stale_or_unauthorized_changes() {
    let (t, mut s, _) = setup();
    s.attach_tts_vault(Arc::new(TestVault));
    exec(&mut s, 1, C::SaveTtsModel(speech(101, 0))).unwrap();
    exec(&mut s, 1, C::SaveTtsModel(speech(102, 0))).unwrap();
    let r = settings(&mut s);
    assert_eq!(r.tts_models.len(), 2);
    assert!(r.tts_models[0].credential_saved);
    assert!(!r.tts_models[0].price_known);
    assert!(!format!("{r:?}").contains("synthetic-key"));
    let mut update = speech(101, 1);
    update.voice = "new-voice".into();
    update.price_known = true;
    update.character_minor_per_million = 42;
    let saved = exec(&mut s, 1, C::SaveTtsModel(update.clone())).unwrap();
    assert_eq!(saved.revision, 2);
    assert!(saved.tts_models[0].credential_saved);
    assert_eq!(saved.tts_models[0].voice, "new-voice");
    assert_eq!(
        exec(&mut s, 1, C::SaveTtsModel(update)),
        Err(Error::RevisionConflict)
    );
    exec(
        &mut s,
        1,
        C::Grant(FoundationGrant {
            workspace_id: id(10),
            principal_id: id(2),
            role: FoundationRole::Operator as i32,
            expected_revision: 0,
        }),
    )
    .unwrap();
    assert_eq!(
        exec(&mut s, 2, C::SaveTtsModel(speech(103, 0))),
        Err(Error::PermissionDenied)
    );
    assert_eq!(
        exec(&mut s, 2, C::DeleteTtsModel(removal(101, 2))),
        Err(Error::PermissionDenied)
    );
    assert_eq!(
        exec(&mut s, 1, C::DeleteTtsModel(removal(101, 1))),
        Err(Error::RevisionConflict)
    );
    let delete = command(C::DeleteTtsModel(removal(101, 2)));
    let r = s
        .execute(&Operation::new(4000).unwrap(), &id(1), &delete)
        .unwrap();
    assert_eq!(r.tts_models.len(), 1);
    assert_eq!(r.tts_models[0].id, id(102));
    assert_eq!(r.revision, 3);
    assert_eq!(
        s.execute(&Operation::new(4000).unwrap(), &id(1), &delete)
            .unwrap()
            .tts_models
            .len(),
        1
    );
    assert_eq!(
        exec(&mut s, 1, C::SaveTtsModel(speech(101, 3))),
        Err(Error::NotFound)
    );
    assert_eq!(
        exec(&mut s, 1, C::SaveTtsModel(speech(101, 0))),
        Err(Error::RevisionConflict)
    );
    let (_, old, _) = crate::llm::raw(
        &s.conn,
        &s.root,
        &id(10),
        &id(101),
        1,
        "publication_tts_config",
    )
    .unwrap();
    assert_eq!(old, 1);
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    assert_eq!(settings(&mut s).tts_models.len(), 1);
}
#[test]
fn model_removal_refuses_inflight_but_preserves_late_usage_after_cancel() {
    let (_t, mut s, _) = setup();
    s.attach_llm_vault(Arc::new(TestVault));
    exec(&mut s, 1, C::SaveLlmModel(text())).unwrap();
    let test = FoundationModelTest {
        workspace_id: id(10),
        task_id: id(200),
        model_id: id(100),
        model_revision: 1,
    };
    exec(&mut s, 1, C::TestLlm(test.clone())).unwrap();
    assert_eq!(
        exec(&mut s, 1, C::DeleteLlmModel(removal(100, 1))),
        Err(Error::Busy)
    );
    s.claim_llm(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(200))
        .unwrap();
    exec(
        &mut s,
        1,
        C::CancelLlm(FoundationLlmTaskRead {
            workspace_id: id(10),
            task_id: id(200),
        }),
    )
    .unwrap();
    let before = settings(&mut s).llm_budget.unwrap();
    exec(&mut s, 1, C::DeleteLlmModel(removal(100, 1))).unwrap();
    let after = settings(&mut s);
    assert!(after.llm_models.is_empty());
    assert_eq!(
        after.llm_budget.unwrap().unknown_minor,
        before.unknown_minor
    );
    assert_eq!(
        exec(
            &mut s,
            1,
            C::TestLlm(FoundationModelTest {
                task_id: id(201),
                model_revision: 2,
                ..test
            })
        ),
        Err(Error::CapabilityUnavailable)
    );
    s.finish_llm(
        &Operation::new(4000).unwrap(),
        &id(1),
        &id(10),
        &LlmWorkerResult {
            task_id: id(200),
            input_tokens: 100,
            output_tokens: 10,
            usage_known: true,
            request_started: true,
            ..Default::default()
        },
    )
    .unwrap();
    let result = exec(
        &mut s,
        1,
        C::ReadLlmTask(FoundationLlmTaskRead {
            workspace_id: id(10),
            task_id: id(200),
        }),
    )
    .unwrap();
    assert_eq!(result.task_state, AgentTaskState::Cancelled as i32);
    assert_eq!(result.charge_status, "unknown");
    assert_eq!(result.actual_minor, 0);
    assert!(result.body.is_empty());
    assert_eq!(
        exec(
            &mut s,
            1,
            C::SaveLlmModel(FoundationLlmModel {
                expected_revision: 2,
                ..text()
            })
        ),
        Err(Error::NotFound)
    );
}
#[test]
fn speech_catalog_is_bounded_and_endpoint_changes_require_new_key() {
    let (_t, mut s, _) = setup();
    s.attach_tts_vault(Arc::new(TestVault));
    for n in 101..109 {
        exec(&mut s, 1, C::SaveTtsModel(speech(n, 0))).unwrap();
    }
    assert_eq!(
        exec(&mut s, 1, C::SaveTtsModel(speech(109, 0))),
        Err(Error::ResourceExhausted)
    );
    let mut update = speech(101, 1);
    update.base_url = "https://other.invalid/v1".into();
    assert_eq!(
        exec(&mut s, 1, C::SaveTtsModel(update)),
        Err(Error::InvalidArgument)
    );
    exec(&mut s, 1, C::DeleteTtsModel(removal(101, 1))).unwrap();
    exec(&mut s, 1, C::SaveTtsModel(speech(109, 0))).unwrap();
    assert_eq!(settings(&mut s).tts_models.len(), 8);
}
