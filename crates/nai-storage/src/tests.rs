use super::*;
use nai_contracts::v1::{Approval, AuthoritativeFact, Money, PerformanceCue, authoritative_fact};
use rusqlite::OptionalExtension;
use std::{fs, time::Duration};
mod approval_workflow;
mod decision_log;
mod knowledge_publication;
mod storage_bounds;

fn op() -> Operation {
    Operation::new(BRINGUP_OPERATION).unwrap()
}

/// The bring-up budget **is** the ceiling -- not "happens to equal thirty seconds".
/// The definition references `Operation::CEILING`, so this equality is a statement
/// about the design rather than about two numbers that were written down separately.
#[test]
fn bringup_budget_is_the_operation_ceiling() {
    assert_eq!(BRINGUP_OPERATION, Operation::CEILING);
    assert!(Operation::new(BRINGUP_OPERATION).is_ok());
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
    doc.content = "x".repeat(super::knowledge::MAX_CONTENT_BYTES + 1);
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

/// R08 "存储维护有界": the decision-payload cap had **no carrier at all** before
/// this round — `grep MAX_PAYLOAD_BYTES src/tests.rs` was 0 — so the bound could
/// have been raised or removed without a single test going red.
#[test]
fn the_decision_payload_is_bounded_and_the_refusal_is_named() {
    let parent = tempfile::tempdir().unwrap();
    let mut storage = store(&parent.path().join("source"));
    setup(&mut storage);
    let cap = super::decision::MAX_PAYLOAD_BYTES;

    // Well inside the bound -> accepted.
    let small = serde_json::json!({"blob": "x".repeat(cap / 2)});
    storage
        .append_decision(
            &op(),
            "test-workspace",
            None,
            DecisionKind::Cancellation,
            &small,
            "r08-small",
        )
        .unwrap();

    // Serializing this one is `cap + overhead` bytes: strictly over the bound.
    let oversize = serde_json::json!({"blob": "x".repeat(cap)});
    assert!(
        serde_json::to_string(&oversize).unwrap().len() > cap,
        "the fixture must actually exceed the declared bound"
    );
    let error = storage
        .append_decision(
            &op(),
            "test-workspace",
            None,
            DecisionKind::Cancellation,
            &oversize,
            "r08-oversize",
        )
        .unwrap_err();
    assert_eq!(
        error.code,
        nai_contracts::v1::ErrorCode::ResourceExhausted,
        "an oversized payload must be refused by name, not truncated silently"
    );
}

/// R08: a snapshot must not be able to smuggle mutable journal state past the
/// approved database hash. The rule (`snapshot.rs`, "mutable journal") likewise
/// had zero references in the test suite.
#[test]
fn a_snapshot_carrying_a_mutable_journal_is_rejected() {
    let parent = tempfile::tempdir().unwrap();
    let mut storage = store(&parent.path().join("source"));
    setup(&mut storage);
    let backup = storage.backup(&op()).unwrap();

    // The clean snapshot restores.
    Store::restore(
        &backup,
        DataPaths::at(parent.path().join("clean")).unwrap(),
        &op(),
    )
    .unwrap();

    // Each sidecar is refused on its own, and the snapshot is otherwise intact:
    // removing it makes the same restore succeed again, so the refusal is
    // attributable to the sidecar and not to a broken backup.
    for (index, suffix) in ["-wal", "-shm", "-journal"].into_iter().enumerate() {
        let sidecar = backup.join(format!("state.sqlite3{suffix}"));
        fs::write(&sidecar, b"test-only-mutable-journal").unwrap();
        let error = Store::restore(
            &backup,
            DataPaths::at(parent.path().join(format!("tampered-{index}"))).unwrap(),
            &op(),
        )
        .err()
        .expect("a snapshot carrying a mutable journal must be refused");
        assert_eq!(
            error.code,
            nai_contracts::v1::ErrorCode::Conflict,
            "a snapshot with a {suffix} sidecar must be refused"
        );
        fs::remove_file(&sidecar).unwrap();
    }

    // The control: with every sidecar gone the snapshot restores again.
    Store::restore(
        &backup,
        DataPaths::at(parent.path().join("clean-again")).unwrap(),
        &op(),
    )
    .unwrap();
}

/// R08 fixture: a chunk built to a size instead of by the splitter, so the bound
/// under test -- not the splitter's own limits -- decides the shape.
fn synthetic_chunk(document: &str, index: usize, content: String) -> crate::ImportChunk {
    crate::ImportChunk {
        chunk_id: format!("{document}#chunk-{index}"),
        content,
        source_span: format!("span-{index}"),
        product_scope: "test-workspace".into(),
        acl_ref: "acl-read".into(),
        tokens: vec![format!("t{index}")],
    }
}
fn synthetic_document(document: &str, chunks: Vec<crate::ImportChunk>) -> crate::ImportedDocument {
    let mut doc = knowledge_doc("占位正文");
    doc.document_id = document.into();
    doc.content = format!("{document} 的正文");
    doc.chunks = chunks;
    doc
}
/// Stage (never publish) a selection, so a refusal can only come from the
/// bounds under test rather than from the publication CAS.
fn stage(storage: &mut Store, snapshot: &str, documents: &[&str]) -> crate::Result<()> {
    let active: Option<String> = storage
        .conn
        .query_row(
            "SELECT snapshot_id FROM knowledge_active_snapshots WHERE workspace_id='test-workspace' AND kb_scope='test-workspace'",
            [],
            |row| row.get(0),
        )
        .optional()
        .unwrap();
    storage.stage_snapshot(
        &op(),
        "operator",
        "test-workspace",
        snapshot,
        "test-workspace",
        &documents
            .iter()
            .map(|document| (*document).to_string())
            .collect::<Vec<_>>(),
        None,
        active.as_deref(),
    )
}
/// Give a file a logical length without writing that many bytes. `checked_file`
/// compares `metadata().len()` before it reads anything, so this reaches the size
/// gate for one `set_len` instead of a real multi-hundred-megabyte write.
fn inflate(path: &std::path::Path, len: u64) {
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .unwrap()
        .set_len(len)
        .unwrap();
}

/// R08 "存储维护有界" 的名册：`模块::常量` 的字面路径、那个常量**自己**的值、以及本项
/// 自称的数。三者分开写，是为了让「把某个上限抬高」与「改动名册」都成为**显式动作**，
/// 而不是悄悄漂移。
///
/// ⚠️ 名册的**条数**是 19 项（W125 普查所得），这里列的是**承载它们的常量**，共 20 个
/// ——因为 ⑥「候选编码 65536 / 依赖 32」是**两个条件、一个条目**。别把这两个数当同一个。
pub(crate) const STORAGE_BOUND_ROSTER: &[(&str, u64, u64)] = &[
    (
        "decision::MAX_PAYLOAD_BYTES",
        decision::MAX_PAYLOAD_BYTES as u64,
        65_536,
    ),
    (
        "decision::TELEMETRY_BUFFER_CAPACITY",
        decision::TELEMETRY_BUFFER_CAPACITY as u64,
        1_024,
    ),
    (
        "knowledge::MAX_CONTENT_BYTES",
        knowledge::MAX_CONTENT_BYTES as u64,
        16 * 1024 * 1024,
    ),
    ("knowledge::MAX_CHUNKS", knowledge::MAX_CHUNKS as u64, 4_096),
    (
        "publication::MAX_CHUNKS",
        publication::MAX_CHUNKS as u64,
        4_096,
    ),
    (
        "publication::MAX_BYTES",
        publication::MAX_BYTES as u64,
        16 * 1024 * 1024,
    ),
    (
        "snapshot::MAX_BYTES",
        snapshot::MAX_BYTES,
        256 * 1024 * 1024,
    ),
    (
        "snapshot::MAX_ARTIFACT",
        snapshot::MAX_ARTIFACT,
        16 * 1024 * 1024,
    ),
    // W129：W125 之后仍然**零覆盖**的那 11 项（12 个条件）。
    (
        "store::MAX_CONFIGURATION_BYTES",
        store::MAX_CONFIGURATION_BYTES as u64,
        65_536,
    ),
    (
        "store::MAX_FACT_BYTES",
        store::MAX_FACT_BYTES as u64,
        65_536,
    ),
    (
        "store::MAX_CANDIDATE_BYTES",
        store::MAX_CANDIDATE_BYTES as u64,
        65_536,
    ),
    (
        "store::MAX_CANDIDATE_DEPENDENCIES",
        store::MAX_CANDIDATE_DEPENDENCIES as u64,
        32,
    ),
    (
        "store::MAX_PENDING_CANDIDATES",
        store::MAX_PENDING_CANDIDATES as u64,
        200,
    ),
    (
        "knowledge::MAX_EVIDENCE_REFS",
        knowledge::MAX_EVIDENCE_REFS as u64,
        32,
    ),
    (
        "knowledge::MAX_REVOCATION_REVISIONS",
        knowledge::MAX_REVOCATION_REVISIONS as u64,
        4_096,
    ),
    (
        "publication::MAX_SNAPSHOT_DOCUMENTS",
        publication::MAX_SNAPSHOT_DOCUMENTS as u64,
        256,
    ),
    (
        "snapshot::MAX_SNAPSHOT_ARTIFACTS",
        snapshot::MAX_SNAPSHOT_ARTIFACTS as u64,
        512,
    ),
    (
        "budget::MAX_LEDGER_ENTRY_BYTES",
        budget::MAX_LEDGER_ENTRY_BYTES as u64,
        65_536,
    ),
    (
        "budget::MAX_STORED_LEDGER_BYTES",
        budget::MAX_STORED_LEDGER_BYTES as u64,
        132_000,
    ),
    (
        "budget::MAX_STORED_LEDGER_HEX",
        budget::MAX_STORED_LEDGER_HEX as u64,
        131_072,
    ),
];

/// 名册之外的顶格常量，第一类：**不是上限**（版本号、引用、哨兵、内嵌数据、迁移脚本、
/// 以及本测试自己的脚手架）。
const NOT_A_BOUND: &[&str] = &[
    // 库的 schema 版本号，不是资源天花板（它的换版规矩在 §2 另有成套六处）。
    "migration::VERSION",
    // 迁移脚本数组本身。
    "migration::SQL",
    // bring-up 上限的家在 kernel（ADR 0055），这里只是引用。
    "lib::BRINGUP_OPERATION",
    // 决策链的起始哨兵哈希，不是天花板。
    "decision::GENESIS_HASH",
    // 内嵌的能力目录字节。
    "snapshot::CATALOG",
    // 本测试自己的脚手架：名册表、源文件表、两张分类表。
    "tests::STORAGE_BOUND_ROSTER",
    "tests::SOURCES",
    "tests::NOT_A_BOUND",
    "tests::SHAPE_LIMITS",
];

/// 名册之外的顶格常量，第二类：**是上限，但按 W125 的口径不收进 R08**——它们约束的是
/// **输入形状**（取值域/编码宽度），不是「一次写入或一份快照/备份可占用的资源」。
/// 具名登记在这里，避免它们变成「没被扫到所以不用管」的后门。
const SHAPE_LIMITS: &[&str] = &[
    "knowledge::MAX_TOKENS_PER_CHUNK",
    "knowledge::MAX_DIMENSION",
];

/// 被扫的源文件：模块名 → 该模块的源文本。**文件层**名册自己也要闭世界（见下）。
const SOURCES: &[(&str, &str)] = &[
    ("budget", include_str!("budget.rs")),
    ("decision", include_str!("decision.rs")),
    ("knowledge", include_str!("knowledge.rs")),
    ("lib", include_str!("lib.rs")),
    ("migration", include_str!("migration.rs")),
    ("paths", include_str!("paths.rs")),
    ("publication", include_str!("publication.rs")),
    ("snapshot", include_str!("snapshot.rs")),
    ("store", include_str!("store.rs")),
    ("tests", include_str!("tests.rs")),
    ("upgrade", include_str!("upgrade.rs")),
];

/// 顶格的常量声明——**三种可见性都算**。只认**顶格**的：函数体内的常量不是模块级的，
/// MUST NOT 混进来。按可见性筛是一种**弱类别**：`MAX_DIMENSION` 那样私有的上限会从
/// 后门溜走，所以这里的谓词只按「顶格」，分类全部交给下面的三张表。
fn declared_constants(module: &str, source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let rest = line
                .strip_prefix("pub(crate) const ")
                .or_else(|| line.strip_prefix("pub const "))
                .or_else(|| line.strip_prefix("const "))?;
            let name = rest.split([':', ' ', '[']).next()?;
            Some(format!("{module}::{name}"))
        })
        .collect()
}

