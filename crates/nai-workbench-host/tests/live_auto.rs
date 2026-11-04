#![cfg(windows)]
//! Real native orchestration. Synthetic HTTP/PCM and real local Qwen modes are distinct.
use nai_contracts::workbench::*;
use nai_workbench_host::{
    foundation::{Owner, random_id},
    public_output::OutputOwner,
};
use prost::Message;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
fn command(op: foundation_command::Operation) -> Vec<u8> {
    FoundationCommand {
        idempotency_key: random_id().unwrap(),
        operation: Some(op),
    }
    .encode_to_vec()
}
fn av_event(value: Value) {
    if let Some(dir) = std::env::var_os("NAI_AV_ACCEPTANCE_DIRECTORY") {
        use std::fs::OpenOptions;
        let mut v = value;
        v["unix_ms"] = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64)
            .into();
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(PathBuf::from(dir).join("timeline.jsonl"))
            .unwrap();
        writeln!(f, "{v}").unwrap();
    }
}
fn result(raw: Vec<u8>) -> FoundationResult {
    match FoundationResponse::decode(raw.as_slice())
        .unwrap()
        .outcome
        .unwrap()
    {
        foundation_response::Outcome::Result(r) => r,
        foundation_response::Outcome::Problem(p) => panic!(
            "native root rejected {:?}",
            WorkbenchError::try_from(p.code)
        ),
    }
}
fn call(o: &mut Owner, op: foundation_command::Operation) -> FoundationResult {
    result(o.call(&command(op)).unwrap())
}
fn serve(
    count: usize,
    fail: bool,
    delay_third: bool,
    slow_faq: bool,
) -> (String, Arc<AtomicUsize>, std::thread::JoinHandle<()>) {
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1", server.local_addr().unwrap());
    server.set_nonblocking(true).unwrap();
    let seen = Arc::new(AtomicUsize::new(0));
    let track = seen.clone();
    let worker = std::thread::spawn(move || {
        let end = Instant::now() + Duration::from_secs(60);
        while track.load(Ordering::SeqCst) < count {
            assert!(Instant::now() < end, "synthetic provider requests missing");
            let (mut s, _) = match server.accept() {
                Ok(s) => s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(e) => panic!("{e}"),
            };
            s.set_nonblocking(false).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut bytes = Vec::new();
            let mut buf = [0; 4096];
            let header = loop {
                let n = s.read(&mut buf).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buf[..n]);
                assert!(bytes.len() < 16384);
                if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    break i + 4;
                }
            };
            let h = String::from_utf8_lossy(&bytes[..header]);
            assert!(h.starts_with("POST /v1/audio/speech "));
            let len = h
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|n| n.trim().parse::<usize>().unwrap())
                })
                .unwrap();
            while bytes.len() < header + len {
                let n = s.read(&mut buf).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buf[..n]);
            }
            let body: Value = serde_json::from_slice(&bytes[header..header + len]).unwrap();
            let index = track.fetch_add(1, Ordering::SeqCst);
            assert_eq!(
                body["input"],
                if slow_faq {
                    "标准款容量为500毫升。"
                } else if count == 4 {
                    [
                        "标准款容量为500毫升。",
                        "标准款重量为100克。",
                        "标准款容量为500毫升。",
                        "标准款长度为10厘米。",
                    ][index]
                } else {
                    [
                        "标准款容量为500毫升。",
                        "标准款重量为100克。",
                        "标准款长度为10厘米。",
                    ][index]
                }
            );
            if fail {
                s.write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                continue;
            }
            if slow_faq && index == 1 {
                std::thread::sleep(Duration::from_millis(11000));
            }
            if delay_third && index == 2 {
                std::thread::sleep(Duration::from_millis(2500));
            }
            let samples = if slow_faq && index == 1 {
                144000u32
            } else {
                48000u32
            };
            let mut wav = Vec::new();
            wav.extend(b"RIFF");
            wav.extend((36 + samples * 2).to_le_bytes());
            wav.extend(b"WAVEfmt ");
            wav.extend(16u32.to_le_bytes());
            wav.extend(1u16.to_le_bytes());
            wav.extend(1u16.to_le_bytes());
            wav.extend(24000u32.to_le_bytes());
            wav.extend(48000u32.to_le_bytes());
            wav.extend(2u16.to_le_bytes());
            wav.extend(16u16.to_le_bytes());
            wav.extend(b"data");
            wav.extend((samples * 2).to_le_bytes());
            wav.resize(44 + samples as usize * 2, 0);
            write!(s,"HTTP/1.1 200 OK\r\nContent-Type: audio/wav\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",wav.len()).unwrap();
            s.write_all(&wav).unwrap();
        }
    });
    (url, seen, worker)
}
#[test]
#[ignore = "requires Windows default audio device; three synthetic silent clips, no external API"]
fn native_auto_prefetch_and_three_public_submissions() {
    run(false, 0);
}
#[test]
#[ignore = "requires authorized local Qwen at 127.0.0.1:8000 and Windows audio device; audible local output, no streaming"]
fn native_auto_real_local_qwen_two_public_sentences() {
    run(true, 0);
}
#[test]
#[ignore = "requires Windows audio device; synthetic silent pause/resume"]
fn native_auto_pause_at_boundary_and_resume() {
    run(false, 1);
}
#[test]
#[ignore = "requires Windows audio device; synthetic silent explicit stop"]
fn native_auto_stop_does_not_consume_next() {
    run(false, 2);
}
#[test]
#[ignore = "requires Windows public output; synthetic provider failure, no playback"]
fn native_auto_failed_tts_never_retries_or_plays() {
    run(false, 3);
}
#[test]
#[ignore = "requires Windows audio; verifies a delayed third synthesis overlaps current output"]
fn native_auto_prefetch_runs_while_current_audio_plays() {
    run(false, 4);
}
#[test]
#[ignore = "requires Windows public output and default audio; synthetic FAQ interruption"]
fn native_auto_question_answer_returns_to_unspoken_sentence() {
    run(false, 5);
}
#[test]
#[ignore = "requires local Qwen and Windows audio; three actual local calls, no streaming"]
fn native_auto_question_real_local_qwen_returns() {
    run(true, 5);
}
#[test]
#[ignore = "requires explicit authorized microphone/headphone IDs and Windows public output; no recording"]
fn native_auto_human_takeover_and_partial_resume() {
    run(false, 6);
}
#[test]
#[ignore = "requires authorized local Qwen, explicit headphones, up to three real requests; no retry"]
fn native_auto_real_local_qwen_human_takeover_resumes() {
    run(true, 7);
}
#[test]
#[ignore = "explicit headphones and isolated OBS; ten-minute media soak, zero model calls"]
fn native_public_human_standby_soak() {
    use nai_workbench_host::human_mic::MicOwner;
    let dir = PathBuf::from(std::env::var_os("NAI_AV_ACCEPTANCE_DIRECTORY").unwrap());
    let root = dir.join("media");
    std::fs::create_dir_all(&root).unwrap();
    let mut mic = MicOwner::open_at(
        &PathBuf::from(std::env::var_os("NAI_HUMAN_MIC_WORKER").unwrap()),
        &root,
    )
    .unwrap();
    let instance = mic.status.instance_id.clone();
    let input = std::env::var("NAI_MIC_ACCEPTANCE_INPUT")
        .unwrap()
        .parse()
        .unwrap();
    let out = std::env::var("NAI_MIC_ACCEPTANCE_OUTPUT")
        .unwrap()
        .parse()
        .unwrap();
    assert!(mic.start(&instance, input, out, true).unwrap().active);
    assert!(!mic.halt().unwrap().active);
    assert!(!mic.poll().unwrap().active);
    let mut output = OutputOwner::open_at(
        &PathBuf::from(std::env::var_os("NAI_PUBLIC_OUTPUT_KEEPER").unwrap()),
        &root,
        false,
    )
    .unwrap();
    av_event(json!({"phase":"windows_ready","model_calls":0}));
    let ready = dir.join("recording.ready");
    let until = Instant::now() + Duration::from_secs(180);
    while !ready.is_file() {
        assert!(Instant::now() < until, "OBS start deadline");
        let p = output.poll().unwrap();
        assert!(!p.terminal && !p.audio_active);
        assert!(!mic.poll().unwrap().active);
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        mic.start_with_standby(&instance, input, out, true, &mut output)
            .unwrap()
            .active
    );
    av_event(json!({"phase":"soak_human_started","seconds":600,"model_calls":0}));
    let start = Instant::now();
    let mut previous = start;
    let mut max_gap = Duration::ZERO;
    let mut ticks = 0u64;
    while start.elapsed() < Duration::from_secs(600) {
        let tick = Instant::now();
        max_gap = max_gap.max(tick.duration_since(previous));
        previous = tick;
        let m = mic.poll().unwrap();
        let p = output.poll().unwrap();
        if !m.active || p.terminal || p.audio_active {
            av_event(
                json!({"phase":"soak_failed","public_reason":p.reason,"terminal":p.terminal,"human_reason":m.reason,"gap_ms":max_gap.as_millis() as u64}),
            );
            panic!("media soak stopped: public={} human={}", p.reason, m.reason);
        }
        ticks += 1;
        if ticks.is_multiple_of(100) {
            av_event(
                json!({"phase":"soak_tick","ticks":ticks,"captured":m.captured_frames,"submitted":m.submitted_frames,"dropped":m.dropped_frames,"underrun":m.underrun_frames,"peak_q16":m.peak_q16,"max_gap_ms":max_gap.as_millis() as u64}),
            );
        }
        std::thread::sleep((Duration::from_millis(100)).saturating_sub(tick.elapsed()));
    }
    let m = mic.halt().unwrap();
    assert!(!m.active);
    av_event(
        json!({"phase":"soak_completed","seconds":start.elapsed().as_secs_f64(),"ticks":ticks,"captured":m.captured_frames,"submitted":m.submitted_frames,"dropped":m.dropped_frames,"underrun":m.underrun_frames,"max_gap_ms":max_gap.as_millis() as u64,"model_calls":0}),
    );
    // Leave actual silent windows visible briefly so the recording observes post-stop state.
    for _ in 0..250 {
        let p = output.poll().unwrap();
        assert!(!p.terminal && !p.audio_active);
        std::thread::sleep(Duration::from_millis(100));
    }
}
#[test]
#[ignore = "requires Windows default audio device; expiry-bound synthetic FAQ response, no actual model"]
fn native_short_playback_window_is_refused_before_audio() {
    run(false, 8);
}
fn run(real: bool, mode: u8) {
    use foundation_command::Operation as C;
    let count = if mode == 8 {
        1
    } else if real {
        2
    } else {
        3
    };
    let mock = if real {
        None
    } else {
        Some(serve(
            if mode == 2 || mode == 6 || mode == 8 {
                2
            } else if mode == 3 {
                1
            } else {
                count + usize::from(mode == 5)
            },
            mode == 3,
            mode == 4,
            mode == 8,
        ))
    };
    let endpoint = mock
        .as_ref()
        .map(|m| m.0.clone())
        .unwrap_or("http://127.0.0.1:8000/v1".into());
    let t = tempfile::tempdir().unwrap();
    let acceptance = std::env::var_os("NAI_AV_ACCEPTANCE_DIRECTORY").map(PathBuf::from);
    let test_path = acceptance.as_deref().unwrap_or(t.path());
    let root = test_path.join("data");
    let engine = PathBuf::from(std::env::var_os("NAI_FOUNDATION_ENGINE").unwrap());
    let keeper = PathBuf::from(std::env::var_os("NAI_PUBLIC_OUTPUT_KEEPER").unwrap());
    let mut owner = Owner::open_at(&engine, &test_path.join("operator.json"), &root).unwrap();
    let ws = random_id().unwrap();
    call(
        &mut owner,
        C::CreateWorkspace(FoundationWorkspace {
            workspace_id: ws.clone(),
            name: "isolated auto acceptance".into(),
            budget_limit_minor: 100,
            currency: "CNY".into(),
        }),
    );
    let mut product: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/contracts/preparation.json"
    ))
    .unwrap();
    for (id, label, value, unit) in [
        ("weight", "重量", "100", "克"),
        ("length", "长度", "10", "厘米"),
    ]
    .into_iter()
    .take(count - 1)
    {
        let mut f = product["facts"][0].clone();
        f["id"] = id.into();
        f["label"] = label.into();
        f["value"] = value.into();
        f["unit"] = unit.into();
        product["facts"].as_array_mut().unwrap().push(f);
    }
    product["talk_tracks"][0]["text"] = if mode == 8 {
        "标准款容量为500毫升。".into()
    } else if real {
        "标准款容量为500毫升。标准款重量为100克。".into()
    } else {
        "标准款容量为500毫升。标准款重量为100克。标准款长度为10厘米。".into()
    };
    product["talk_tracks"][0]["fact_ids"] = if mode == 8 {
        json!(["capacity"])
    } else if real {
        json!(["capacity", "weight"])
    } else {
        json!(["capacity", "weight", "length"])
    };
    let mut save = |body: Value| {
        let id = random_id().unwrap();
        let r = call(
            &mut owner,
            C::PutPreparation(FoundationObject {
                workspace_id: ws.clone(),
                object_id: id.clone(),
                expected_revision: 0,
                body: body.to_string(),
            }),
        );
        json!({"object_id":id,"revision":r.revision,"sha256":r.sha256})
    };
    if mode == 8 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        product["facts"][0]["valid_until_ms"] = (now + 22000).into();
    }
    let p = save(product);
    let h = save(
        json!({"schema_version":1,"kind":"host_draft","name":"host","style":"professional","preferences":""}),
    );
    let series =
        save(json!({"schema_version":1,"kind":"series_draft","name":"series","goal":"specs"}));
    let show = save(
        json!({"schema_version":1,"kind":"show_draft","name":"auto specs","product":p,"host":h,"series":series,"cues":[{"id":"intro","title":"规格","talk_track_id":"opening","mandatory_fact_ids":if mode==8{json!(["capacity"])}else if real{json!(["capacity","weight"])}else{json!(["capacity","weight","length"])},"allow_questions":true}]}),
    );
    let report_id = random_id().unwrap();
    let report = call(
        &mut owner,
        C::PublicationImpact(FoundationImpact {
            workspace_id: ws.clone(),
            report_id: report_id.clone(),
            show_id: show["object_id"].as_str().unwrap().into(),
            show_revision: 1,
            previous_snapshot_id: String::new(),
        }),
    );
    let snap_id = random_id().unwrap();
    let snap = call(
        &mut owner,
        C::Publish(FoundationPublish {
            workspace_id: ws.clone(),
            snapshot_id: snap_id.clone(),
            report_id,
            report_sha256: report.sha256,
            acknowledge_unknown: true,
        }),
    );
    let run_id = random_id().unwrap();
    let c = call(
        &mut owner,
        C::CreateLiveRun(LiveRunCreate {
            scope: Some(QuestionPoolRead {
                workspace_id: ws.clone(),
                snapshot_id: snap_id,
                snapshot_sha256: snap.sha256,
            }),
            run_id: run_id.clone(),
        }),
    )
    .live_context
    .unwrap();
    let scope = LiveRunRead {
        workspace_id: ws.clone(),
        run_id: run_id.clone(),
    };
    let plan = call(&mut owner, C::ReadLiveSpeechPlan(scope.clone()))
        .live_speech_plan
        .unwrap();
    let c = call(
        &mut owner,
        C::ControlLiveBatch(LiveBatchControl {
            run: Some(scope.clone()),
            expected_revision: c.revision,
            expected_epoch: c.epoch,
            approval_id: random_id().unwrap(),
            plan_sha256: plan.plan_sha256.clone(),
            acknowledge_rights: true,
            revoke: false,
        }),
    )
    .live_context
    .unwrap();
    let mid = random_id().unwrap();
    call(
        &mut owner,
        C::SaveTtsModel(FoundationTtsModel {
            workspace_id: ws.clone(),
            model_id: mid.clone(),
            name: "isolated local speech".into(),
            base_url: endpoint,
            model: "qwen3-tts-1.7b-customvoice".into(),
            voice: "serena".into(),
            protocol: "qwen_local".into(),
            language: "chinese".into(),
            enabled: true,
            ..Default::default()
        }),
    );
    let mut recording_mic = None;
    if mode == 7 && std::env::var_os("NAI_AV_WAIT_READY").is_some() {
        let binary = PathBuf::from(std::env::var_os("NAI_HUMAN_MIC_WORKER").unwrap());
        let mut mic = nai_workbench_host::human_mic::MicOwner::open_at(&binary, &root).unwrap();
        let instance = mic.status.instance_id.clone();
        let i = std::env::var("NAI_MIC_ACCEPTANCE_INPUT")
            .unwrap()
            .parse()
            .unwrap();
        let o = std::env::var("NAI_MIC_ACCEPTANCE_OUTPUT")
            .unwrap()
            .parse()
            .unwrap();
        assert!(mic.start(&instance, i, o, true).unwrap().active);
        assert!(!mic.halt().unwrap().active);
        assert!(!mic.poll().unwrap().active); // complete native device release before starting keeper leases
        recording_mic = Some(mic);
    }
    let mut output =
        OutputOwner::open_at(&keeper, &root, std::env::var_os("NAI_AV_VISIBLE").is_none()).unwrap();
    if let Some(mic) = recording_mic.as_mut() {
        let ready = acceptance.as_ref().unwrap().join("recording.ready");
        av_event(json!({"phase":"windows_ready","recording":false}));
        let deadline = Instant::now() + Duration::from_secs(180);
        while !ready.is_file() {
            assert!(
                Instant::now() < deadline,
                "OBS start deadline; no model requests dispatched"
            );
            assert!(!mic.poll().unwrap().active);
            let s = owner.renew_public_output(&mut output, &root).unwrap();
            assert!(
                !s.audio_active && !s.terminal && s.standby,
                "standby lease lost before model dispatch"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        av_event(json!({"phase":"recording_ready"}));
    }
    let start = command(C::StartLiveAuto(LiveAutoStart {
        run: Some(scope.clone()),
        grant_id: random_id().unwrap(),
        expected_revision: c.revision,
        expected_epoch: c.epoch,
        plan_sha256: plan.plan_sha256,
        model_id: mid.clone(),
        model_revision: 1,
        max_cost_minor: if mode == 5 { 40 } else { 30 },
        acknowledge_rights: true,
        ..Default::default()
    }));
    let started = result(
        owner
            .call_with_public_output(&start, Some(&mut output), &root)
            .unwrap(),
    );
    assert_eq!(started.live_auto.unwrap().state, "preparing");
    let end = Instant::now() + Duration::from_secs(if mode == 7 { 120 } else { 60 });
    let mut saw_audio = false;
    let mut saw_question_audio = false;
    let mut saw_caption = false;
    let mut saw_mouth = false;
    let mut saw_overlap = false;
    let mut question_requested = false;
    let mut saw_question_return = false;
    let mut clips = std::collections::BTreeMap::new();
    let mut seen = 0;
    let began = Instant::now();
    let mut pause_requested = false;
    let mut paused_at = None;
    let mut human_resumed = false;
    loop {
        let next = Instant::now() + Duration::from_millis(100);
        assert!(Instant::now() < end, "auto completion deadline");
        let status = owner
            .renew_public_output(&mut output, &root)
            .unwrap_or_else(|e| {
                panic!(
                    "renew {e} status={:?} elapsed_ms={}",
                    output.current(),
                    began.elapsed().as_millis()
                )
            });
        saw_audio |= status.audio_active;
        saw_caption |= status.presentation.as_ref().is_some_and(|p| {
            p.caption_visible && p.presentation_count > 0 && p.composited_sha256.len() == 64
        });
        if let Some(p) = status.window_prepared.as_ref() {
            saw_mouth |= p.frame.as_ref().is_some_and(|f| f.openness_q16 > 0);
            if let Some(r) = p.receipt.as_ref() {
                clips.insert(p.preparation_id.clone(), r.sample_count);
            }
        }
        let auto_result = result(
            owner
                .call_with_public_output(
                    &command(C::ReadLiveAuto(scope.clone())),
                    Some(&mut output),
                    &root,
                )
                .unwrap(),
        );
        let auto_revision = auto_result.revision;
        let view = auto_result.live_auto.unwrap();
        saw_question_audio |= status.audio_active && view.interruption_state == "playing";
        av_event(
            json!({"phase":"auto_tick","state":view.state,"reason":view.reason,"grant":view.grant_id,"cursor":view.sentence_cursor,"reserved":view.reserved_minor,"audio_active":status.audio_active,"tasks":view.tasks.iter().map(|t|json!({"id":t.task_id,"state":t.task_state,"sentence":t.sentence_cursor})).collect::<Vec<_>>()}),
        );
        assert!(view.tasks.len() <= 2);
        if (mode == 5 || mode == 8) && status.audio_active && !question_requested {
            let source = c.source.as_ref().unwrap();
            let qid = random_id().unwrap();
            let mut ask = |op| {
                result(
                    owner
                        .call_with_public_output(&command(op), Some(&mut output), &root)
                        .unwrap(),
                )
            };
            ask(C::EnqueueQuestion(QuestionPoolEnqueue {
                scope: Some(QuestionPoolRead {
                    workspace_id: ws.clone(),
                    snapshot_id: source.snapshot_id.clone(),
                    snapshot_sha256: source.snapshot_sha256.clone(),
                }),
                question_id: qid.clone(),
                question: "容量多大？".into(),
            }));
            let r = ask(C::RunQuestion(QuestionPoolRun {
                workspace_id: ws.clone(),
                question_id: qid,
                expected_revision: 1,
                task_id: random_id().unwrap(),
            }));
            let e = r.question_pool.unwrap().entries.into_iter().next().unwrap();
            ask(C::ReadFaq(FoundationFaqRead {
                workspace_id: ws.clone(),
                task_id: e.task_id.clone(),
            }));
            let fresh = ask(C::ReadLiveAuto(scope.clone()));
            let (task, sha) = (e.task_id, e.task_sha256);

            let r = result(
                owner
                    .call_with_public_output(
                        &command(C::ControlLiveQuestion(LiveQuestionControl {
                            run: Some(scope.clone()),
                            grant_id: view.grant_id.clone(),
                            interruption_id: random_id().unwrap(),
                            faq_task_id: task,
                            faq_task_sha256: sha,
                            expected_revision: fresh.revision,
                            expected_epoch: fresh.epoch,
                            acknowledge_answer: true,
                            cancel: false,
                        })),
                        Some(&mut output),
                        &root,
                    )
                    .unwrap(),
            );
            assert!(
                ["question_pausing", "question_waiting"]
                    .contains(&r.live_auto.unwrap().state.as_str())
            );
            question_requested = true;
        }
        if mode == 5 && view.interruption_state == "completed" && !saw_question_return {
            assert_eq!(
                view.sentence_cursor, 1,
                "answer must resume at first unspoken script sentence"
            );
            assert_eq!(view.return_cursor, 1);
            saw_question_return = true;
            eprintln!(
                "QUESTION_RETURN cursor={} elapsed_ms={}",
                view.sentence_cursor,
                began.elapsed().as_millis()
            );
        }

        saw_overlap |= status.audio_active
            && view.tasks.iter().any(|t| {
                t.sentence_cursor > view.sentence_cursor
                    && t.task_state == AgentTaskState::Running as i32
            });
        if view.sentence_cursor > seen {
            seen = view.sentence_cursor;
            eprintln!(
                "AUTO_SUBMITTED index={seen} elapsed_ms={}",
                began.elapsed().as_millis()
            );
        }
        if (mode == 6 || mode == 7 && !human_resumed) && status.audio_active {
            let binary = PathBuf::from(
                std::env::var_os("NAI_HUMAN_MIC_WORKER").expect("explicit microphone worker"),
            );
            let mut mic = recording_mic.take().unwrap_or_else(|| {
                nai_workbench_host::human_mic::MicOwner::open_at(&binary, &root).unwrap()
            });
            let instance = mic.status.instance_id.clone();
            eprintln!(
                "MIC_NATIVE_DEVICES inputs={:?} outputs={:?}",
                mic.status.inputs, mic.status.outputs
            );
            let id = random_id().unwrap();
            let taken = result(
                owner
                    .call_with_public_output(
                        &command(C::ControlLiveTakeover(LiveTakeoverControl {
                            run: Some(scope.clone()),
                            takeover_id: id.clone(),
                            expected_epoch: c.epoch,
                            acknowledge: true,
                            ..Default::default()
                        })),
                        Some(&mut output),
                        &root,
                    )
                    .unwrap(),
            )
            .live_context
            .unwrap();
            assert!(taken.human_takeover && taken.partial_sentence);
            assert!(taken.epoch > c.epoch);
            assert!(!output.current().audio_active);
            // This ignored native test requires the explicitly selected headphone route.
            let input = std::env::var("NAI_MIC_ACCEPTANCE_INPUT")
                .unwrap()
                .parse()
                .unwrap();
            let out = std::env::var("NAI_MIC_ACCEPTANCE_OUTPUT")
                .unwrap()
                .parse()
                .unwrap();
            assert!(
                mic.start_with_standby(&instance, input, out, true, &mut output)
                    .unwrap()
                    .active
            );
            av_event(json!({"phase":"human_active","epoch":taken.epoch}));
            for _ in 0..if mode == 7 { 100 } else { 20 } {
                assert!(mic.poll().unwrap().active);
                assert!(
                    !owner
                        .renew_public_output(&mut output, &root)
                        .unwrap()
                        .audio_active
                );
                std::thread::sleep(Duration::from_millis(80));
            }
            assert!(!mic.halt().unwrap().active);
            av_event(
                json!({"phase":"human_muted","captured":mic.status.captured_frames,"submitted":mic.status.submitted_frames,"dropped":mic.status.dropped_frames}),
            );
            let resumed = result(
                owner
                    .call_with_public_output(
                        &command(C::ControlLiveTakeover(LiveTakeoverControl {
                            run: Some(scope.clone()),
                            takeover_id: id,
                            expected_epoch: taken.epoch,
                            acknowledge: true,
                            resume: true,
                            skip_partial_sentence: true,
                        })),
                        Some(&mut output),
                        &root,
                    )
                    .unwrap(),
            )
            .live_context
            .unwrap();
            assert!(!resumed.human_takeover);
            assert_eq!(resumed.sentence_cursor, 1);
            assert_eq!(resumed.submitted_sentences, 0);
            assert_eq!(resumed.state, LiveRunState::Ready as i32);
            assert!(
                !owner
                    .renew_public_output(&mut output, &root)
                    .unwrap()
                    .audio_active
            );
            if mode == 7 {
                let p = call(&mut owner, C::ReadLiveSpeechPlan(scope.clone()))
                    .live_speech_plan
                    .unwrap();
                let current = call(
                    &mut owner,
                    C::ControlLiveBatch(LiveBatchControl {
                        run: Some(scope.clone()),
                        expected_revision: p.revision,
                        expected_epoch: p.epoch,
                        approval_id: random_id().unwrap(),
                        plan_sha256: p.plan_sha256.clone(),
                        acknowledge_rights: true,
                        revoke: false,
                    }),
                )
                .live_context
                .unwrap();
                let remaining = 30 - view.reserved_minor;
                assert!(remaining >= 10);
                let r = result(
                    owner
                        .call_with_public_output(
                            &command(C::StartLiveAuto(LiveAutoStart {
                                run: Some(scope.clone()),
                                grant_id: random_id().unwrap(),
                                expected_revision: current.revision,
                                expected_epoch: current.epoch,
                                plan_sha256: p.plan_sha256,
                                model_id: mid.clone(),
                                model_revision: 1,
                                max_cost_minor: remaining,
                                acknowledge_rights: true,
                                ..Default::default()
                            })),
                            Some(&mut output),
                            &root,
                        )
                        .unwrap(),
                )
                .live_auto
                .unwrap();
                assert_ne!(r.grant_id, view.grant_id);
                assert_eq!(r.sentence_cursor, 1);
                human_resumed = true;
                av_event(
                    json!({"phase":"new_auto_grant","epoch":current.epoch,"remaining_cap":remaining,"old_reserved":view.reserved_minor}),
                );
                recording_mic = Some(mic);
                eprintln!(
                    "HUMAN_REAL_RESUMED epoch={} remaining_cap={} old_reserved={}",
                    current.epoch, remaining, view.reserved_minor
                );
                continue;
            }
            let (_, seen, worker) = mock.unwrap();
            assert_eq!(seen.load(Ordering::SeqCst), 2);
            worker.join().unwrap();
            eprintln!(
                "HUMAN_TAKEOVER_NATIVE_PASSED: AI off, explicit headphones, software mute acknowledged, new epoch, one partial sentence skipped; no OBS assertion"
            );
            return;
        }
        if mode == 2 && status.audio_active {
            let stopped = result(
                owner
                    .call_with_public_output(
                        &command(C::StopLiveAuto(LiveAutoStop {
                            run: Some(scope.clone()),
                            grant_id: view.grant_id.clone(),
                            reason: "OPERATOR_STOP".into(),
                        })),
                        Some(&mut output),
                        &root,
                    )
                    .unwrap(),
            )
            .live_auto
            .unwrap();
            assert_eq!(stopped.state, "stopped");
            let until = Instant::now() + Duration::from_millis(800);
            while Instant::now() < until {
                let s = owner.renew_public_output(&mut output, &root).unwrap();
                assert!(!s.audio_active);
                std::thread::sleep(Duration::from_millis(50));
            }
            let a = result(
                owner
                    .call_with_public_output(
                        &command(C::ReadLiveAuto(scope.clone())),
                        Some(&mut output),
                        &root,
                    )
                    .unwrap(),
            )
            .live_auto
            .unwrap();
            assert_eq!(a.state, "stopped");
            assert_eq!(a.reserved_minor, 20);
            let (_, seen, worker) = mock.unwrap();
            assert_eq!(seen.load(Ordering::SeqCst), 2);
            worker.join().unwrap();
            return;
        }
        if mode == 1
            && ((!pause_requested && status.audio_active)
                || (view.state == "paused"
                    && paused_at
                        .is_some_and(|t: Instant| t.elapsed() >= Duration::from_millis(800))))
        {
            let action = if pause_requested {
                LiveRunAction::Resume
            } else {
                LiveRunAction::Pause
            };
            result(
                owner
                    .call_with_public_output(
                        &command(C::ControlLiveRun(LiveRunControl {
                            workspace_id: scope.workspace_id.clone(),
                            run_id: scope.run_id.clone(),
                            expected_revision: auto_revision,
                            expected_epoch: c.epoch,
                            action: action as i32,
                        })),
                        Some(&mut output),
                        &root,
                    )
                    .unwrap(),
            );
            pause_requested = true;
            if action == LiveRunAction::Resume {
                paused_at = None;
            }
        }
        if mode == 1 && view.state == "paused" {
            assert_eq!(view.sentence_cursor, 1);
            assert!(!status.audio_active);
            paused_at.get_or_insert(Instant::now());
        }
        if mode == 8 && ["failed", "stopped", "blocked"].contains(&view.state.as_str()) {
            assert_eq!(view.reason, "POLICY_BLOCKED");
            assert!(saw_audio);
            assert!(!saw_question_audio);
            assert!(!status.audio_active);
            let (_, seen, worker) = mock.unwrap();
            assert_eq!(seen.load(Ordering::SeqCst), 2);
            worker.join().unwrap();
            eprintln!(
                "FAQ_WINDOW_REFUSED_BEFORE_ANSWER_AUDIO: scripted audio then slow synthetic FAQ, no retry"
            );
            return;
        }
        if mode == 3 && view.state == "failed" {
            assert_eq!(view.reason, "TTS_FAILED");
            assert_eq!(view.reserved_minor, 10);
            assert!(!saw_audio);
            let until = Instant::now() + Duration::from_millis(600);
            while Instant::now() < until {
                assert!(
                    !owner
                        .renew_public_output(&mut output, &root)
                        .unwrap()
                        .audio_active
                );
                std::thread::sleep(Duration::from_millis(50));
            }
            let (_, seen, worker) = mock.unwrap();
            assert_eq!(seen.load(Ordering::SeqCst), 1);
            worker.join().unwrap();
            return;
        }
        if view.state == "completed" {
            break;
        }
        assert!(
            !["failed", "stopped", "blocked"].contains(&view.state.as_str()),
            "auto stopped {} {} output_reason={} terminal={} elapsed_ms={}",
            view.state,
            view.reason,
            status.reason,
            status.terminal,
            began.elapsed().as_millis()
        );
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
    }
    let final_state = call(&mut owner, C::ReadLiveRun(scope))
        .live_context
        .unwrap();
    assert!(saw_audio);
    if mode == 4 {
        assert!(saw_overlap, "prefetch did not overlap current audio");
    }
    assert!(saw_caption, "actual caption presentation was not observed");
    if real {
        assert!(
            saw_mouth,
            "real speech never produced nonzero RMS mouth frames"
        );
    }
    if mode == 7 {
        assert!(human_resumed);
        assert!(clips.len() >= 2);
    } else {
        assert_eq!(clips.len(), count + usize::from(mode == 5));
    }
    if mode == 5 {
        assert!(question_requested && saw_question_return);
    }
    eprintln!(
        "AUTO_MEDIA clips={} samples={} caption={} nonzero_mouth={}",
        clips.len(),
        clips.values().sum::<u64>(),
        saw_caption,
        saw_mouth
    );
    assert_eq!(
        final_state.submitted_sentences,
        count as u32 - u32::from(mode == 7)
    );
    if mode == 7 {
        assert_eq!(final_state.skipped_sentences, 1);
    }
    assert_eq!(final_state.cursor, 1);
    assert_eq!(final_state.state, LiveRunState::Completed as i32);
    av_event(
        json!({"phase":"completed","submitted_sentences":final_state.submitted_sentences,"skipped_sentences":final_state.skipped_sentences}),
    );
    let cleared_by = Instant::now() + Duration::from_secs(1);
    loop {
        let status = owner.renew_public_output(&mut output, &root).unwrap();
        assert!(!status.audio_active);
        assert!(!status.terminal);
        if status.presentation.is_none() {
            break;
        }
        assert!(
            Instant::now() < cleared_by,
            "caption not cleared after completion"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    if let Some((_, seen, worker)) = mock {
        assert_eq!(seen.load(Ordering::SeqCst), count + usize::from(mode == 5));
        worker.join().unwrap();
    }
    if mode == 7
        && let Some(seconds) = std::env::var_os("NAI_AV_SOAK_SECONDS")
    {
        let seconds = seconds
            .to_str()
            .unwrap()
            .parse::<u64>()
            .unwrap()
            .clamp(60, 600);
        let mut mic = recording_mic
            .take()
            .expect("recording mode required for soak");
        let instance = mic.status.instance_id.clone();
        let i = std::env::var("NAI_MIC_ACCEPTANCE_INPUT")
            .unwrap()
            .parse()
            .unwrap();
        let o = std::env::var("NAI_MIC_ACCEPTANCE_OUTPUT")
            .unwrap()
            .parse()
            .unwrap();
        assert!(
            mic.start_with_standby(&instance, i, o, true, &mut output)
                .unwrap()
                .active
        );
        av_event(json!({"phase":"soak_human_started","seconds":seconds}));
        let end = Instant::now() + Duration::from_secs(seconds);
        let mut ticks = 0u64;
        while Instant::now() < end {
            let next = Instant::now() + Duration::from_millis(100);
            let m = mic.poll().unwrap();
            assert!(m.active);
            let s = owner.renew_public_output(&mut output, &root).unwrap();
            assert!(!s.audio_active && !s.terminal);
            ticks += 1;
            if ticks.is_multiple_of(100) {
                av_event(
                    json!({"phase":"soak_tick","ticks":ticks,"captured":m.captured_frames,"submitted":m.submitted_frames,"dropped":m.dropped_frames,"underrun":m.underrun_frames}),
                );
            }
            std::thread::sleep(next.saturating_duration_since(Instant::now()));
        }
        let m = mic.halt().unwrap();
        assert!(!m.active);
        av_event(
            json!({"phase":"soak_completed","seconds":seconds,"ticks":ticks,"captured":m.captured_frames,"submitted":m.submitted_frames,"dropped":m.dropped_frames,"underrun":m.underrun_frames}),
        );
    }
}
