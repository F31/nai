use super::*;
use nai_contracts::v1::RetrievalStatus;

fn active(storage: &Store, kb: &str) -> Option<String> {
    storage.conn.query_row("SELECT snapshot_id FROM knowledge_active_snapshots WHERE workspace_id='test-workspace' AND kb_scope=?1",[kb],|row|row.get(0)).optional().unwrap()
}
fn build(storage: &mut Store, name: &str, index: Option<&str>) -> SnapshotPublish {
    let expected = active(storage, "test-workspace");
    storage
        .stage_snapshot(
            &op(),
            "operator",
            "test-workspace",
            name,
            "test-workspace",
            &["doc-kb".into()],
            index,
            expected.as_deref(),
        )
        .unwrap();
    storage
        .index_snapshot(&op(), "operator", "test-workspace", name)
        .unwrap()
}
fn evidence(storage: &mut Store) -> (RetrievalStatus, Vec<nai_contracts::v1::Evidence>) {
    storage
        .retrieve_local(
            &op(),
            "operator",
            "test-workspace",
            "test-workspace",
            &[],
            &nai_knowledge_core::tokenize("保修"),
            None,
            4,
        )
        .unwrap()
}
fn cue(snapshot: &str, utterance: &str) -> PerformanceCue {
    PerformanceCue {
        session_id: "knowledge-session".into(),
        utterance_id: utterance.into(),
        generation_epoch: 0,
        display_text: "保修说明".into(),
        speech_text: "保修说明".into(),
        knowledge_snapshot_ref: snapshot.into(),
        evidence_refs: vec!["chunk:doc-kb#chunk-0".into()],
        expires_at_utc_ms: (utc_ms().unwrap() + 60000) as u64,
        ..Default::default()
    }
}
fn approval(hash: &str) -> Approval {
    Approval {
        approval_id: "knowledge-approval".into(),
        session_id: "knowledge-session".into(),
        generation_epoch: 0,
        candidate_hash: hash.into(),
        hash_algorithm: "sha256-canonical-v1".into(),
        action_scope: "speak".into(),
        expires_at_utc_ms: (utc_ms().unwrap() + 30000) as u64,
        ..Default::default()
    }
}

#[test]
fn staged_and_indexed_snapshots_are_invisible_and_resume_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .import_document(&op(), "operator", &knowledge_doc("保修旧内容"))
        .unwrap();
    assert_eq!(
        evidence(&mut storage).0,
        RetrievalStatus::RetrievalUnavailable
    );
    let first = publish_prepared(&mut storage, None);
    storage
        .import_document(&op(), "operator", &knowledge_doc("保修新内容"))
        .unwrap();
    storage
        .stage_snapshot(
            &op(),
            "operator",
            "test-workspace",
            "next",
            "test-workspace",
            &["doc-kb".into()],
            None,
            Some(&first.snapshot_id),
        )
        .unwrap();
    assert!(evidence(&mut storage).1[0].text.contains("旧内容"));
    let sealed = storage
        .index_snapshot(&op(), "operator", "test-workspace", "next")
        .unwrap();
    assert!(evidence(&mut storage).1[0].text.contains("旧内容"));
    let cancelled = op();
    cancelled.cancel();
    assert_eq!(
        storage
            .publish_snapshot(&cancelled, "operator", &sealed)
            .unwrap_err()
            .code,
        nai_contracts::v1::ErrorCode::Cancelled
    );
    drop(storage);
    let mut storage = store(dir.path());
    assert_eq!(active(&storage, "test-workspace"), Some(first.snapshot_id));
    assert!(evidence(&mut storage).1[0].text.contains("旧内容"));
    storage
        .publish_snapshot(&op(), "operator", &sealed)
        .unwrap();
    assert!(evidence(&mut storage).1[0].text.contains("新内容"));
}

