use super::*;
use nai_contracts::workbench::{LiveRunAction as A, LiveRunState as S};
fn prepared() -> (TempDir, Store, FoundationResult, LiveContext) {
    prepared_text(None)
}
fn prepared_text(text: Option<&str>) -> (TempDir, Store, FoundationResult, LiveContext) {
    let (t, s, snap, c) = prepared_result(text);
    (t, s, snap, c.unwrap())
}
fn prepared_result(text: Option<&str>) -> (TempDir, Store, FoundationResult, Result<LiveContext>) {
    let (t, mut s) = init();
    let prior = exec(
        &mut s,
        1,
        C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(83),
            revision: 0,
        }),
    )
    .unwrap();
    let mut show: Value = serde_json::from_str(&prior.body).unwrap();
    if let Some(text) = text {
        let prior = exec(
            &mut s,
            1,
            C::ReadObject(FoundationReadObject {
                workspace_id: id(10),
                object_id: id(80),
                revision: 0,
            }),
        )
        .unwrap();
        let mut product: Value = serde_json::from_str(&prior.body).unwrap();
        product["talk_tracks"][0]["text"] = text.into();
        let r = save(&mut s, 80, prior.revision, product);
        show["product"] = pointer(80, &r);
    }
    let mut cue = show["cues"][0].clone();
    cue["id"] = "details".into();
    show["cues"].as_array_mut().unwrap().push(cue);
    save(&mut s, 83, 1, show);
    let r = report(&mut s, 85, None);
    let snap = exec(&mut s, 1, publish(90, &r)).unwrap();
    let c = exec(
        &mut s,
        1,
        C::CreateLiveRun(LiveRunCreate {
            scope: Some(QuestionPoolRead {
                workspace_id: id(10),
                snapshot_id: id(90),
                snapshot_sha256: snap.sha256.clone(),
            }),
            run_id: id(120),
        }),
    )
    .map(|r| r.live_context.unwrap());
    (t, s, snap, c)
}
#[test]
fn live_speech_plan_long_unicode_is_lossless_and_total_sentence_bound_is_enforced() {
    let text = format!("容量500.5毫升。{}结束。", "长文字😀".repeat(50));
    let (_t, mut s, _, _) = prepared_text(Some(&text));
    let read = || {
        C::ReadLiveSpeechPlan(LiveRunRead {
            workspace_id: id(10),
            run_id: id(120),
        })
    };
    let p = exec(&mut s, 1, read()).unwrap().live_speech_plan.unwrap();
    assert_eq!(
        p.sentences
            .iter()
            .filter(|s| s.cue_id == "intro")
            .map(|s| s.text.as_str())
            .collect::<String>(),
        text
    );
    assert!(p.sentences.len() > 2);
    assert!(p.sentences.iter().all(|s| s.text.chars().count() <= 120));
    // Two units reference the same 65-sentence track, exceeding the global 128 limit.
    let (_t, s, _, c) = prepared_result(Some(&"短。".repeat(65)));
    assert_eq!(c, Err(Error::ResourceExhausted));
    assert_eq!(
        s.conn
            .query_row(
                "SELECT COUNT(*) FROM sessions WHERE id=?1",
                [id(120)],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}
fn read_run(s: &mut Store) -> LiveContext {
    exec(
        s,
        1,
        C::ReadLiveRun(LiveRunRead {
            workspace_id: id(10),
            run_id: id(120),
        }),
    )
    .unwrap()
    .live_context
    .unwrap()
}
fn control(c: &LiveContext, a: A) -> C {
    C::ControlLiveRun(LiveRunControl {
        workspace_id: id(10),
        run_id: c.run_id.clone(),
        expected_revision: c.revision,
        expected_epoch: c.epoch,
        action: a as i32,
    })
}
#[test]
fn live_queue_pause_cas_recovery_and_end_do_not_self_report_submission() {
    let (t, mut s, _, c) = prepared();
    assert_eq!(c.cursor, 0);
    assert_eq!(c.current_cue_id, "intro");
    let p = exec(&mut s, 1, control(&c, A::Pause))
        .unwrap()
        .live_context
        .unwrap();
    assert_eq!(p.state, S::Paused as i32);
    assert_eq!(
        exec(&mut s, 1, control(&c, A::Resume)),
        Err(Error::RevisionConflict)
    );
    let r = exec(&mut s, 1, control(&p, A::Resume))
        .unwrap()
        .live_context
        .unwrap();
    assert_eq!(r.cursor, 0);
    assert_eq!(r.submitted_count, 0);
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    let old = read_run(&mut s);
    assert_eq!(old.reason, "SESSION_RECOVERY_REQUIRED");
    assert!(exec(&mut s, 1, control(&old, A::Resume)).is_err());
    let restored = exec(&mut s, 1, control(&old, A::Recover))
        .unwrap()
        .live_context
        .unwrap();
    assert!(restored.epoch > old.epoch);
    assert_eq!(restored.state, S::Paused as i32);
    let end = exec(&mut s, 1, control(&restored, A::End))
        .unwrap()
        .live_context
        .unwrap();
    assert_eq!(end.state, S::Ended as i32);
    assert_eq!(end.submitted_count, 0);
    assert!(exec(&mut s, 1, control(&end, A::Resume)).is_err());
}
#[test]
fn live_queue_generic_write_and_cross_actor_read_cannot_forge_progress() {
    let (_t, mut s, _, c) = prepared();
    let (mut v, _, _) = crate::llm::raw(
        &s.conn,
        &s.root,
        &id(10),
        &c.run_id,
        0,
        "publication_live_run",
    )
    .unwrap();
    v["cursor"] = 1.into();
    assert!(
        exec(
            &mut s,
            1,
            C::PutPreparation(FoundationObject {
                workspace_id: id(10),
                object_id: c.run_id.clone(),
                expected_revision: c.revision,
                body: v.to_string()
            })
        )
        .is_err()
    );
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
    assert_eq!(
        exec(
            &mut s,
            2,
            C::ReadLiveRun(LiveRunRead {
                workspace_id: id(10),
                run_id: c.run_id.clone()
            })
        ),
        Err(Error::PermissionDenied)
    );
    assert_eq!(
        exec(
            &mut s,
            2,
            C::ReadObject(FoundationReadObject {
                workspace_id: id(10),
                object_id: c.run_id,
                revision: 0
            })
        ),
        Err(Error::PermissionDenied)
    );
    assert_eq!(read_run(&mut s).cursor, 0);
}
#[test]
fn live_queue_audit_failure_rolls_back_control_and_source_updates_block_resume() {
    let (_t, mut s, _, c) = prepared();
    s.conn.execute_batch("CREATE TRIGGER fail_live BEFORE INSERT ON audit_events WHEN NEW.action='live_run_control' BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert!(exec(&mut s, 1, control(&c, A::Pause)).is_err());
    assert_eq!(read_run(&mut s).revision, c.revision);
    s.conn.execute_batch("DROP TRIGGER fail_live;").unwrap();
    save(&mut s, 80, 1, serde_json::from_str(PRODUCT_DRAFT).unwrap());
    assert_eq!(read_run(&mut s).state, S::Unavailable as i32);
    assert!(exec(&mut s, 1, control(&c, A::Resume)).is_err());
    exec(&mut s, 1, control(&c, A::End)).unwrap();
}
// Synthetic root event injection. This is state/transaction evidence, not device playback.
fn media(s: &mut Store, c: &LiveContext) -> PublicMediaBundle {
    media_at(s, c, c.sentence_cursor, 125)
}
fn media_at(s: &mut Store, c: &LiveContext, index: u32, base: u64) -> PublicMediaBundle {
    exec(
        s,
        1,
        C::SaveTtsModel(FoundationTtsModel {
            workspace_id: id(10),
            model_id: id(base),
            name: "test-only local transport".into(),
            base_url: "http://127.0.0.1:8000/v1".into(),
            model: "qwen3-tts".into(),
            voice: "serena".into(),
            protocol: "qwen_local".into(),
            language: "chinese".into(),
            enabled: true,
            ..Default::default()
        }),
    )
    .unwrap();
    let card = exec(
        s,
        1,
        C::PreviewLiveSentence(LiveSentencePreview {
            run: Some(LiveRunRead {
                workspace_id: id(10),
                run_id: c.run_id.clone(),
            }),
            preview_id: id(base + 1),
            sentence_cursor: index,
        }),
    )
    .unwrap();
    let p = FoundationSpeechPlan {
        workspace_id: id(10),
        preview_id: id(base + 1),
        preview_sha256: card.sha256,
        model_id: id(base),
        model_revision: 1,
        max_cost_minor: 1,
        task_id: id(base + 2),
    };
    let q = exec(s, 1, C::CheckPreviewSpeech(p.clone()))
        .unwrap()
        .speech_plan
        .unwrap();
    exec(
        s,
        1,
        C::SynthesizePreviewSpeech(FoundationSpeechStart {
            plan: Some(p),
            task_id: id(base + 2),
            approval_sha256: q.approval_sha256,
        }),
    )
    .unwrap();
    s.tts_dispatch(
        &Operation::new(4000).unwrap(),
        &id(1),
        &id(10),
        &id(base + 2),
    )
    .unwrap();
    s.finish_tts(
        &Operation::new(4000).unwrap(),
        &id(1),
        &id(10),
        &id(base + 2),
        true,
        "",
        2,
        &"a".repeat(64),
    )
    .unwrap();
    let source = c.source.as_ref().unwrap();
    PublicMediaBundle {
        protocol_major: 1,
        bundle_id: id(base + 3),
        workspace_id: id(10),
        session_id: c.session_id.clone(),
        epoch: c.epoch,
        output_id: id(base + 4),
        snapshot_id: source.snapshot_id.clone(),
        snapshot_sha256: source.snapshot_sha256.clone(),
        facts_sha256: source.facts_sha256.clone(),
        approval_id: id(base + 5),
        approved_sha256: "b".repeat(64),
        guard_permit_id: id(base + 6),
        caption_sha256: "c".repeat(64),
        audio: Some(PublicMediaAudio {
            artifact_id: id(base + 2),
            pcm_sha256: "a".repeat(64),
            sample_count: 2,
            sample_rate: 24000,
            channels: 1,
        }),
        lease_ms: 500,
        scope: DeliveryScope::Public as i32,
        ..Default::default()
    }
}
fn event(s: &mut Store, b: &PublicMediaBundle, a: &str) -> Result<()> {
    s.record_public_execution(&Operation::new(4000).unwrap(), &id(1), b, a)
}
#[test]
fn live_root_completion_is_atomic_idempotent_and_pause_waits_for_boundary() {
    let (_t, mut s, _, c) = prepared();
    let b = media(&mut s, &c);
    assert!(event(&mut s, &b, "public_submission_complete").is_err());
    assert_eq!(read_run(&mut s).cursor, 0);
    event(&mut s, &b, "public_execution_requested").unwrap();
    let running = read_run(&mut s);
    assert_eq!(running.state, S::Dispatching as i32);
    let paused = exec(&mut s, 1, control(&running, A::Pause))
        .unwrap()
        .live_context
        .unwrap();
    assert_eq!(paused.state, S::PauseRequested as i32);
    assert_eq!(paused.cursor, 0);
    let mut wrong = b.clone();
    wrong.epoch += 1;
    assert!(event(&mut s, &wrong, "public_submission_complete").is_err());
    s.conn.execute_batch("CREATE TRIGGER fail_complete BEFORE INSERT ON audit_events WHEN NEW.action='public_submission_complete' BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert!(event(&mut s, &b, "public_submission_complete").is_err());
    assert_eq!(read_run(&mut s).cursor, 0);
    s.conn.execute_batch("DROP TRIGGER fail_complete;").unwrap();
    event(&mut s, &b, "public_submission_complete").unwrap();
    let completed = read_run(&mut s);
    assert_eq!(completed.cursor, 1);
    assert_eq!(completed.state, S::Paused as i32);
    assert_eq!(completed.submitted_count, 1);
    event(&mut s, &b, "public_submission_complete").unwrap();
    assert_eq!(read_run(&mut s).revision, completed.revision);
    assert!(event(&mut s, &b, "public_execution_requested").is_err());
    let resumed = exec(&mut s, 1, control(&completed, A::Resume))
        .unwrap()
        .live_context
        .unwrap();
    assert_eq!(resumed.current_cue_id, "details");
    assert_eq!(resumed.cursor, 1);
}
#[test]
fn live_restart_of_dispatched_sentence_requires_explicit_skip_not_replay() {
    let (t, mut s, _, c) = prepared();
    let b = media(&mut s, &c);
    event(&mut s, &b, "public_execution_requested").unwrap();
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    let old = read_run(&mut s);
    let r = exec(&mut s, 1, control(&old, A::Recover))
        .unwrap()
        .live_context
        .unwrap();
    assert_eq!(r.state, S::Interrupted as i32);
    assert!(exec(&mut s, 1, control(&r, A::Resume)).is_err());
    assert!(event(&mut s, &b, "public_submission_complete").is_err());
    let skipped = exec(&mut s, 1, control(&r, A::SkipInterrupted))
        .unwrap()
        .live_context
        .unwrap();
    assert_eq!(skipped.cursor, 1);
    assert_eq!(skipped.skipped_count, 1);
    assert_eq!(skipped.submitted_count, 0);
    assert_eq!(skipped.state, S::Ready as i32);
}
#[test]
fn live_root_rejects_ambiguous_non_json_session_object_without_progress() {
    let (_t, mut s, _, _) = prepared();
    exec(
        &mut s,
        1,
        C::PutObject(FoundationObject {
            workspace_id: id(10),
            object_id: id(550),
            expected_revision: 0,
            body: "普通备注，不是队列".into(),
        }),
    )
    .unwrap();
    exec(
        &mut s,
        1,
        C::CreateSession(FoundationSession {
            workspace_id: id(10),
            session_id: id(550),
        }),
    )
    .unwrap();
    let b = PublicMediaBundle {
        workspace_id: id(10),
        session_id: id(550),
        epoch: 1,
        approval_id: id(551),
        ..Default::default()
    };
    assert_eq!(
        event(&mut s, &b, "public_execution_stopped"),
        Err(Error::IntegrityFailed)
    );
    assert_eq!(read_run(&mut s).cursor, 0);
}
#[test]
fn live_speech_plan_is_lossless_read_only_and_source_bound() {
    let (_t, mut s, _, c) = prepared();
    let read = || {
        C::ReadLiveSpeechPlan(LiveRunRead {
            workspace_id: id(10),
            run_id: id(120),
        })
    };
    let p = exec(&mut s, 1, read()).unwrap().live_speech_plan.unwrap();
    assert_eq!(p.run_id, c.run_id);
    assert_eq!(p.revision, c.revision);
    assert_eq!(p.prefetch_limit, 2);
    assert_eq!(p.max_sentence_chars, 120);
    assert_eq!(p.sentences.len(), 2);
    assert_eq!(p.current_start, 0);
    assert_eq!(p.sentences[0].cue_id, "intro");
    assert_eq!(p.sentences[1].cue_id, "details");
    let card = exec(
        &mut s,
        1,
        C::Preview(FoundationPreview {
            workspace_id: id(10),
            preview_id: id(570),
            snapshot_id: id(90),
            snapshot_sha256: c.source.as_ref().unwrap().snapshot_sha256.clone(),
            cue_id: "intro".into(),
            session_id: id(120),
            expected_epoch: c.epoch,
        }),
    )
    .unwrap();
    let body: Value = serde_json::from_str(&card.body).unwrap();
    assert_eq!(p.sentences[0].text, body["text"].as_str().unwrap());
    assert_eq!(read_run(&mut s).revision, c.revision);
    assert_eq!(exec(&mut s, 2, read()), Err(Error::PermissionDenied));
    let old = exec(
        &mut s,
        1,
        C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(80),
            revision: 0,
        }),
    )
    .unwrap();
    let mut product: Value = serde_json::from_str(&old.body).unwrap();
    product["facts"][0]["value"] = "550".into();
    save(&mut s, 80, old.revision, product);
    assert!(exec(&mut s, 1, read()).is_err());
    assert_eq!(read_run(&mut s).cursor, 0);
}
#[test]
fn live_speech_plan_stopped_session_needs_explicit_recovery() {
    let (t, mut s, _, _) = prepared();
    drop(s);
    s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    let read = || {
        C::ReadLiveSpeechPlan(LiveRunRead {
            workspace_id: id(10),
            run_id: id(120),
        })
    };
    assert_eq!(exec(&mut s, 1, read()), Err(Error::Cancelled));
    let c = read_run(&mut s);
    exec(&mut s, 1, control(&c, A::Recover)).unwrap();
    assert!(exec(&mut s, 1, read()).unwrap().live_speech_plan.is_some());
}
fn plan(s: &mut Store) -> LiveSpeechPlan {
    exec(
        s,
        1,
        C::ReadLiveSpeechPlan(LiveRunRead {
            workspace_id: id(10),
            run_id: id(120),
        }),
    )
    .unwrap()
    .live_speech_plan
    .unwrap()
}
fn batch(c: &LiveContext, p: &LiveSpeechPlan, n: u64, revoke: bool) -> C {
    C::ControlLiveBatch(LiveBatchControl {
        run: Some(LiveRunRead {
            workspace_id: id(10),
            run_id: c.run_id.clone(),
        }),
        expected_revision: c.revision,
        expected_epoch: c.epoch,
        approval_id: id(n),
        plan_sha256: p.plan_sha256.clone(),
        acknowledge_rights: !revoke,
        revoke,
    })
}
#[test]
fn live_sentence_identity_is_contiguous_and_next_prefetch_cannot_play_first() {
    let (_t, mut s, _, c) = prepared_text(Some("容量500毫升。重量100克。"));
    assert!(c.sentence_mode);
    assert_eq!(c.sentence_total, 4);
    let next = media_at(&mut s, &c, 1, 600);
    assert!(event(&mut s, &next, "public_execution_requested").is_err());
    assert!(
        exec(
            &mut s,
            1,
            C::PreviewLiveSentence(LiveSentencePreview {
                run: Some(LiveRunRead {
                    workspace_id: id(10),
                    run_id: c.run_id.clone()
                }),
                preview_id: id(650),
                sentence_cursor: 2
            })
        )
        .is_err()
    );
    let first = media_at(&mut s, &c, 0, 610);
    event(&mut s, &first, "public_execution_requested").unwrap();
    let before = read_run(&mut s);
    exec(&mut s, 1, control(&before, A::Pause)).unwrap();
    event(&mut s, &first, "public_submission_complete").unwrap();
    let mid = read_run(&mut s);
    assert_eq!(mid.cursor, 0);
    assert_eq!(mid.submitted_count, 0);
    assert_eq!(mid.sentence_cursor, 1);
    assert_eq!(mid.current_sentence_index, 1);
    assert_eq!(mid.submitted_sentences, 1);
    assert_eq!(mid.state, S::Paused as i32);
    event(&mut s, &first, "public_submission_complete").unwrap();
    assert_eq!(read_run(&mut s).sentence_cursor, 1);
    assert!(event(&mut s, &next, "public_execution_requested").is_err());
    exec(&mut s, 1, control(&mid, A::Resume)).unwrap();
    event(&mut s, &next, "public_execution_requested").unwrap();
    event(&mut s, &next, "public_submission_complete").unwrap();
    let end = read_run(&mut s);
    assert_eq!(end.cursor, 1);
    assert_eq!(end.sentence_cursor, 2);
    assert_eq!(end.current_sentence_index, 0);
    assert_eq!(end.submitted_count, 1);
    assert!(event(&mut s, &first, "public_execution_requested").is_err());
}
#[test]
fn live_batch_text_grant_is_hashed_cas_revocable_and_not_a_media_permit() {
    let (_t, mut s, _, c) = prepared();
    let p = plan(&mut s);
    assert_eq!(p.plan_sha256, nai_contracts::live_speech_plan_hash(&p));
    let mut no = batch(&c, &p, 700, false);
    if let C::ControlLiveBatch(x) = &mut no {
        x.acknowledge_rights = false;
    }
    assert_eq!(exec(&mut s, 1, no), Err(Error::PermissionDenied));
    let mut wrong = batch(&c, &p, 700, false);
    if let C::ControlLiveBatch(x) = &mut wrong {
        x.plan_sha256 = "a".repeat(64);
    }
    assert_eq!(exec(&mut s, 1, wrong), Err(Error::RevisionConflict));
    assert_eq!(
        exec(&mut s, 2, batch(&c, &p, 700, false)),
        Err(Error::PermissionDenied)
    );
    s.conn.execute_batch("CREATE TRIGGER fail_batch BEFORE INSERT ON audit_events WHEN NEW.action='live_batch_control' BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert!(exec(&mut s, 1, batch(&c, &p, 700, false)).is_err());
    assert!(plan(&mut s).batch_approval.is_none());
    s.conn.execute_batch("DROP TRIGGER fail_batch;").unwrap();
    exec(&mut s, 1, batch(&c, &p, 700, false)).unwrap();
    let approved = plan(&mut s);
    let grant = approved.batch_approval.as_ref().unwrap();
    assert!(grant.approved);
    assert!(!grant.revoked);
    assert_eq!(approved.plan_sha256, p.plan_sha256);
    assert_eq!(read_run(&mut s).sentence_cursor, 0);
    assert_eq!(
        s.conn
            .query_row("SELECT COUNT(*) FROM public_media_approvals", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        exec(&mut s, 1, batch(&c, &p, 701, false)),
        Err(Error::RevisionConflict)
    );
    let c = read_run(&mut s);
    exec(&mut s, 1, batch(&c, &approved, 700, true)).unwrap();
    let revoked = plan(&mut s);
    assert!(revoked.batch_approval.as_ref().unwrap().revoked);
    assert!(!revoked.batch_approval.as_ref().unwrap().approved);
    let c = read_run(&mut s);
    assert_eq!(
        exec(&mut s, 1, batch(&c, &p, 700, false)),
        Err(Error::RevisionConflict)
    );

    let c = read_run(&mut s);
    exec(&mut s, 1, batch(&c, &p, 701, false)).unwrap();
    let c = read_run(&mut s);
    assert_eq!(
        exec(&mut s, 1, batch(&c, &p, 700, false)),
        Err(Error::RevisionConflict)
    );
}
#[test]
fn live_batch_expiry_restart_and_fact_change_cannot_revive_text_authority() {
    let (t, mut s, _, c) = prepared();
    let p = plan(&mut s);
    exec(&mut s, 1, batch(&c, &p, 710, false)).unwrap();
    let (mut v, r, _) = crate::llm::raw(
        &s.conn,
        &s.root,
        &id(10),
        &id(120),
        0,
        "publication_live_run",
    )
    .unwrap();
    v["batch_approval"]["expires_at_ms"] = 1.into();
    let tx = s.conn.transaction().unwrap();
    crate::llm::put(&tx, &s.root, &id(10), &id(120), r, &v, 1000).unwrap();
    tx.commit().unwrap();
    assert!(!plan(&mut s).batch_approval.unwrap().approved);
    let c = read_run(&mut s);
    exec(&mut s, 1, batch(&c, &p, 711, false)).unwrap();
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    let c = read_run(&mut s);
    exec(&mut s, 1, control(&c, A::Recover)).unwrap();
    let restored = plan(&mut s);
    assert_ne!(p.plan_sha256, restored.plan_sha256);
    assert!(!restored.batch_approval.unwrap().approved);
    save(&mut s, 80, 1, serde_json::from_str(PRODUCT_DRAFT).unwrap());
    assert!(
        exec(
            &mut s,
            1,
            C::ReadLiveSpeechPlan(LiveRunRead {
                workspace_id: id(10),
                run_id: id(120)
            })
        )
        .is_err()
    );
}
#[test]
fn live_old_unit_queue_is_retained_but_cannot_execute_or_gain_new_grants() {
    let (_t, mut s, _, _) = prepared();
    let card = exec(
        &mut s,
        1,
        C::PreviewLiveSentence(LiveSentencePreview {
            run: Some(LiveRunRead {
                workspace_id: id(10),
                run_id: id(120),
            }),
            preview_id: id(750),
            sentence_cursor: 0,
        }),
    )
    .unwrap();
    let mut old_card: Value = serde_json::from_str(&card.body).unwrap();
    old_card.as_object_mut().unwrap().remove("live_sentence");
    let tx = s.conn.transaction().unwrap();
    crate::publication::insert(
        &tx,
        &s.root,
        &id(10),
        &id(751),
        &old_card,
        old_card["created_at_ms"].as_i64().unwrap(),
    )
    .unwrap();
    tx.commit().unwrap();
    let (mut v, r, _) = crate::llm::raw(
        &s.conn,
        &s.root,
        &id(10),
        &id(120),
        0,
        "publication_live_run",
    )
    .unwrap();
    for key in [
        "sentence_mode",
        "sentence_cursor",
        "sentence_total",
        "current_sentence_index",
        "submitted_sentences",
        "skipped_sentences",
    ] {
        v.as_object_mut().unwrap().remove(key);
    }
    let tx = s.conn.transaction().unwrap();
    crate::llm::put(&tx, &s.root, &id(10), &id(120), r, &v, 1000).unwrap();
    tx.commit().unwrap();
    let old = exec(
        &mut s,
        1,
        C::ReadPreview(FoundationPreviewRead {
            workspace_id: id(10),
            preview_id: id(751),
        }),
    )
    .unwrap();
    assert_ne!(old.state, DeliveryState::Delivering as i32);
    assert!(old.body.is_empty());
    let c = read_run(&mut s);
    assert_eq!(c.reason, "QUEUE_RECREATE_REQUIRED");
    assert_eq!(c.state, S::Unavailable as i32);
    assert_eq!(
        exec(&mut s, 1, control(&c, A::Resume)),
        Err(Error::CapabilityUnavailable)
    );
    assert!(
        exec(
            &mut s,
            1,
            C::ReadLiveSpeechPlan(LiveRunRead {
                workspace_id: id(10),
                run_id: id(120)
            })
        )
        .is_err()
    );
    exec(&mut s, 1, control(&c, A::End)).unwrap();
    assert_eq!(read_run(&mut s).state, S::Ended as i32);
}
#[test]
fn live_sentence_interruption_skips_remaining_unit_and_never_counts_it_as_submitted() {
    let (_t, mut s, _, c) = prepared_text(Some("容量500毫升。重量100克。"));
    let b = media_at(&mut s, &c, 0, 730);
    event(&mut s, &b, "public_execution_requested").unwrap();
    event(&mut s, &b, "public_execution_stopped").unwrap();
    let stopped = read_run(&mut s);
    assert_eq!(stopped.state, S::Interrupted as i32);
    let skipped = exec(&mut s, 1, control(&stopped, A::SkipInterrupted))
        .unwrap()
        .live_context
        .unwrap();
    assert_eq!(skipped.cursor, 1);
    assert_eq!(skipped.sentence_cursor, 2);
    assert_eq!(skipped.skipped_sentences, 2);
    assert_eq!(skipped.submitted_sentences, 0);
    assert_eq!(skipped.skipped_count, 1);
    assert_eq!(plan(&mut s).current_start, 2);
    assert!(event(&mut s, &b, "public_execution_requested").is_err());
    let stale = exec(
        &mut s,
        1,
        C::ReadPreview(FoundationPreviewRead {
            workspace_id: id(10),
            preview_id: id(731),
        }),
    )
    .unwrap();
    assert_ne!(stale.state, DeliveryState::Delivering as i32);
}
#[path = "live_auto_tests.rs"]
mod auto_tests;

#[test]
fn human_takeover_retires_epoch_and_skips_only_the_unconfirmed_sentence() {
    let (_t, mut s, _, c) = prepared_text(Some("标准款容量为500毫升。标准款容量为500毫升。"));
    let b = media(&mut s, &c);
    event(&mut s, &b, "public_execution_requested").unwrap();
    let c = read_run(&mut s);
    let request = |epoch, resume, skip| {
        C::ControlLiveTakeover(LiveTakeoverControl {
            run: Some(LiveRunRead {
                workspace_id: id(10),
                run_id: id(120),
            }),
            takeover_id: id(970),
            expected_epoch: epoch,
            resume,
            acknowledge: true,
            skip_partial_sentence: skip,
        })
    };
    let taken = exec(&mut s, 1, request(c.epoch, false, false))
        .unwrap()
        .live_context
        .unwrap();
    assert!(taken.human_takeover && taken.partial_sentence);
    assert_eq!(taken.epoch, c.epoch + 1);
    assert_eq!(taken.sentence_cursor, 0);
    assert!(event(&mut s, &b, "public_submission_complete").is_err());
    assert!(exec(&mut s, 1, control(&taken, A::Resume)).is_err());
    assert!(exec(&mut s, 1, request(taken.epoch, true, false)).is_err());
    let resumed = exec(&mut s, 1, request(taken.epoch, true, true))
        .unwrap()
        .live_context
        .unwrap();
    assert!(!resumed.human_takeover);
    assert_eq!(resumed.sentence_cursor, 1);
    assert_eq!(resumed.cursor, 0);
    assert_eq!(resumed.current_sentence_index, 1);
    assert_eq!(resumed.skipped_sentences, 1);
    assert_eq!(resumed.submitted_sentences, 0);
    assert_eq!(resumed.state, S::Ready as i32);
    assert!(exec(&mut s, 1, request(resumed.epoch, false, false)).is_err());
}
#[test]
fn human_takeover_requires_actor_ack_and_current_epoch_and_audit_is_atomic() {
    for failure in 0..4 {
        let (_t, mut s, _, c) = prepared();
        let x = LiveTakeoverControl {
            run: Some(LiveRunRead {
                workspace_id: id(10),
                run_id: id(120),
            }),
            takeover_id: id(971),
            expected_epoch: if failure == 1 { c.epoch + 1 } else { c.epoch },
            resume: false,
            acknowledge: failure != 0,
            skip_partial_sentence: false,
        };
        if failure == 3 {
            s.conn.execute_batch("CREATE TRIGGER no_takeover BEFORE INSERT ON audit_events WHEN NEW.action='live_takeover_control' BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
        }
        assert!(
            exec(
                &mut s,
                if failure == 2 { 2 } else { 1 },
                C::ControlLiveTakeover(x)
            )
            .is_err()
        );
        let after = read_run(&mut s);
        assert!(!after.human_takeover);
        assert_eq!(after.epoch, c.epoch);
    }
}
#[test]
fn human_takeover_at_boundary_preserves_next_sentence_and_end_cannot_leave_mic_authority() {
    let (_t, mut s, _, c) = prepared();
    let x = LiveTakeoverControl {
        run: Some(LiveRunRead {
            workspace_id: id(10),
            run_id: id(120),
        }),
        takeover_id: id(972),
        expected_epoch: c.epoch,
        acknowledge: true,
        ..Default::default()
    };
    let c = exec(&mut s, 1, C::ControlLiveTakeover(x.clone()))
        .unwrap()
        .live_context
        .unwrap();
    assert!(!c.partial_sentence);
    let r = exec(
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
    assert_eq!(r.sentence_cursor, 0);
    assert_eq!(r.skipped_sentences, 0);
}

#[test]
fn human_takeover_restart_never_reopens_old_capture_authority() {
    let (t, mut s, _, c) = prepared();
    let x = LiveTakeoverControl {
        run: Some(LiveRunRead {
            workspace_id: id(10),
            run_id: id(120),
        }),
        takeover_id: id(974),
        expected_epoch: c.epoch,
        acknowledge: true,
        ..Default::default()
    };
    exec(&mut s, 1, C::ControlLiveTakeover(x)).unwrap();
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    let c = read_run(&mut s);
    assert!(!c.human_takeover);
    let x = LiveTakeoverControl {
        run: Some(LiveRunRead {
            workspace_id: id(10),
            run_id: id(120),
        }),
        takeover_id: id(975),
        expected_epoch: c.epoch,
        acknowledge: true,
        ..Default::default()
    };
    let next = exec(&mut s, 1, C::ControlLiveTakeover(x))
        .unwrap()
        .live_context
        .unwrap();
    assert!(next.human_takeover);
    assert!(next.epoch > c.epoch);
}
