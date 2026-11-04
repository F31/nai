use super::*;
use serde_json::{Value, json};
struct Vault;
impl nai_runtime_core::llm::CredentialVault for Vault {
    fn seal(&self, _: &str, _: &str, _: &str, _: &[u8]) -> Result<Value> {
        Ok(json!({"reference":id(600),"nonce":"00".repeat(12),"ciphertext":"aa".repeat(32)}))
    }
    fn open(&self, _: &str, _: &str, _: &str, _: &Value) -> Result<zeroize::Zeroizing<Vec<u8>>> {
        panic!("preview must not decrypt")
    }
}
fn preview() -> C {
    C::PreviewSharedModels(FoundationLlmRead {
        workspace_id: String::new(),
    })
}
fn model(ws: u64) -> C {
    C::SaveLlmModel(FoundationLlmModel {
        workspace_id: id(ws),
        model_id: id(100),
        expected_revision: 0,
        name: "相同名称".into(),
        base_url: "https://example.invalid/v1".into(),
        model: "test".into(),
        enabled: true,
        api_key: b"synthetic-key".to_vec(),
        prices_confirmed: true,
        input_minor_per_million: 1,
        output_minor_per_million: 1,
    })
}
#[test]
fn preview_includes_recycled_sources_and_flags_collisions_without_secrets_or_writes() {
    let (t, mut s, _) = setup();
    s.attach_llm_vault(Arc::new(Vault));
    exec(&mut s, 1, model(10)).unwrap();
    exec(
        &mut s,
        1,
        C::CreateWorkspace(FoundationWorkspace {
            workspace_id: id(85),
            name: "回收区".into(),
            budget_limit_minor: 0,
            currency: "CNY".into(),
        }),
    )
    .unwrap();
    exec(&mut s, 1, model(85)).unwrap();
    exec(
        &mut s,
        1,
        C::WorkspaceLifecycle(FoundationWorkspaceLifecycle {
            workspace_id: id(85),
            expected_revision: 1,
            action: WorkspaceAction::Recycle as i32,
        }),
    )
    .unwrap();
    let before: i64 = s
        .conn
        .query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap();
    let p = exec(&mut s, 1, preview())
        .unwrap()
        .shared_model_preview
        .unwrap();
    let after: i64 = s
        .conn
        .query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(p.rows.len(), 2);
    assert!(!p.executable);
    assert_eq!(p.plan_sha256.len(), 64);
    assert!(p.rows.iter().any(|r| r.recycled));
    for r in &p.rows {
        assert!(r.credential_saved);
        assert!(r.issues.contains(&"DUPLICATE_NAME".into()));
        assert!(r.issues.contains(&"DUPLICATE_MODEL_ID".into()));
    }
    let wire = p.encode_to_vec();
    assert!(!wire.windows(10).any(|w| w == b"ciphertext"));
    assert!(!wire.windows(13).any(|w| w == b"synthetic-key"));
    let again = exec(&mut s, 1, preview())
        .unwrap()
        .shared_model_preview
        .unwrap();
    assert_eq!(p, again);
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    assert_eq!(
        p,
        exec(&mut s, 1, preview())
            .unwrap()
            .shared_model_preview
            .unwrap()
    );
}
#[test]
fn preview_is_root_admin_only_and_ignores_page_workspace_scope() {
    let (_, mut s, _) = setup();
    assert_eq!(
        exec(&mut s, 2, preview()).unwrap_err(),
        Error::PermissionDenied
    );
    assert_eq!(
        exec(
            &mut s,
            1,
            C::PreviewSharedModels(FoundationLlmRead {
                workspace_id: id(10)
            })
        )
        .unwrap_err(),
        Error::InvalidArgument
    );
}