/// R08 "存储维护有界": the item is only as good as its roster. Every magnitude
/// below is a declared policy value, so raising one is a deliberate act rather
/// than a silent drift -- and the *behaviour* of each bound is pinned separately
/// by `tests::storage_bounds`, because a magnitude pin alone would not notice a
/// bound that is declared and then never consulted.
#[test]
fn the_declared_storage_write_bounds_are_the_magnitudes_the_item_claims() {
    for &(name, actual, declared) in STORAGE_BOUND_ROSTER {
        assert_eq!(
            actual, declared,
            "{name} is a declared magnitude, not an accident"
        );
    }
}

/// 名册的**闭世界**。`the_declared_storage_write_bounds_are_the_magnitudes_the_item_claims`
/// 只答「名册上那些数对不对」；「有没有上限**不在这张名册上**」是它的补集，而按清单扫的
/// 盲区恰恰就是清单的补集。三层一起闭：
/// ① 文件层：`lib.rs` 的每个 `mod` 都有被扫的源文件；
/// ② 元素层：扫到的每个顶格常量**恰好**归入三类之一（名册 / 形状校验按口径不收 /
///    不是上限）——三分是划分，MUST NOT 有第四种出路；
/// ③ 反方向：名册与两张清单上的每个名字都**真的**扫得到（否则那一行是装饰）。
#[test]
fn every_declared_crate_constant_is_classified_by_the_roster() {
    use std::collections::BTreeSet;

    // ① 文件层
    let lib = SOURCES
        .iter()
        .find(|(module, _)| *module == "lib")
        .expect("lib.rs MUST 被扫")
        .1;
    let modules: BTreeSet<&str> = lib
        .lines()
        .filter_map(|line| line.strip_prefix("mod ")?.strip_suffix(';'))
        .collect();
    let scanned: BTreeSet<&str> = SOURCES
        .iter()
        .map(|(module, _)| *module)
        .filter(|module| *module != "lib")
        .collect();
    assert_eq!(
        modules, scanned,
        "每个模块都 MUST 被扫到，否则它的上限会整块掉出名册"
    );

    // 元素层与反方向
    let declared: BTreeSet<String> = SOURCES
        .iter()
        .flat_map(|(module, source)| declared_constants(module, source))
        .collect();
    let bucket = |names: &[&str]| -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    };
    let roster: BTreeSet<String> = STORAGE_BOUND_ROSTER
        .iter()
        .map(|(name, _, _)| (*name).to_string())
        .collect();
    let shapes = bucket(SHAPE_LIMITS);
    let allowed = bucket(NOT_A_BOUND);

    let unclassified: Vec<&String> = declared
        .iter()
        .filter(|name| {
            !roster.contains(*name) && !shapes.contains(*name) && !allowed.contains(*name)
        })
        .collect();
    assert!(
        unclassified.is_empty(),
        "这些常量既不（在名册上）、也不（按口径不收）、也不（不是上限）：{unclassified:?}"
    );
    // 三分是划分：两张清单之间 MUST NOT 相交，名册与它们也 MUST NOT 相交。
    assert!(
        roster.is_disjoint(&shapes),
        "「是上限」与「按口径不收」MUST NOT 同时成立"
    );
    assert!(
        roster.is_disjoint(&allowed),
        "「是上限」与「不是上限」MUST NOT 同时成立"
    );
    assert!(
        shapes.is_disjoint(&allowed),
        "「按口径不收」与「不是上限」MUST NOT 同时成立"
    );
    // 反方向：清单上的名字 MUST 都扫得到（否则那几行是装饰）。
    let dangling: Vec<&String> = roster
        .iter()
        .chain(shapes.iter())
        .chain(allowed.iter())
        .filter(|name| !declared.contains(*name))
        .collect();
    assert!(
        dangling.is_empty(),
        "清单列了扫不到的常量，那几行是装饰：{dangling:?}"
    );
}

