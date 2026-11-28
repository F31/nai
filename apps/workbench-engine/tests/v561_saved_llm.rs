#![cfg(windows)]
//! Explicit one-time selected-model import. No legacy DB access in live runtime.
use nai_runtime_core::llm::CredentialVault;
#[test]
#[ignore = "one real configured LLM request, no GPU or TTS"]
fn configured_llm_answer_only() {
    let p: serde_json::Value = serde_json::from_slice(
        &std::fs::read(std::env::var_os("NAI_IMPORT_PROFILE").expect("profile")).unwrap(),
    )
    .unwrap();
    let config: nai_workbench_engine::v561_reply_llm::Config =
        serde_json::from_value(p["llm"].clone()).unwrap();
    let dir =
        std::path::PathBuf::from(std::env::var_os("NAI_LLM_PROBE_DIR").expect("fresh artifacts"));
    assert!(!dir.exists());
    let text = nai_workbench_engine::v561_reply_llm::generate(
        &config,
        "介绍什么？",
        "欢迎。介绍产品。",
        1,
        &nai_runtime_core::foundation::Operation::model_task(45000).unwrap(),
        &dir,
    )
    .expect("actual bounded answer");
    assert!(!text.is_empty());
    println!("actual answer accepted; chars={}", text.chars().count());
}
#[test]
#[ignore = "explicit selected DB record and destination credential/profile only"]
fn import_selected_saved_llm() {
    let root = std::path::PathBuf::from(std::env::var_os("NAI_IMPORT_ROOT").expect("source root"));
    let id = std::env::var("NAI_IMPORT_MODEL").expect("selected model");
    let profile =
        std::path::PathBuf::from(std::env::var_os("NAI_IMPORT_PROFILE").expect("destination"));
    let reference = std::env::var("NAI_IMPORT_REFERENCE").expect("destination credential");
    let c = rusqlite::Connection::open_with_flags(
        root.join("foundation.sqlite"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("read-only DB");
    let body: String = c.query_row("SELECT v.body FROM shared_models m JOIN shared_model_versions v ON v.model_id=m.id AND v.revision=m.revision WHERE m.id=?1 AND m.kind='publication_llm_config'",[&id],|r|r.get(0)).expect("selected model");
    let body: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(body["enabled"], true);
    let endpoint = body["base_url"].as_str().expect("endpoint");
    assert_eq!(
        endpoint,
        std::env::var("NAI_IMPORT_ENDPOINT").expect("expected endpoint")
    );
    assert_eq!(
        body["model"],
        std::env::var("NAI_IMPORT_MODEL_NAME").expect("expected model")
    );
    let marker: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("foundation-root.json")).unwrap()).unwrap();
    let scope = format!(
        "{}:{}",
        marker["owner"].as_str().unwrap(),
        root.canonicalize().unwrap().display()
    );
    let vault = nai_llm_openai::vault::NativeVault::new(scope);
    let key = vault
        .open(
            "00000000-0000-4000-8000-000000004802",
            &id,
            endpoint,
            &body["secret"],
        )
        .expect("existing credential binding");
    let (service, account) = nai_llm_openai::credential::parse(&reference).unwrap();
    let entry = keyring::Entry::new(&service, &account).unwrap();
    match entry.get_secret() {
        Ok(existing) => {
            let existing = zeroize::Zeroizing::new(existing);
            assert!(
                existing.as_slice() == key.as_slice(),
                "destination differs; refusing overwrite"
            );
        }
        Err(keyring::Error::NoEntry) => {
            nai_llm_openai::credential::store(&reference, &key).expect("save reference")
        }
        Err(_) => panic!("destination credential unavailable; no overwrite"),
    }
    let original = std::fs::read(&profile).unwrap();
    let mut p: serde_json::Value = serde_json::from_slice(&original).unwrap();
    assert!(p.get("llm").is_none(), "destination already configured");
    let mut backup = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(profile.with_extension("before-llm.json"))
        .unwrap();
    use std::io::Write;
    backup.write_all(&original).unwrap();
    backup.sync_all().unwrap();
    p["llm"] = serde_json::json!({"base_url":endpoint,"model":body["model"],"api_key_ref":reference,"deadline_ms":15000,"max_answer_chars":32});
    std::fs::write(&profile, serde_json::to_vec_pretty(&p).unwrap()).unwrap();
    println!("selected model imported; source DB unchanged; no API call");
}
