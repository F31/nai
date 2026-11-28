#![cfg(windows)]
//! Actual supplied-answer media workflow, not LLM acceptance.
use nai_contracts::v561_live::{Event, Kind};
use nai_workbench_host::v561_live::Controller;
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
fn wait(c: &Controller, request: u64, kind: Kind, seconds: u64) -> Event {
    let began = Instant::now();
    loop {
        let e = c.status();
        if e.request_sequence == request
            && !e.busy
            && (e.kind == kind as i32 || e.kind == Kind::Failed as i32)
        {
            return e;
        }
        assert!(
            began.elapsed() < Duration::from_secs(seconds),
            "waiting {kind:?}: {e:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn progress(c: &Controller, dir: &Path, request: u64) {
    let began = Instant::now();
    let path = dir.join(format!("play-{request}/sentence-1/session.json"));
    loop {
        if let Ok(bytes) = std::fs::read(&path)
            && let Ok(v) = serde_json::from_slice::<Value>(&bytes)
            && v["samples"] == v["played_samples"]
            && v["shown_frames"].as_u64().unwrap_or(0) > 0
        {
            break;
        }
        assert!(began.elapsed() < Duration::from_secs(14));
        std::thread::sleep(Duration::from_millis(10));
    }
    // The actual first-sentence receipt has been written. Let root commit its
    // cursor and enter the second sentence, rather than stopping before first sound.
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(c.status().kind, Kind::Starting as i32);
}
#[test]
#[ignore = "explicit local GPU/device: warmup + two baseline sentences + one answer; cancellation may start one more call"]
fn actual_supplied_reply_resumes_second_sentence_and_stop_cancels_recovery() {
    run_reply(false);
}
#[test]
#[ignore = "explicit configured LLM + local TTS + GPU/device; at most two LLM requests"]
fn actual_llm_reply_resumes_second_sentence_and_stop_cancels_recovery() {
    run_reply(true);
}
fn run_reply(llm: bool) {
    let runtime = PathBuf::from(std::env::var_os("NAI_LIVE_RUNTIME").expect("runtime"));
    let profile = PathBuf::from(std::env::var_os("NAI_LIVE_PROFILE").expect("profile"));
    let dir = PathBuf::from(std::env::var_os("NAI_LIVE_ARTIFACTS").expect("artifacts"));
    let input: Value = serde_json::from_slice(
        &std::fs::read(std::env::var_os("NAI_REPLY_INPUT").expect("input")).unwrap(),
    )
    .unwrap();
    let text = input["baseline"].as_str().unwrap().to_owned();
    let question = input["question"].as_str().unwrap().to_owned();
    let answer = input["answer"].as_str().unwrap().to_owned();
    let c = Controller::new(
        runtime.join("v561_live_control.exe"),
        runtime.join("nai-v561-stream-audio-output.exe"),
        profile,
        dir.clone(),
    )
    .unwrap();
    c.prepare().unwrap();
    let ready = wait(&c, 1, Kind::Prepared, 245);
    assert_eq!(ready.kind, Kind::Prepared as i32, "{}", ready.code);
    assert!(c.reply_text(question.clone(), answer.clone()).is_err());
    c.prepare_content(text.clone()).unwrap();
    let e = wait(&c, 2, Kind::ContentPrepared, 46);
    assert_eq!(e.kind, Kind::ContentPrepared as i32, "{}", e.code);
    c.play_prepared(text.clone()).unwrap();
    progress(&c, &dir, 3);
    if llm {
        c.ask(question.clone()).unwrap();
    } else {
        c.reply_text(question.clone(), answer.clone()).unwrap();
    }
    assert!(c.reply_text(question.clone(), answer.clone()).is_err());
    let done = wait(&c, 4, Kind::Completed, 48);
    assert_eq!(done.kind, Kind::Completed as i32, "{}", done.code);
    let report: Value =
        serde_json::from_slice(&std::fs::read(dir.join("reply-4/reply.json")).unwrap()).unwrap();
    assert_eq!(report["status"], "completed");
    if llm {
        assert_eq!(report["scope"], "llm_reply_and_sentence_resume");
        let meta: Value =
            serde_json::from_slice(&std::fs::read(dir.join("reply-4/llm.json")).unwrap()).unwrap();
        assert_eq!(meta["request_started"], true);
        assert_eq!(meta["error"], "");
    }
    assert_eq!(report["resume_sentence"], 1);
    assert_eq!(report["worker_pid"], ready.worker_pid);
    assert_eq!(report["samples"], report["played_samples"]);
    assert_eq!(report["answer"]["model_api_calls"], 1);
    assert_eq!(report["resumed"]["model_api_calls"], 0);
    assert_eq!(report["resumed"]["start_sentence"], 1);
    assert_eq!(report["resumed"]["sentences"].as_array().unwrap().len(), 1);
    assert_eq!(report["resumed"]["sentences"][0]["sentence_index"], 1);
    let old: Value =
        serde_json::from_slice(&std::fs::read(dir.join("play-3/sentence-1/session.json")).unwrap())
            .unwrap();
    assert!(report["resumed"]["epoch"].as_u64().unwrap() > old["epoch"].as_u64().unwrap());
    c.play_prepared(text).unwrap();
    progress(&c, &dir, 5);
    if llm {
        c.ask(question).unwrap();
    } else {
        c.reply_text(question, answer).unwrap();
    }
    let began = Instant::now();
    loop {
        let e = c.status();
        if e.request_sequence == 6
            && e.code
                == if llm {
                    "GENERATING_ANSWER"
                } else {
                    "REPLY_TEXT"
                }
        {
            break;
        }
        assert!(began.elapsed() < Duration::from_secs(5), "{e:?}");
        std::thread::sleep(Duration::from_millis(5));
    }
    let stop = Instant::now();
    c.stop();
    let stopped = wait(&c, 7, Kind::Stopped, 5);
    assert_eq!(stopped.kind, Kind::Stopped as i32, "{}", stopped.code);
    let stopped_ms = stop.elapsed().as_millis();
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(c.status().kind, Kind::Stopped as i32);
    assert!(!dir.join("reply-6/resume").exists());
    c.shutdown_wait().unwrap();
    std::fs::write(dir.join("native-summary.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "scope":if llm{"llm_reply_and_auto_resume_not_obs"}else{"supplied_text_reply_and_auto_resume_not_llm_or_obs"},"report":report,"stop_ack_ms":stopped_ms,
        "no_resume_after_stop":true,"model_api_calls_min":4,"model_api_calls_max":5,"shutdown":true})).unwrap()).unwrap();
}

