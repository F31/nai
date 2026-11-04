use super::*;
use nai_runtime_core::llm::CredentialVault;
struct Vault;
impl CredentialVault for Vault {
    fn seal(&self, _: &str, _: &str, _: &str, _: &[u8]) -> Result<Value> {
        Ok(json!({"reference":id(600),"nonce":"00".repeat(12),"ciphertext":"aa".repeat(32)}))
    }
    fn open(&self, _: &str, _: &str, _: &str, _: &Value) -> Result<zeroize::Zeroizing<Vec<u8>>> {
        Ok(zeroize::Zeroizing::new(b"synthetic".to_vec()))
    }
}
fn model(rev: u64, voice: &str) -> C {
    C::SaveTtsModel(FoundationTtsModel {
        protocol: "openai_speech".into(),
        language: String::new(),
        workspace_id: id(10),
        model_id: id(100),
        expected_revision: rev,
        name: "synthetic".into(),
        base_url: "https://example.invalid/v1".into(),
        model: "test".into(),
        voice: voice.into(),
        enabled: true,
        api_key: b"synthetic".to_vec(),
        price_known: true,
        character_minor_per_million: 100,
    })
}
fn prepared() -> (TempDir, Store, FoundationSpeechPlan) {
    let (t, mut s) = init();
    s.attach_tts_vault(Arc::new(Vault));
    exec(&mut s, 1, model(0, "test:voice")).unwrap();
    let report = report(&mut s, 85, None);
    let snap = exec(&mut s, 1, publish(90, &report)).unwrap();
    let card = exec(
        &mut s,
        1,
        C::Preview(FoundationPreview {
            workspace_id: id(10),
            preview_id: id(91),
            snapshot_id: id(90),
            snapshot_sha256: snap.sha256,
            cue_id: "intro".into(),
            session_id: id(92),
            expected_epoch: 0,
        }),
    )
    .unwrap();
    let p = FoundationSpeechPlan {
        workspace_id: id(10),
        preview_id: id(91),
        preview_sha256: card.sha256,
        model_id: id(100),
        model_revision: 1,
        max_cost_minor: 5,
        task_id: id(110),
    };
    (t, s, p)
}
fn quote(s: &mut Store, p: &FoundationSpeechPlan) -> SpeechPlanView {
    exec(s, 1, C::CheckPreviewSpeech(p.clone()))
        .unwrap()
        .speech_plan
        .unwrap()
}
fn request(p: &FoundationSpeechPlan, q: &SpeechPlanView) -> C {
    C::SynthesizePreviewSpeech(FoundationSpeechStart {
        plan: Some(p.clone()),
        task_id: p.task_id.clone(),
        approval_sha256: q.approval_sha256.clone(),
    })
}
fn stop(s: &mut Store) {
    exec(
        s,
        1,
        C::StopPreview(FoundationPreviewStop {
            workspace_id: id(10),
            session_id: id(92),
            expected_epoch: 1,
        }),
    )
    .unwrap();
}
#[test]
fn public_candidate_source_is_current_read_only_and_never_a_public_permission() {
    let (_t, mut s, p) = prepared();
    let q = quote(&mut s, &p);
    exec(&mut s, 1, request(&p, &q)).unwrap();
    s.tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &p.task_id)
        .unwrap();
    s.finish_tts(
        &Operation::new(4000).unwrap(),
        &id(1),
        &id(10),
        &p.task_id,
        true,
        "",
        2,
        &"a".repeat(64),
    )
    .unwrap();
    let task = s
        .tts_current(&Operation::new(4000).unwrap(), &id(1), &id(10), &p.task_id)
        .unwrap();
    let x = FoundationPublicCandidate {
        workspace_id: id(10),
        preview_id: p.preview_id.clone(),
        preview_sha256: p.preview_sha256.clone(),
        task_id: p.task_id.clone(),
        task_sha256: task.sha256,
        output_observation: None,
    };
    let count = |s: &Store| {
        s.conn.query_row("SELECT (SELECT COUNT(*) FROM audit_events)+(SELECT COUNT(*) FROM public_media_approvals)+(SELECT COUNT(*) FROM budget_reservations)+(SELECT COUNT(*) FROM command_results)",[],|r|r.get::<_,i64>(0)).unwrap()
    };
    let before = count(&s);
    for _ in 0..2 {
        let c = s
            .public_candidate_source(&Operation::new(4000).unwrap(), &id(1), &x)
            .unwrap();
        assert_eq!(c.text, q.text);
        assert_eq!(c.caption_sha256, hash(q.text.as_bytes()));
        assert!(!c.approval_available);
        assert_eq!(c.audio.unwrap().sample_count, 2);
        assert_eq!(
            c.axes
                .iter()
                .filter(|a| a.state == PublicCandidateAxisState::Passed as i32)
                .count(),
            1
        );
        assert_eq!(count(&s), before);
    }
    let mut observed = s
        .public_candidate_source(&Operation::new(4000).unwrap(), &id(1), &x)
        .unwrap();
    observed.visual = Some(PublicMediaVisual {
        artifact_id: id(501),
        package_sha256: "c".repeat(64),
        license_sha256: "d".repeat(64),
        width: 512,
        height: 288,
        fps: 30,
    });
    observed.output_instance = Some(PublicOutputStatus {
        output_id: id(502),
        protocol_major: 1,
        seq: 1,
        challenge: 1,
        standby_frames: 1,
        window_open: true,
        standby: true,
        reason: PublicOutputReason::StandbyOnly as i32,
        visual: observed.visual.clone(),
        ..Default::default()
    });
    let reviewed = s
        .review_public_candidate(&Operation::new(4000).unwrap(), &id(1), &x, &observed)
        .unwrap();
    assert!(reviewed.policy_passed);
    assert_eq!(reviewed.object.as_ref().unwrap().output_target_id, id(502));
    assert_eq!(count(&s), before);
    let op = Operation::new(4000).unwrap();
    let source = s.public_candidate_review(&op, &id(1), &x).unwrap();
    assert_eq!(source.check().task_sha256, x.task_sha256);
    op.cancel();
    assert!(matches!(source.review(&observed), Err(Error::Cancelled)));
    for mutation in 0..5 {
        let mut bad = observed.clone();
        match mutation {
            0 => {
                bad.text = "另一句".into();
                bad.caption_sha256 = hash(bad.text.as_bytes());
            }
            1 => bad.facts_sha256 = "f".repeat(64),
            2 => bad.epoch += 1,
            3 => bad.audio.as_mut().unwrap().pcm_sha256 = "f".repeat(64),
            _ => bad.preview_sha256 = "f".repeat(64),
        }
        assert!(
            s.review_public_candidate(&Operation::new(4000).unwrap(), &id(1), &x, &bad)
                .is_err()
        );
    }
    let mut missing = observed.clone();
    missing.audio = None;
    assert_eq!(
        s.review_public_candidate(&Operation::new(4000).unwrap(), &id(1), &x, &missing)
            .unwrap()
            .reason,
        PublicGuardReason::MediaNotReady as i32
    );
    for failure in 0..4 {
        let mut bad = x.clone();
        match failure {
            0 => bad.workspace_id = id(11),
            1 => bad.preview_id = id(999),
            2 => bad.task_sha256 = "f".repeat(64),
            _ => bad.preview_sha256 = "f".repeat(64),
        }
        assert!(
            s.public_candidate_source(&Operation::new(4000).unwrap(), &id(1), &bad)
                .is_err()
        );
    }
    stop(&mut s);
    assert!(
        s.review_public_candidate(&Operation::new(4000).unwrap(), &id(1), &x, &observed)
            .is_err()
    );
    assert!(
        s.public_candidate_source(&Operation::new(4000).unwrap(), &id(1), &x)
            .is_err()
    );
}
#[test]
fn speech_plan_is_read_only_and_approval_is_single_task_text_voice_and_limit_bound() {
    let (_t, mut s, p) = prepared();
    let counts = |s: &Store| -> (i64, i64, i64) {
        (
            s.conn
                .query_row("SELECT count(*) FROM objects", [], |r| r.get(0))
                .unwrap(),
            s.conn
                .query_row("SELECT count(*) FROM audit_events", [], |r| r.get(0))
                .unwrap(),
            s.conn
                .query_row("SELECT count(*) FROM budget_reservations", [], |r| r.get(0))
                .unwrap(),
        )
    };
    let before = counts(&s);
    let q = quote(&mut s, &p);
    assert_eq!(before, counts(&s));
    assert_eq!(q.target, "private_native_audio");
    let product: Value = serde_json::from_str(PRODUCT_DRAFT).unwrap();
    assert_eq!(q.text, product["talk_tracks"][0]["text"]);
    let mut wrong = p.clone();
    wrong.task_id = id(111);
    assert_ne!(q.approval_sha256, quote(&mut s, &wrong).approval_sha256);
    assert_eq!(
        exec(&mut s, 1, request(&wrong, &q)),
        Err(Error::RevisionConflict)
    );
    wrong = p.clone();
    wrong.max_cost_minor += 1;
    assert_eq!(
        exec(&mut s, 1, request(&wrong, &q)),
        Err(Error::RevisionConflict)
    );
    assert_eq!(before, counts(&s));
    exec(&mut s, 1, request(&p, &q)).unwrap();
    let dispatch = s
        .tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(110))
        .unwrap();
    assert_eq!(dispatch.text, q.text);
    assert!(
        s.tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(110))
            .is_err()
    );
}
#[test]
fn speech_approval_rechecks_source_session_model_role_and_cross_workspace() {
    for change in 0..5 {
        let (_t, mut s, mut p) = prepared();
        let q = quote(&mut s, &p);
        match change {
            0 => stop(&mut s),
            1 => {
                exec(&mut s, 1, model(1, "test:new-voice")).unwrap();
            }
            2 => {
                s.conn
                    .execute("UPDATE roles SET revision=revision+1", [])
                    .unwrap();
            }
            3 => {
                let snap = read(&mut s, 90);
                exec(
                    &mut s,
                    1,
                    C::RevokePublication(FoundationPublicationRevoke {
                        workspace_id: id(10),
                        revocation_id: id(120),
                        snapshot_id: id(90),
                        snapshot_sha256: snap.sha256,
                        reason: "wrong facts".into(),
                    }),
                )
                .unwrap();
            }
            _ => p.workspace_id = id(999),
        }
        assert!(exec(&mut s, 1, request(&p, &q)).is_err());
        assert_eq!(
            s.conn
                .query_row("SELECT count(*) FROM budget_reservations", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}
#[test]
fn late_speech_result_and_completed_audio_fail_closed_after_stop_or_revocation() {
    for revoke in [false, true] {
        for late in [false, true] {
            let (_t, mut s, p) = prepared();
            let q = quote(&mut s, &p);
            exec(&mut s, 1, request(&p, &q)).unwrap();
            s.tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(110))
                .unwrap();
            if !late {
                s.finish_tts(
                    &Operation::new(4000).unwrap(),
                    &id(1),
                    &id(10),
                    &id(110),
                    true,
                    "",
                    24000,
                    &"aa".repeat(32),
                )
                .unwrap();
            }
            if revoke {
                let snap = read(&mut s, 90);
                exec(
                    &mut s,
                    1,
                    C::RevokePublication(FoundationPublicationRevoke {
                        workspace_id: id(10),
                        revocation_id: id(120),
                        snapshot_id: id(90),
                        snapshot_sha256: snap.sha256,
                        reason: "wrong facts".into(),
                    }),
                )
                .unwrap();
            } else {
                stop(&mut s);
            }
            if late {
                s.finish_tts(
                    &Operation::new(4000).unwrap(),
                    &id(1),
                    &id(10),
                    &id(110),
                    true,
                    "",
                    24000,
                    &"aa".repeat(32),
                )
                .unwrap();
            }
            let r = s
                .tts_current(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(110))
                .unwrap();
            assert!(r.body.is_empty());
            assert_eq!(r.charge_status, "unknown");
            assert!(
                crate::tts_task::commit_gate(
                    &s.conn,
                    &s.root,
                    &s.fence,
                    &Operation::new(4000).unwrap(),
                    &id(1),
                    &id(10),
                    &id(110)
                )
                .is_err()
            );
        }
    }
}
#[test]
fn approved_speech_cancel_and_restart_do_not_recover_permission_or_dispatch() {
    let (t, mut s, p) = prepared();
    let q = quote(&mut s, &p);
    exec(&mut s, 1, request(&p, &q)).unwrap();
    exec(
        &mut s,
        1,
        C::CancelTts(FoundationLlmTaskRead {
            workspace_id: id(10),
            task_id: id(110),
        }),
    )
    .unwrap();
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    let r = s
        .tts_current(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(110))
        .unwrap();
    assert_eq!(r.task_state, AgentTaskState::Cancelled as i32);
    assert_eq!(r.charge_status, "released");
    assert!(r.body.is_empty());
    assert!(
        s.tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(110))
            .is_err()
    );
}
#[test]
fn stopping_one_session_preserves_an_unrelated_approved_audio_source() {
    let (_t, mut s, p) = prepared();
    let q = quote(&mut s, &p);
    exec(&mut s, 1, request(&p, &q)).unwrap();
    s.tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &p.task_id)
        .unwrap();
    s.finish_tts(
        &Operation::new(4000).unwrap(),
        &id(1),
        &id(10),
        &p.task_id,
        true,
        "",
        24000,
        &"aa".repeat(32),
    )
    .unwrap();
    let snap = read(&mut s, 90);
    let card = exec(
        &mut s,
        1,
        C::Preview(FoundationPreview {
            workspace_id: id(10),
            preview_id: id(93),
            snapshot_id: id(90),
            snapshot_sha256: snap.sha256,
            cue_id: "intro".into(),
            session_id: id(94),
            expected_epoch: 0,
        }),
    )
    .unwrap();
    let other = FoundationSpeechPlan {
        preview_id: id(93),
        preview_sha256: card.sha256,
        task_id: id(111),
        ..p.clone()
    };
    let q = quote(&mut s, &other);
    exec(&mut s, 1, request(&other, &q)).unwrap();
    s.tts_dispatch(
        &Operation::new(4000).unwrap(),
        &id(1),
        &id(10),
        &other.task_id,
    )
    .unwrap();
    s.finish_tts(
        &Operation::new(4000).unwrap(),
        &id(1),
        &id(10),
        &other.task_id,
        true,
        "",
        24000,
        &"bb".repeat(32),
    )
    .unwrap();
    stop(&mut s);
    assert!(
        s.tts_current(&Operation::new(4000).unwrap(), &id(1), &id(10), &p.task_id)
            .unwrap()
            .body
            .is_empty()
    );
    assert!(
        !s.tts_current(
            &Operation::new(4000).unwrap(),
            &id(1),
            &id(10),
            &other.task_id
        )
        .unwrap()
        .body
        .is_empty()
    );
}