/// R08: the import bounds are inclusive, so "at the cap" and "one over the cap"
/// must land on opposite sides. The content bound previously had a single
/// carrier that *copied* the literal, which pinned the number without ever
/// naming the bound it belonged to.
#[test]
fn the_import_bounds_are_enforced_on_both_sides_of_the_declared_cap() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    let content_cap = super::knowledge::MAX_CONTENT_BYTES;
    let chunk_cap = super::knowledge::MAX_CHUNKS;

    let mut at_cap = knowledge_doc("界内正文");
    at_cap.content = "x".repeat(content_cap);
    storage.import_document(&op(), "operator", &at_cap).unwrap();

    let mut over_cap = knowledge_doc("越界正文");
    over_cap.content = "x".repeat(content_cap + 1);
    assert_eq!(
        storage
            .import_document(&op(), "operator", &over_cap)
            .unwrap_err()
            .reason,
        "document exceeds import bound",
        "one byte over the declared content cap must be refused by name"
    );

    let wide = synthetic_document(
        "doc-at-cap",
        (0..chunk_cap)
            .map(|index| synthetic_chunk("doc-at-cap", index, format!("块{index}")))
            .collect(),
    );
    storage.import_document(&op(), "operator", &wide).unwrap();

    let too_wide = synthetic_document(
        "doc-over-cap",
        (0..chunk_cap + 1)
            .map(|index| synthetic_chunk("doc-over-cap", index, format!("块{index}")))
            .collect(),
    );
    assert_eq!(
        storage
            .import_document(&op(), "operator", &too_wide)
            .unwrap_err()
            .reason,
        "invalid chunk count",
        "one chunk over the declared count cap must be refused by name"
    );
}

