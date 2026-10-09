use super::*;

#[test]
fn decision_failure_rolls_back_approval_and_reject_state() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .create_session(&op(), "test-workspace", "operator", "session-test")
        .unwrap();
    storage
        .publish_fact(&op(), "operator", &fact(), None)
        .unwrap();
    let hash = setup_approval(&mut storage, &candidate());
    storage.conn.execute_batch("CREATE TRIGGER fail_approval_decision BEFORE INSERT ON decision_log BEGIN SELECT RAISE(ABORT,'injected approval log failure'); END;").unwrap();
    assert!(
        storage
            .reject_candidate(
                &op(),
                "test-workspace",
                "operator",
                "session-test",
                0,
                &hash
            )
            .is_err()
    );
    assert_eq!(
        storage
            .list_pending_approvals(&op(), "test-workspace", "operator", "session-test", 0, 200)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        storage
            .conn
            .query_row(
                "SELECT COUNT(*) FROM audit_records WHERE action='approval.rejected'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    let approval = Approval {
        approval_id: "rollback-test".into(),
        session_id: "session-test".into(),
        candidate_hash: hash.clone(),
        hash_algorithm: "sha256-canonical-v1".into(),
        generation_epoch: 0,
        expires_at_utc_ms: candidate().expires_at_utc_ms,
        ..Default::default()
    };
    assert!(
        storage
            .approve(&op(), "test-workspace", "operator", &approval)
            .is_err()
    );
    assert!(
        !storage
            .approval_valid(&op(), "test-workspace", "session-test", 0, &hash)
            .unwrap()
    );
    storage
        .conn
        .execute_batch("DROP TRIGGER fail_approval_decision;")
        .unwrap();
    storage
        .reject_candidate(
            &op(),
            "test-workspace",
            "operator",
            "session-test",
            0,
            &hash,
        )
        .unwrap();
    storage
        .verify_decision_chain(&op(), "test-workspace")
        .unwrap();
}

#[test]
fn reopening_store_revokes_pending_approvals_instead_of_resuming_them() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .create_session(&op(), "test-workspace", "operator", "session-test")
        .unwrap();
    storage
        .publish_fact(&op(), "operator", &fact(), None)
        .unwrap();
    let hash = setup_approval(&mut storage, &candidate());
    drop(storage);
    let mut recovered = store(dir.path());
    assert!(
        recovered
            .list_pending_approvals(&op(), "test-workspace", "operator", "session-test", 0, 200)
            .unwrap()
            .is_empty()
    );
    assert!(
        recovered
            .reject_candidate(
                &op(),
                "test-workspace",
                "operator",
                "session-test",
                0,
                &hash
            )
            .is_err()
    );
}

fn setup_approval(store: &mut Store, cue: &PerformanceCue) -> String {
    store
        .record_candidate(
            &op(),
            "test-workspace",
            "operator",
            cue,
            &[("sku", "price", "1")],
        )
        .unwrap()
}