#[test]
fn unknown_price_tests_skip_reservations_while_published_speech_keeps_confirmed_cap() {
    for published in [false, true] {
        for sent in [false, true] {
            let (_t, mut s, mut p) = prepared();
            let C::SaveTtsModel(mut m) = model(1, "test:voice") else {
                unreachable!()
            };
            m.price_known = false;
            m.character_minor_per_million = 0;
            exec(&mut s, 1, C::SaveTtsModel(m)).unwrap();
            p.model_revision = 2;
            let q = quote(&mut s, &p);
            assert!(!q.estimate_known);
            assert_eq!(q.reserve_minor, p.max_cost_minor);
            assert_eq!(q.target, "private_native_audio");
            let action = if published {
                request(&p, &q)
            } else {
                C::TestTts(FoundationTtsTest {
                    workspace_id: p.workspace_id.clone(),
                    task_id: p.task_id.clone(),
                    model_id: p.model_id.clone(),
                    model_revision: p.model_revision,
                })
            };
            let queued = exec(&mut s, 1, action).unwrap();
            assert_eq!(
                queued.reserved_minor,
                if published { p.max_cost_minor } else { 0 }
            );
            s.tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &p.task_id)
                .unwrap();
            s.finish_tts(
                &Operation::new(4000).unwrap(),
                &id(1),
                &id(10),
                &p.task_id,
                sent,
                if sent { "" } else { "CANCELLED" },
                if sent { 24000 } else { 0 },
                if sent {
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                } else {
                    ""
                },
            )
            .unwrap();
            let r = exec(
                &mut s,
                1,
                C::ReadTtsTask(FoundationLlmTaskRead {
                    workspace_id: id(10),
                    task_id: p.task_id.clone(),
                }),
            )
            .unwrap();
            assert_eq!(
                r.charge_status,
                if sent {
                    "unknown"
                } else if published {
                    "released"
                } else {
                    "not_sent"
                }
            );
            assert_eq!(
                r.llm_budget.map_or(0, |b| b.unknown_minor),
                if sent && published {
                    p.max_cost_minor
                } else {
                    0
                }
            );
        }
    }
}

