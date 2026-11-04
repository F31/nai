//! Scope/transaction tests use synthetic completion metadata, never device evidence.
use super::*;
fn setup_auto(limit: u64) -> (TempDir, Store, LiveAutoView) {
    let (t, mut s, _, c) = prepared();
    let p = plan(&mut s);
    exec(&mut s, 1, batch(&c, &p, 800, false)).unwrap();
    let c = read_run(&mut s);
    exec(
        &mut s,
        1,
        C::SaveTtsModel(FoundationTtsModel {
            workspace_id: id(10),
            model_id: id(801),
            name: "scope fixture".into(),
            base_url: "http://127.0.0.1:8000/v1".into(),
            protocol: "qwen_local".into(),
            model: "qwen3-tts".into(),
            voice: "serena".into(),
            language: "chinese".into(),
            enabled: true,
            ..Default::default()
        }),
    )
    .unwrap();
    let a = exec(
        &mut s,
        1,
        C::StartLiveAuto(LiveAutoStart {
            run: Some(LiveRunRead {
                workspace_id: id(10),
                run_id: id(120),
            }),
            grant_id: id(802),
            expected_revision: c.revision,
            expected_epoch: c.epoch,
            plan_sha256: p.plan_sha256,
            model_id: id(801),
            model_revision: 1,
            max_cost_minor: limit,
            output_id: id(803),
            visual_sha256: "a".repeat(64),
            license_sha256: "b".repeat(64),
            stop_nonce: String::new(),
            acknowledge_rights: true,
        }),
    )
    .unwrap()
    .live_auto
    .unwrap();
    (t, s, a)
}
fn prepare_at(index: u32, n: u64) -> C {
    C::PrepareLiveAuto(LiveAutoPrepare {
        run: Some(LiveRunRead {
            workspace_id: id(10),
            run_id: id(120),
        }),
        grant_id: id(802),
        sentence_cursor: index,
        preview_id: id(n),
        task_id: id(n + 1),
    })
}
fn read_auto(s: &mut Store) -> LiveAutoView {
    exec(
        s,
        1,
        C::ReadLiveAuto(LiveRunRead {
            workspace_id: id(10),
            run_id: id(120),
        }),
    )
    .unwrap()
    .live_auto
    .unwrap()
}
fn finish(s: &mut Store, idn: u64) {
    s.tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(idn))
        .unwrap();
    s.finish_tts(
        &Operation::new(4000).unwrap(),
        &id(1),
        &id(10),
        &id(idn),
        true,
        "",
        2,
        &"c".repeat(64),
    )
    .unwrap();
}
#[test]
fn auto_scope_prepares_only_two_real_tasks_and_reserves_conservatively() {
    let (_t, mut s, a) = setup_auto(20);
    assert_eq!(a.state, "preparing");
    assert_eq!(a.sentence_total, 2);
    assert_eq!(
        exec(&mut s, 2, prepare_at(0, 810)),
        Err(Error::PermissionDenied)
    );
    assert!(exec(&mut s, 1, prepare_at(2, 810)).is_err());
    let first = exec(&mut s, 1, prepare_at(0, 810)).unwrap();
    assert_eq!(first.task_state, AgentTaskState::Queued as i32);
    assert_eq!(first.live_auto.unwrap().reserved_minor, 10);
    assert_eq!(exec(&mut s, 1, prepare_at(1, 812)), Err(Error::Busy));
    finish(&mut s, 811);
    let second = exec(&mut s, 1, prepare_at(1, 812)).unwrap();
    assert_eq!(second.live_auto.unwrap().tasks.len(), 2);
    assert_eq!(read_auto(&mut s).reserved_minor, 20);
    assert_eq!(
        exec(&mut s, 1, prepare_at(0, 814)),
        Err(Error::RevisionConflict)
    );
    assert_eq!(read_run(&mut s).sentence_cursor, 0);
}
#[test]
fn auto_budget_stop_and_nonce_do_not_refund_unknown_or_revive_tasks() {
    let (t, mut s, _) = setup_auto(10);
    exec(&mut s, 1, prepare_at(0, 820)).unwrap();
    finish(&mut s, 821);
    assert_eq!(
        exec(&mut s, 1, prepare_at(1, 822)),
        Err(Error::ResourceExhausted)
    );
    assert_eq!(read_auto(&mut s).reserved_minor, 10);
    std::fs::write(t.path().join("public-output.stop"), id(899)).unwrap();
    assert_eq!(read_auto(&mut s).state, "blocked");
    let old = exec(
        &mut s,
        1,
        C::ReadTtsTask(FoundationLlmTaskRead {
            workspace_id: id(10),
            task_id: id(821),
        }),
    )
    .unwrap();
    assert!(old.body.is_empty());
    assert_eq!(old.charge_status, "unknown");
    exec(
        &mut s,
        1,
        C::StopLiveAuto(LiveAutoStop {
            run: Some(LiveRunRead {
                workspace_id: id(10),
                run_id: id(120),
            }),
            grant_id: id(802),
            reason: "OPERATOR_STOP".into(),
        }),
    )
    .unwrap();
    assert_eq!(read_auto(&mut s).state, "stopped");
    std::fs::remove_file(t.path().join("public-output.stop")).unwrap();
    assert!(exec(&mut s, 1, prepare_at(1, 824)).is_err());
    assert_eq!(read_auto(&mut s).reserved_minor, 10);
}
#[test]
fn auto_audit_failure_rolls_back_dispatch_and_restart_never_resumes() {
    let (t, mut s, _) = setup_auto(20);
    s.conn.execute_batch("CREATE TRIGGER fail_auto BEFORE INSERT ON audit_events WHEN NEW.action='live_auto_prepare' BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert!(exec(&mut s, 1, prepare_at(0, 830)).is_err());
    let a = read_auto(&mut s);
    assert!(a.tasks.is_empty());
    assert_eq!(a.reserved_minor, 0);
    s.conn.execute_batch("DROP TRIGGER fail_auto;").unwrap();
    exec(&mut s, 1, prepare_at(0, 830)).unwrap();
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    assert_eq!(read_auto(&mut s).state, "blocked");
    assert!(exec(&mut s, 1, prepare_at(1, 832)).is_err());
}
#[test]
fn auto_text_revocation_and_model_revision_invalidate_pending_audio() {
    for model_change in [false, true] {
        let (_t, mut s, _) = setup_auto(20);
        exec(&mut s, 1, prepare_at(0, 840)).unwrap();
        if model_change {
            let (old, rev, _) = crate::llm::raw(
                &s.conn,
                &s.root,
                &id(10),
                &id(801),
                0,
                "publication_tts_config",
            )
            .unwrap();
            let mut new = old;
            new["voice"] = "vivian".into();
            let tx = s.conn.transaction().unwrap();
            crate::llm::put(&tx, &s.root, &id(10), &id(801), rev, &new, 1000).unwrap();
            tx.commit().unwrap();
        } else {
            let c = read_run(&mut s);
            let p = plan(&mut s);
            exec(&mut s, 1, batch(&c, &p, 800, true)).unwrap();
        }
        assert_eq!(read_auto(&mut s).state, "blocked");
        assert!(
            s.tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(841))
                .is_err()
        );
    }
}