#[test]
fn final_transaction_failure_rolls_back_pointer_revocation_and_approvals() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .import_document(&op(), "operator", &knowledge_doc("保修旧内容"))
        .unwrap();
    let first = publish_prepared(&mut storage, None);
    storage
        .create_session(&op(), "test-workspace", "operator", "knowledge-session")
        .unwrap();
    storage
        .bind_session_knowledge(
            &op(),
            "test-workspace",
            "operator",
            "knowledge-session",
            "test-workspace",
            &first.snapshot_id,
        )
        .unwrap();
    let hash = storage
        .record_candidate(
            &op(),
            "test-workspace",
            "operator",
            &cue(&first.snapshot_id, "pending"),
            &[],
        )
        .unwrap();
    storage
        .approve(&op(), "test-workspace", "operator", &approval(&hash))
        .unwrap();
    storage
        .import_document(&op(), "operator", &knowledge_doc("保修新内容"))
        .unwrap();
    let sealed = build(&mut storage, "atomic-next", None);
    storage.conn.execute_batch("CREATE TEMP TRIGGER fail_publish BEFORE INSERT ON audit_records WHEN NEW.action='knowledge.published' BEGIN SELECT RAISE(ABORT,'injected commit-stage failure'); END;").unwrap();
    assert!(
        storage
            .publish_snapshot(&op(), "operator", &sealed)
            .is_err()
    );
    assert_eq!(
        active(&storage, "test-workspace"),
        Some(first.snapshot_id.clone())
    );
    assert!(
        storage
            .approval_valid(&op(), "test-workspace", "knowledge-session", 0, &hash)
            .unwrap()
    );
    assert_eq!(
        storage
            .conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge_document_revocations",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    storage
        .conn
        .execute_batch("DROP TRIGGER fail_publish;")
        .unwrap();
    storage
        .publish_snapshot(&op(), "operator", &sealed)
        .unwrap();
    assert!(
        !storage
            .approval_valid(&op(), "test-workspace", "knowledge-session", 0, &hash)
            .unwrap()
    );
}

#[test]
fn facts_permissions_and_competing_active_switches_are_real_cas_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .import_document(&op(), "operator", &knowledge_doc("保修内容"))
        .unwrap();
    let first = publish_prepared(&mut storage, None);
    let changed_facts = build(&mut storage, "facts-cas", None);
    storage
        .publish_fact(&op(), "operator", &fact(), None)
        .unwrap();
    assert_eq!(
        storage
            .publish_snapshot(&op(), "operator", &changed_facts)
            .unwrap_err()
            .code,
        nai_contracts::v1::ErrorCode::Conflict
    );
    assert_eq!(active(&storage, "test-workspace"), Some(first.snapshot_id));
    let changed_permission = build(&mut storage, "permission-cas", None);
    storage
        .set_permission(
            &op(),
            "test-workspace",
            "reader",
            "knowledge.read",
            "*",
            true,
        )
        .unwrap();
    assert_eq!(
        storage
            .publish_snapshot(&op(), "operator", &changed_permission)
            .unwrap_err()
            .code,
        nai_contracts::v1::ErrorCode::Conflict
    );
    let a = build(&mut storage, "race-a", None);
    let b = build(&mut storage, "race-b", None);
    storage.publish_snapshot(&op(), "operator", &a).unwrap();
    assert_eq!(
        storage
            .publish_snapshot(&op(), "operator", &b)
            .unwrap_err()
            .code,
        nai_contracts::v1::ErrorCode::Conflict
    );
    assert_eq!(active(&storage, "test-workspace"), Some(a.snapshot_id));
}

#[test]
fn server_manifest_rejects_claimed_hash_changed_source_and_tampered_index() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .import_document(&op(), "operator", &knowledge_doc("保修内容"))
        .unwrap();
    let sealed = build(&mut storage, "sealed", None);
    let mut claimed = sealed.clone();
    claimed.manifest_hash = "f".repeat(64);
    assert!(
        storage
            .publish_snapshot(&op(), "operator", &claimed)
            .is_err()
    );
    assert!(active(&storage, "test-workspace").is_none());
    storage.conn.execute("UPDATE knowledge_snapshot_chunks SET source_span='tampered' WHERE snapshot_id='sealed'",[]).unwrap();
    assert_eq!(
        storage
            .publish_snapshot(&op(), "operator", &sealed)
            .unwrap_err()
            .code,
        nai_contracts::v1::ErrorCode::Conflict
    );
    let stale = build(&mut storage, "stale-source", None);
    storage
        .import_document(&op(), "operator", &knowledge_doc("保修后续内容"))
        .unwrap();
    assert_eq!(
        storage
            .publish_snapshot(&op(), "operator", &stale)
            .unwrap_err()
            .code,
        nai_contracts::v1::ErrorCode::Conflict
    );
    assert!(active(&storage, "test-workspace").is_none());
    storage
        .conn
        .execute(
            "DELETE FROM knowledge_fts WHERE workspace_id='test-workspace'",
            [],
        )
        .unwrap();
    assert!(
        storage
            .stage_snapshot(
                &op(),
                "operator",
                "test-workspace",
                "missing-lexical",
                "test-workspace",
                &["doc-kb".into()],
                None,
                None
            )
            .is_err()
    );
    assert_eq!(
        storage
            .conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge_snapshots WHERE snapshot_id='missing-lexical'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}