#[test]
fn speech_plan_and_task_share_the_question_pool_source_identity() {
    let (_t, mut s, p) = prepared();
    let q = quote(&mut s, &p);
    let c = q.agent_context.clone().unwrap();
    let pool = exec(
        &mut s,
        1,
        C::ReadQuestionPool(QuestionPoolRead {
            workspace_id: id(10),
            snapshot_id: c.snapshot_id.clone(),
            snapshot_sha256: c.snapshot_sha256.clone(),
        }),
    )
    .unwrap();
    assert_eq!(Some(c.clone()), pool.question_pool.unwrap().context);
    let queued = exec(&mut s, 1, request(&p, &q)).unwrap();
    assert_eq!(queued.agent_context, Some(c));
    let dispatch = s
        .tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &p.task_id)
        .unwrap();
    assert!(dispatch.deadline_ms > 0 && dispatch.deadline_ms <= 15000);
}
#[test]
fn speech_context_missing_or_source_changed_rejects_late_audio_with_unknown_charge() {
    for case in 0..3 {
        let (_t, mut s, p) = prepared();
        let q = quote(&mut s, &p);
        exec(&mut s, 1, request(&p, &q)).unwrap();
        s.tts_dispatch(&Operation::new(4000).unwrap(), &id(1), &id(10), &p.task_id)
            .unwrap();
        if case == 2 {
            save(&mut s, 80, 1, serde_json::from_str(PRODUCT_DRAFT).unwrap());
        } else {
            let (mut v, rev, _) = crate::llm::raw(
                &s.conn,
                &s.root,
                &id(10),
                &p.task_id,
                0,
                "publication_tts_preview",
            )
            .unwrap();
            if case == 0 {
                v.as_object_mut().unwrap().remove("task_context");
            } else {
                v["task_context"]["context_sha256"] = "b".repeat(64).into();
            }
            let tx = s.conn.transaction().unwrap();
            crate::llm::put(&tx, &s.root, &id(10), &p.task_id, rev, &v, utc().unwrap()).unwrap();
            tx.commit().unwrap();
        }
        s.finish_tts(
            &Operation::new(4000).unwrap(),
            &id(1),
            &id(10),
            &p.task_id,
            true,
            "",
            2,
            &"a".repeat(64),
        )
        .unwrap();
        let r = s
            .tts_current(&Operation::new(4000).unwrap(), &id(1), &id(10), &p.task_id)
            .unwrap();
        assert!(r.body.is_empty());
        assert!(r.agent_context.is_none());
        assert_eq!(r.charge_status, "unknown");
        assert_ne!(r.task_state, AgentTaskState::Completed as i32);
    }
}