/// R08: the publication bounds are checked against the **sum over the selected
/// documents**, so a selection can be refused while every document in it is
/// individually admissible. A per-document test would call that a pass.
#[test]
fn the_published_snapshot_bounds_are_the_sum_over_the_selected_documents() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    let chunk_cap = super::publication::MAX_CHUNKS as usize;
    let byte_cap = super::publication::MAX_BYTES as usize;

    let wide = synthetic_document(
        "doc-wide",
        (0..chunk_cap)
            .map(|index| synthetic_chunk("doc-wide", index, format!("块{index}")))
            .collect(),
    );
    storage.import_document(&op(), "operator", &wide).unwrap();
    let extra = synthetic_document(
        "doc-extra",
        (0..2)
            .map(|index| synthetic_chunk("doc-extra", index, format!("块{index}")))
            .collect(),
    );
    storage.import_document(&op(), "operator", &extra).unwrap();

    stage(&mut storage, "snap-wide", &["doc-wide"]).unwrap();
    let count_refusal =
        stage(&mut storage, "snap-wide-extra", &["doc-wide", "doc-extra"]).unwrap_err();
    assert_eq!(count_refusal.reason, "snapshot chunk count bound");
    assert_eq!(
        count_refusal.code,
        nai_contracts::v1::ErrorCode::ResourceExhausted
    );

    // Each heavy document fits on its own (its own chunk bytes stay under the
    // ceiling); only the sum over the selection crosses it.
    let half = byte_cap / 2 + 1;
    for (document, filler) in [("doc-left", "l"), ("doc-right", "r")] {
        let heavy = synthetic_document(
            document,
            vec![synthetic_chunk(document, 0, filler.repeat(half))],
        );
        storage.import_document(&op(), "operator", &heavy).unwrap();
    }
    stage(&mut storage, "snap-left", &["doc-left"]).unwrap();
    let byte_refusal =
        stage(&mut storage, "snap-left-right", &["doc-left", "doc-right"]).unwrap_err();
    assert_eq!(
        byte_refusal.reason, "snapshot chunk byte bound",
        "two individually admissible documents must still be refused on their sum"
    );
}