#[test]
#[ignore = "explicit 300/1800-second local AV + up to four configured LLM attempts; optional isolated OBS handshake"]
fn mixed_questions_cached_replays_and_stop_keep_one_window() {
    let seconds = std::env::var("NAI_MIXED_SECONDS")
        .ok()
        .map(|v| v.parse::<u64>().expect("duration"))
        .unwrap_or(300);
    assert!([300, 1800].contains(&seconds));
    let runtime = PathBuf::from(std::env::var_os("NAI_LIVE_RUNTIME").expect("runtime"));
    let dir = PathBuf::from(std::env::var_os("NAI_LIVE_ARTIFACTS").expect("artifacts"));
    let input: Value = serde_json::from_slice(
        &std::fs::read(std::env::var_os("NAI_REPLY_INPUT").expect("input")).unwrap(),
    )
    .unwrap();
    let text = input["baseline"].as_str().unwrap().to_owned();
    let questions = input["questions"].as_array().unwrap();
    assert_eq!(questions.len(), 3);
    let c = Controller::new(
        runtime.join("v561_live_control.exe"),
        runtime.join("nai-v561-stream-audio-output.exe"),
        PathBuf::from(std::env::var_os("NAI_LIVE_PROFILE").expect("profile")),
        dir.clone(),
    )
    .unwrap();
    let began = Instant::now();
    let mut timeline = Vec::<Value>::new();
    c.prepare().unwrap();
    let ready = wait(&c, 1, Kind::Prepared, 245);
    assert_eq!(ready.kind, Kind::Prepared as i32, "{}", ready.code);
    c.prepare_content(text.clone()).unwrap();
    let prepared = wait(&c, 2, Kind::ContentPrepared, 60);
    assert_eq!(
        prepared.kind,
        Kind::ContentPrepared as i32,
        "{}",
        prepared.code
    );
    std::fs::write(dir.join("mixed-ready.json"),serde_json::to_vec(&serde_json::json!({"worker_pid":ready.worker_pid,"waiting_for_obs":std::env::var_os("NAI_WAIT_OBS").is_some()})).unwrap()).unwrap();
    if std::env::var_os("NAI_WAIT_OBS").is_some() {
        let start = Instant::now();
        while !dir.join("obs.ready").is_file() {
            assert!(
                start.elapsed() < Duration::from_secs(120),
                "OBS handshake timeout"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    let mixed = Instant::now();
    let mut seq = 2u64;
    let mut replies = Vec::new();
    let mut windows = Vec::new();
    let mut replay_count = 0;
    let replay = |seq: &mut u64, replay_count: &mut u64| {
        *seq += 1;
        c.play_prepared(text.clone()).unwrap();
        let done = wait(&c, *seq, Kind::Completed, 30);
        assert_eq!(done.kind, Kind::Completed as i32, "{}", done.code);
        let report: Value = serde_json::from_slice(
            &std::fs::read(dir.join(format!("play-{seq}/script.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(report["model_api_calls"], 0);
        assert_eq!(report["samples"], report["played_samples"]);
        *replay_count += 1;
        assert!(*replay_count <= seconds + 32);
        if (*replay_count).is_multiple_of(10) {
            let progress = serde_json::json!({"elapsed_ms":mixed.elapsed().as_millis(),"duration_seconds":seconds,"cached_replays":*replay_count,"request_sequence":*seq});
            std::fs::write(
                dir.join("mixed-progress.json"),
                serde_json::to_vec(&progress).unwrap(),
            )
            .unwrap();
            println!("mixed_progress {progress}");
        }
    };
    for (index, q) in questions.iter().enumerate() {
        let due = Duration::from_secs(index as u64 * (seconds / 3));
        while mixed.elapsed() < due {
            replay(&mut seq, &mut replay_count);
        }
        seq += 1;
        let play = seq;
        c.play_prepared(text.clone()).unwrap();
        progress(&c, &dir, play);
        let old: Value = serde_json::from_slice(
            &std::fs::read(dir.join(format!("play-{play}/sentence-1/session.json"))).unwrap(),
        )
        .unwrap();
        let window = old["output_window_identity"]
            .as_str()
            .expect("window identity")
            .to_owned();
        seq += 1;
        let task = seq;
        c.ask(q.as_str().unwrap().into()).unwrap();
        assert!(c.ask("重复提问".into()).is_err());
        let done = wait(&c, task, Kind::Completed, 60);
        assert_eq!(done.kind, Kind::Completed as i32, "{}", done.code);
        let report: Value = serde_json::from_slice(
            &std::fs::read(dir.join(format!("reply-{task}/reply.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(report["worker_pid"], ready.worker_pid);
        assert_eq!(report["samples"], report["played_samples"]);
        assert_eq!(report["resume_sentence"], 1);
        assert_eq!(report["resumed"]["model_api_calls"], 0);
        assert_eq!(report["answer"]["model_api_calls"], 1);
        assert_eq!(report["answer"]["output_window_creations"], 0);
        assert_eq!(report["output_window_identity"], window);
        assert!(report["llm_waiting_window_updates"].as_u64().unwrap() > 0);
        for row in report["answer"]["results"]
            .as_array()
            .unwrap()
            .iter()
            .chain(report["resumed"]["sentences"].as_array().unwrap())
        {
            assert_eq!(row["output_window_identity"], window);
        }
        let meta: Value = serde_json::from_slice(
            &std::fs::read(dir.join(format!("reply-{task}/llm.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(meta["request_started"], true);
        assert_eq!(meta["error"], "");
        timeline.push(serde_json::json!({"elapsed_ms":mixed.elapsed().as_millis(),"task":task,"action":"question_completed","llm_ms":meta["elapsed_ms"]}));
        windows.push(window);
        replies.push(report);
    }
    assert!(
        windows.iter().all(|w| w == &windows[0]),
        "window changed across questions"
    );
    seq += 1;
    let play = seq;
    c.play_prepared(text.clone()).unwrap();
    progress(&c, &dir, play);
    seq += 1;
    let cancelled = seq;
    c.ask("还可以介绍什么？".into()).unwrap();
    let pending = Instant::now();
    loop {
        let e = c.status();
        if e.request_sequence == cancelled && e.code == "GENERATING_ANSWER" {
            break;
        }
        assert!(pending.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(5));
    }
    seq += 1;
    let stopped_at = Instant::now();
    c.stop();
    let stopped = wait(&c, seq, Kind::Stopped, 5);
    assert_eq!(stopped.kind, Kind::Stopped as i32);
    let stop_ms = stopped_at.elapsed().as_millis();
    timeline.push(serde_json::json!({"elapsed_ms":mixed.elapsed().as_millis(),"action":"operator_stop","stop_ack_ms":stop_ms}));
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(c.status().kind, Kind::Stopped as i32);
    assert!(!dir.join(format!("reply-{cancelled}/resume")).exists());
    while mixed.elapsed() < Duration::from_secs(seconds) {
        replay(&mut seq, &mut replay_count);
    }
    std::fs::write(dir.join("mixed-complete.json"),serde_json::to_vec_pretty(&serde_json::json!({"scope":"bounded_mixed_local_llm_av","duration_seconds":seconds,"mixed_ms":mixed.elapsed().as_millis(),"total_ms":began.elapsed().as_millis(),"worker_pid":ready.worker_pid,"replies":replies,"timeline":timeline,"cached_replays":replay_count,"stop_ack_ms":stop_ms,"no_late_resume":true,"llm_attempts_min":3,"llm_attempts_max":4})).unwrap()).unwrap();
    // Hold valid standby for the OBS output timer to close its file normally.
    if std::env::var_os("NAI_WAIT_OBS").is_some() {
        let end = Instant::now();
        while !dir.join("obs.stopped").is_file() {
            assert!(
                end.elapsed() < Duration::from_secs(90),
                "OBS stop acknowledgement timeout"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    c.shutdown_wait().unwrap();
}

#[test]
#[ignore = "explicit GPU prepare + eight cached replays for phase measurement; three local TTS calls, no LLM"]
fn profile_cached_audio_transition_without_questions() {
    let runtime = PathBuf::from(std::env::var_os("NAI_LIVE_RUNTIME").expect("runtime"));
    let dir = PathBuf::from(std::env::var_os("NAI_LIVE_ARTIFACTS").expect("artifacts"));
    let input: Value = serde_json::from_slice(
        &std::fs::read(std::env::var_os("NAI_REPLY_INPUT").expect("input")).unwrap(),
    )
    .unwrap();
    let text = input["baseline"].as_str().unwrap().to_owned();
    let c = Controller::new(
        runtime.join("v561_live_control.exe"),
        runtime.join("nai-v561-stream-audio-output.exe"),
        PathBuf::from(std::env::var_os("NAI_LIVE_PROFILE").expect("profile")),
        dir.clone(),
    )
    .unwrap();
    c.prepare().unwrap();
    let ready = wait(&c, 1, Kind::Prepared, 245);
    assert_eq!(ready.kind, Kind::Prepared as i32, "{}", ready.code);
    c.prepare_content(text.clone()).unwrap();
    let prepared = wait(&c, 2, Kind::ContentPrepared, 60);
    assert_eq!(
        prepared.kind,
        Kind::ContentPrepared as i32,
        "{}",
        prepared.code
    );
    for seq in 3..=10 {
        c.play_prepared(text.clone()).unwrap();
        let done = wait(&c, seq, Kind::Completed, 30);
        assert_eq!(done.kind, Kind::Completed as i32, "{}", done.code);
        let v: Value = serde_json::from_slice(
            &std::fs::read(dir.join(format!("play-{seq}/script.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(v["model_api_calls"], 0);
        assert_eq!(v["samples"], v["played_samples"]);
    }
    c.shutdown_wait().unwrap();
}

#[test]
#[ignore = "explicit local GPU/TTS/device; externally minimize only this test's public window; no LLM"]
fn zero_client_area_stops_cached_playback_without_false_completion() {
    let runtime = PathBuf::from(std::env::var_os("NAI_LIVE_RUNTIME").expect("runtime"));
    let dir = PathBuf::from(std::env::var_os("NAI_LIVE_ARTIFACTS").expect("artifacts"));
    let input: Value = serde_json::from_slice(
        &std::fs::read(std::env::var_os("NAI_REPLY_INPUT").expect("input")).unwrap(),
    )
    .unwrap();
    let text = input["baseline"].as_str().unwrap().to_owned();
    let c = Controller::new(
        runtime.join("v561_live_control.exe"),
        runtime.join("nai-v561-stream-audio-output.exe"),
        PathBuf::from(std::env::var_os("NAI_LIVE_PROFILE").expect("profile")),
        dir.clone(),
    )
    .unwrap();
    c.prepare().unwrap();
    assert_eq!(wait(&c, 1, Kind::Prepared, 245).kind, Kind::Prepared as i32);
    c.prepare_content(text.clone()).unwrap();
    assert_eq!(
        wait(&c, 2, Kind::ContentPrepared, 60).kind,
        Kind::ContentPrepared as i32
    );
    std::fs::write(
        dir.join("mixed-ready.json"),
        b"{\"scope\":\"zero_area_fault_not_av_acceptance\"}",
    )
    .unwrap();
    if std::env::var_os("NAI_WAIT_OBS").is_some() {
        let start = Instant::now();
        while !dir.join("obs.ready").exists() {
            assert!(start.elapsed() < Duration::from_secs(120));
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    let mut failed = None;
    for seq in 3..=40 {
        c.play_prepared(text.clone()).unwrap();
        let event = wait(&c, seq, Kind::Completed, 30);
        if event.kind == Kind::Failed as i32 {
            assert_eq!(event.code, "OUTPUT_WINDOW_UNAVAILABLE");
            let path = dir.join(format!("play-{seq}/script.json"));
            if path.exists() {
                let report: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
                assert_eq!(
                    report["status"], "failed",
                    "Failure artifacts must not claim completion"
                );
            }
            failed = Some(seq);
            break;
        }
        assert_eq!(event.kind, Kind::Completed as i32);
    }
    let seq =
        failed.expect("No zero-area event was injected or observed; do not grant fault acceptance");
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(
        c.status().kind,
        Kind::Failed as i32,
        "unexpected automatic recovery"
    );
    std::fs::write(dir.join("mixed-complete.json"),serde_json::to_vec_pretty(&serde_json::json!({"scope":"zero_area_fault_not_av_acceptance","code":"OUTPUT_WINDOW_UNAVAILABLE","failed_request":seq,"no_false_completion":true,"no_auto_resume":true,"llm_calls":0})).unwrap()).unwrap();
    if std::env::var_os("NAI_WAIT_OBS").is_some() {
        let start = Instant::now();
        while !dir.join("obs.stopped").exists() {
            assert!(start.elapsed() < Duration::from_secs(60));
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    c.shutdown_wait().unwrap();
}
