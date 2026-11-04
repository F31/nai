use super::*;
fn prepared() -> (TempDir, Store, FoundationResult) {
    let (t, mut s) = init();
    let report = report(&mut s, 85, None);
    let snapshot = exec(&mut s, 1, publish(90, &report)).unwrap();
    (t, s, snapshot)
}
fn ask(snapshot: &FoundationResult, n: u64, q: &str) -> C {
    C::AskFaq(FoundationFaqRequest {
        workspace_id: id(10),
        task_id: id(n),
        snapshot_id: id(90),
        snapshot_sha256: snapshot.sha256.clone(),
        question: q.into(),
    })
}
fn read_task(n: u64) -> C {
    C::ReadFaq(FoundationFaqRead {
        workspace_id: id(10),
        task_id: id(n),
    })
}
fn resolve(task: &FoundationResult, decision: FaqDecision) -> C {
    C::ResolveFaq(FoundationFaqResolve {
        workspace_id: id(10),
        task_id: task.resource_id.clone(),
        task_sha256: task.sha256.clone(),
        decision: decision as i32,
        edited_text: String::new(),
        acknowledge_edit: false,
    })
}
#[test]
fn grounded_task_adoption_is_once_and_changes_draft_only() {
    let (_t, mut s, snapshot) = prepared();
    let command = command(ask(&snapshot, 91, "容量 多大?"));
    let task = s
        .execute(&Operation::new(4000).unwrap(), &id(1), &command)
        .unwrap();
    let body: Value = serde_json::from_str(&task.body).unwrap();
    assert_eq!(task.task_state, AgentTaskState::Completed as i32);
    assert_eq!(task.suggestion_state, SuggestionState::Proposed as i32);
    assert_eq!(body["provider_calls"], 0);
    assert_eq!(body["api_cost_minor"], 0);
    assert_eq!(body["evidence"][0]["conditions"], "标准款");
    let adopted = exec(&mut s, 1, resolve(&task, FaqDecision::Accept)).unwrap();
    assert_eq!(adopted.adopted_object_id, id(80));
    assert_eq!(adopted.adopted_revision, 2);
    assert_eq!(adopted.suggestion_state, SuggestionState::Accepted as i32);
    assert!(adopted.body.is_empty());
    assert_eq!(
        exec(&mut s, 1, resolve(&task, FaqDecision::Accept)),
        Err(Error::RevisionConflict)
    );
    let current = exec(
        &mut s,
        1,
        C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(80),
            revision: 0,
        }),
    )
    .unwrap();
    let p: Value = serde_json::from_str(&current.body).unwrap();
    assert_eq!(p["talk_tracks"].as_array().unwrap().len(), 2);
    assert_eq!(p["talk_tracks"][1]["text"], body["text"]);
    assert_eq!(read(&mut s, 90).sha256, snapshot.sha256);
    let replay = s
        .execute(&Operation::new(4000).unwrap(), &id(1), &command)
        .unwrap();
    assert_eq!(replay.suggestion_state, SuggestionState::Accepted as i32);
    assert!(replay.body.is_empty());
}
#[test]
fn unknown_question_cannot_be_adopted_and_cancel_survives_restart() {
    let (t, mut s, snapshot) = prepared();
    let task = exec(&mut s, 1, ask(&snapshot, 91, "能治疗疾病吗？")).unwrap();
    assert_eq!(task.faq_reason, FaqReason::NoMatch as i32);
    assert_eq!(task.suggestion_state, SuggestionState::Unspecified as i32);
    assert_eq!(
        exec(&mut s, 1, resolve(&task, FaqDecision::Accept)),
        Err(Error::CapabilityUnavailable)
    );
    exec(&mut s, 1, resolve(&task, FaqDecision::Cancel)).unwrap();
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    let current = exec(&mut s, 1, read_task(91)).unwrap();
    assert_eq!(current.task_state, AgentTaskState::Completed as i32);
    assert_eq!(current.suggestion_state, SuggestionState::Rejected as i32);
    assert_eq!(current.faq_reason, FaqReason::Cancelled as i32);
    assert!(current.body.is_empty());
}
#[test]
fn product_revision_fact_revocation_expiry_and_role_changes_invalidate_suggestions() {
    for case in 0..16 {
        let change = case % 8;
        let question = if case < 8 {
            "容量多大"
        } else {
            "这个杯子的容量有多大？"
        };
        let (_t, mut s, snapshot) = prepared();
        let task = exec(&mut s, 1, ask(&snapshot, 91, question)).unwrap();
        let v: Value = serde_json::from_str(&task.body).unwrap();
        match change {
            0 => {
                save(&mut s, 80, 1, serde_json::from_str(PRODUCT_DRAFT).unwrap());
            }
            1 => {
                exec(
                    &mut s,
                    1,
                    C::RevokePublication(FoundationPublicationRevoke {
                        workspace_id: id(10),
                        snapshot_id: id(90),
                        snapshot_sha256: snapshot.sha256.clone(),
                        revocation_id: id(92),
                        reason: "错误事实".into(),
                    }),
                )
                .unwrap();
            }
            2 => {}
            3 => {
                s.conn.execute("UPDATE roles SET revision=revision+1 WHERE workspace_id=?1 AND principal_id=?2",params![id(10),id(1)]).unwrap();
            }
            _ => (),
        }
        if (4..=6).contains(&change) {
            let target = match change {
                4 => 83,
                5 => 81,
                _ => 82,
            };
            let saved = exec(
                &mut s,
                1,
                C::ReadObject(FoundationReadObject {
                    workspace_id: id(10),
                    object_id: id(target),
                    revision: 0,
                }),
            )
            .unwrap();
            let mut doc: Value = serde_json::from_str(&saved.body).unwrap();
            doc["name"] = "修改后的备播上下文".into();
            save(&mut s, target, 1, doc);
        } else if change == 7 {
            s.fence.stop();
        }
        let now = if change == 2 {
            v["expires_at_ms"].as_i64().unwrap()
        } else {
            utc().unwrap()
        };
        let current = crate::assistance::read(
            &s.conn,
            &s.root,
            &s.fence,
            &Operation::new(4000).unwrap(),
            &id(1),
            &id(10),
            &id(91),
            now,
        )
        .unwrap();
        assert!(current.body.is_empty());
        assert_eq!(current.suggestion_state, SuggestionState::Expired as i32);
        if change != 2 {
            assert!(exec(&mut s, 1, resolve(&task, FaqDecision::Accept)).is_err());
        }
        exec(&mut s, 1, resolve(&task, FaqDecision::Ignore)).unwrap();
    }
}
#[test]
fn audit_failure_rolls_back_adoption_and_terminal_then_edit_requires_confirmation() {
    let (_t, mut s, snapshot) = prepared();
    let task = exec(&mut s, 1, ask(&snapshot, 91, "容量多大")).unwrap();
    s.conn.execute_batch("CREATE TRIGGER fail_faq BEFORE INSERT ON audit_events WHEN NEW.action='faq_resolve' BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert_eq!(
        exec(&mut s, 1, resolve(&task, FaqDecision::Accept)),
        Err(Error::IoFailed)
    );
    assert_eq!(object(&s.conn, &id(10), &id(80), 0).unwrap().0, 1);
    assert_eq!(
        exec(&mut s, 1, read_task(91)).unwrap().suggestion_state,
        SuggestionState::Proposed as i32
    );
    s.conn.execute_batch("DROP TRIGGER fail_faq").unwrap();
    let C::ResolveFaq(mut edit) = resolve(&task, FaqDecision::Edit) else {
        unreachable!()
    };
    edit.edited_text = "标准款容量500毫升，请核对适用型号。".into();
    assert_eq!(
        exec(&mut s, 1, C::ResolveFaq(edit.clone())),
        Err(Error::InvalidArgument)
    );
    edit.acknowledge_edit = true;
    let edited = exec(&mut s, 1, C::ResolveFaq(edit)).unwrap();
    assert_eq!(edited.suggestion_state, SuggestionState::Edited as i32);
    assert_eq!(edited.adopted_revision, 2);
}
#[test]
fn access_hash_limit_and_direct_dispatch_cannot_bypass_task_rules() {
    let (_t, mut s, snapshot) = prepared();
    let task = exec(&mut s, 1, ask(&snapshot, 91, "容量多大")).unwrap();
    assert_eq!(exec(&mut s, 2, read_task(91)), Err(Error::PermissionDenied));
    exec(
        &mut s,
        1,
        C::Grant(FoundationGrant {
            workspace_id: id(10),
            principal_id: id(2),
            role: FoundationRole::Viewer as i32,
            expected_revision: 0,
        }),
    )
    .unwrap();
    assert_eq!(exec(&mut s, 2, read_task(91)), Err(Error::PermissionDenied));
    assert_eq!(
        exec(
            &mut s,
            2,
            C::ReadObject(FoundationReadObject {
                workspace_id: id(10),
                object_id: id(91),
                revision: 0
            })
        ),
        Err(Error::PermissionDenied)
    );
    let C::ResolveFaq(mut wrong) = resolve(&task, FaqDecision::Accept) else {
        unreachable!()
    };
    wrong.task_sha256 = "0".repeat(64);
    assert_eq!(
        exec(&mut s, 1, C::ResolveFaq(wrong)),
        Err(Error::RevisionConflict)
    );
    for n in 92..103 {
        exec(&mut s, 1, ask(&snapshot, n, "容量多大")).unwrap();
    }
    assert_eq!(
        exec(&mut s, 1, ask(&snapshot, 103, "容量多大")),
        Err(Error::ResourceExhausted)
    );
    assert!(nai_runtime_core::preparation::kind(&task.body).is_some());
    assert_eq!(
        exec(
            &mut s,
            1,
            C::PrepareDelivery(FoundationDelivery {
                workspace_id: id(10),
                delivery_id: id(110),
                session_id: id(30),
                object_id: id(91),
                object_revision: 1,
                object_sha256: task.sha256.clone(),
                target: FoundationTarget::PrivateCard as i32,
                scope: DeliveryScope::Private as i32,
                ttl_ms: 30000
            })
        ),
        Err(Error::CapabilityUnavailable)
    );
}