#[test]
fn frozen_bindings_do_not_follow_publication_and_replaced_documents_cannot_be_reused() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .import_document(&op(), "operator", &knowledge_doc("保修旧内容"))
        .unwrap();
    let first = publish_prepared(&mut storage, None);
    storage
        .create_session(&op(), "test-workspace", "operator", "knowledge-session")
        .unwrap();
    let bound = storage
        .bind_session_knowledge(
            &op(),
            "test-workspace",
            "operator",
            "knowledge-session",
            "test-workspace",
            &first.snapshot_id,
        )
        .unwrap();
    assert_eq!(bound.knowledge_snapshot_ref, first.snapshot_id);
    assert!(
        storage
            .bind_session_knowledge(
                &op(),
                "test-workspace",
                "operator",
                "knowledge-session",
                "wrong-scope",
                &first.snapshot_id
            )
            .is_err()
    );
    let unchanged = publish_prepared(&mut storage, None);
    let result = storage
        .retrieve_session_knowledge(
            &op(),
            "operator",
            "test-workspace",
            "knowledge-session",
            &[],
            &nai_knowledge_core::tokenize("保修"),
            None,
            4,
        )
        .unwrap();
    assert!(result.1[0].text.contains("旧内容"));
    assert_eq!(
        storage
            .bind_session_knowledge(
                &op(),
                "test-workspace",
                "operator",
                "knowledge-session",
                "test-workspace",
                &unchanged.snapshot_id
            )
            .unwrap_err()
            .code,
        nai_contracts::v1::ErrorCode::Conflict
    );
    storage
        .import_document(&op(), "operator", &knowledge_doc("保修新内容"))
        .unwrap();
    let changed = publish_prepared(&mut storage, None);
    storage.publish_snapshot(&op(), "operator", &first).unwrap();
    assert_eq!(
        active(&storage, "test-workspace"),
        Some(changed.snapshot_id.clone())
    );
    assert_eq!(
        storage
            .session(&op(), "test-workspace", "operator", "knowledge-session")
            .unwrap()
            .knowledge_snapshot_ref,
        first.snapshot_id
    );
    assert!(
        storage
            .retrieve_session_knowledge(
                &op(),
                "operator",
                "test-workspace",
                "knowledge-session",
                &[],
                &nai_knowledge_core::tokenize("保修"),
                None,
                4
            )
            .unwrap()
            .1
            .is_empty()
    );
    assert!(
        storage
            .record_candidate(
                &op(),
                "test-workspace",
                "operator",
                &cue(&first.snapshot_id, "stale"),
                &[]
            )
            .is_err()
    );
    assert!(
        storage
            .record_candidate(
                &op(),
                "test-workspace",
                "operator",
                &cue(&changed.snapshot_id, "forged"),
                &[]
            )
            .is_err()
    );
    assert!(evidence(&mut storage).1[0].text.contains("新内容"));
    storage
        .create_session(&op(), "test-workspace", "operator", "new-session")
        .unwrap();
    assert_eq!(
        storage
            .bind_session_knowledge(
                &op(),
                "test-workspace",
                "operator",
                "new-session",
                "test-workspace",
                &changed.snapshot_id
            )
            .unwrap()
            .knowledge_snapshot_ref,
        changed.snapshot_id
    );
}

