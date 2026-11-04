#![cfg(windows)]
//! Isolated real authenticated IPC. No provider, microphone or public output.
use foundation_command::Operation as C;
use nai_contracts::workbench::*;
use nai_workbench_host::foundation::{Owner, random_id};
use prost::Message;
use serde_json::{Value, json};
use std::path::PathBuf;
fn call(o: &mut Owner, op: C) -> FoundationResult {
    let bytes = o
        .call(
            &FoundationCommand {
                idempotency_key: random_id().unwrap(),
                operation: Some(op),
            }
            .encode_to_vec(),
        )
        .unwrap();
    match FoundationResponse::decode(bytes.as_slice())
        .unwrap()
        .outcome
        .unwrap()
    {
        foundation_response::Outcome::Result(r) => r,
        foundation_response::Outcome::Problem(p) => {
            panic!("IPC problem {:?}", WorkbenchError::try_from(p.code))
        }
    }
}
fn save(o: &mut Owner, ws: &str, v: Value) -> Value {
    let id = random_id().unwrap();
    let r = call(
        o,
        C::PutPreparation(FoundationObject {
            workspace_id: ws.into(),
            object_id: id.clone(),
            expected_revision: 0,
            body: v.to_string(),
        }),
    );
    json!({"object_id":id,"revision":r.revision,"sha256":r.sha256})
}
#[test]
fn ended_run_note_preview_atomic_drafts_survive_restart_and_source_revoke_blocks_read() {
    let t = tempfile::tempdir().unwrap();
    let base = std::env::var_os("NAI_REVIEW_UI_SEED")
        .map(PathBuf::from)
        .unwrap_or_else(|| t.path().to_path_buf());
    let identity = base.join("operator-v43.json");
    let data = base.join("foundation-v2");
    let engine =
        PathBuf::from(std::env::var_os("NAI_FOUNDATION_ENGINE").expect("build and set engine"));
    let mut o = Owner::open_at(&engine, &identity, &data).unwrap();
    let ws = random_id().unwrap();
    call(
        &mut o,
        C::CreateWorkspace(FoundationWorkspace {
            workspace_id: ws.clone(),
            name: "isolated review acceptance".into(),
            currency: "CNY".into(),
            budget_limit_minor: 0,
        }),
    );
    let product = save(
        &mut o,
        &ws,
        serde_json::from_str(include_str!(
            "../../../tests/fixtures/contracts/preparation.json"
        ))
        .unwrap(),
    );
    let host = save(
        &mut o,
        &ws,
        json!({"schema_version":1,"kind":"host_draft","name":"host","style":"professional","preferences":""}),
    );
    let series = save(
        &mut o,
        &ws,
        json!({"schema_version":1,"kind":"series_draft","name":"series","goal":"specs"}),
    );
    let show = save(
        &mut o,
        &ws,
        json!({"schema_version":1,"kind":"show_draft","name":"本场","product":product,"host":host,"series":series,"cues":[{"id":"intro","title":"规格","talk_track_id":"opening","mandatory_fact_ids":["capacity"],"allow_questions":true}]}),
    );
    let report_id = random_id().unwrap();
    let r = call(
        &mut o,
        C::PublicationImpact(FoundationImpact {
            workspace_id: ws.clone(),
            report_id: report_id.clone(),
            show_id: show["object_id"].as_str().unwrap().into(),
            show_revision: 1,
            previous_snapshot_id: String::new(),
        }),
    );
    let snapshot_id = random_id().unwrap();
    let snap = call(
        &mut o,
        C::Publish(FoundationPublish {
            workspace_id: ws.clone(),
            snapshot_id: snapshot_id.clone(),
            report_id,
            report_sha256: r.sha256,
            acknowledge_unknown: true,
        }),
    );
    let live_id = random_id().unwrap();
    let live = call(
        &mut o,
        C::CreateLiveRun(LiveRunCreate {
            scope: Some(QuestionPoolRead {
                workspace_id: ws.clone(),
                snapshot_id: snapshot_id.clone(),
                snapshot_sha256: snap.sha256.clone(),
            }),
            run_id: live_id.clone(),
        }),
    )
    .live_context
    .unwrap();
    let live_source = ReviewSource {
        workspace_id: ws.clone(),
        run_id: live_id.clone(),
        run_kind: ReviewRunKind::Live as i32,
    };
    let live_question = call(
        &mut o,
        C::EnqueueRunQuestion(ReviewRunQuestion {
            source: Some(live_source.clone()),
            expected_epoch: live.epoch,
            question: Some(QuestionPoolEnqueue {
                scope: Some(QuestionPoolRead {
                    workspace_id: ws.clone(),
                    snapshot_id: snapshot_id.clone(),
                    snapshot_sha256: snap.sha256.clone(),
                }),
                question_id: random_id().unwrap(),
                question: "容量多大？".into(),
            }),
        }),
    );
    assert_eq!(live_question.question_pool.unwrap().entries.len(), 1);
    let read = call(
        &mut o,
        C::ReadRunQuestionPool(RunQuestionPoolRead {
            source: Some(live_source),
            scope: Some(QuestionPoolRead {
                workspace_id: ws.clone(),
                snapshot_id: snapshot_id.clone(),
                snapshot_sha256: snap.sha256.clone(),
            }),
            expected_epoch: live.epoch,
        }),
    );
    assert_eq!(read.question_pool.unwrap().entries.len(), 1);
    call(
        &mut o,
        C::ControlLiveRun(LiveRunControl {
            workspace_id: ws.clone(),
            run_id: live_id.clone(),
            expected_revision: live.revision,
            expected_epoch: live.epoch,
            action: LiveRunAction::End as i32,
        }),
    );
    let live_review = call(
        &mut o,
        C::ReadReview(ReviewSource {
            workspace_id: ws.clone(),
            run_id: live_id,
            run_kind: ReviewRunKind::Live as i32,
        }),
    )
    .review_context
    .unwrap();
    assert!(live_review.facts_unchanged);
    assert_eq!(live_review.questions.len(), 1);
    assert_eq!(live_review.questions[0].delivery, "unconfirmed");
    assert_eq!(live_review.published_faq.len(), 1);
    let run_id = random_id().unwrap();
    let mut doc = json!({"schema_version":1,"kind":"manual_run_draft","name":"本场记录","show":show,"snapshot":{"object_id":snapshot_id,"revision":snap.revision,"sha256":snap.sha256},"cue_ids":["intro"],"cursor":0,"status":"open","marks":[],"mode":"private_manual","evidence":"self_reported"});
    call(
        &mut o,
        C::PutPreparation(FoundationObject {
            workspace_id: ws.clone(),
            object_id: run_id.clone(),
            expected_revision: 0,
            body: doc.to_string(),
        }),
    );
    let source = ReviewSource {
        workspace_id: ws.clone(),
        run_id: run_id.clone(),
        run_kind: ReviewRunKind::Manual as i32,
    };
    let q = call(
        &mut o,
        C::EnqueueRunQuestion(ReviewRunQuestion {
            source: Some(source.clone()),
            expected_epoch: 0,
            question: Some(QuestionPoolEnqueue {
                scope: Some(QuestionPoolRead {
                    workspace_id: ws.clone(),
                    snapshot_id: snapshot_id.clone(),
                    snapshot_sha256: snap.sha256.clone(),
                }),
                question_id: random_id().unwrap(),
                question: "容量多大？".into(),
            }),
        }),
    )
    .question_pool
    .unwrap()
    .entries
    .remove(0);
    call(
        &mut o,
        C::RunQuestion(QuestionPoolRun {
            workspace_id: ws.clone(),
            question_id: q.id,
            expected_revision: q.revision,
            task_id: random_id().unwrap(),
        }),
    );
    doc["status"] = "ended".into();
    call(
        &mut o,
        C::PutPreparation(FoundationObject {
            workspace_id: ws.clone(),
            object_id: run_id.clone(),
            expected_revision: 1,
            body: doc.to_string(),
        }),
    );
    let source = ReviewSource {
        workspace_id: ws.clone(),
        run_id,
        run_kind: ReviewRunKind::Manual as i32,
    };
    let questions = call(&mut o, C::ReadReview(source.clone()))
        .review_context
        .unwrap()
        .questions;
    assert_eq!(questions.len(), 1);
    assert_eq!(questions[0].processing, "waiting_human");
    let note_id = random_id().unwrap();
    let context = call(
        &mut o,
        C::SaveReviewNote(ReviewNoteSave {
            question_source: questions[0].record.clone(),
            source: Some(source.clone()),
            note_id: note_id.clone(),
            expected_revision: 0,
            title: "容量FAQ要更直接".into(),
            proposal: Some(ReviewFaq {
                id: "capacity_question".into(),
                question: "标准款容量是多少？".into(),
                text: "标准款容量为500毫升。".into(),
                fact_ids: vec!["capacity".into()],
            }),
        }),
    )
    .review_context
    .unwrap();
    assert_eq!(context.notes[0].evidence, "self_reported");
    let job_id = random_id().unwrap();
    let product_id = random_id().unwrap();
    let show_id = random_id().unwrap();
    let preview = call(
        &mut o,
        C::PlanReviewDraft(ReviewDraftPlan {
            source: Some(source),
            note_id,
            note_revision: 1,
            job_id: job_id.clone(),
            target_product_id: product_id.clone(),
            target_show_id: show_id.clone(),
            expected_product_revision: context.product.unwrap().revision,
            expected_show_revision: context.show.unwrap().revision,
        }),
    )
    .review_draft
    .unwrap();
    assert_eq!(preview.state, "preview");
    if std::env::var_os("NAI_REVIEW_UI_SEED").is_some() {
        drop(o);
        return;
    }
    drop(o);
    let mut o = Owner::open_at(&engine, &identity, &data).unwrap();
    let apply = ReviewDraftApply {
        workspace_id: ws.clone(),
        job_id: job_id.clone(),
        expected_revision: preview.revision,
        expected_sha256: preview.sha256,
        acknowledge: true,
    };
    for _ in 0..2 {
        let j = call(&mut o, C::ApplyReviewDraft(apply.clone()))
            .review_draft
            .unwrap();
        assert_eq!(j.state, "applied");
        assert_eq!(j.saved_product.unwrap().object_id, product_id);
        assert_eq!(j.saved_show.unwrap().object_id, show_id);
    }
    let p = call(
        &mut o,
        C::ReadObject(FoundationReadObject {
            workspace_id: ws.clone(),
            object_id: product_id.clone(),
            revision: 0,
        }),
    );
    let s = call(
        &mut o,
        C::ReadObject(FoundationReadObject {
            workspace_id: ws.clone(),
            object_id: show_id,
            revision: 0,
        }),
    );
    assert_eq!(
        serde_json::from_str::<Value>(&s.body).unwrap()["product"]["object_id"],
        product_id
    );
    assert_eq!(
        serde_json::from_str::<Value>(&p.body).unwrap()["faq"][0]["question"],
        "标准款容量是多少？"
    );
    call(
        &mut o,
        C::RevokePublication(FoundationPublicationRevoke {
            workspace_id: ws.clone(),
            revocation_id: random_id().unwrap(),
            snapshot_id,
            snapshot_sha256: snap.sha256,
            reason: "验收撤销".into(),
        }),
    );
    let raw = o
        .call(
            &FoundationCommand {
                idempotency_key: random_id().unwrap(),
                operation: Some(C::ReadReviewDraft(ReviewDraftRead {
                    workspace_id: ws,
                    job_id,
                })),
            }
            .encode_to_vec(),
        )
        .unwrap();
    assert!(matches!(
        FoundationResponse::decode(raw.as_slice()).unwrap().outcome,
        Some(foundation_response::Outcome::Problem(_))
    ));
}