/// R08: a snapshot database over the declared ceiling is refused *before a single
/// byte of it is read*, and the refusal is attributable -- the size gate and the
/// hash gate are different named refusals, so "too big" cannot be read as
/// "corrupt" or the other way round.
#[test]
fn a_backup_over_the_declared_ceiling_is_refused_by_the_size_gate_not_by_hash() {
    let parent = tempfile::tempdir().unwrap();
    let mut storage = store(&parent.path().join("source"));
    setup(&mut storage);
    let backup = storage.backup(&op()).unwrap();
    let database = backup.join("state.sqlite3");

    let original = fs::read(&database).unwrap();
    let mut flipped = original.clone();
    flipped[0] ^= 0xff;
    fs::write(&database, &flipped).unwrap();
    let mismatch = Store::restore(
        &backup,
        DataPaths::at(parent.path().join("flipped")).unwrap(),
        &op(),
    )
    .err()
    .expect("a snapshot whose bytes changed must be refused");
    assert_eq!(mismatch.reason, "database hash/size mismatch");
    fs::write(&database, &original).unwrap();

    inflate(&database, super::snapshot::MAX_BYTES + 1);
    let oversized = Store::restore(
        &backup,
        DataPaths::at(parent.path().join("oversized")).unwrap(),
        &op(),
    )
    .err()
    .expect("a snapshot database over the declared ceiling must be refused");
    assert_eq!(
        oversized.code,
        nai_contracts::v1::ErrorCode::ResourceExhausted
    );
    assert_eq!(oversized.reason, "snapshot file limit");
}

