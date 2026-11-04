use super::*;
fn setup_review() -> (TempDir, Store, ReviewSource) {
    let (t, mut s) = init();
    let r = report(&mut s, 85, None);
    let snap = exec(&mut s, 1, publish(90, &r)).unwrap();
    let m: Value = serde_json::from_str(&snap.body).unwrap();
    let mut doc = json!({"schema_version":1,"kind":"manual_run_draft","name":"本场人工记录","show":m["refs"]["show"],"snapshot":pointer(90,&snap),"cue_ids":["intro"],"cursor":0,"status":"open","marks":[],"mode":"private_manual","evidence":"self_reported"});
    save(&mut s, 94, 0, doc.clone());
    doc["status"] = "ended".into();
    save(&mut s, 94, 1, doc);
    (
        t,
        s,
        ReviewSource {
            workspace_id: id(10),
            run_id: id(94),
            run_kind: ReviewRunKind::Manual as i32,
        },
    )
}
fn ctx(s: &mut Store, source: &ReviewSource) -> ReviewContext {
    exec(s, 1, C::ReadReview(source.clone()))
        .unwrap()
        .review_context
        .unwrap()
}
fn note(source: &ReviewSource, n: u64, revision: u64) -> C {
    C::SaveReviewNote(ReviewNoteSave {
        question_source: None,
        source: Some(source.clone()),
        note_id: id(n),
        expected_revision: revision,
        title: "容量问题需要说清规格".into(),
        proposal: Some(ReviewFaq {
            id: "capacity_question".into(),
            question: "标准款容量是多少？".into(),
            text: "标准款容量为500毫升。".into(),
            fact_ids: vec!["capacity".into()],
        }),
    })
}
fn plan(c: &ReviewContext, n: u64, job: u64) -> C {
    C::PlanReviewDraft(ReviewDraftPlan {
        source: c.source.clone(),
        note_id: id(n),
        note_revision: c
            .notes
            .iter()
            .find(|v| v.note_id == id(n))
            .unwrap()
            .revision,
        job_id: id(job),
        target_product_id: id(job + 1),
        target_show_id: id(job + 2),
        expected_product_revision: c.product.as_ref().unwrap().revision,
        expected_show_revision: c.show.as_ref().unwrap().revision,
    })
}
fn apply(j: &ReviewDraftView) -> C {
    C::ApplyReviewDraft(ReviewDraftApply {
        workspace_id: id(10),
        job_id: j.job_id.clone(),
        expected_revision: j.revision,
        expected_sha256: j.sha256.clone(),
        acknowledge: true,
    })
}
fn get(s: &mut Store, n: u64) -> Result<FoundationResult> {
    exec(
        s,
        1,
        C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(n),
            revision: 0,
        }),
    )
}
fn prepared() -> (TempDir, Store, ReviewSource, ReviewDraftView) {
    let (t, mut s, src) = setup_review();
    exec(&mut s, 1, note(&src, 1001, 0)).unwrap();
    let c = ctx(&mut s, &src);
    let j = exec(&mut s, 1, plan(&c, 1001, 1010))
        .unwrap()
        .review_draft
        .unwrap();
    (t, s, src, j)
}
#[test]
fn review_draft_atomic_pair_is_private_and_business_retries_do_not_duplicate() {
    let (_t, mut s, src, j) = prepared();
    assert_eq!(j.state, "preview");
    assert_eq!(get(&mut s, 1011).unwrap_err(), Error::NotFound);
    let original = get(&mut s, 80).unwrap();
    let r = exec(&mut s, 1, apply(&j)).unwrap().review_draft.unwrap();
    assert_eq!(r.state, "applied");
    let product = get(&mut s, 1011).unwrap();
    let show = get(&mut s, 1012).unwrap();
    let p: Value = serde_json::from_str(&product.body).unwrap();
    let sh: Value = serde_json::from_str(&show.body).unwrap();
    assert_eq!(p["faq"].as_array().unwrap().len(), 1);
    assert_eq!(p["faq"][0]["question"], "标准款容量是多少？");
    assert_eq!(sh["product"], pointer(1011, &product));
    assert_eq!(get(&mut s, 80).unwrap().body, original.body);
    assert_eq!(
        exec(&mut s, 1, apply(&j))
            .unwrap()
            .review_draft
            .unwrap()
            .saved_product,
        r.saved_product
    );
    let c = ctx(&mut s, &src);
    let again = exec(&mut s, 1, plan(&c, 1001, 1020))
        .unwrap()
        .review_draft
        .unwrap();
    assert_eq!(again.job_id, j.job_id);
    assert_eq!(get(&mut s, 1021).unwrap_err(), Error::NotFound);
    assert!(p.get("approval").is_none());
    assert_eq!(
        p["facts"],
        serde_json::from_str::<Value>(&original.body).unwrap()["facts"]
    );
}
#[test]
fn review_concurrent_change_returns_three_way_and_does_not_create_targets() {
    let (_t, mut s, src, j) = prepared();
    let current = get(&mut s, 80).unwrap();
    let mut p: Value = serde_json::from_str(&current.body).unwrap();
    p["faq"][0]["text"] = "人工并发修改。".into();
    save(&mut s, 80, current.revision, p);
    let r = exec(&mut s, 1, apply(&j)).unwrap().review_draft.unwrap();
    assert_eq!(r.state, "conflict");
    assert_eq!(r.reason, "DRAFT_CHANGED");
    assert_eq!(r.current.unwrap().text, "人工并发修改。");
    assert_eq!(r.before.unwrap().text, "标准款容量为500毫升。");
    assert_eq!(get(&mut s, 1011).unwrap_err(), Error::NotFound);
    let c = ctx(&mut s, &src);
    let fresh = exec(&mut s, 1, plan(&c, 1001, 1030))
        .unwrap()
        .review_draft
        .unwrap();
    assert_eq!(fresh.state, "preview");
    assert_eq!(
        exec(&mut s, 1, apply(&fresh))
            .unwrap()
            .review_draft
            .unwrap()
            .state,
        "applied"
    );
    assert_eq!(get(&mut s, 80).unwrap().revision, 2);
}
#[test]
fn review_audit_failure_rolls_back_both_drafts_and_job_state() {
    let (_t, mut s, _src, j) = prepared();
    s.conn.execute_batch("CREATE TRIGGER fail_review BEFORE INSERT ON audit_events WHEN NEW.action='review_draft_apply' BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert_eq!(exec(&mut s, 1, apply(&j)).unwrap_err(), Error::IoFailed);
    assert_eq!(get(&mut s, 1011).unwrap_err(), Error::NotFound);
    assert_eq!(get(&mut s, 1012).unwrap_err(), Error::NotFound);
    let r = exec(
        &mut s,
        1,
        C::ReadReviewDraft(ReviewDraftRead {
            workspace_id: id(10),
            job_id: j.job_id,
        }),
    )
    .unwrap()
    .review_draft
    .unwrap();
    assert_eq!(r.state, "preview");
}
#[test]
fn review_revoked_deleted_or_cross_actor_sources_do_not_replay_old_text() {
    for mode in 0..3 {
        let (_t, mut s, src, j) = prepared();
        match mode {
            0 => {
                let snapshot_sha256 = ctx(&mut s, &src).snapshot_sha256;
                exec(
                    &mut s,
                    1,
                    C::RevokePublication(FoundationPublicationRevoke {
                        workspace_id: id(10),
                        revocation_id: id(1090),
                        snapshot_id: id(90),
                        snapshot_sha256,
                        reason: "撤销证据".into(),
                    }),
                )
                .unwrap();
            }
            1 => {
                exec(
                    &mut s,
                    1,
                    C::DeleteObject(FoundationDelete {
                        workspace_id: id(10),
                        object_id: id(94),
                        expected_revision: 2,
                    }),
                )
                .unwrap();
            }
            _ => {
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
            }
        }
        let read = C::ReadReviewDraft(ReviewDraftRead {
            workspace_id: id(10),
            job_id: j.job_id.clone(),
        });
        assert!(exec(&mut s, if mode == 2 { 2 } else { 1 }, read).is_err());
        assert!(get(&mut s, 1011).is_err());
        assert!(
            exec(
                &mut s,
                1,
                C::ReadObject(FoundationReadObject {
                    workspace_id: id(10),
                    object_id: id(1010),
                    revision: 1
                })
            )
            .is_err()
        );
    }
}
#[test]
fn review_facts_changed_and_missing_ack_do_not_apply() {
    let (_t, mut s, src, j) = prepared();
    let mut no_ack = apply(&j);
    if let C::ApplyReviewDraft(x) = &mut no_ack {
        x.acknowledge = false;
    }
    assert_eq!(
        exec(&mut s, 1, no_ack).unwrap_err(),
        Error::PermissionDenied
    );
    let p = get(&mut s, 80).unwrap();
    let mut v: Value = serde_json::from_str(&p.body).unwrap();
    v["facts"][0]["value"] = "600".into();
    save(&mut s, 80, p.revision, v);
    let r = exec(&mut s, 1, apply(&j)).unwrap().review_draft.unwrap();
    assert_eq!(r.reason, "FACTS_CHANGED");
    assert_eq!(r.state, "conflict");
    assert!(!ctx(&mut s, &src).facts_unchanged);
    assert_eq!(get(&mut s, 1011).unwrap_err(), Error::NotFound);
}
#[test]
fn review_note_change_invalidates_preview_and_reused_job_cannot_change_targets() {
    let (_t, mut s, src, j) = prepared();
    let c = ctx(&mut s, &src);
    let mut reused = plan(&c, 1001, 1010);
    if let C::PlanReviewDraft(x) = &mut reused {
        x.target_show_id = id(1099);
    }
    assert_eq!(
        exec(&mut s, 1, reused).unwrap_err(),
        Error::RevisionConflict
    );
    let mut changed = note(&src, 1001, 1);
    if let C::SaveReviewNote(x) = &mut changed {
        x.proposal.as_mut().unwrap().question = "容量请再明确一下？".into();
    }
    exec(&mut s, 1, changed).unwrap();
    let blocked = exec(&mut s, 1, apply(&j)).unwrap().review_draft.unwrap();
    assert_eq!(blocked.reason, "NOTE_CHANGED");
    assert_eq!(blocked.state, "conflict");
    assert_eq!(get(&mut s, 1011).unwrap_err(), Error::NotFound);
}
#[test]
fn review_second_target_collision_rolls_back_first_target() {
    let (_t, mut s, _src, j) = prepared();
    let p = get(&mut s, 80).unwrap();
    save(&mut s, 1012, 0, serde_json::from_str(&p.body).unwrap());
    assert_eq!(
        exec(&mut s, 1, apply(&j)).unwrap_err(),
        Error::AlreadyExists
    );
    assert_eq!(get(&mut s, 1011).unwrap_err(), Error::NotFound);
    let read = exec(
        &mut s,
        1,
        C::ReadReviewDraft(ReviewDraftRead {
            workspace_id: id(10),
            job_id: j.job_id,
        }),
    )
    .unwrap()
    .review_draft
    .unwrap();
    assert_eq!(read.state, "preview");
    assert!(read.saved_product.is_none());
}
#[test]
fn review_note_bound_source_must_be_ended_and_caps_three_notes() {
    let (_t, mut s, src) = setup_review();
    let current = get(&mut s, 94).unwrap();
    let mut v: Value = serde_json::from_str(&current.body).unwrap();
    v["status"] = "open".into();
    let id_new = 95;
    save(&mut s, id_new, 0, v);
    let mut open = src.clone();
    open.run_id = id(id_new);
    assert_eq!(
        exec(&mut s, 1, C::ReadReview(open)).unwrap_err(),
        Error::CapabilityUnavailable
    );
    for n in 1001..1004 {
        exec(&mut s, 1, note(&src, n, 0)).unwrap();
    }
    assert_eq!(
        exec(&mut s, 1, note(&src, 1004, 0)).unwrap_err(),
        Error::ResourceExhausted
    );
    assert_eq!(ctx(&mut s, &src).notes.len(), 3);
}

fn question_review_open(s: &mut Store, src: &ReviewSource) -> FoundationResult {
    let r = get(s, 94).unwrap();
    let mut d: Value = serde_json::from_str(&r.body).unwrap();
    d["status"] = "open".into();
    save(s, 95, 0, d);
    let _ = src;
    get(s, 90).unwrap()
}
fn question_review_source() -> ReviewSource {
    ReviewSource {
        workspace_id: id(10),
        run_id: id(95),
        run_kind: ReviewRunKind::Manual as i32,
    }
}
fn question_review_enqueue(snapshot: &FoundationResult, n: u64) -> C {
    C::EnqueueRunQuestion(ReviewRunQuestion {
        source: Some(question_review_source()),
        expected_epoch: 0,
        question: Some(QuestionPoolEnqueue {
            scope: Some(QuestionPoolRead {
                workspace_id: id(10),
                snapshot_id: id(90),
                snapshot_sha256: snapshot.sha256.clone(),
            }),
            question_id: id(n),
            question: "容量多大？".into(),
        }),
    })
}
fn question_review_end(s: &mut Store) {
    let r = get(s, 95).unwrap();
    let mut d: Value = serde_json::from_str(&r.body).unwrap();
    d["status"] = "ended".into();
    save(s, 95, r.revision, d);
}
#[test]
fn question_review_run_binding_dedup_and_atomic_note_draft() {
    let (_t, mut s, src) = setup_review();
    let snap = question_review_open(&mut s, &src);
    let first = exec(&mut s, 1, question_review_enqueue(&snap, 1100))
        .unwrap()
        .question_pool
        .unwrap()
        .entries
        .remove(0);
    let next = exec(&mut s, 1, question_review_enqueue(&snap, 1101))
        .unwrap()
        .question_pool
        .unwrap()
        .entries
        .remove(0);
    assert_eq!(first.id, next.id);
    assert_eq!(next.occurrences, 2);
    let private = exec(
        &mut s,
        1,
        C::ReadQuestionPool(QuestionPoolRead {
            workspace_id: id(10),
            snapshot_id: id(90),
            snapshot_sha256: snap.sha256.clone(),
        }),
    )
    .unwrap();
    assert!(private.question_pool.unwrap().entries.is_empty());
    let r = exec(
        &mut s,
        1,
        C::RunQuestion(QuestionPoolRun {
            workspace_id: id(10),
            question_id: next.id.clone(),
            expected_revision: next.revision,
            task_id: id(1102),
        }),
    )
    .unwrap();
    assert_eq!(
        r.question_pool.unwrap().entries[0].state,
        AgentTaskState::WaitingHuman as i32
    );
    question_review_end(&mut s);
    let source = question_review_source();
    let c = ctx(&mut s, &source);
    assert_eq!(c.questions.len(), 1);
    let q = &c.questions[0];
    assert_eq!(q.processing, "waiting_human");
    assert_eq!(q.delivery, "unconfirmed");
    assert_eq!(q.question, "容量多大？");
    assert_eq!(q.occurrences, 2);
    assert!(exec(&mut s, 1, question_review_enqueue(&snap, 1103)).is_err());
    let mut n = note(&source, 1110, 0);
    if let C::SaveReviewNote(v) = &mut n {
        v.question_source = q.record.clone();
    }
    let c = exec(&mut s, 1, n.clone()).unwrap().review_context.unwrap();
    assert_eq!(c.notes[0].question_source, c.questions[0].record);
    if let C::SaveReviewNote(v) = &mut n {
        v.note_id = id(1111);
    }
    assert_eq!(exec(&mut s, 1, n), Err(Error::AlreadyExists));
    let j = exec(&mut s, 1, plan(&c, 1110, 1120))
        .unwrap()
        .review_draft
        .unwrap();
    assert_eq!(
        exec(&mut s, 1, apply(&j))
            .unwrap()
            .review_draft
            .unwrap()
            .state,
        "applied"
    );
}
#[test]
fn question_review_audit_failure_rolls_back_parent_and_evidence() {
    let (_t, mut s, src) = setup_review();
    let snap = question_review_open(&mut s, &src);
    s.conn.execute_batch("CREATE TRIGGER fail_question_review BEFORE INSERT ON audit_events WHEN NEW.action='question_enqueue_for_run' BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert_eq!(
        exec(&mut s, 1, question_review_enqueue(&snap, 1130)),
        Err(Error::IoFailed)
    );
    assert_eq!(get(&mut s, 1130), Err(Error::NotFound));
    s.conn
        .execute_batch("DROP TRIGGER fail_question_review")
        .unwrap();
    question_review_end(&mut s);
    assert!(ctx(&mut s, &question_review_source()).questions.is_empty());
}
#[test]
fn question_review_replay_cannot_return_question_after_run_ends() {
    let (_t, mut s, src) = setup_review();
    let snap = question_review_open(&mut s, &src);
    let c = command(question_review_enqueue(&snap, 1140));
    s.execute(&Operation::new(4000).unwrap(), &id(1), &c)
        .unwrap();
    question_review_end(&mut s);
    assert!(
        s.execute(&Operation::new(4000).unwrap(), &id(1), &c)
            .is_err()
    );
    let c = ctx(&mut s, &question_review_source());
    assert_eq!(c.questions.len(), 1);
    assert_eq!(c.questions[0].processing, "queued");
    assert!(
        exec(
            &mut s,
            1,
            C::ReadObject(FoundationReadObject {
                workspace_id: id(10),
                object_id: c.questions[0].record.as_ref().unwrap().object_id.clone(),
                revision: 0
            })
        )
        .is_err()
    );
}

#[test]
fn question_review_cross_run_and_epoch_are_not_inferred_from_snapshot() {
    let (_t, mut s, src) = setup_review();
    let snap = question_review_open(&mut s, &src);
    let q = exec(&mut s, 1, question_review_enqueue(&snap, 1150))
        .unwrap()
        .resource_id;
    let r = get(&mut s, 95).unwrap();
    save(&mut s, 96, 0, serde_json::from_str(&r.body).unwrap());
    let mut other = question_review_enqueue(&snap, 1151);
    if let C::EnqueueRunQuestion(x) = &mut other {
        x.source.as_mut().unwrap().run_id = id(96);
    }
    let second = exec(&mut s, 1, other).unwrap();
    assert_ne!(q, second.resource_id);
    let mut stale = question_review_enqueue(&snap, 1152);
    if let C::EnqueueRunQuestion(x) = &mut stale {
        x.expected_epoch = 1;
    }
    assert_eq!(exec(&mut s, 1, stale), Err(Error::RevisionConflict));
    let mut wrong = question_review_enqueue(&snap, 1153);
    if let C::EnqueueRunQuestion(x) = &mut wrong {
        x.question
            .as_mut()
            .unwrap()
            .scope
            .as_mut()
            .unwrap()
            .workspace_id = id(99);
    }
    assert!(exec(&mut s, 1, wrong).is_err());
    question_review_end(&mut s);
    let c = ctx(&mut s, &question_review_source());
    assert_eq!(c.questions.len(), 1);
    assert_eq!(c.questions[0].question_id, q);
    assert!(exec(&mut s, 2, C::ReadReview(question_review_source())).is_err());
    let p = get(&mut s, 90).unwrap();
    exec(
        &mut s,
        1,
        C::RevokePublication(FoundationPublicationRevoke {
            workspace_id: id(10),
            revocation_id: id(1199),
            snapshot_id: id(90),
            snapshot_sha256: p.sha256,
            reason: "撤销".into(),
        }),
    )
    .unwrap();
    assert!(exec(&mut s, 1, C::ReadReview(question_review_source())).is_err());
}

#[test]
fn question_review_late_receipt_invalidates_note_and_draft_without_recovering_answer() {
    let (_t, mut s, src) = setup_review();
    let snap = question_review_open(&mut s, &src);
    exec(&mut s, 1, question_review_enqueue(&snap, 1160)).unwrap();
    question_review_end(&mut s);
    let source = question_review_source();
    let c = ctx(&mut s, &source);
    let record = c.questions[0].record.as_ref().unwrap().clone();
    let mut n = note(&source, 1161, 0);
    if let C::SaveReviewNote(x) = &mut n {
        x.question_source = Some(record.clone());
    }
    let c = exec(&mut s, 1, n.clone()).unwrap().review_context.unwrap();
    let j = exec(&mut s, 1, plan(&c, 1161, 1170))
        .unwrap()
        .review_draft
        .unwrap();
    let (mut v, r, _) = crate::llm::raw(
        &s.conn,
        &s.root,
        &id(10),
        &record.object_id,
        0,
        "publication_review_question",
    )
    .unwrap();
    assert!(v.get("answer").is_none());
    assert!(v.get("text").is_none());
    v["delivery"] = "submitted".into();
    let tx = s.conn.transaction().unwrap();
    crate::llm::put(&tx, &s.root, &id(10), &record.object_id, r, &v, 1_000).unwrap();
    tx.commit().unwrap();
    let blocked = exec(&mut s, 1, apply(&j)).unwrap().review_draft.unwrap();
    assert_eq!(blocked.reason, "QUESTION_CHANGED");
    assert_eq!(get(&mut s, 1171), Err(Error::NotFound));
    if let C::SaveReviewNote(x) = &mut n {
        x.expected_revision = 1;
    }
    assert_eq!(exec(&mut s, 1, n), Err(Error::RevisionConflict));
    let c = ctx(&mut s, &source);
    assert_eq!(c.questions[0].delivery, "submitted");
    assert_eq!(c.notes[0].question_source, Some(record));
}

#[test]
fn question_review_runtime_expiry_retains_only_original_question_for_review() {
    let (_t, mut s, src) = setup_review();
    let snap = question_review_open(&mut s, &src);
    let e = exec(&mut s, 1, question_review_enqueue(&snap, 1180))
        .unwrap()
        .question_pool
        .unwrap()
        .entries
        .remove(0);
    let e = exec(
        &mut s,
        1,
        C::RunQuestion(QuestionPoolRun {
            workspace_id: id(10),
            question_id: e.id,
            expected_revision: e.revision,
            task_id: id(1181),
        }),
    )
    .unwrap()
    .question_pool
    .unwrap()
    .entries
    .remove(0);
    question_review_end(&mut s);
    let now = e.expires_at_ms as i64;
    let r = crate::review::read(
        &s.conn,
        &s.root,
        &s.fence,
        &Operation::new(4000).unwrap(),
        &id(1),
        &question_review_source(),
        now,
    )
    .unwrap()
    .review_context
    .unwrap();
    assert!(r.questions[0].runtime_expired);
    assert_eq!(r.questions[0].question, "容量多大？");
    let c = crate::assistance::read(
        &s.conn,
        &s.root,
        &s.fence,
        &Operation::new(4000).unwrap(),
        &id(1),
        &id(10),
        &e.task_id,
        now,
    )
    .unwrap();
    assert!(c.body.is_empty());
    assert_eq!(c.suggestion_state, SuggestionState::Expired as i32);
}
#[test]
fn question_review_processing_outcomes_are_distinct_from_public_delivery() {
    for (decision, expected) in [
        (FaqDecision::Accept, "adopted"),
        (FaqDecision::Ignore, "ignored"),
        (FaqDecision::Cancel, "cancelled"),
    ] {
        let (_t, mut s, src) = setup_review();
        let snap = question_review_open(&mut s, &src);
        let e = exec(&mut s, 1, question_review_enqueue(&snap, 1190))
            .unwrap()
            .question_pool
            .unwrap()
            .entries
            .remove(0);
        let e = exec(
            &mut s,
            1,
            C::RunQuestion(QuestionPoolRun {
                workspace_id: id(10),
                question_id: e.id,
                expected_revision: e.revision,
                task_id: id(1191),
            }),
        )
        .unwrap()
        .question_pool
        .unwrap()
        .entries
        .remove(0);
        let faq = exec(
            &mut s,
            1,
            C::ReadFaq(FoundationFaqRead {
                workspace_id: id(10),
                task_id: e.task_id.clone(),
            }),
        )
        .unwrap();
        exec(
            &mut s,
            1,
            C::ResolveFaq(FoundationFaqResolve {
                workspace_id: id(10),
                task_id: e.task_id,
                task_sha256: faq.sha256,
                decision: decision as i32,
                ..Default::default()
            }),
        )
        .unwrap();
        question_review_end(&mut s);
        let c = ctx(&mut s, &question_review_source());
        assert_eq!(c.questions[0].processing, expected);
        assert_eq!(c.questions[0].delivery, "unconfirmed");
    }
}

#[test]
fn question_review_capacity_is_bounded_across_runtime_windows_and_rolls_back_overflow() {
    let (_t, mut s, src) = setup_review();
    let snap = question_review_open(&mut s, &src);
    let now = utc().unwrap();
    for i in 0..=64 {
        let C::EnqueueRunQuestion(q) = question_review_enqueue(&snap, 1200 + i) else {
            unreachable!()
        };
        let tx = s.conn.transaction().unwrap();
        let r = crate::question_pool::enqueue_run(
            &tx,
            &s.root,
            &s.fence,
            &Operation::new(4000).unwrap(),
            &id(1),
            &q,
            now + i as i64 * 120_001,
        );
        if i == 64 {
            assert_eq!(r, Err(Error::ResourceExhausted));
            drop(tx);
        } else {
            r.unwrap();
            tx.commit().unwrap();
        }
    }
    assert_eq!(get(&mut s, 1264), Err(Error::NotFound));
    question_review_end(&mut s);
    assert_eq!(ctx(&mut s, &question_review_source()).questions.len(), 64);
}