#[test]
fn consumed_slot_is_retired_without_growing_run_object_or_refunding_scope() {
    let (_t, mut s, _) = setup_auto(20);
    exec(&mut s, 1, prepare_at(0, 850)).unwrap();
    finish(&mut s, 851);
    let c = read_run(&mut s);
    let source = c.source.as_ref().unwrap();
    // Synthetic root submission metadata, not device or approval evidence.
    let b = PublicMediaBundle {
        protocol_major: 1,
        workspace_id: id(10),
        session_id: c.session_id.clone(),
        epoch: c.epoch,
        output_id: id(803),
        snapshot_id: source.snapshot_id.clone(),
        snapshot_sha256: source.snapshot_sha256.clone(),
        facts_sha256: source.facts_sha256.clone(),
        approval_id: id(852),
        approved_sha256: "d".repeat(64),
        audio: Some(PublicMediaAudio {
            artifact_id: id(851),
            pcm_sha256: "c".repeat(64),
            sample_count: 2,
            sample_rate: 24000,
            channels: 1,
        }),
        ..Default::default()
    };
    event(&mut s, &b, "public_execution_requested").unwrap();
    event(&mut s, &b, "public_submission_complete").unwrap();
    assert_eq!(read_run(&mut s).sentence_cursor, 1);
    exec(&mut s, 1, prepare_at(1, 854)).unwrap();
    let (v, _, _) = crate::llm::raw(
        &s.conn,
        &s.root,
        &id(10),
        &id(120),
        0,
        "publication_live_run",
    )
    .unwrap();
    assert_eq!(v["auto_grant"]["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(v["auto_grant"]["tasks"][0]["index"], 1);
    assert_eq!(read_auto(&mut s).reserved_minor, 20);
    assert!(
        s.tts_current(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(851))
            .unwrap()
            .body
            .is_empty()
    );
}

fn selected_question(s: &mut Store) -> (String, String) {
    let c = read_run(s);
    let src = c.source.unwrap();
    exec(
        s,
        1,
        C::EnqueueRunQuestion(ReviewRunQuestion {
            source: Some(ReviewSource {
                workspace_id: id(10),
                run_id: id(120),
                run_kind: ReviewRunKind::Live as i32,
            }),
            expected_epoch: c.epoch,
            question: Some(QuestionPoolEnqueue {
                scope: Some(QuestionPoolRead {
                    workspace_id: id(10),
                    snapshot_id: src.snapshot_id,
                    snapshot_sha256: src.snapshot_sha256,
                }),
                question_id: id(880),
                question: "容量多大？".into(),
            }),
        }),
    )
    .unwrap();
    let r = exec(
        s,
        1,
        C::RunQuestion(QuestionPoolRun {
            workspace_id: id(10),
            question_id: id(880),
            expected_revision: 1,
            task_id: id(881),
        }),
    )
    .unwrap();
    let row = &r.question_pool.unwrap().entries[0];
    (row.task_id.clone(), row.task_sha256.clone())
}
fn question_control(s: &mut Store, task: String, sha: String, cancel: bool) -> C {
    let c = read_run(s);
    C::ControlLiveQuestion(LiveQuestionControl {
        run: Some(LiveRunRead {
            workspace_id: id(10),
            run_id: id(120),
        }),
        grant_id: id(802),
        interruption_id: id(882),
        faq_task_id: task,
        faq_task_sha256: sha,
        expected_revision: c.revision,
        expected_epoch: c.epoch,
        acknowledge_answer: !cancel,
        cancel,
    })
}
#[test]
fn question_pauses_prepares_original_faq_and_returns_without_consuming_script() {
    let (_t, mut s, _) = setup_auto(40);
    let (task, sha) = selected_question(&mut s);
    let request = question_control(&mut s, task, sha, false);
    let a = exec(&mut s, 1, request).unwrap().live_auto.unwrap();
    assert_eq!(a.state, "question_waiting");
    assert_eq!(a.answer, "标准款容量为500毫升。");
    assert_eq!(read_run(&mut s).state, LiveRunState::Paused as i32);
    let c = read_run(&mut s);
    assert!(exec(&mut s, 1, control(&c, LiveRunAction::Resume)).is_err());
    exec(&mut s, 1, prepare_at(0, 884)).unwrap();
    finish(&mut s, 885);
    let c = read_run(&mut s);
    let src = c.source.unwrap();
    let b = PublicMediaBundle {
        workspace_id: id(10),
        session_id: id(120),
        epoch: c.epoch,
        output_id: id(803),
        snapshot_id: src.snapshot_id,
        snapshot_sha256: src.snapshot_sha256,
        facts_sha256: src.facts_sha256,
        approval_id: id(886),
        approved_sha256: "d".repeat(64),
        audio: Some(PublicMediaAudio {
            artifact_id: id(885),
            pcm_sha256: "c".repeat(64),
            sample_count: 2,
            sample_rate: 24000,
            channels: 1,
        }),
        ..Default::default()
    };
    let mut wrong = b.clone();
    wrong.audio.as_mut().unwrap().sample_count = 3;
    assert!(event(&mut s, &wrong, "public_execution_requested").is_err());
    event(&mut s, &b, "public_execution_requested").unwrap();
    assert_eq!(read_auto(&mut s).interruption_state, "playing");
    let evidence = |s: &Store| {
        crate::review_questions::list(
            &s.conn,
            &s.root,
            &s.fence,
            &Operation::new(4000).unwrap(),
            &id(1),
            &ReviewSource {
                workspace_id: id(10),
                run_id: id(120),
                run_kind: ReviewRunKind::Live as i32,
            },
            utc().unwrap(),
        )
        .unwrap()
    };
    assert_eq!(evidence(&s)[0].delivery, "requested");
    assert_ne!(evidence(&s)[0].processing, "adopted");
    event(&mut s, &b, "public_submission_complete").unwrap();
    assert_eq!(evidence(&s)[0].delivery, "submitted");
    event(&mut s, &b, "public_submission_complete").unwrap();
    let c = read_run(&mut s);
    assert_eq!(c.sentence_cursor, 0);
    assert_eq!(c.submitted_sentences, 0);
    assert_eq!(c.state, LiveRunState::Ready as i32);
    assert_eq!(read_auto(&mut s).interruption_state, "completed");
    let request = question_control(&mut s, id(881), String::new(), false);
    let C::ControlLiveQuestion(mut request) = request else {
        unreachable!()
    };
    request.interruption_id = id(889);
    assert!(exec(&mut s, 1, C::ControlLiveQuestion(request)).is_err());

    assert!(exec(&mut s, 1, prepare_at(0, 888)).is_ok());
}
#[test]
fn question_source_hash_duplicate_parent_cancel_and_actor_are_rejected() {
    for failure in 0..4 {
        let (_t, mut s, _) = setup_auto(40);
        let (task, mut sha) = selected_question(&mut s);
        if failure == 0 {
            sha = "f".repeat(64)
        }
        let request = question_control(&mut s, task, sha, false);
        if failure == 0 || failure == 1 {
            assert!(exec(&mut s, if failure == 1 { 2 } else { 1 }, request).is_err());
            continue;
        }
        exec(&mut s, 1, request.clone()).unwrap();
        assert!(exec(&mut s, 1, request).is_err());
        if failure == 2 {
            exec(
                &mut s,
                1,
                C::CancelQuestion(QuestionPoolCancel {
                    workspace_id: id(10),
                    question_id: id(880),
                    expected_revision: 3,
                }),
            )
            .unwrap();
            assert!(exec(&mut s, 1, prepare_at(0, 884)).is_err());
        } else {
            let C::ControlLiveQuestion(mut request) =
                question_control(&mut s, String::new(), String::new(), true)
            else {
                unreachable!()
            };
            request.expected_revision -= 1;
            exec(&mut s, 1, C::ControlLiveQuestion(request)).unwrap();
            assert_eq!(read_auto(&mut s).state, "stopped");
            assert_eq!(read_run(&mut s).sentence_cursor, 0);
            assert!(exec(&mut s, 1, prepare_at(0, 884)).is_err());
        }
    }
}
#[test]
fn question_audit_failure_rolls_back_pause_and_selection() {
    let (_t, mut s, _) = setup_auto(40);
    let (task, sha) = selected_question(&mut s);
    s.conn.execute_batch("CREATE TRIGGER fail_q BEFORE INSERT ON audit_events WHEN NEW.action='live_question_control' BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    let request = question_control(&mut s, task, sha, false);
    assert!(exec(&mut s, 1, request).is_err());
    assert_eq!(read_run(&mut s).state, LiveRunState::Ready as i32);
    assert!(read_auto(&mut s).interruption_id.is_empty());
}

#[test]
fn expired_question_stops_scope_and_allows_explicit_script_resume_without_rewind() {
    let (_t, mut s, _) = setup_auto(40);
    let (task, sha) = selected_question(&mut s);
    let req = question_control(&mut s, task, sha, false);
    exec(&mut s, 1, req).unwrap();
    let (mut v, r) = crate::live::load(&s.conn, &s.root, &id(10), &id(120), &id(1)).unwrap();
    v["auto_grant"]["question"]["expires_at_ms"] = 0.into();
    let tx = s.conn.transaction().unwrap();
    crate::llm::put(&tx, &s.root, &id(10), &id(120), r, &v, 1000).unwrap();
    tx.commit().unwrap();
    assert_eq!(read_auto(&mut s).state, "blocked");
    assert!(exec(&mut s, 1, prepare_at(0, 884)).is_err());
    exec(
        &mut s,
        1,
        C::StopLiveAuto(LiveAutoStop {
            run: Some(LiveRunRead {
                workspace_id: id(10),
                run_id: id(120),
            }),
            grant_id: id(802),
            reason: "AUTO_FAILED".into(),
        }),
    )
    .unwrap();
    assert_eq!(read_auto(&mut s).interruption_state, "cancelled");
    let c = read_run(&mut s);
    assert_eq!(c.sentence_cursor, 0);
    exec(&mut s, 1, control(&c, LiveRunAction::Resume)).unwrap();
    assert_eq!(read_run(&mut s).sentence_cursor, 0);
}

#[test]
fn question_network_deadline_cannot_outlive_auto_scope() {
    let (_t, mut s, _) = setup_auto(40);
    let (task, sha) = selected_question(&mut s);
    let (mut v, r) = crate::live::load(&s.conn, &s.root, &id(10), &id(120), &id(1)).unwrap();
    let until = crate::utc().unwrap() + 5000;
    v["auto_grant"]["expires_at_ms"] = until.into();
    let tx = s.conn.transaction().unwrap();
    crate::llm::put(
        &tx,
        &s.root,
        &id(10),
        &id(120),
        r,
        &v,
        crate::utc().unwrap(),
    )
    .unwrap();
    tx.commit().unwrap();
    let req = question_control(&mut s, task, sha, false);
    exec(&mut s, 1, req).unwrap();
    exec(&mut s, 1, prepare_at(0, 884)).unwrap();
    let p = s
        .tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(885))
        .unwrap();
    assert!(p.deadline_ms > 0 && p.deadline_ms <= 5000);
}

#[test]
fn human_takeover_during_faq_does_not_skip_the_next_script_sentence() {
    let (_t, mut s, _) = setup_auto(40);
    let (task, sha) = selected_question(&mut s);
    let req = question_control(&mut s, task, sha, false);
    exec(&mut s, 1, req).unwrap();
    exec(&mut s, 1, prepare_at(0, 884)).unwrap();
    finish(&mut s, 885);
    let c = read_run(&mut s);
    let src = c.source.unwrap();
    let b = PublicMediaBundle {
        workspace_id: id(10),
        session_id: id(120),
        epoch: c.epoch,
        output_id: id(803),
        snapshot_id: src.snapshot_id,
        snapshot_sha256: src.snapshot_sha256,
        facts_sha256: src.facts_sha256,
        approval_id: id(886),
        approved_sha256: "d".repeat(64),
        audio: Some(PublicMediaAudio {
            artifact_id: id(885),
            pcm_sha256: "c".repeat(64),
            sample_count: 2,
            sample_rate: 24000,
            channels: 1,
        }),
        ..Default::default()
    };
    event(&mut s, &b, "public_execution_requested").unwrap();
    let c = read_run(&mut s);
    let x = LiveTakeoverControl {
        run: Some(LiveRunRead {
            workspace_id: id(10),
            run_id: id(120),
        }),
        takeover_id: id(973),
        expected_epoch: c.epoch,
        acknowledge: true,
        ..Default::default()
    };
    let c = exec(&mut s, 1, C::ControlLiveTakeover(x.clone()))
        .unwrap()
        .live_context
        .unwrap();
    assert!(!c.partial_sentence);
    assert_eq!(read_auto(&mut s).interruption_state, "cancelled");
    let c = exec(
        &mut s,
        1,
        C::ControlLiveTakeover(LiveTakeoverControl {
            resume: true,
            expected_epoch: c.epoch,
            ..x
        }),
    )
    .unwrap()
    .live_context
    .unwrap();
    assert_eq!(c.sentence_cursor, 0);
    assert_eq!(c.skipped_sentences, 0);
    assert!(event(&mut s, &b, "public_submission_complete").is_err());
}

#[test]
fn selected_answer_has_fixed_response_window_but_network_keeps_fifteen_second_cap() {
    let (_t, mut s, _) = setup_auto(40);
    let (task, sha) = selected_question(&mut s);
    let req = question_control(&mut s, task, sha, false);
    exec(&mut s, 1, req).unwrap();
    let (v, _) = crate::live::load(&s.conn, &s.root, &id(10), &id(120), &id(1)).unwrap();
    let q = &v["auto_grant"]["question"];
    assert_eq!(
        q["expires_at_ms"].as_i64().unwrap() - q["created_at_ms"].as_i64().unwrap(),
        30_000
    );
    exec(&mut s, 1, prepare_at(0, 884)).unwrap();
    let p = s
        .tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(885))
        .unwrap();
    assert!(p.deadline_ms > 0 && p.deadline_ms <= 15_000);
    let (after, _) = crate::live::load(&s.conn, &s.root, &id(10), &id(120), &id(1)).unwrap();
    assert_eq!(
        after["auto_grant"]["question"]["expires_at_ms"],
        q["expires_at_ms"]
    );
}
