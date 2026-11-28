#![cfg(windows)]
//! Explicit feasibility probe, not an implemented LLM/interjection feature.
use nai_contracts::v561_live::{Event, Kind};
use nai_workbench_host::v561_live::Controller;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
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
fn read(dir: &std::path::Path, request: u64) -> Value {
    serde_json::from_slice(&std::fs::read(dir.join(format!("play-{request}/script.json"))).unwrap())
        .unwrap()
}

#[test]
#[ignore = "explicit local GPU/TTS; bounded five calls, no retries, no LLM"]
fn cached_stop_fresh_reply_resume_lower_bound() {
    let runtime = PathBuf::from(std::env::var_os("NAI_LIVE_RUNTIME").expect("runtime"));
    let profile = PathBuf::from(std::env::var_os("NAI_LIVE_PROFILE").expect("profile"));
    let dir = PathBuf::from(std::env::var_os("NAI_LIVE_ARTIFACTS").expect("artifacts"));
    let input: Value = serde_json::from_slice(
        &std::fs::read(std::env::var_os("NAI_FRESH_PROBE_INPUT").expect("input")).unwrap(),
    )
    .unwrap();
    let baseline = input["baseline"].as_str().unwrap().to_string();
    let answers = input["fresh_answers"].as_array().unwrap();
    assert_eq!(answers.len(), 2);
    assert!(answers.iter().all(|v| {
        v.as_str()
            .is_some_and(|s| !s.is_empty() && s.chars().count() <= 80 && s != baseline)
    }));
    let c = Controller::new(
        runtime.join("v561_live_control.exe"),
        runtime.join("nai-v561-stream-audio-output.exe"),
        profile,
        dir.clone(),
    )
    .unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    let began = Instant::now();
    let mut rows = Vec::new();
    let mut failure: Option<String> = None;
    let outcome = (|| -> Result<(), String> {
        c.prepare()?;
        let prepared = wait(&c, 1, Kind::Prepared, 245);
        if prepared.kind != Kind::Prepared as i32 {
            return Err(prepared.code);
        }
        let worker = prepared.worker_pid;
        c.prepare_content(baseline.clone())?;
        let e = wait(&c, 2, Kind::ContentPrepared, 46);
        if e.kind != Kind::ContentPrepared as i32 {
            return Err(e.code);
        }
        let mut request = 2;
        for answer in answers {
            request += 1;
            c.play_prepared(baseline.clone())?;
            // Stop only after actual samples and frames have advanced.
            let progress = Instant::now();
            loop {
                let path = dir.join(format!("play-{request}/sentence-1/session.json"));
                if let Ok(bytes) = std::fs::read(path)
                    && let Ok(v) = serde_json::from_slice::<Value>(&bytes)
                    && v["played_samples"].as_u64().unwrap_or(0) > 0
                    && v["shown_frames"].as_u64().unwrap_or(0) > 0
                {
                    break;
                }
                if progress.elapsed() > Duration::from_secs(12) {
                    return Err("BASELINE_PROGRESS_TIMEOUT".into());
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let stop = Instant::now();
            c.stop();
            request += 1;
            let stopped = wait(&c, request, Kind::Stopped, 5);
            if stopped.kind != Kind::Stopped as i32 {
                return Err(stopped.code);
            }
            let stop_ms = stop.elapsed().as_millis();
            request += 1;
            let submitted = Instant::now();
            c.start(answer.as_str().unwrap().into())?;
            let e = wait(&c, request, Kind::Completed, 48);
            let wall_ms = submitted.elapsed().as_millis();
            let report = std::fs::read(dir.join(format!("play-{request}/script.json")))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                .unwrap_or(Value::Null);
            rows.push(json!({"request":request,"stop_ack_ms":stop_ms,"submit_to_terminal_ms":wall_ms,"event_code":e.code,"same_worker":e.worker_pid == worker,"report":report}));
            if e.kind != Kind::Completed as i32 {
                return Err(e.code);
            }
            if e.worker_pid != worker {
                return Err("WORKER_REPLACED".into());
            }
            request += 1;
            c.play_prepared(baseline.clone())?;
            let e = wait(&c, request, Kind::Completed, 46);
            if e.kind != Kind::Completed as i32 {
                return Err(e.code);
            }
            let resumed = read(&dir, request);
            if resumed["model_api_calls"] != 0 {
                return Err("RESUME_GENERATED".into());
            }
            rows.last_mut().unwrap()["resumed"] = resumed;
        }
        Ok(())
    })();
    if let Err(e) = outcome {
        failure = Some(e);
    }
    let shutdown = c.shutdown_wait().is_ok();
    std::fs::write(
        dir.join("fresh-reply-summary.json"),
        serde_json::to_vec_pretty(&json!({
            "scope":"cached_stop_fresh_text_resume_lower_bound_not_llm_or_concurrent_mixed_load",
            "elapsed_ms":began.elapsed().as_millis(),"failure":failure,"shutdown":shutdown,
            "model_api_calls_max":5,"automatic_retries":0,"llm_included":false,
            "simultaneous_cached_playback_and_generation":false,"rows":rows
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(failure.is_none(), "{failure:?}");
    assert!(shutdown);
}