#[test]
fn incomplete_vector_seal_rolls_back_and_published_vectors_survive_preparation_changes() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .import_document(&op(), "operator", &knowledge_doc("保修向量内容"))
        .unwrap();
    storage
        .stage_snapshot(
            &op(),
            "operator",
            "test-workspace",
            "vectors",
            "test-workspace",
            &["doc-kb".into()],
            Some("index"),
            None,
        )
        .unwrap();
    assert!(
        storage
            .index_snapshot(&op(), "operator", "test-workspace", "vectors")
            .is_err()
    );
    assert_eq!(
        storage
            .conn
            .query_row("SELECT COUNT(*) FROM knowledge_snapshot_fts", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let manifest = VectorManifest {
        index_id: "index".into(),
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
    let sealed = storage
        .index_snapshot(&op(), "operator", "test-workspace", "vectors")
        .unwrap();
    storage
        .publish_snapshot(&op(), "operator", &sealed)
        .unwrap();
    assert!(
        storage
            .index_snapshot(&op(), "operator", "test-workspace", "vectors")
            .is_err()
    );
    storage
        .import_document(&op(), "operator", &knowledge_doc("保修准备区新内容"))
        .unwrap();
    let retrieved = storage
        .retrieve_local(
            &op(),
            "operator",
            "test-workspace",
            "test-workspace",
            &[],
            &[],
            Some(("index", vec![1.0, 0.0])),
            1,
        )
        .unwrap();
    assert_eq!(retrieved.0, RetrievalStatus::RetrievalComplete);
    assert_eq!(retrieved.1[0].text, "保修向量内容");
    let partial = storage
        .retrieve_local(
            &op(),
            "operator",
            "test-workspace",
            "test-workspace",
            &[],
            &nai_knowledge_core::tokenize("保修"),
            Some(("wrong-model", vec![1.0, 0.0])),
            1,
        )
        .unwrap();
    assert_eq!(partial.0, RetrievalStatus::RetrievalPartial);
    assert_eq!(partial.1.len(), 1);
}

#[test]
fn snapshot_selection_isolates_knowledge_scopes_and_workspaces() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .import_document(&op(), "operator", &knowledge_doc("保修甲库内容"))
        .unwrap();
    publish_prepared(&mut storage, None);
    let mut other = knowledge_doc("保修乙库内容");
    other.document_id = "doc-other".into();
    for chunk in &mut other.chunks {
        chunk.chunk_id = chunk.chunk_id.replace("doc-kb", "doc-other");
    }
    storage.import_document(&op(), "operator", &other).unwrap();
    storage
        .stage_snapshot(
            &op(),
            "operator",
            "test-workspace",
            "other-kb",
            "other",
            &["doc-other".into()],
            None,
            None,
        )
        .unwrap();
    let sealed = storage
        .index_snapshot(&op(), "operator", "test-workspace", "other-kb")
        .unwrap();
    storage
        .publish_snapshot(&op(), "operator", &sealed)
        .unwrap();
    assert_eq!(evidence(&mut storage).1.len(), 1);
    assert!(evidence(&mut storage).1[0].text.contains("甲库"));
    let found = storage
        .retrieve_local(
            &op(),
            "operator",
            "test-workspace",
            "other",
            &[],
            &nai_knowledge_core::tokenize("保修"),
            None,
            4,
        )
        .unwrap();
    assert_eq!(found.1.len(), 1);
    assert!(found.1[0].text.contains("乙库"));
    storage
        .create_workspace(&op(), "isolated", "isolated")
        .unwrap();
    storage
        .set_permission(&op(), "isolated", "operator", "knowledge.read", "*", true)
        .unwrap();
    assert_eq!(
        storage
            .retrieve_local(
                &op(),
                "operator",
                "isolated",
                "test-workspace",
                &[],
                &nai_knowledge_core::tokenize("保修"),
                None,
                4
            )
            .unwrap()
            .0,
        RetrievalStatus::RetrievalUnavailable
    );
}

#[test]
fn document_expiry_is_rechecked_for_approval_and_preplay() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    let mut doc = knowledge_doc("保修限时内容");
    doc.valid_until_ms = Some(utc_ms().unwrap() + 2000);
    storage.import_document(&op(), "operator", &doc).unwrap();
    let sealed = publish_prepared(&mut storage, None);
    storage
        .create_session(&op(), "test-workspace", "operator", "knowledge-session")
        .unwrap();
    storage
        .bind_session_knowledge(
            &op(),
            "test-workspace",
            "operator",
            "knowledge-session",
            "test-workspace",
            &sealed.snapshot_id,
        )
        .unwrap();
    let hash = storage
        .record_candidate(
            &op(),
            "test-workspace",
            "operator",
            &cue(&sealed.snapshot_id, "expiring"),
            &[],
        )
        .unwrap();
    storage
        .approve(&op(), "test-workspace", "operator", &approval(&hash))
        .unwrap();
    assert!(
        storage
            .approval_valid(&op(), "test-workspace", "knowledge-session", 0, &hash)
            .unwrap()
    );
    std::thread::sleep(Duration::from_millis(2100));
    assert!(
        !storage
            .approval_valid(&op(), "test-workspace", "knowledge-session", 0, &hash)
            .unwrap()
    );
    assert_eq!(
        storage
            .approve(&op(), "test-workspace", "operator", &approval(&hash))
            .unwrap_err()
            .code,
        nai_contracts::v1::ErrorCode::EvidenceExpired
    );
    assert!(evidence(&mut storage).1.is_empty());
}

