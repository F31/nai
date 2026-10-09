use super::*;
use nai_contracts::v1::{Approval, AuthoritativeFact, Money, PerformanceCue, authoritative_fact};
use rusqlite::OptionalExtension;
use std::{fs, time::Duration};
mod approval_workflow;
mod decision_log;
mod knowledge_publication;

fn op() -> Operation {
    Operation::new(Duration::from_secs(30)).unwrap()
}
fn budget_limits() -> nai_budget_core::Limits {
    let ceiling = nai_budget_core::Ceiling {
        cost_minor: 10,
        units: [("requests".into(), 100)].into(),
        concurrent: [("provider_slots".into(), 2)].into(),
    };
    nai_budget_core::Limits {
        currency: "CNY".into(),
        global: ceiling.clone(),
        sessions: [("a".into(), ceiling.clone()), ("b".into(), ceiling.clone())].into(),
        providers: [("p".into(), ceiling)].into(),
        max_entries: 10,
    }
}
fn budget_entry(session: &str, request: &str, cost: u64) -> nai_contracts::v1::UsageLedgerEntry {
    nai_contracts::v1::UsageLedgerEntry {
        workspace_id: "test-workspace".into(),
        session_id: session.into(),
        provider_ref: "p".into(),
        request_id: request.into(),
        estimate: Some(nai_contracts::v1::BudgetCharge {
            currency: "CNY".into(),
            cost_minor: cost,
            units: [("requests".into(), 1)].into(),
            price_revision: "price-1".into(),
        }),
        concurrent: [("provider_slots".into(), 1)].into(),
        state: nai_contracts::v1::BillingState::Reserved as i32,
        ..Default::default()
    }
}

#[test]
fn durable_budget_preserves_cancelled_unknown_usage_across_restart_and_reconciles_once() {
    use nai_budget_core::Change;
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .set_permission(
            &op(),
            "test-workspace",
            "operator",
            "budget.manage",
            "*",
            true,
        )
        .unwrap();
    for session in ["a", "b"] {
        storage
            .create_session(&op(), "test-workspace", "operator", session)
            .unwrap();
    }
    let initial = budget_entry("a", "request-1", 7);
    assert!(
        storage
            .reserve_usage(&op(), "unknown", budget_limits(), initial.clone())
            .is_err()
    );
    let e = storage
        .reserve_usage(&op(), "operator", budget_limits(), initial.clone())
        .unwrap();
    let claim = storage
        .dispatch_usage(
            &op(),
            "test-workspace",
            "operator",
            budget_limits(),
            "p",
            "request-1",
            e.revision,
        )
        .unwrap();
    assert!(claim.acquired);
    let repeated = storage
        .dispatch_usage(
            &op(),
            "test-workspace",
            "operator",
            budget_limits(),
            "p",
            "request-1",
            e.revision,
        )
        .unwrap();
    assert!(!repeated.acquired);
    assert_eq!(repeated.entry, claim.entry);
    let e = claim.entry;
    let e = storage
        .update_usage(
            &op(),
            "test-workspace",
            "operator",
            budget_limits(),
            "p",
            "request-1",
            e.revision,
            Change::Cancel,
        )
        .unwrap();
    assert_eq!(
        storage
            .budget_book(&op(), "test-workspace", "operator", budget_limits())
            .unwrap()
            .total()
            .unwrap()
            .concurrent["provider_slots"],
        1
    );
    let e = storage
        .update_usage(
            &op(),
            "test-workspace",
            "operator",
            budget_limits(),
            "p",
            "request-1",
            e.revision,
            Change::Finish(None),
        )
        .unwrap();
    drop(storage);
    let mut storage = store(dir.path());
    assert_eq!(
        storage
            .reserve_usage(&op(), "operator", budget_limits(), initial)
            .unwrap(),
        e
    );
    assert!(
        storage
            .reserve_usage(
                &op(),
                "operator",
                budget_limits(),
                budget_entry("b", "request-2", 4)
            )
            .is_err()
    );
    let mut actual = e.estimate.clone().unwrap();
    actual.cost_minor = 9;
    let settled = storage
        .update_usage(
            &op(),
            "test-workspace",
            "operator",
            budget_limits(),
            "p",
            "request-1",
            e.revision,
            Change::Reconcile {
                id: "bill-1".into(),
                actual: actual.clone(),
            },
        )
        .unwrap();
    assert_eq!(
        storage
            .update_usage(
                &op(),
                "test-workspace",
                "operator",
                budget_limits(),
                "p",
                "request-1",
                0,
                Change::Reconcile {
                    id: "bill-1".into(),
                    actual
                }
            )
            .unwrap(),
        settled
    );
    let total = storage
        .budget_book(&op(), "test-workspace", "operator", budget_limits())
        .unwrap()
        .total()
        .unwrap();
    assert_eq!(total.cost_minor, 9);
    assert!(total.concurrent.is_empty());
}

