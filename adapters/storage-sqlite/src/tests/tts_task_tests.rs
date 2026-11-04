use super::*;
use nai_runtime_core::llm::CredentialVault;
use serde_json::{Value, json};
struct Vault;
impl CredentialVault for Vault {
    fn seal(&self, _: &str, _: &str, _: &str, _: &[u8]) -> Result<Value> {
        Ok(json!({"reference":id(600),"nonce":"00".repeat(12),"ciphertext":"aa".repeat(32)}))
    }
    fn open(&self, _: &str, _: &str, _: &str, _: &Value) -> Result<zeroize::Zeroizing<Vec<u8>>> {
        Ok(zeroize::Zeroizing::new(b"synthetic-tts".to_vec()))
    }
}
fn prepared() -> (TempDir, Store) {
    let (t, mut s, _) = setup();
    s.attach_tts_vault(Arc::new(Vault));
    exec(
        &mut s,
        1,
        C::SaveTtsModel(FoundationTtsModel {
            protocol: "openai_speech".into(),
            language: String::new(),
            workspace_id: id(10),
            model_id: id(100),
            name: "synthetic".into(),
            base_url: "https://example.invalid/v1".into(),
            model: "test".into(),
            voice: "test:voice".into(),
            enabled: true,
            api_key: b"synthetic".to_vec(),
            price_known: true,
            character_minor_per_million: 100,
            ..Default::default()
        }),
    )
    .unwrap();
    (t, s)
}
fn start(n: u64) -> C {
    C::TestTts(FoundationTtsTest {
        workspace_id: id(10),
        task_id: id(n),
        model_id: id(100),
        model_revision: 1,
    })
}
fn task(n: u64) -> FoundationLlmTaskRead {
    FoundationLlmTaskRead {
        workspace_id: id(10),
        task_id: id(n),
    }
}
#[test]
fn local_qwen_model_without_vault_roundtrips_and_dispatches_without_a_fake_credential() {
    let (_t, mut s, _) = setup();
    let model = FoundationTtsModel {
        workspace_id: id(10),
        model_id: id(100),
        name: "本地Qwen".into(),
        base_url: "http://127.0.0.1:8000/v1".into(),
        model: "qwen3-tts-1.7b-customvoice".into(),
        voice: "serena".into(),
        protocol: "qwen_local".into(),
        language: "chinese".into(),
        enabled: true,
        ..Default::default()
    };
    let saved = exec(&mut s, 1, C::SaveTtsModel(model.clone())).unwrap();
    assert!(!saved.tts_models[0].credential_saved);
    assert_eq!(saved.tts_models[0].protocol, "qwen_local");
    assert_eq!(saved.tts_models[0].language, "chinese");
    exec(&mut s, 1, start(110)).unwrap();
    let p = s
        .tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(110))
        .unwrap();
    assert!(p.api_key.is_empty());
    assert_eq!(p.protocol, "qwen_local");
    assert_eq!(p.language, "chinese");
    let mut bad = model;
    bad.model_id = id(102);
    bad.base_url = "http://example.invalid:8000/v1".into();
    assert_eq!(
        exec(&mut s, 1, C::SaveTtsModel(bad)),
        Err(Error::InvalidArgument)
    );
}
#[test]
fn settings_test_bypasses_disabled_budget_creates_no_reservation_and_persists_safe_failure() {
    let (_t, mut s) = prepared();
    s.conn
        .execute("UPDATE workspaces SET budget_limit=0 WHERE id=?1", [id(10)])
        .unwrap();
    let r = exec(&mut s, 1, start(110)).unwrap();
    assert_eq!(r.reserved_minor, 0);
    assert!(r.llm_budget.is_none());
    s.tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(110))
        .unwrap();
    s.finish_tts(
        &Operation::new(4000).unwrap(),
        &id(1),
        &id(10),
        &id(110),
        true,
        "BALANCE_INSUFFICIENT",
        0,
        "",
    )
    .unwrap();
    let r = exec(&mut s, 1, C::ReadTtsTask(task(110))).unwrap();
    assert_eq!(r.task_reason, "BALANCE_INSUFFICIENT");
    assert_eq!(r.charge_status, "unknown");
    assert!(r.body.is_empty());
    assert_eq!(
        s.conn
            .query_row("SELECT COUNT(*) FROM budget_reservations", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert!(
        s.tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(110))
            .is_err()
    );
}
#[test]
fn unknown_worker_error_never_persists_raw_details() {
    let (_t, mut s) = prepared();
    exec(&mut s, 1, start(110)).unwrap();
    s.tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(110))
        .unwrap();
    s.finish_tts(
        &Operation::new(4000).unwrap(),
        &id(1),
        &id(10),
        &id(110),
        true,
        "raw-secret-key provider diagnostic",
        0,
        "",
    )
    .unwrap();
    let r = exec(&mut s, 1, C::ReadTtsTask(task(110))).unwrap();
    assert_eq!(r.task_reason, "PROVIDER_FAILED");
}
#[test]
fn timeout_stage_codes_survive_task_persistence_without_raw_errors() {
    for reason in [
        "CONNECT_TIMEOUT",
        "CONNECTION_FAILED",
        "RESPONSE_TIMEOUT",
        "AUDIO_DOWNLOAD_TIMEOUT",
    ] {
        let (_t, mut s) = prepared();
        exec(&mut s, 1, start(110)).unwrap();
        s.tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(110))
            .unwrap();
        s.finish_tts(
            &Operation::new(4000).unwrap(),
            &id(1),
            &id(10),
            &id(110),
            true,
            reason,
            0,
            "",
        )
        .unwrap();
        let r = exec(&mut s, 1, C::ReadTtsTask(task(110))).unwrap();
        assert_eq!(r.task_reason, reason);
        assert!(r.body.is_empty());
        assert_eq!(r.charge_status, "unknown");
    }
}
#[test]
fn queued_cancel_is_durable_and_late_completion_cannot_restore_result() {
    let (t, mut s) = prepared();
    let r = exec(&mut s, 1, start(110)).unwrap();
    assert_eq!(r.charge_status, "not_sent");
    assert!(r.body.is_empty());
    let c = exec(&mut s, 1, C::CancelTts(task(110))).unwrap();
    assert_eq!(c.charge_status, "not_sent");
    assert_eq!(c.task_state, AgentTaskState::Cancelled as i32);
    assert!(
        s.tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(110))
            .is_err()
    );
    s.finish_tts(
        &Operation::new(4000).unwrap(),
        &id(1),
        &id(10),
        &id(110),
        true,
        "",
        24000,
        &"ab".repeat(32),
    )
    .unwrap();
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    let r = exec(&mut s, 1, C::ReadTtsTask(task(110))).unwrap();
    assert_eq!(r.task_state, AgentTaskState::Cancelled as i32);
    assert!(r.body.is_empty());
    assert_eq!(r.charge_status, "not_sent");
}
#[test]
fn dispatch_completion_keeps_unknown_and_restart_never_dispatches_again() {
    let (t, mut s) = prepared();
    exec(&mut s, 1, start(110)).unwrap();
    let p = s
        .tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(110))
        .unwrap();
    assert_eq!(&*p.api_key, b"synthetic-tts");
    assert_eq!(exec(&mut s, 1, start(111)).unwrap_err(), Error::Busy);
    s.finish_tts(
        &Operation::new(4000).unwrap(),
        &id(1),
        &id(10),
        &id(110),
        true,
        "",
        24000,
        &"ab".repeat(32),
    )
    .unwrap();
    let r = exec(&mut s, 1, C::ReadTtsTask(task(110))).unwrap();
    assert_eq!(r.task_state, AgentTaskState::Completed as i32);
    assert_eq!(r.charge_status, "unknown");
    assert!(!r.body.contains("synthetic-tts"));
    assert!(r.body.contains("sample_count"));
    exec(&mut s, 1, start(111)).unwrap();
    s.tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(111))
        .unwrap();
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    let r = exec(&mut s, 1, C::ReadTtsTask(task(111))).unwrap();
    assert_eq!(r.task_state, AgentTaskState::Failed as i32);
    assert_eq!(r.charge_status, "unknown");
    assert!(r.body.is_empty());
    assert!(
        s.tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(111))
            .is_err()
    );
}
#[test]
fn test_admission_is_atomic_and_stale_model_rejects() {
    let (_t, mut s) = prepared();
    let mut x = match start(110) {
        C::TestTts(x) => x,
        _ => unreachable!(),
    };
    x.model_revision = 2;
    assert_eq!(
        exec(&mut s, 1, C::TestTts(x)).unwrap_err(),
        Error::RevisionConflict
    );
    s.conn.execute_batch("CREATE TRIGGER reject_tts_audit BEFORE INSERT ON audit_events BEGIN SELECT RAISE(ABORT,'synthetic audit failure'); END;").unwrap();
    assert!(exec(&mut s, 1, start(110)).is_err());
    let count: i64 = s
        .conn
        .query_row("SELECT COUNT(*) FROM budget_reservations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
    s.conn
        .execute_batch("DROP TRIGGER reject_tts_audit")
        .unwrap();
    exec(&mut s, 1, start(110)).unwrap();
}
#[test]
fn cancellation_intent_survives_audit_failure_and_blocks_dispatch_after_restart() {
    let (t, mut s) = prepared();
    exec(&mut s, 1, start(110)).unwrap();
    s.conn.execute_batch("CREATE TRIGGER reject_tts_cancel BEFORE INSERT ON audit_events BEGIN SELECT RAISE(ABORT,'synthetic audit failure'); END;").unwrap();
    assert!(exec(&mut s, 1, C::CancelTts(task(110))).is_err());
    assert!(
        s.tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(110))
            .is_err()
    );
    s.conn
        .execute_batch("DROP TRIGGER reject_tts_cancel")
        .unwrap();
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    assert!(
        s.tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(110))
            .is_err()
    );
    let r = exec(&mut s, 1, C::ReadTtsTask(task(110))).unwrap();
    assert_eq!(r.charge_status, "not_sent");
    assert!(r.body.is_empty());
}
#[test]
fn unknown_price_test_is_allowed_but_global_stop_still_blocks_dispatch() {
    let (_t, mut s) = prepared();
    exec(
        &mut s,
        1,
        C::SaveTtsModel(FoundationTtsModel {
            protocol: "openai_speech".into(),
            language: String::new(),
            workspace_id: id(10),
            model_id: id(100),
            expected_revision: 1,
            name: "synthetic".into(),
            base_url: "https://example.invalid/v1".into(),
            model: "test".into(),
            voice: "test:voice".into(),
            enabled: true,
            price_known: false,
            ..Default::default()
        }),
    )
    .unwrap();
    let mut x = match start(110) {
        C::TestTts(x) => x,
        _ => unreachable!(),
    };
    x.model_revision = 2;
    assert_eq!(exec(&mut s, 1, C::TestTts(x)).unwrap().reserved_minor, 0);
    let (_t, mut s) = prepared();
    exec(&mut s, 1, start(110)).unwrap();
    s.fence.stop();
    assert!(
        s.tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(110))
            .is_err()
    );
}