#[test]
fn list_read_and_reject_close_awaiting_workflow() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .create_session(&op(), "test-workspace", "operator", "session-test")
        .unwrap();
    storage
        .publish_fact(&op(), "operator", &fact(), None)
        .unwrap();
    let mut first = candidate();
    first.utterance_id = "approval-first".into();
    let hash_first = setup_approval(&mut storage, &first);
    let mut second = candidate();
    second.utterance_id = "approval-second".into();
    let hash_second = setup_approval(&mut storage, &second);
    let pending = storage
        .list_pending_approvals(&op(), "test-workspace", "operator", "session-test", 0, 200)
        .unwrap();
    assert_eq!(pending.len(), 2);
    assert!(pending.iter().all(|p| p.approval_id.is_none()));
    let read = storage
        .read_approval(
            &op(),
            "test-workspace",
            "operator",
            "session-test",
            0,
            &hash_first,
        )
        .unwrap();
    assert_eq!(read.candidate_hash, hash_first);
    assert!(read.evidence_count >= 1);
    assert_eq!(
        storage
            .list_pending_approvals(&op(), "test-workspace", "operator", "session-test", 0, 1)
            .unwrap()
            .len(),
        1
    );
    assert!(
        storage
            .list_pending_approvals(&op(), "test-workspace", "operator", "session-test", 0, 201)
            .is_err()
    );
    assert!(
        storage
            .list_pending_approvals(&op(), "test-workspace", "viewer", "session-test", 0, 200)
            .is_err()
    );
    // Reject the first, then it disappears and cannot be listed.
    storage
        .reject_candidate(
            &op(),
            "test-workspace",
            "operator",
            "session-test",
            0,
            &hash_first,
        )
        .unwrap();
    let status: i32 = storage
        .conn
        .query_row(
            "SELECT status FROM utterances WHERE workspace_id='test-workspace' AND text_hash=?1",
            [&hash_first],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, 14);
    assert_eq!(
        storage
            .list_pending_approvals(&op(), "test-workspace", "operator", "session-test", 0, 200)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        storage
            .list_pending_approvals(&op(), "test-workspace", "operator", "session-test", 0, 200)
            .unwrap()[0]
            .candidate_hash,
        hash_second
    );
    // Double reject fails; reject on a missing candidate also fails.
    assert!(
        storage
            .reject_candidate(
                &op(),
                "test-workspace",
                "operator",
                "session-test",
                0,
                &hash_first
            )
            .is_err()
    );
    assert!(
        storage
            .reject_candidate(
                &op(),
                "test-workspace",
                "operator",
                "session-test",
                0,
                "deadbeef"
            )
            .is_err()
    );
}

#[test]
fn approved_candidates_cannot_be_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .publish_fact(&op(), "operator", &fact(), None)
        .unwrap();
    let (cue, hash) = approved(&mut storage);
    let pending = storage
        .list_pending_approvals(&op(), "test-workspace", "operator", &cue.session_id, 0, 200)
        .unwrap();
    assert!(
        pending.is_empty(),
        "approved candidate is filtered from pending"
    );
    assert!(pending.is_empty());
    assert!(
        storage
            .reject_candidate(
                &op(),
                "test-workspace",
                "operator",
                "session-test",
                0,
                &hash
            )
            .is_err()
    );
    assert_eq!(
        storage
            .reject_candidate(
                &op(),
                "test-workspace",
                "operator",
                "session-test",
                0,
                &hash
            )
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
}

#[test]
fn list_is_bounded_and_filters_expired_wrong_epoch_and_updated_facts() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .create_session(&op(), "test-workspace", "operator", "session-test")
        .unwrap();
    storage
        .publish_fact(&op(), "operator", &fact(), None)
        .unwrap();
    for index in 0..200 {
        let mut cue = candidate();
        cue.utterance_id = format!("bounded-{index}");
        setup_approval(&mut storage, &cue);
    }
    let overflow = PerformanceCue {
        utterance_id: "bounded-overflow".into(),
        ..candidate()
    };
    assert_eq!(
        storage
            .record_candidate(
                &op(),
                "test-workspace",
                "operator",
                &overflow,
                &[("sku", "price", "1")]
            )
            .unwrap_err()
            .code,
        ErrorCode::ResourceExhausted
    );
    assert_eq!(
        storage
            .list_pending_approvals(&op(), "test-workspace", "operator", "session-test", 0, 200)
            .unwrap()
            .len(),
        200
    );
    assert!(
        storage
            .list_pending_approvals(&op(), "test-workspace", "operator", "session-test", 1, 200)
            .unwrap()
            .is_empty()
    );
    assert!(
        storage
            .list_pending_approvals(&op(), "test-workspace", "operator", "session-test", 0, 0)
            .is_err()
    );
    storage
        .conn
        .execute(
            "UPDATE utterances SET expires_at_ms=1 WHERE utterance_id='bounded-0'",
            [],
        )
        .unwrap();
    assert!(
        storage
            .reject_candidate(
                &op(),
                "test-workspace",
                "operator",
                "session-test",
                0,
                &candidate_hash(&PerformanceCue {
                    utterance_id: "bounded-0".into(),
                    ..candidate()
                })
            )
            .is_err()
    );
    storage
        .publish_fact(&op(), "operator", &fact(), Some("1"))
        .unwrap();
    assert!(
        storage
            .list_pending_approvals(&op(), "test-workspace", "operator", "session-test", 0, 200)
            .unwrap()
            .is_empty()
    );
}