#[test]
fn budget_storage_rejects_scope_conflict_and_legacy_unknown_records_instead_of_zeroing() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .set_permission(
            &op(),
            "test-workspace",
            "operator",
            "budget.manage",
            "*",
            true,
        )
        .unwrap();
    storage
        .create_session(&op(), "test-workspace", "operator", "a")
        .unwrap();
    storage
        .create_session(&op(), "test-workspace", "operator", "b")
        .unwrap();
    storage
        .reserve_usage(
            &op(),
            "operator",
            budget_limits(),
            budget_entry("a", "1", 7),
        )
        .unwrap();
    assert!(
        storage
            .reserve_usage(
                &op(),
                "operator",
                budget_limits(),
                budget_entry("b", "1", 7)
            )
            .is_err()
    );
    assert_eq!(
        storage
            .budget_book(&op(), "test-workspace", "operator", budget_limits())
            .unwrap()
            .total()
            .unwrap()
            .cost_minor,
        7
    );
    storage
        .conn
        .execute("UPDATE usage_ledger SET estimate_json='{}'", [])
        .unwrap();
    assert!(
        storage
            .budget_book(&op(), "test-workspace", "operator", budget_limits())
            .is_err()
    );
}
fn store(root: &std::path::Path) -> Store {
    Store::open(DataPaths::at(root).unwrap(), &op()).unwrap()
}
fn setup(store: &mut Store) {
    store
        .create_workspace(&op(), "test-workspace", "test only")
        .unwrap();
    for permission in [
        "knowledge.read",
        "knowledge.import",
        "knowledge.revoke",
        "knowledge.publish",
        "session.manage",
        "approval.write",
        "approval.read",
        "config.read",
        "config.write",
    ] {
        store
            .set_permission(&op(), "test-workspace", "operator", permission, "*", true)
            .unwrap();
    }
}
fn fact() -> AuthoritativeFact {
    AuthoritativeFact {
        workspace_id: "test-workspace".into(),
        product_id: "sku".into(),
        field_key: "price".into(),
        authority_ref: "test-authority".into(),
        value: Some(authoritative_fact::Value::Money(Money {
            minor_units: 129900,
            currency: "CNY".into(),
            scale: 2,
        })),
        publisher_ref: "model-claimed-publisher".into(),
        ..Default::default()
    }
}
fn candidate() -> PerformanceCue {
    PerformanceCue {
        utterance_id: "utterance-test".into(),
        session_id: "session-test".into(),
        speech_text: "测试价格".into(),
        display_text: "测试价格".into(),
        risk: 2,
        expires_at_utc_ms: utc_ms().unwrap() as u64 + 60000,
        ..Default::default()
    }
}
fn approved(store: &mut Store) -> (PerformanceCue, String) {
    store
        .create_session(&op(), "test-workspace", "operator", "session-test")
        .unwrap();
    let cue = candidate();
    let hash = store
        .record_candidate(
            &op(),
            "test-workspace",
            "operator",
            &cue,
            &[("sku", "price", "1")],
        )
        .unwrap();
    store
        .approve(
            &op(),
            "test-workspace",
            "operator",
            &Approval {
                approval_id: "approval-test".into(),
                session_id: cue.session_id.clone(),
                candidate_hash: hash.clone(),
                hash_algorithm: "sha256-canonical-v1".into(),
                actor_id: "model-claim".into(),
                action_scope: "speak".into(),
                expires_at_utc_ms: cue.expires_at_utc_ms,
                ..Default::default()
            },
        )
        .unwrap();
    (cue, hash)
}