/// R08: the same two gates on a registered immutable artifact. The artifact is
/// covered where the D03 declaration puts it -- "registered immutable files enter
/// the backup with count/size/path limits" -- and the size limit is enforced per
/// file, not only on the database.
#[test]
fn a_backup_artifact_over_the_declared_ceiling_is_refused_by_the_size_gate() {
    let parent = tempfile::tempdir().unwrap();
    let mut storage = store(&parent.path().join("source"));
    setup(&mut storage);
    let relative = storage
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
    let artifact = backup.join(&relative);

    let original = fs::read(&artifact).unwrap();
    let mut flipped = original.clone();
    flipped[0] ^= 0xff;
    fs::write(&artifact, &flipped).unwrap();
    let mismatch = Store::restore(
        &backup,
        DataPaths::at(parent.path().join("flipped")).unwrap(),
        &op(),
    )
    .err()
    .expect("a snapshot artifact whose bytes changed must be refused");
    assert_eq!(mismatch.reason, "restore artifact invalid");
    fs::write(&artifact, &original).unwrap();

    inflate(&artifact, super::snapshot::MAX_ARTIFACT + 1);
    let oversized = Store::restore(
        &backup,
        DataPaths::at(parent.path().join("oversized")).unwrap(),
        &op(),
    )
    .err()
    .expect("an artifact over the declared ceiling must be refused");
    assert_eq!(
        oversized.code,
        nai_contracts::v1::ErrorCode::ResourceExhausted
    );
    assert_eq!(oversized.reason, "snapshot file limit");
}

/// R08: the same per-file ceiling is enforced where the artifact is *written*,
/// and it is inclusive -- so a single exported constant backs two enforcement
/// sites and both must agree on where the boundary sits.
#[test]
fn an_artifact_at_the_declared_ceiling_is_accepted_and_one_byte_over_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    let cap = super::snapshot::MAX_ARTIFACT as usize;

    storage
        .publish_artifact(
            &op(),
            "test-workspace",
            "operator",
            "doc-at-cap",
            "assets",
            &vec![b'x'; cap],
        )
        .unwrap();
    assert_eq!(
        storage
            .publish_artifact(
                &op(),
                "test-workspace",
                "operator",
                "doc-over-cap",
                "assets",
                &vec![b'y'; cap + 1],
            )
            .unwrap_err()
            .reason,
        "artifact type/size invalid",
        "one byte over the declared per-file ceiling must be refused by name"
    );
}
