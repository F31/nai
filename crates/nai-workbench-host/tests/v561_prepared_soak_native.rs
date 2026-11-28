#![cfg(windows)]
//! Explicit real-media endurance runner; no automatic model retries.
use nai_contracts::v561_live::Kind;
use nai_workbench_host::v561_live::Controller;
use std::{
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    time::{Duration, Instant},
};

fn wait(c: &Controller, kind: Kind, request: u64, seconds: u64) -> nai_contracts::v561_live::Event {
    let began = Instant::now();
    loop {
        let event = c.status();
        if event.request_sequence == request
            && !event.busy
            && (event.kind == kind as i32 || event.kind == Kind::Failed as i32)
        {
            return event;
        }
        assert!(
            began.elapsed() < Duration::from_secs(seconds),
            "waiting {kind:?}: {event:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "explicit 30-minute real device/OBS run; one local warmup plus four synthesis calls, no replay calls"]
fn actual_prepared_plan_thirty_minute_soak() {
    let runtime = PathBuf::from(std::env::var_os("NAI_LIVE_RUNTIME").expect("runtime"));
    let profile = PathBuf::from(std::env::var_os("NAI_LIVE_PROFILE").expect("profile"));
    let dir = PathBuf::from(std::env::var_os("NAI_LIVE_ARTIFACTS").expect("artifacts"));
    let text = std::env::var("NAI_LIVE_TEST_TEXT").expect("explicit four-sentence text");
    let c = Controller::new(
        runtime.join("v561_live_control.exe"),
        runtime.join("nai-v561-stream-audio-output.exe"),
        profile,
        dir.clone(),
    )
    .unwrap();
    c.prepare().unwrap();
    let ready = wait(&c, Kind::Prepared, 1, 245);
    assert_eq!(ready.kind, Kind::Prepared as i32, "{}", ready.code);
    c.prepare_content(text.clone()).unwrap();
    let content = wait(&c, Kind::ContentPrepared, 2, 46);
    assert_eq!(
        content.kind,
        Kind::ContentPrepared as i32,
        "{}",
        content.code
    );
    std::thread::sleep(Duration::from_secs(4)); // explicit OBS attachment, not submit latency
    let mut rows = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join("soak-rounds.jsonl"))
        .unwrap();
    let began = Instant::now();
    let mut rounds = 0u64;
    let mut previous_epoch = 0u64;
    let mut total_samples = 0u64;
    let mut max_gap = 0u64;
    let mut gap_failures = 0u64;
    let mut first_failures = 0u64;
    let mut max_first = 0u128;
    let mut identity = None;
    while began.elapsed() < Duration::from_secs(1800) {
        assert!(rounds < 4096, "bounded round count exhausted");
        let request = rounds + 3;
        let submitted = Instant::now();
        c.play_prepared(text.clone()).unwrap();
        let done = wait(&c, Kind::Completed, request, 46);
        let observed = submitted.elapsed().as_millis();
        assert_eq!(done.kind, Kind::Completed as i32, "{}", done.code);
        assert_eq!(done.worker_pid, ready.worker_pid, "worker replaced");
        let path = dir.join(format!("play-{request}/script.json"));
        assert!(path.metadata().unwrap().len() < 1048576);
        let script: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let sentences = script["sentences"].as_array().unwrap();
        assert_eq!(sentences.len(), 4);
        assert_eq!(script["status"], "completed");
        assert_eq!(script["model_api_calls"], 0);
        assert_eq!(script["output_window_count"], 1);
        assert_eq!(script["output_device_count"], 1);
        assert_eq!(script["samples"], script["played_samples"]);
        let mut end = None;
        let mut keys = Vec::new();
        let mut sample_sum = 0u64;
        for sentence in sentences {
            let epoch = sentence["epoch"].as_u64().unwrap();
            assert!(epoch > previous_epoch);
            previous_epoch = epoch;
            assert_eq!(sentence["device_process_id"], script["device_pid"]);
            assert_eq!(sentence["model_api_calls"], 0);
            assert_eq!(sentence["samples"], sentence["played_samples"]);
            assert_eq!(sentence["status"], "completed");
            sample_sum += sentence["samples"].as_u64().unwrap();
            let start = sentence["device_first_played_script_ms"].as_u64().unwrap();
            let completed = sentence["device_completed_script_ms"].as_u64().unwrap();
            assert!(completed >= start);
            if let Some(previous_end) = end {
                let gap = start.checked_sub(previous_end).unwrap();
                assert_eq!(
                    sentence["gap_from_previous_device_end_ms"].as_u64(),
                    Some(gap)
                );
                max_gap = max_gap.max(gap);
                gap_failures += u64::from(gap > 300);
            }
            end = Some(completed);
            keys.push((
                sentence["asset_identity"].clone(),
                sentence["pcm_sha256"].clone(),
            ));
        }
        assert_eq!(Some(sample_sum), script["played_samples"].as_u64());
        if let Some(previous) = &identity {
            assert_eq!(&keys, previous);
        } else {
            identity = Some(keys);
        }
        let elapsed = script["elapsed_ms"].as_u64().unwrap() as u128;
        assert!(observed >= elapsed);
        let first = observed - elapsed
            + sentences[0]["device_first_played_script_ms"]
                .as_u64()
                .unwrap() as u128;
        max_first = max_first.max(first);
        first_failures += u64::from(first > 3000);
        total_samples += sample_sum;
        rounds += 1;
        let heartbeat = serde_json::json!({"scope":"prepared_plan_replay_soak_not_new_generation","rounds":rounds,"elapsed_ms":began.elapsed().as_millis(),"worker_pid":ready.worker_pid,"total_samples":total_samples,"max_gap_ms":max_gap,"gap_budget_failures":gap_failures,"first_budget_failures":first_failures,"max_first_upper_ms":max_first,"replay_model_calls":0});
        serde_json::to_writer(&mut rows, &serde_json::json!({"request":request,"observed_ms":observed,"submit_first_sample_upper_ms":first,"script":script})) .unwrap();
        writeln!(rows).unwrap();
        rows.flush().unwrap();
        serde_json::to_writer_pretty(
            std::fs::File::create(dir.join("soak-heartbeat.json")).unwrap(),
            &heartbeat,
        )
        .unwrap();
        println!(
            "round={rounds} elapsed={}s max_gap={max_gap}ms gap_over={gap_failures} first_over={first_failures}",
            began.elapsed().as_secs()
        );
    }
    let elapsed = began.elapsed().as_millis();
    c.shutdown_wait().unwrap();
    serde_json::to_writer_pretty(std::fs::File::create_new(dir.join("soak-summary.json")).unwrap(), &serde_json::json!({"scope":"prepared_plan_replay_soak_not_new_generation","status":"completed","elapsed_ms":elapsed,"rounds":rounds,"total_samples":total_samples,"max_gap_ms":max_gap,"gap_budget_failures":gap_failures,"first_budget_failures":first_failures,"max_first_upper_ms":max_first,"model_api_calls":5,"replay_model_calls":0,"shutdown":true,"budget_passed":gap_failures==0&&first_failures==0})).unwrap();
}
