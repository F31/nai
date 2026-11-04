use super::*;
fn prepared() -> (TempDir, Store, FoundationResult) {
    let (t, mut s) = init();
    let report = report(&mut s, 85, None);
    let snapshot = exec(&mut s, 1, publish(90, &report)).unwrap();
    (t, s, snapshot)
}
fn scope(s: &FoundationResult) -> QuestionPoolRead {
    QuestionPoolRead {
        workspace_id: id(10),
        snapshot_id: id(90),
        snapshot_sha256: s.sha256.clone(),
    }
}
fn enqueue(s: &FoundationResult, n: u64, q: &str) -> C {
    C::EnqueueQuestion(QuestionPoolEnqueue {
        scope: Some(scope(s)),
        question_id: id(n),
        question: q.into(),
    })
}
fn entry(r: &FoundationResult) -> QuestionPoolEntry {
    r.question_pool.as_ref().unwrap().entries[0].clone()
}
fn run(e: &QuestionPoolEntry, n: u64) -> C {
    C::RunQuestion(QuestionPoolRun {
        workspace_id: id(10),
        question_id: e.id.clone(),
        expected_revision: e.revision,
        task_id: id(n),
    })
}
fn cancel(e: &QuestionPoolEntry) -> C {
    C::CancelQuestion(QuestionPoolCancel {
        workspace_id: id(10),
        question_id: e.id.clone(),
        expected_revision: e.revision,
    })
}
fn child(s: &mut Store, e: &QuestionPoolEntry) -> FoundationResult {
    exec(
        s,
        1,
        C::ReadFaq(FoundationFaqRead {
            workspace_id: id(10),
            task_id: e.task_id.clone(),
        }),
    )
    .unwrap()
}
fn accept(t: &FoundationResult) -> C {
    C::ResolveFaq(FoundationFaqResolve {
        workspace_id: id(10),
        task_id: t.resource_id.clone(),
        task_sha256: t.sha256.clone(),
        decision: FaqDecision::Accept as i32,
        ..Default::default()
    })
}
#[test]
fn question_pool_dedup_run_cancel_tree_and_restart() {
    let (t, mut s, snapshot) = prepared();
    let first = entry(&exec(&mut s, 1, enqueue(&snapshot, 100, "容量多大？")).unwrap());
    let merged = entry(&exec(&mut s, 1, enqueue(&snapshot, 101, "容量 多大?")).unwrap());
    assert_eq!(merged.id, first.id);
    assert_eq!(merged.occurrences, 2);
    assert_eq!(merged.expires_at_ms, first.expires_at_ms);
    assert_eq!(
        exec(&mut s, 1, run(&first, 102)),
        Err(Error::RevisionConflict)
    );
    let command = command(run(&merged, 102));
    let ready = entry(
        &s.execute(&Operation::new(4000).unwrap(), &id(1), &command)
            .unwrap(),
    );
    assert_eq!(ready.state, AgentTaskState::WaitingHuman as i32);
    let task = child(&mut s, &ready);
    let v: Value = serde_json::from_str(&task.body).unwrap();
    assert_eq!(v["parent_task"]["parent_id"], first.id);
    assert_eq!(v["provider_calls"], 0);
    assert!(v["expires_at_ms"].as_u64().unwrap() <= first.expires_at_ms);
    let replay = entry(
        &s.execute(&Operation::new(4000).unwrap(), &id(1), &command)
            .unwrap(),
    );
    assert_eq!(replay.task_id, ready.task_id);
    assert_eq!(
        exec(&mut s, 1, run(&ready, 103)),
        Err(Error::RevisionConflict)
    );
    let closed = entry(&exec(&mut s, 1, cancel(&ready)).unwrap());
    assert_eq!(closed.state, AgentTaskState::Cancelled as i32);
    assert!(exec(&mut s, 1, accept(&task)).is_err());
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    let rows = exec(&mut s, 1, C::ReadQuestionPool(scope(&snapshot))).unwrap();
    assert_eq!(entry(&rows).state, AgentTaskState::Cancelled as i32);
    let current = child(&mut s, &ready);
    assert!(current.body.is_empty());
    assert_eq!(current.faq_reason, FaqReason::Cancelled as i32);
}
#[test]
fn question_pool_adoption_changes_only_draft_and_does_not_get_undone() {
    let (_t, mut s, snapshot) = prepared();
    let q = entry(&exec(&mut s, 1, enqueue(&snapshot, 100, "容量多大")).unwrap());
    let ready = entry(&exec(&mut s, 1, run(&q, 102)).unwrap());
    let task = child(&mut s, &ready);
    let adopted = exec(&mut s, 1, accept(&task)).unwrap();
    assert_eq!(adopted.adopted_object_id, id(80));
    assert_eq!(adopted.adopted_revision, 2);
    // Source head changed by adoption; cancellation still closes the parent without undoing the draft.
    exec(&mut s, 1, cancel(&ready)).unwrap();
    let product = exec(
        &mut s,
        1,
        C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(80),
            revision: 0,
        }),
    )
    .unwrap();
    assert_eq!(product.revision, 2);
    assert_eq!(read(&mut s, 90).sha256, snapshot.sha256);
}
#[test]
fn question_pool_cancel_audit_failure_is_unconfirmed_and_rolls_back_both_nodes() {
    let (_t, mut s, snapshot) = prepared();
    let q = entry(&exec(&mut s, 1, enqueue(&snapshot, 100, "容量多大")).unwrap());
    let ready = entry(&exec(&mut s, 1, run(&q, 102)).unwrap());
    s.conn.execute_batch("CREATE TRIGGER fail_cancel BEFORE INSERT ON audit_events WHEN NEW.action='question_cancel' BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert!(exec(&mut s, 1, cancel(&ready)).is_err());
    let rows = exec(&mut s, 1, C::ReadQuestionPool(scope(&snapshot))).unwrap();
    assert_eq!(entry(&rows).revision, ready.revision);
    assert_eq!(entry(&rows).state, AgentTaskState::WaitingHuman as i32);
    assert_eq!(
        child(&mut s, &ready).suggestion_state,
        SuggestionState::Proposed as i32
    );
    s.conn.execute_batch("DROP TRIGGER fail_cancel;").unwrap();
    exec(&mut s, 1, cancel(&ready)).unwrap();
}
#[test]
fn question_pool_expiry_role_and_source_changes_block_children() {
    for case in 0..3 {
        let (_t, mut s, snapshot) = prepared();
        let q = entry(&exec(&mut s, 1, enqueue(&snapshot, 100, "容量多大")).unwrap());
        let ready = entry(&exec(&mut s, 1, run(&q, 102)).unwrap());
        let task = child(&mut s, &ready);
        let now = if case == 0 {
            q.expires_at_ms as i64
        } else {
            utc().unwrap()
        };
        if case == 1 {
            s.conn.execute("UPDATE roles SET revision=revision+1 WHERE workspace_id=?1 AND principal_id=?2",params![id(10),id(1)]).unwrap();
        }
        if case == 2 {
            save(&mut s, 80, 1, serde_json::from_str(PRODUCT_DRAFT).unwrap());
        }
        let current = crate::assistance::read(
            &s.conn,
            &s.root,
            &s.fence,
            &Operation::new(4000).unwrap(),
            &id(1),
            &id(10),
            &ready.task_id,
            now,
        )
        .unwrap();
        assert!(current.body.is_empty());
        assert_eq!(current.suggestion_state, SuggestionState::Expired as i32);
        if case != 0 {
            assert!(exec(&mut s, 1, accept(&task)).is_err());
        }
    }
}
#[test]
fn question_pool_is_bounded_and_cancelled_rows_still_count() {
    let (_t, mut s, snapshot) = prepared();
    for n in 0..32 {
        let q = entry(&exec(&mut s, 1, enqueue(&snapshot, 100 + n, &format!("问题{n}"))).unwrap());
        if n == 0 {
            exec(&mut s, 1, cancel(&q)).unwrap();
        }
    }
    assert_eq!(
        exec(&mut s, 1, enqueue(&snapshot, 200, "第33条")),
        Err(Error::ResourceExhausted)
    );
    let all = exec(&mut s, 1, C::ReadQuestionPool(scope(&snapshot))).unwrap();
    assert_eq!(all.question_pool.unwrap().entries.len(), 32);
    // Advancing virtual time drops the whole window; persisted history is not deleted.
    let later = crate::question_pool::read(
        &s.conn,
        &s.root,
        &s.fence,
        &Operation::new(4000).unwrap(),
        &id(1),
        &scope(&snapshot),
        utc().unwrap() + 120001,
    )
    .unwrap();
    assert!(later.question_pool.unwrap().entries.is_empty());
}
#[test]
fn question_pool_other_actor_cannot_read_parent_or_cancel_and_index_is_private() {
    let (_t, mut s, snapshot) = prepared();
    let q = entry(&exec(&mut s, 1, enqueue(&snapshot, 100, "容量多大")).unwrap());
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
    assert_eq!(exec(&mut s, 2, cancel(&q)), Err(Error::PermissionDenied));
    assert_eq!(
        exec(
            &mut s,
            2,
            C::ReadObject(FoundationReadObject {
                workspace_id: id(10),
                object_id: q.id.clone(),
                revision: 0
            })
        ),
        Err(Error::PermissionDenied)
    );
    let indices: Vec<String> = s
        .conn
        .prepare("SELECT id FROM objects WHERE workspace_id=?1 AND id!=?2")
        .unwrap()
        .query_map(params![id(10), q.id], |r| r.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    let mut checked = false;
    for object_id in indices {
        let (_, sha) = object(&s.conn, &id(10), &object_id, 0).unwrap();
        let Ok(v) = serde_json::from_str::<Value>(&body(&s.root, &sha).unwrap()) else {
            continue;
        };
        if v["kind"] == "publication_question_index" {
            assert_eq!(
                exec(
                    &mut s,
                    1,
                    C::ReadObject(FoundationReadObject {
                        workspace_id: id(10),
                        object_id,
                        revision: 0
                    })
                ),
                Err(Error::PermissionDenied)
            );
            checked = true;
        }
    }
    assert!(checked);
}