#[test]
fn retrieved_quote_adopts_only_to_draft_or_cancels_durably() {
    for cancel in [false, true] {
        let (t, mut s, snapshot) = prepared();
        let task = exec(&mut s, 1, ask(&snapshot, 91, "这个杯子的容量有多大？")).unwrap();
        assert_eq!(task.faq_reason, FaqReason::Retrieved as i32);
        assert_eq!(task.suggestion_state, SuggestionState::Proposed as i32);
        let body: Value = serde_json::from_str(&task.body).unwrap();
        assert_eq!(body["retrieval"]["strategy"], "faq-lexical-zh-v1");
        assert_eq!(body["retrieval"]["method"], "lexical");
        assert_eq!(body["provider_calls"], 0);
        assert_eq!(body["evidence"][0]["conditions"], "标准款");
        let result = exec(
            &mut s,
            1,
            resolve(
                &task,
                if cancel {
                    FaqDecision::Cancel
                } else {
                    FaqDecision::Accept
                },
            ),
        )
        .unwrap();
        assert!(result.body.is_empty());
        if !cancel {
            assert_eq!(result.adopted_revision, 2);
            assert_eq!(read(&mut s, 90).sha256, snapshot.sha256);
        }
        drop(s);
        let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
        let current = exec(&mut s, 1, read_task(91)).unwrap();
        assert!(current.body.is_empty());
        assert_eq!(
            current.faq_reason,
            if cancel {
                FaqReason::Cancelled
            } else {
                FaqReason::Resolved
            } as i32
        );
        assert_eq!(
            exec(&mut s, 1, resolve(&task, FaqDecision::Accept)),
            Err(Error::RevisionConflict)
        );
    }
}