#[test]
fn full_table_catalog_foreign_keys_and_durable_settings() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    for table in [
        "workspaces",
        "runtime_profiles",
        "plugin_installations",
        "capability_bindings",
        "sessions",
        "session_snapshots",
        "event_inbox",
        "utterances",
        "evidence_links",
        "approvals",
        "facts",
        "documents",
        "document_revisions",
        "chunks",
        "knowledge_snapshots",
        "vector_index_manifests",
        "knowledge_sources",
        "provider_profiles",
        "usage_ledger",
        "voice_assets",
        "avatar_assets",
        "tts_cache",
        "scenes",
        "show_plans",
        "memory_records",
        "devices",
        "roles",
        "audit_records",
        "upgrade_journal",
        "schema_migrations",
        "permission_grants",
        "fact_revisions",
        "invalidation_outbox",
        "managed_artifacts",
    ] {
        assert_eq!(
            storage
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            1,
            "{table}"
        );
    }
    assert_eq!(
        storage
            .conn
            .pragma_query_value(None, "foreign_keys", |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        storage
            .conn
            .pragma_query_value(None, "synchronous", |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        storage
            .conn
            .pragma_query_value(None, "journal_mode", |r| r.get::<_, String>(0))
            .unwrap(),
        "wal"
    );
    assert!(
        storage
            .conn
            .execute(
                "INSERT INTO sessions(session_id,workspace_id) VALUES('bad','missing')",
                []
            )
            .is_err()
    );
    assert_eq!(
        storage
            .conn
            .query_row("SELECT COUNT(*) FROM memory_records", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn revocation_checkpoint_cas_revokes_approval_and_cancels_pending() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .publish_fact(&op(), "operator", &fact(), None)
        .unwrap();
    let (cue, hash) = approved(&mut storage);
    let before = storage
        .session(&op(), "test-workspace", "operator", &cue.session_id)
        .unwrap();
    let mut after = before.clone();
    after.generation_epoch += 1;
    after.state_revision += 1;
    after.media_state = nai_contracts::v1::MediaState::Muted as i32;
    assert!(
        storage
            .checkpoint_revocation(&op(), "unknown", &before, &after)
            .is_err()
    );
    storage
        .checkpoint_revocation(&op(), "operator", &before, &after)
        .unwrap();
    assert!(
        storage
            .checkpoint_revocation(&op(), "operator", &before, &after)
            .is_err()
    );
    assert!(
        !storage
            .approval_valid(&op(), "test-workspace", &cue.session_id, 0, &hash)
            .unwrap()
    );
    let status: i32 = storage
        .conn
        .query_row(
            "SELECT status FROM utterances WHERE utterance_id=?1",
            [&cue.utterance_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, 12);
    assert_eq!(
        storage
            .session(&op(), "test-workspace", "operator", &cue.session_id)
            .unwrap()
            .generation_epoch,
        1
    );
}

#[test]
fn single_owner_cancel_deadline_and_sql_progress_interrupt() {
    let dir = tempfile::tempdir().unwrap();
    let storage = store(dir.path());
    assert_eq!(
        Store::open(DataPaths::at(dir.path()).unwrap(), &op())
            .err()
            .unwrap()
            .code,
        ErrorCode::Conflict
    );
    let stopped = op();
    stopped.cancel();
    assert_eq!(
        storage.start(&stopped).unwrap_err().code,
        ErrorCode::Cancelled
    );
    assert_eq!(
        storage
            .start(&Operation::new(Duration::ZERO).unwrap())
            .unwrap_err()
            .code,
        ErrorCode::DeadlineExceeded
    );
    storage
        .start(&Operation::new(Duration::from_millis(5)).unwrap())
        .unwrap();
    assert!(storage.conn.query_row("WITH RECURSIVE work(x) AS(SELECT 0 UNION ALL SELECT x+1 FROM work WHERE x<100000000) SELECT SUM(x) FROM work",[],|r|r.get::<_,i64>(0)).is_err());
    drop(storage);
    let _reopened = store(dir.path());
}

#[test]
fn schema_layout_drift_is_not_hidden_by_valid_migration_records() {
    let dir = tempfile::tempdir().unwrap();
    let storage = store(dir.path());
    storage
        .conn
        .execute_batch("DROP TABLE voice_assets")
        .unwrap();
    drop(storage);
    assert_eq!(
        Store::open(DataPaths::at(dir.path()).unwrap(), &op())
            .err()
            .unwrap()
            .code,
        ErrorCode::VersionIncompatible
    );
}

#[test]
fn exact_facts_cas_permissions_and_immutable_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    let first = storage
        .publish_fact(&op(), "operator", &fact(), None)
        .unwrap();
    assert_eq!(first.revision, "1");
    assert_eq!(first.publisher_ref, "operator");
    assert_eq!(
        storage
            .fact(&op(), "test-workspace", "operator", "sku", "price")
            .unwrap()
            .value,
        fact().value
    );
    assert_eq!(
        storage
            .publish_fact(&op(), "operator", &fact(), None)
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    assert_eq!(
        storage
            .fact(&op(), "test-workspace", "outsider", "sku", "price")
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );
    storage
        .create_workspace(&op(), "other-workspace", "other")
        .unwrap();
    storage
        .set_permission(
            &op(),
            "other-workspace",
            "operator",
            "knowledge.read",
            "*",
            true,
        )
        .unwrap();
    assert_eq!(
        storage
            .fact(&op(), "other-workspace", "operator", "sku", "price")
            .unwrap_err()
            .code,
        ErrorCode::EvidenceExpired
    );
    let mut invalid = fact();
    invalid.value = Some(authoritative_fact::Value::Decimal("NaN".into()));
    assert!(
        storage
            .publish_fact(&op(), "operator", &invalid, Some("1"))
            .is_err()
    );
    assert!(
        storage
            .conn
            .execute("UPDATE fact_revisions SET fact_proto=X'00'", [])
            .is_err()
    );
}

#[test]
fn fact_update_invalidates_approval_atomically_and_permission_revocation_denies_reads() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .publish_fact(&op(), "operator", &fact(), None)
        .unwrap();
    let (cue, hash) = approved(&mut storage);
    assert!(
        storage
            .approval_valid(&op(), "test-workspace", &cue.session_id, 0, &hash)
            .unwrap()
    );
    let mut changed = cue.clone();
    changed.speech_text.push('变');
    assert!(
        !storage
            .approval_valid(
                &op(),
                "test-workspace",
                &cue.session_id,
                0,
                &store::candidate_hash(&changed)
            )
            .unwrap()
    );
    assert_eq!(
        storage
            .conn
            .query_row("SELECT actor_id FROM approvals", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "operator"
    );
    storage
        .publish_fact(&op(), "operator", &fact(), Some("1"))
        .unwrap();
    assert!(
        !storage
            .approval_valid(&op(), "test-workspace", &cue.session_id, 0, &hash)
            .unwrap()
    );
    assert_eq!(
        storage
            .conn
            .query_row(
                "SELECT COUNT(*) FROM invalidation_outbox WHERE reason='fact_changed'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    storage
        .set_permission(
            &op(),
            "test-workspace",
            "operator",
            "knowledge.read",
            "*",
            false,
        )
        .unwrap();
    assert_eq!(
        storage
            .fact(&op(), "test-workspace", "operator", "sku", "price")
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        storage
            .conn
            .query_row("SELECT generation_epoch FROM sessions", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn expired_facts_block_existing_and_new_approvals() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .publish_fact(&op(), "operator", &fact(), None)
        .unwrap();
    let (cue, hash) = approved(&mut storage);
    // Simulate clock passage at the persisted boundary, without flaky sleeps.
    storage
        .conn
        .execute(
            "UPDATE facts SET valid_until_ms=?1",
            [utc_ms().unwrap() - 1],
        )
        .unwrap();
    assert!(
        !storage
            .approval_valid(&op(), "test-workspace", &cue.session_id, 0, &hash)
            .unwrap()
    );
    assert!(
        storage
            .fact(&op(), "test-workspace", "operator", "sku", "price")
            .is_err()
    );
    assert!(
        storage
            .approve(
                &op(),
                "test-workspace",
                "operator",
                &Approval {
                    approval_id: "late".into(),
                    session_id: cue.session_id,
                    candidate_hash: hash,
                    hash_algorithm: "sha256-canonical-v1".into(),
                    expires_at_utc_ms: cue.expires_at_utc_ms,
                    ..Default::default()
                }
            )
            .is_err()
    );
}

#[test]
fn configuration_uses_same_schema_and_never_accepts_plaintext_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    let runtime: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/contracts/runtime.json"
    ))
    .unwrap();
    assert_eq!(
        storage
            .save_config(
                &op(),
                ("test-workspace", "operator"),
                ConfigKind::Runtime,
                "main",
                &runtime,
                0
            )
            .unwrap(),
        1
    );
    assert_eq!(
        storage
            .load_config(
                &op(),
                "test-workspace",
                "operator",
                ConfigKind::Runtime,
                "main"
            )
            .unwrap(),
        runtime
    );
    assert!(
        storage
            .save_config(
                &op(),
                ("test-workspace", "operator"),
                ConfigKind::Runtime,
                "main",
                &runtime,
                0
            )
            .is_err()
    );
    let mut bad = runtime.clone();
    bad["api_key"] = "test-only-secret".into();
    assert!(
        storage
            .save_config(
                &op(),
                ("test-workspace", "operator"),
                ConfigKind::Runtime,
                "main",
                &bad,
                1
            )
            .is_err()
    );
    let provider = serde_json::json!({"schema_version":1,"id":"test","protocol":"http_stream","endpoint":"https://example.invalid/tts","model":"test","credential_ref":"credential:test","region":"test","data_policy_ref":"test","timeout_ms":1000,"max_retries":1,"max_concurrency":1,"price_revision":"test"});
    storage
        .save_config(
            &op(),
            ("test-workspace", "operator"),
            ConfigKind::Provider,
            "test",
            &provider,
            0,
        )
        .unwrap();
    let mut bad = provider;
    bad["endpoint"] = "https://user:secret@example.invalid".into();
    assert!(
        storage
            .save_config(
                &op(),
                ("test-workspace", "operator"),
                ConfigKind::Provider,
                "test",
                &bad,
                1
            )
            .is_err()
    );
}

#[test]
fn online_backup_restores_wal_content_files_and_revokes_old_authority() {
    let parent = tempfile::tempdir().unwrap();
    let mut storage = store(&parent.path().join("source"));
    setup(&mut storage);
    storage
        .publish_fact(&op(), "operator", &fact(), None)
        .unwrap();
    storage
        .conn
        .pragma_update(None, "wal_autocheckpoint", 0)
        .unwrap();
    let (cue, hash) = approved(&mut storage);
    let original = storage
        .publish_artifact(
            &op(),
            "test-workspace",
            "operator",
            "doc",
            "knowledge",
            b"test-only-original-document",
        )
        .unwrap();
    storage.conn.execute("INSERT INTO devices VALUES('test-workspace','test-device','public-ref','operator',NULL)",[]).unwrap();
    let runtime: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/contracts/runtime.json"
    ))
    .unwrap();
    storage
        .save_config(
            &op(),
            ("test-workspace", "operator"),
            ConfigKind::Runtime,
            "main",
            &runtime,
            0,
        )
        .unwrap();
    assert!(
        fs::metadata(storage.paths.root.join("db/state.sqlite3-wal"))
            .unwrap()
            .len()
            > 0
    );
    let backup = storage.backup(&op()).unwrap();
    assert!(Store::restore(&backup, DataPaths::at(storage.paths.root()).unwrap(), &op()).is_err());
    let mut restored = Store::restore(
        &backup,
        DataPaths::at(parent.path().join("restored")).unwrap(),
        &op(),
    )
    .unwrap();
    assert!(
        !restored
            .approval_valid(&op(), "test-workspace", &cue.session_id, 0, &hash)
            .unwrap()
    );
    assert!(
        restored
            .fact(&op(), "test-workspace", "operator", "sku", "price")
            .is_err()
    );
    assert!(
        restored
            .conn
            .query_row("SELECT revoked_at_ms IS NOT NULL FROM devices", [], |r| {
                r.get::<_, bool>(0)
            })
            .unwrap()
    );
    for permission in ["knowledge.read", "session.manage", "config.read"] {
        restored
            .set_permission(&op(), "test-workspace", "operator", permission, "*", true)
            .unwrap();
    }
    assert_eq!(
        restored
            .fact(&op(), "test-workspace", "operator", "sku", "price")
            .unwrap()
            .revision,
        "1"
    );
    assert_eq!(
        restored
            .load_config(
                &op(),
                "test-workspace",
                "operator",
                ConfigKind::Runtime,
                "main"
            )
            .unwrap(),
        runtime
    );
    assert_eq!(
        fs::read(restored.paths.root.join(original)).unwrap(),
        b"test-only-original-document"
    );
    let session = restored
        .session(&op(), "test-workspace", "operator", &cue.session_id)
        .unwrap();
    assert!(session.generation_epoch > 0);
    assert_eq!(session.mode, 2);
    assert_eq!(session.media_state, 0);
    restored
        .create_session(&op(), "test-workspace", "operator", "new-rehearsal")
        .unwrap();
    assert_eq!(
        restored
            .session(&op(), "test-workspace", "operator", "new-rehearsal")
            .unwrap()
            .mode,
        1
    );
    assert!(
        restored
            .conn
            .query_row("SELECT COUNT(*) FROM audit_records", [], |r| r
                .get::<_, i64>(0))
            .unwrap()
            > 0
    );
    // The source remains usable and its later commits are not overwritten by restore.
    storage
        .publish_fact(&op(), "operator", &fact(), Some("1"))
        .unwrap();
    assert_eq!(
        storage
            .fact(&op(), "test-workspace", "operator", "sku", "price")
            .unwrap()
            .revision,
        "2"
    );
}

#[test]
fn corrupted_database_artifact_and_traversal_never_activate_destination() {
    let parent = tempfile::tempdir().unwrap();
    let mut storage = store(&parent.path().join("source"));
    setup(&mut storage);
    storage
        .publish_artifact(
            &op(),
            "test-workspace",
            "operator",
            "doc",
            "assets",
            b"test-asset",
        )
        .unwrap();
    let backup = storage.backup(&op()).unwrap();
    let destination = parent.path().join("restore");
    let manifest_path = backup.join("manifest.json");
    let original = fs::read(&manifest_path).unwrap();
    let mut manifest: serde_json::Value = serde_json::from_slice(&original).unwrap();
    manifest["artifacts"][0]["path"] = "../../escape".into();
    fs::write(&manifest_path, manifest.to_string()).unwrap();
    assert!(Store::restore(&backup, DataPaths::at(&destination).unwrap(), &op()).is_err());
    assert!(!destination.exists());
    fs::write(&manifest_path, &original).unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&original).unwrap();
    fs::write(
        backup.join(manifest["artifacts"][0]["path"].as_str().unwrap()),
        b"changed",
    )
    .unwrap();
    assert!(Store::restore(&backup, DataPaths::at(&destination).unwrap(), &op()).is_err());
    assert!(!destination.exists());
    fs::write(backup.join("state.sqlite3"), b"not-a-database").unwrap();
    assert!(Store::restore(&backup, DataPaths::at(&destination).unwrap(), &op()).is_err());
    assert!(!destination.exists());
}

#[test]
#[cfg(unix)]
fn symlink_paths_are_rejected() {
    let parent = tempfile::tempdir().unwrap();
    let outside = parent.path().join("outside");
    fs::create_dir(&outside).unwrap();
    let link = parent.path().join("link");
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    assert!(DataPaths::at(&link).is_err());
    let mut storage = store(&parent.path().join("data"));
    setup(&mut storage);
    let relative = storage
        .publish_artifact(
            &op(),
            "test-workspace",
            "operator",
            "doc",
            "knowledge",
            b"test",
        )
        .unwrap();
    let file = storage.paths.root.join(relative);
    fs::remove_file(&file).unwrap();
    std::os::unix::fs::symlink(outside.join("missing"), file).unwrap();
    assert!(storage.backup(&op()).is_err());
}

fn knowledge_doc(text: &str) -> crate::ImportedDocument {
    let chunks =
        nai_knowledge_core::split_document("doc-kb", text, "sku-kb", "acl-read", 256, 64).unwrap();
    crate::ImportedDocument {
        workspace_id: "test-workspace".into(),
        document_id: "doc-kb".into(),
        source: "fixture".into(),
        parser_version: "txt-1".into(),
        product_scope: "test-workspace".into(),
        acl_ref: "acl-read".into(),
        original_artifact_ref: None,
        valid_from_ms: None,
        valid_until_ms: None,
        content: text.into(),
        chunks: chunks
            .into_iter()
            .map(|chunk| crate::ImportChunk {
                chunk_id: chunk.chunk_id,
                content: chunk.content.clone(),
                source_span: chunk.source_span,
                product_scope: "test-workspace".into(),
                acl_ref: "acl-read".into(),
                tokens: nai_knowledge_core::tokenize(&chunk.content),
            })
            .collect(),
    }
}

fn publish_prepared(storage: &mut Store, index: Option<&str>) -> SnapshotPublish {
    publish_prepared_in_scope(storage, index, "test-workspace")
}

fn publish_prepared_in_scope(
    storage: &mut Store,
    index: Option<&str>,
    kb: &str,
) -> SnapshotPublish {
    let number: i64 = storage
        .conn
        .query_row("SELECT COUNT(*) FROM knowledge_snapshots", [], |row| {
            row.get(0)
        })
        .unwrap();
    let snapshot = format!("snapshot-{}", number + 1);
    let expected:Option<String>=storage.conn.query_row("SELECT snapshot_id FROM knowledge_active_snapshots WHERE workspace_id='test-workspace' AND kb_scope=?1",[kb],|row|row.get(0)).optional().unwrap();
    let documents:Vec<String>=storage.conn.prepare("SELECT document_id FROM documents WHERE workspace_id='test-workspace' AND status!='revoked' ORDER BY document_id").unwrap().query_map([],|row|row.get(0)).unwrap().collect::<std::result::Result<_,_>>().unwrap();
    storage
        .stage_snapshot(
            &op(),
            "operator",
            "test-workspace",
            &snapshot,
            kb,
            &documents,
            index,
            expected.as_deref(),
        )
        .unwrap();
    let sealed = storage
        .index_snapshot(&op(), "operator", "test-workspace", &snapshot)
        .unwrap();
    storage
        .publish_snapshot(&op(), "operator", &sealed)
        .unwrap();
    sealed
}

#[test]
fn empty_and_oversized_imports_are_rejected_and_facts_follow_permission() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    let mut doc = knowledge_doc("第一行内容\n第二行内容比较长一些\n第三行结尾");
    doc.content.clear();
    assert!(storage.import_document(&op(), "operator", &doc).is_err());
    doc.content = "x".repeat(16 * 1024 * 1024 + 1);
    assert!(storage.import_document(&op(), "operator", &doc).is_err());
    assert_eq!(
        storage
            .conn
            .query_row("SELECT COUNT(*) FROM documents", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert!(
        storage
            .import_document(&op(), "unknown", &knowledge_doc("有效的知识内容"))
            .is_err()
    );
    storage
        .import_document(&op(), "operator", &knowledge_doc("有效的知识内容"))
        .unwrap();
}

#[test]
fn chinese_fts_recall_and_empty_vs_partial_status() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .import_document(
            &op(),
            "operator",
            &knowledge_doc("产品保修期为一千二百九十九天，售后热线二十四小时。"),
        )
        .unwrap();
    publish_prepared(&mut storage, None);
    let tokens = nai_knowledge_core::tokenize("保修期");
    let (status, evidence) = storage
        .retrieve_local(
            &op(),
            "operator",
            "test-workspace",
            "test-workspace",
            &[],
            &tokens,
            None,
            8,
        )
        .unwrap();
    assert!(matches!(
        status,
        nai_contracts::v1::RetrievalStatus::RetrievalComplete
            | nai_contracts::v1::RetrievalStatus::RetrievalPartial
    ));
    assert!(evidence.iter().any(|e| e.text.contains("保修期")));
    assert!(!evidence[0].source_span.is_empty());
    assert_eq!(evidence[0].revision.len(), 64);
    let (status, evidence) = storage
        .retrieve_local(
            &op(),
            "operator",
            "test-workspace",
            "test-workspace",
            &[],
            &["无关词".into()],
            None,
            8,
        )
        .unwrap();
    assert_eq!(status, nai_contracts::v1::RetrievalStatus::RetrievalEmpty);
    assert!(evidence.is_empty());
}

#[test]
fn vector_identity_isolation_and_snapshot_cas() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .import_document(
            &op(),
            "operator",
            &knowledge_doc("支持向量检索的知识内容。"),
        )
        .unwrap();
    let manifest_a = crate::VectorManifest {
        index_id: "idx-a".into(),
        embedding_provider: "test".into(),
        model_version: "m-a".into(),
        dimension: 3,
        normalization: "l2".into(),
        chunker_version: "c-1".into(),
        content_hash: "a".repeat(64),
    };
    let manifest_b = crate::VectorManifest {
        index_id: "idx-b".into(),
        model_version: "m-b".into(),
        ..manifest_a.clone()
    };
    let vectors = [("doc-kb#chunk-0".into(), vec![1.0f32, 0.0, 0.0])].into();
    storage
        .index_vectors(&op(), "operator", "test-workspace", &manifest_a, &vectors)
        .unwrap();
    storage
        .index_vectors(&op(), "operator", "test-workspace", &manifest_b, &vectors)
        .unwrap();
    let publish = publish_prepared(&mut storage, Some("idx-a"));
    let mut changed_identity = manifest_a.clone();
    changed_identity.model_version = "changed-model".into();
    assert_eq!(
        storage
            .index_vectors(
                &op(),
                "operator",
                "test-workspace",
                &changed_identity,
                &vectors
            )
            .unwrap_err()
            .code,
        nai_contracts::v1::ErrorCode::Conflict
    );
    let (status, evidence) = storage
        .retrieve_local(
            &op(),
            "operator",
            "test-workspace",
            "test-workspace",
            &[],
            &["保修".into()],
            Some(("idx-a", vec![1.0, 0.0, 0.0])),
            4,
        )
        .unwrap();
    assert!(matches!(
        status,
        nai_contracts::v1::RetrievalStatus::RetrievalComplete
            | nai_contracts::v1::RetrievalStatus::RetrievalPartial
    ));
    assert!(
        evidence
            .iter()
            .any(|e| e.evidence_id.starts_with("chunk:") && !e.text.is_empty())
    );
    assert!(
        storage
            .index_vectors(
                &op(),
                "operator",
                "test-workspace",
                &manifest_a,
                &[("doc-kb#chunk-0".into(), vec![1.0, 0.0])].into()
            )
            .is_err()
    );

    storage
        .publish_snapshot(&op(), "operator", &publish)
        .unwrap();
    storage
        .publish_snapshot(&op(), "operator", &publish)
        .unwrap();
    let mut changed = publish.clone();
    changed.manifest_hash = "c".repeat(64);
    assert_eq!(
        storage
            .publish_snapshot(&op(), "operator", &changed)
            .unwrap_err()
            .code,
        nai_contracts::v1::ErrorCode::Conflict
    );
}

#[test]
fn revocation_filters_pending_evidence_and_removes_index_rows() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .import_document(&op(), "operator", &knowledge_doc("撤销前的有效知识内容。"))
        .unwrap();
    publish_prepared(&mut storage, None);
    storage
        .conn
        .execute(
            "INSERT INTO sessions(session_id,workspace_id) VALUES('kb-session','test-workspace')",
            [],
        )
        .unwrap();
    storage.conn.execute("INSERT INTO utterances(utterance_id,workspace_id,session_id,generation_epoch,text_hash,expires_at_ms) VALUES('kb-utterance','test-workspace','kb-session',0,'kb-hash',9999999999999)",[]).unwrap();
    storage.conn.execute("INSERT INTO evidence_links(workspace_id,utterance_id,evidence_id) VALUES('test-workspace','kb-utterance','chunk:doc-kb#chunk-0')",[]).unwrap();
    let tokens = nai_knowledge_core::tokenize("撤销");
    let (before, _) = storage
        .retrieve_local(
            &op(),
            "operator",
            "test-workspace",
            "test-workspace",
            &[],
            &tokens,
            None,
            8,
        )
        .unwrap();
    assert!(matches!(
        before,
        nai_contracts::v1::RetrievalStatus::RetrievalComplete
            | nai_contracts::v1::RetrievalStatus::RetrievalPartial
    ));
    let count = storage
        .revoke_knowledge(&op(), "operator", "test-workspace", None, Some("doc-kb"))
        .unwrap();
    assert!(count >= 1);
    let (after, evidence) = storage
        .retrieve_local(
            &op(),
            "operator",
            "test-workspace",
            "test-workspace",
            &[],
            &tokens,
            None,
            8,
        )
        .unwrap();
    assert_eq!(after, nai_contracts::v1::RetrievalStatus::RetrievalEmpty);
    assert!(evidence.is_empty());
    assert_eq!(
        storage
            .conn
            .query_row(
                "SELECT status FROM utterances WHERE utterance_id='kb-utterance'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        12
    );
    assert_eq!(
        storage
            .conn
            .query_row(
                "SELECT status FROM documents WHERE document_id='doc-kb'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        "revoked"
    );
}

#[test]
fn knowledge_filters_acl_products_time_and_vector_candidates() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    let mut doc = knowledge_doc("检索需要权限和有效时间。");
    doc.product_scope = "sku-kb".into();
    for chunk in &mut doc.chunks {
        chunk.product_scope = "sku-kb".into();
    }
    storage.import_document(&op(), "operator", &doc).unwrap();
    publish_prepared_in_scope(&mut storage, None, "kb");
    storage
        .set_permission(
            &op(),
            "test-workspace",
            "limited",
            "knowledge.read",
            "kb",
            true,
        )
        .unwrap();
    let tokens = nai_knowledge_core::tokenize("检索");
    assert!(
        storage
            .retrieve_local(
                &op(),
                "limited",
                "test-workspace",
                "kb",
                &[],
                &tokens,
                None,
                1
            )
            .unwrap()
            .1
            .is_empty()
    );
    storage
        .set_permission(
            &op(),
            "test-workspace",
            "limited",
            "knowledge.read",
            "acl-read",
            true,
        )
        .unwrap();
    assert_eq!(
        storage
            .retrieve_local(
                &op(),
                "limited",
                "test-workspace",
                "kb",
                &["sku-kb".into()],
                &tokens,
                None,
                1
            )
            .unwrap()
            .1
            .len(),
        1
    );
    assert!(
        storage
            .retrieve_local(
                &op(),
                "limited",
                "test-workspace",
                "kb",
                &["other".into()],
                &tokens,
                None,
                1
            )
            .unwrap()
            .1
            .is_empty()
    );
    let manifest = VectorManifest {
        index_id: "filtered-index".into(),
        embedding_provider: "fixture".into(),
        model_version: "v1".into(),
        dimension: 2,
        normalization: "l2".into(),
        chunker_version: "v1".into(),
        content_hash: "f".repeat(64),
    };
    storage
        .index_vectors(
            &op(),
            "operator",
            "test-workspace",
            &manifest,
            &[("doc-kb#chunk-0".into(), vec![1.0, 0.0])].into(),
        )
        .unwrap();
    publish_prepared_in_scope(&mut storage, Some("filtered-index"), "kb");
    assert!(
        storage
            .retrieve_local(
                &op(),
                "limited",
                "test-workspace",
                "kb",
                &["other".into()],
                &[],
                Some(("filtered-index", vec![1.0, 0.0])),
                1
            )
            .unwrap()
            .1
            .is_empty()
    );
    doc.valid_from_ms = Some(utc_ms().unwrap() + 60000);
    storage.import_document(&op(), "operator", &doc).unwrap();
    publish_prepared_in_scope(&mut storage, None, "kb");
    assert!(
        storage
            .retrieve_local(
                &op(),
                "limited",
                "test-workspace",
                "kb",
                &[],
                &tokens,
                None,
                1
            )
            .unwrap()
            .1
            .is_empty()
    );
    assert_eq!(
        storage
            .conn
            .query_row("SELECT COUNT(*) FROM knowledge_vectors", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let stopped = op();
    stopped.cancel();
    assert_eq!(
        storage
            .retrieve_local(
                &stopped,
                "operator",
                "test-workspace",
                "kb",
                &[],
                &tokens,
                None,
                1
            )
            .unwrap_err()
            .code,
        nai_contracts::v1::ErrorCode::Cancelled
    );
}

#[test]
fn knowledge_failed_vector_batch_rolls_back_manifest_and_rows() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    let manifest = VectorManifest {
        index_id: "rollback-index".into(),
        embedding_provider: "fixture".into(),
        model_version: "v1".into(),
        dimension: 2,
        normalization: "l2".into(),
        chunker_version: "v1".into(),
        content_hash: "f".repeat(64),
    };
    assert!(
        storage
            .index_vectors(
                &op(),
                "operator",
                "test-workspace",
                &manifest,
                &[("missing-chunk".into(), vec![1.0, 0.0])].into()
            )
            .is_err()
    );
    assert_eq!(
        storage
            .conn
            .query_row("SELECT COUNT(*) FROM vector_index_manifests", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