#[test]
fn consistent_backup_restores_published_knowledge_but_not_output_authority() {
    let parent = tempfile::tempdir().unwrap();
    let mut storage = store(&parent.path().join("source"));
    setup(&mut storage);
    storage
        .import_document(&op(), "operator", &knowledge_doc("保修备份内容"))
        .unwrap();
    let sealed = publish_prepared(&mut storage, None);
    storage
        .create_session(&op(), "test-workspace", "operator", "knowledge-session")
        .unwrap();
    storage
        .bind_session_knowledge(
            &op(),
            "test-workspace",
            "operator",
            "knowledge-session",
            "test-workspace",
            &sealed.snapshot_id,
        )
        .unwrap();
    let hash = storage
        .record_candidate(
            &op(),
            "test-workspace",
            "operator",
            &cue(&sealed.snapshot_id, "backup-pending"),
            &[],
        )
        .unwrap();
    storage
        .approve(&op(), "test-workspace", "operator", &approval(&hash))
        .unwrap();
    let backup = storage.backup(&op()).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(backup.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["db_schema_version"], 5);
    let mut restored = Store::restore(
        &backup,
        DataPaths::at(parent.path().join("restored")).unwrap(),
        &op(),
    )
    .unwrap();
    assert!(
        restored
            .retrieve_local(
                &op(),
                "operator",
                "test-workspace",
                "test-workspace",
                &[],
                &nai_knowledge_core::tokenize("保修"),
                None,
                4
            )
            .is_err()
    );
    for permission in ["knowledge.read", "session.manage"] {
        restored
            .set_permission(&op(), "test-workspace", "operator", permission, "*", true)
            .unwrap();
    }
    assert!(evidence(&mut restored).1[0].text.contains("备份"));
    assert_eq!(
        restored
            .session(&op(), "test-workspace", "operator", "knowledge-session")
            .unwrap()
            .knowledge_snapshot_ref,
        sealed.snapshot_id
    );
    assert!(
        !restored
            .approval_valid(&op(), "test-workspace", "knowledge-session", 0, &hash)
            .unwrap()
    );
    assert!(
        restored
            .session(&op(), "test-workspace", "operator", "knowledge-session")
            .unwrap()
            .generation_epoch
            > 0
    );
}

#[test]
fn removing_shared_document_from_one_kb_does_not_revoke_other_kb() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .import_document(&op(), "operator", &knowledge_doc("保修共享内容"))
        .unwrap();
    let main = publish_prepared(&mut storage, None);
    let other = publish_prepared_in_scope(&mut storage, None, "other-kb");
    storage
        .create_session(&op(), "test-workspace", "operator", "knowledge-session")
        .unwrap();
    storage
        .bind_session_knowledge(
            &op(),
            "test-workspace",
            "operator",
            "knowledge-session",
            "other-kb",
            &other.snapshot_id,
        )
        .unwrap();
    let hash = storage
        .record_candidate(
            &op(),
            "test-workspace",
            "operator",
            &cue(&other.snapshot_id, "other-pending"),
            &[],
        )
        .unwrap();
    storage
        .approve(&op(), "test-workspace", "operator", &approval(&hash))
        .unwrap();
    let mut replacement = knowledge_doc("保修替换库内容");
    replacement.document_id = "replacement".into();
    for chunk in &mut replacement.chunks {
        chunk.chunk_id = chunk.chunk_id.replace("doc-kb", "replacement");
    }
    storage
        .import_document(&op(), "operator", &replacement)
        .unwrap();
    storage
        .stage_snapshot(
            &op(),
            "operator",
            "test-workspace",
            "replace-main",
            "test-workspace",
            &["replacement".into()],
            None,
            Some(&main.snapshot_id),
        )
        .unwrap();
    let sealed = storage
        .index_snapshot(&op(), "operator", "test-workspace", "replace-main")
        .unwrap();
    storage
        .publish_snapshot(&op(), "operator", &sealed)
        .unwrap();
    assert!(
        storage
            .approval_valid(&op(), "test-workspace", "knowledge-session", 0, &hash)
            .unwrap()
    );
    assert!(
        storage
            .retrieve_session_knowledge(
                &op(),
                "operator",
                "test-workspace",
                "knowledge-session",
                &[],
                &nai_knowledge_core::tokenize("保修"),
                None,
                4
            )
            .unwrap()
            .1[0]
            .text
            .contains("共享")
    );
    storage
        .revoke_knowledge(&op(), "operator", "test-workspace", None, Some("doc-kb"))
        .unwrap();
    assert!(
        !storage
            .approval_valid(&op(), "test-workspace", "knowledge-session", 0, &hash)
            .unwrap()
    );
}

