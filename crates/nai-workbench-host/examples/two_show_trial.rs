//! Explicit real-material engineering trial; never a natural-user/platform success claim.
//! Uses local Qwen, at most five bounded calls, no retry, recording, microphone or streaming.
use nai_contracts::workbench::{foundation_command::Operation as C, *};
use nai_workbench_host::{
    foundation::{Owner, random_id},
    public_output::OutputOwner,
};
use prost::Message;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn id() -> String {
    random_id().expect("random identity")
}
fn command(op: C) -> Vec<u8> {
    FoundationCommand {
        idempotency_key: id(),
        operation: Some(op),
    }
    .encode_to_vec()
}
fn result(raw: Vec<u8>) -> FoundationResult {
    match FoundationResponse::decode(raw.as_slice())
        .expect("response")
        .outcome
        .expect("outcome")
    {
        foundation_response::Outcome::Result(r) => r,
        foundation_response::Outcome::Problem(p) => {
            panic!("root refused: {:?}", WorkbenchError::try_from(p.code))
        }
    }
}
fn call(o: &mut Owner, op: C) -> FoundationResult {
    result(o.call(&command(op)).expect("IPC"))
}
fn media(o: &mut Owner, p: &mut OutputOwner, root: &Path, op: C) -> FoundationResult {
    result(
        o.call_with_public_output(&command(op), Some(p), root)
            .expect("media IPC"),
    )
}
fn event(dir: &Path, phase: &str, data: Value) {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis();
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("events.jsonl"))
        .expect("events");
    writeln!(f, "{}", json!({"unix_ms":ms,"phase":phase,"data":data})).expect("event write");
}
fn save(o: &mut Owner, ws: &str, body: Value) -> Value {
    let n = id();
    let r = call(
        o,
        C::PutPreparation(FoundationObject {
            workspace_id: ws.into(),
            object_id: n.clone(),
            expected_revision: 0,
            body: body.to_string(),
        }),
    );
    json!({"object_id":n,"revision":r.revision,"sha256":r.sha256})
}
fn read(o: &mut Owner, ws: &str, n: &str) -> FoundationResult {
    call(
        o,
        C::ReadObject(FoundationReadObject {
            workspace_id: ws.into(),
            object_id: n.into(),
            revision: 0,
        }),
    )
}
fn publish(o: &mut Owner, ws: &str, show: &Value) -> (String, FoundationResult) {
    let report = id();
    let r = call(
        o,
        C::PublicationImpact(FoundationImpact {
            workspace_id: ws.into(),
            report_id: report.clone(),
            show_id: show["object_id"].as_str().expect("show").into(),
            show_revision: show["revision"].as_u64().expect("revision"),
            previous_snapshot_id: String::new(),
        }),
    );
    let snap = id();
    let r = call(
        o,
        C::Publish(FoundationPublish {
            workspace_id: ws.into(),
            snapshot_id: snap.clone(),
            report_id: report,
            report_sha256: r.sha256,
            acknowledge_unknown: true,
        }),
    );
    (snap, r)
}
#[allow(clippy::too_many_arguments)]
fn perform(
    o: &mut Owner,
    root: &Path,
    dir: &Path,
    ws: &str,
    snap: &str,
    sha: &str,
    model: &str,
    keeper: &Path,
    label: &str,
) -> (ReviewSource, Value) {
    let began = Instant::now();
    let run = id();
    let scope = LiveRunRead {
        workspace_id: ws.into(),
        run_id: run.clone(),
    };
    let c = call(
        o,
        C::CreateLiveRun(LiveRunCreate {
            scope: Some(QuestionPoolRead {
                workspace_id: ws.into(),
                snapshot_id: snap.into(),
                snapshot_sha256: sha.into(),
            }),
            run_id: run.clone(),
        }),
    )
    .live_context
    .expect("live context");
    let p = call(o, C::ReadLiveSpeechPlan(scope.clone()))
        .live_speech_plan
        .expect("plan");
    let c = call(
        o,
        C::ControlLiveBatch(LiveBatchControl {
            run: Some(scope.clone()),
            expected_revision: c.revision,
            expected_epoch: c.epoch,
            approval_id: id(),
            plan_sha256: p.plan_sha256.clone(),
            acknowledge_rights: true,
            revoke: false,
        }),
    )
    .live_context
    .expect("approved scope");
    let source = ReviewSource {
        workspace_id: ws.into(),
        run_id: run,
        run_kind: ReviewRunKind::Live as i32,
    };
    let mut output = OutputOwner::open_at(keeper, root, false).expect("public output");
    let grant = id();
    media(
        o,
        &mut output,
        root,
        C::StartLiveAuto(LiveAutoStart {
            run: Some(scope.clone()),
            grant_id: grant.clone(),
            expected_revision: c.revision,
            expected_epoch: c.epoch,
            plan_sha256: p.plan_sha256,
            model_id: model.into(),
            model_revision: 1,
            max_cost_minor: 40,
            acknowledge_rights: true,
            ..Default::default()
        }),
    );
    let mut question = None;
    let mut tasks = BTreeSet::new();
    let mut captions = BTreeSet::new();
    let mut audio = false;
    let mut mouth = false;
    let mut devices = BTreeSet::new();
    let mut submitted = BTreeMap::new();
    let mut last = String::new();
    loop {
        assert!(
            began.elapsed() < Duration::from_secs(90),
            "bounded trial timed out; no automatic retry"
        );
        let status = o
            .renew_public_output(&mut output, root)
            .unwrap_or_else(|e| {
                panic!("healthy public output: {e}; status={:?}", output.current())
            });
        let auto = media(o, &mut output, root, C::ReadLiveAuto(scope.clone()));
        let view = auto.live_auto.expect("auto");
        for t in &view.tasks {
            tasks.insert(t.task_id.clone());
        }
        audio |= status.audio_active;
        if let Some(p) = &status.presentation
            && p.caption_visible
            && p.presentation_count > 0
            && p.composited_sha256.len() == 64
        {
            for text in [
                "本机工作台支持手动提词。",
                "本机工作台支持节目编排。",
                "本机工作台支持手动提词。本机工作台支持节目编排。",
            ] {
                if p.caption_sha256 == nai_contracts::public_caption_hash(text) {
                    captions.insert(text.to_owned());
                }
            }
        }
        if let Some(p) = &status.window_prepared {
            if p.device_ready && !p.device_name.is_empty() {
                devices.insert(p.device_name.clone());
            }
            mouth |= p.frame.as_ref().is_some_and(|f| f.openness_q16 > 0);
            if p.submitted_samples > 0 {
                submitted.insert(p.preparation_id.clone(), p.submitted_samples);
            }
        }
        let marker = format!(
            "{}:{}:{}",
            view.state, view.interruption_state, view.sentence_cursor
        );
        if marker != last {
            event(
                dir,
                "state",
                json!({"show":label,"run_id":source.run_id,"state":view.state,"interruption":view.interruption_state,"cursor":view.sentence_cursor,"reserved_minor":view.reserved_minor}),
            );
            last = marker;
        }
        if status.audio_active && question.is_none() {
            let q = id();
            let e = media(
                o,
                &mut output,
                root,
                C::EnqueueRunQuestion(ReviewRunQuestion {
                    source: Some(source.clone()),
                    expected_epoch: c.epoch,
                    question: Some(QuestionPoolEnqueue {
                        scope: Some(QuestionPoolRead {
                            workspace_id: ws.into(),
                            snapshot_id: snap.into(),
                            snapshot_sha256: sha.into(),
                        }),
                        question_id: q.clone(),
                        question: "如何准备直播内容？".into(),
                    }),
                }),
            )
            .question_pool
            .expect("question pool")
            .entries
            .remove(0);
            let e = media(
                o,
                &mut output,
                root,
                C::RunQuestion(QuestionPoolRun {
                    workspace_id: ws.into(),
                    question_id: e.id,
                    expected_revision: e.revision,
                    task_id: id(),
                }),
            )
            .question_pool
            .expect("question result")
            .entries
            .remove(0);
            let fresh = media(o, &mut output, root, C::ReadLiveAuto(scope.clone()));
            media(
                o,
                &mut output,
                root,
                C::ControlLiveQuestion(LiveQuestionControl {
                    run: Some(scope.clone()),
                    grant_id: grant.clone(),
                    interruption_id: id(),
                    faq_task_id: e.task_id,
                    faq_task_sha256: e.task_sha256,
                    expected_revision: fresh.revision,
                    expected_epoch: fresh.epoch,
                    acknowledge_answer: true,
                    cancel: false,
                }),
            );
            event(
                dir,
                "operator_scripted_question",
                json!({"show":label,"question_id":q,"natural_audience_question":false}),
            );
            question = Some(q);
        }
        if view.state == "completed" && view.interruption_state == "completed" {
            break;
        }
        assert!(
            !["failed", "stopped", "blocked"].contains(&view.state.as_str()),
            "trial stopped: {} {}",
            view.state,
            view.reason
        );
        std::thread::sleep(Duration::from_millis(40));
    }
    let current = media(o, &mut output, root, C::ReadLiveRun(scope.clone()))
        .live_context
        .expect("checkpoint");
    media(
        o,
        &mut output,
        root,
        C::ControlLiveRun(LiveRunControl {
            workspace_id: ws.into(),
            run_id: scope.run_id.clone(),
            expected_revision: current.revision,
            expected_epoch: current.epoch,
            action: LiveRunAction::End as i32,
        }),
    );
    assert!(
        audio && mouth && !captions.is_empty(),
        "actual audio, caption and mouth progress required"
    );
    assert!(tasks.len() <= 3, "bounded synthesis count per show");
    let mut charges = vec![];
    for task in tasks {
        let r = media(
            o,
            &mut output,
            root,
            C::ReadTtsTask(FoundationLlmTaskRead {
                workspace_id: ws.into(),
                task_id: task.clone(),
            }),
        );
        charges.push(json!({"task_id":task,"state":r.task_state,"charge_status":r.charge_status,"actual_minor":if r.charge_status=="known" {Some(r.actual_minor)} else {None},"reason":r.task_reason}));
    }
    event(
        dir,
        "show_finished",
        json!({"show":label,"elapsed_ms":began.elapsed().as_millis(),"submitted_sentences":current.submitted_sentences,"question_id":question,"audio_observed":audio,"mouth_observed":mouth,"captions":captions}),
    );
    (
        source,
        json!({"run_id":scope.run_id,"snapshot_id":snap,"snapshot_sha256":sha,"elapsed_ms":began.elapsed().as_millis(),"question_id":question,"audio_observed":audio,"mouth_observed":mouth,"captions":captions,"output_devices":devices,"software_submissions":submitted,"model_tasks":charges,"human_active_time_ms":null,"hardware_completion":"unknown","platform_completion":"not_tested"}),
    )
}
fn run(dir: &Path, product_path: &Path) -> Value {
    let start = Instant::now();
    let root = dir.join("foundation-v2");
    let engine = PathBuf::from(std::env::var_os("NAI_FOUNDATION_ENGINE").expect("explicit engine"));
    let keeper =
        PathBuf::from(std::env::var_os("NAI_PUBLIC_OUTPUT_KEEPER").expect("explicit keeper"));
    let mut owner =
        Owner::open_at(&engine, &dir.join("operator-v43.json"), &root).expect("trial owner");
    let ws = id();
    call(
        &mut owner,
        C::CreateWorkspace(FoundationWorkspace {
            workspace_id: ws.clone(),
            name: "nai真实资料两场工程验证（非自然用户试点）".into(),
            currency: "CNY".into(),
            budget_limit_minor: 100,
        }),
    );
    let product: Value = serde_json::from_slice(&fs::read(product_path).expect("product source"))
        .expect("product JSON");
    let product = save(&mut owner, &ws, product);
    let host = save(
        &mut owner,
        &ws,
        json!({"schema_version":1,"kind":"host_draft","name":"nai产品介绍","style":"professional","preferences":"仅讲已核对的本机功能"}),
    );
    let series = save(
        &mut owner,
        &ws,
        json!({"schema_version":1,"kind":"series_draft","name":"nai产品功能","goal":"两场核对实际FAQ复用"}),
    );
    let show = save(
        &mut owner,
        &ws,
        json!({"schema_version":1,"kind":"show_draft","name":"nai功能介绍首场","product":product,"host":host,"series":series,"cues":[{"id":"intro","title":"本机功能","talk_track_id":"opening","mandatory_fact_ids":["prompter"],"allow_questions":true}]}),
    );
    let model = id();
    call(
        &mut owner,
        C::SaveTtsModel(FoundationTtsModel {
            workspace_id: ws.clone(),
            model_id: model.clone(),
            name: "本地Qwen有界验收".into(),
            base_url: "http://127.0.0.1:8000/v1".into(),
            model: "qwen3-tts".into(),
            voice: "serena".into(),
            language: "chinese".into(),
            protocol: "qwen_local".into(),
            enabled: true,
            ..Default::default()
        }),
    );
    let (snap, first_snapshot) = publish(&mut owner, &ws, &show);
    event(
        dir,
        "first_published",
        json!({"workspace_id":ws,"product":product,"show":show,"snapshot_id":snap,"snapshot_sha256":first_snapshot.sha256}),
    );
    let (source, first) = perform(
        &mut owner,
        &root,
        dir,
        &ws,
        &snap,
        &first_snapshot.sha256,
        &model,
        &keeper,
        "first",
    );
    fs::write(
        dir.join("first-show.json"),
        serde_json::to_vec_pretty(&first).expect("first JSON"),
    )
    .expect("first evidence");
    let review_started = Instant::now();
    let context = call(&mut owner, C::ReadReview(source.clone()))
        .review_context
        .expect("review");
    assert_eq!(context.questions.len(), 1);
    assert_eq!(context.questions[0].delivery, "submitted");
    let note = id();
    let context = call(
        &mut owner,
        C::SaveReviewNote(ReviewNoteSave {
            source: Some(source.clone()),
            note_id: note.clone(),
            expected_revision: 0,
            title: "说明节目编排如何辅助备播".into(),
            proposal: Some(ReviewFaq {
                id: "prepare".into(),
                question: "如何准备直播内容？".into(),
                text: "本机工作台支持节目编排。".into(),
                fact_ids: vec!["program".into()],
            }),
            question_source: context.questions[0].record.clone(),
        }),
    )
    .review_context
    .expect("saved note");
    let job = call(
        &mut owner,
        C::PlanReviewDraft(ReviewDraftPlan {
            source: Some(source),
            note_id: note,
            note_revision: context.notes[0].revision,
            job_id: id(),
            target_product_id: id(),
            target_show_id: id(),
            expected_product_revision: context.product.expect("product").revision,
            expected_show_revision: context.show.expect("show").revision,
        }),
    )
    .review_draft
    .expect("preview");
    assert_eq!(job.reason, "READY");
    let applied = call(
        &mut owner,
        C::ApplyReviewDraft(ReviewDraftApply {
            workspace_id: ws.clone(),
            job_id: job.job_id.clone(),
            expected_revision: job.revision,
            expected_sha256: job.sha256,
            acknowledge: true,
        }),
    )
    .review_draft
    .expect("applied");
    assert_eq!(applied.state, "applied");
    let saved_show = applied.saved_show.as_ref().expect("new show");
    let next_show = json!({"object_id":saved_show.object_id,"revision":saved_show.revision,"sha256":saved_show.sha256});
    let (next_snap, next_snapshot) = publish(&mut owner, &ws, &next_show);
    event(
        dir,
        "review_republished",
        json!({"job_id":job.job_id,"saved_product":applied.saved_product.as_ref().map(|p|json!({"id":p.object_id,"revision":p.revision,"sha256":p.sha256})),"show":next_show,"snapshot_id":next_snap,"snapshot_sha256":next_snapshot.sha256,"elapsed_ms":review_started.elapsed().as_millis()}),
    );
    let review_elapsed_ms = review_started.elapsed().as_millis();
    fs::write(dir.join("review.json"), serde_json::to_vec_pretty(&json!({"job_id":job.job_id,"before":job.before.as_ref().map(|f|&f.text),"proposal":job.proposal.as_ref().map(|f|&f.text),"machine_elapsed_ms":review_elapsed_ms,"new_snapshot_id":next_snap,"new_snapshot_sha256":next_snapshot.sha256})).expect("review JSON")).expect("review evidence");
    let (second_source, second) = perform(
        &mut owner,
        &root,
        dir,
        &ws,
        &next_snap,
        &next_snapshot.sha256,
        &model,
        &keeper,
        "second",
    );
    fs::write(
        dir.join("second-show.json"),
        serde_json::to_vec_pretty(&second).expect("second JSON"),
    )
    .expect("second evidence");
    let final_review = call(&mut owner, C::ReadReview(second_source))
        .review_context
        .expect("second review");
    assert_eq!(final_review.questions[0].delivery, "submitted");
    assert!(
        second["captions"]
            .as_array()
            .expect("captions")
            .iter()
            .any(|s| s
                .as_str()
                .is_some_and(|t| t.contains("本机工作台支持节目编排。"))),
        "second show must actually submit the added FAQ fact"
    );
    let original = read(
        &mut owner,
        &ws,
        product["object_id"].as_str().expect("original product"),
    );
    assert_eq!(
        original.sha256.as_str(),
        product["sha256"].as_str().expect("source hash")
    );
    json!({"scope":"real_material_local_engineering_two_shows","natural_user_trial":false,"source_document":"docs/nai-v4.8-technical-whitepaper.docx","product_source":product_path.display().to_string(),"workspace_id":ws,"first":first,"second":second,"review_job_id":job.job_id,"review_machine_elapsed_ms":review_elapsed_ms,"total_machine_elapsed_ms":start.elapsed().as_millis(),"human_baseline_ms":null,"human_active_time_ms":null,"model_call_limit":5,"automatic_retries":0,"recording":false,"microphone":false,"streaming":false,"acc_upgraded":false})
}
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    assert_eq!(
        args.len(),
        3,
        "two_show_trial <new-evidence-dir> <product.json>"
    );
    let dir = PathBuf::from(&args[1]);
    let product = PathBuf::from(&args[2]);
    assert!(
        dir.is_absolute() && product.is_absolute(),
        "absolute paths required"
    );
    fs::create_dir_all(&dir).expect("evidence directory");
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join("started.once"))
        .expect("fresh ledger required; do not delete to retry");
    let outcome = std::panic::catch_unwind(|| run(&dir, &product));
    let (value, ok) = match outcome {
        Ok(v) => (json!({"status":"passed","evidence":v}), true),
        Err(_) => (
            json!({"status":"failed","reason":"see retained events and stderr; no automatic retry","natural_user_trial":false}),
            false,
        ),
    };
    fs::write(
        dir.join("result.json"),
        serde_json::to_vec_pretty(&value).expect("JSON"),
    )
    .expect("result");
    if !ok {
        std::process::exit(1);
    }
    println!("REAL_MATERIAL_TWO_SHOW_ENGINEERING_PASSED; not natural-user/platform certification");
}