#[test]
fn frozen_fact_evidence_never_changes_to_latest_live_version() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .publish_fact(&op(), "operator", &fact(), None)
        .unwrap();
    storage
        .import_document(&op(), "operator", &knowledge_doc("保修知识内容"))
        .unwrap();
    let sealed = publish_prepared(&mut storage, None);
    storage
        .create_session(&op(), "test-workspace", "operator", "knowledge-session")
        .unwrap();
    storage
        .bind_session_knowledge(
            &op(),
            "test-workspace",
            "operator",
            "knowledge-session",
            "test-workspace",
            &sealed.snapshot_id,
        )
        .unwrap();
    let result = storage
        .retrieve_session_knowledge(
            &op(),
            "operator",
            "test-workspace",
            "knowledge-session",
            &["sku".into()],
            &["price".into()],
            None,
            4,
        )
        .unwrap();
    let fact_evidence = result
        .1
        .iter()
        .find(|e| e.evidence_id == "fact:sku:price:1")
        .unwrap();
    assert_eq!(fact_evidence.fact_versions[0].revision, "1");
    storage
        .publish_fact(&op(), "operator", &fact(), Some("1"))
        .unwrap();
    assert!(
        storage
            .retrieve_session_knowledge(
                &op(),
                "operator",
                "test-workspace",
                "knowledge-session",
                &["sku".into()],
                &["price".into()],
                None,
                4
            )
            .unwrap()
            .1
            .is_empty()
    );
    let mut candidate = cue(&sealed.snapshot_id, "latest-fact");
    candidate.evidence_refs = vec!["fact:sku:price:2".into()];
    assert_eq!(
        storage
            .record_candidate(
                &op(),
                "test-workspace",
                "operator",
                &candidate,
                &[("sku", "price", "2")]
            )
            .unwrap_err()
            .code,
        nai_contracts::v1::ErrorCode::EvidenceExpired
    );
}

#[test]
fn remote_delete_targets_are_kb_scoped_after_local_tombstone() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .import_document(&op(), "operator", &knowledge_doc("保修共享内容"))
        .unwrap();
    let main = publish_prepared(&mut storage, None);
    let other = publish_prepared_in_scope(&mut storage, None, "other-kb");
    let mut replacement = knowledge_doc("保修替换库内容");
    replacement.document_id = "replacement".into();
    for chunk in &mut replacement.chunks {
        chunk.chunk_id = chunk.chunk_id.replace("doc-kb", "replacement");
    }
    storage
        .import_document(&op(), "operator", &replacement)
        .unwrap();
    storage
        .stage_snapshot(
            &op(),
            "operator",
            "test-workspace",
            "replace",
            "test-workspace",
            &["replacement".into()],
            None,
            Some(&main.snapshot_id),
        )
        .unwrap();
    let sealed = storage
        .index_snapshot(&op(), "operator", "test-workspace", "replace")
        .unwrap();
    storage
        .publish_snapshot(&op(), "operator", &sealed)
        .unwrap();
    let targets = storage
        .remote_delete_targets(&op(), "test-workspace", "test-workspace", "operator")
        .unwrap();
    assert_eq!(targets[0].0, "doc-kb");
    assert!(
        storage
            .remote_delete_targets(&op(), "test-workspace", "other-kb", "operator")
            .unwrap()
            .is_empty()
    );
    storage
        .revoke_knowledge(&op(), "operator", "test-workspace", None, Some("doc-kb"))
        .unwrap();
    assert!(
        storage
            .remote_delete_targets(&op(), "test-workspace", "other-kb", "operator")
            .unwrap()
            .iter()
            .any(|(document, _)| document == "doc-kb")
    );
    let _ = other;
}
