#![cfg(windows)]
use nai_contracts::v561_live::{Event, Kind};
use nai_workbench_host::v561_live::Controller;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
fn wait(c: &Controller, kind: Kind, seconds: u64) -> Event {
    let began = Instant::now();
    loop {
        let v = c.status();
        if !v.busy && (v.kind == kind as i32 || v.kind == Kind::Failed as i32) {
            return v;
        }
        assert!(
            began.elapsed() < Duration::from_secs(seconds),
            "waiting {kind:?}: {v:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
#[ignore = "explicit real GPU/device: explicit bounded local TTS: one warmup plus one per semantic sentence; replays use no models"]
fn actual_prepare_once_replay_twice_stop_and_rebind() {
    let runtime = PathBuf::from(std::env::var_os("NAI_LIVE_RUNTIME").expect("runtime"));
    let profile = PathBuf::from(std::env::var_os("NAI_LIVE_PROFILE").expect("profile"));
    let artifacts = PathBuf::from(std::env::var_os("NAI_LIVE_ARTIFACTS").expect("artifacts"));
    let c = Controller::new(
        runtime.join("v561_live_control.exe"),
        runtime.join("nai-v561-stream-audio-output.exe"),
        profile,
        artifacts.clone(),
    )
    .unwrap();
    let text = std::env::var("NAI_LIVE_TEST_TEXT")
        .unwrap_or_else(|_| "你好，这是数字人直播。".to_string());
    c.prepare().unwrap();
    let prepared = wait(&c, Kind::Prepared, 245);
    assert_eq!(prepared.kind, Kind::Prepared as i32, "{}", prepared.code);
    c.prepare_content(text.clone()).unwrap();
    let content = wait(&c, Kind::ContentPrepared, 46);
    assert_eq!(
        content.kind,
        Kind::ContentPrepared as i32,
        "{}",
        content.code
    );
    assert!(content.content_prepared);
    // Explicit external-recorder attachment time, outside the measured submit
    // boundary. Never alter production admission or first-sample measurements.
    let settle_ms = std::env::var("NAI_OBS_ATTACH_MS")
        .ok()
        .map(|v| v.parse::<u64>().unwrap())
        .unwrap_or(0);
    assert!(settle_ms <= 5000);
    std::thread::sleep(Duration::from_millis(settle_ms));
    let mut reports = Vec::new();
    for request in [3, 4] {
        let began = Instant::now();
        c.play_prepared(text.clone()).unwrap();
        let done = wait(&c, Kind::Completed, 46);
        let observed = began.elapsed().as_millis();
        assert_eq!(done.kind, Kind::Completed as i32, "{}", done.code);
        let report: serde_json::Value = serde_json::from_slice(
            &std::fs::read(artifacts.join(format!("play-{request}/sentence-1/session.json")))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(report["model_api_calls"], 0);
        assert_eq!(report["video_source_mode"], "prepared_asset");
        assert_eq!(report["samples"], report["played_samples"]);
        let script: serde_json::Value = serde_json::from_slice(
            &std::fs::read(artifacts.join(format!("play-{request}/script.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(script["status"], "completed");
        assert_eq!(script["samples"], script["played_samples"]);
        assert_eq!(script["output_device_count"], 1);
        assert_eq!(script["output_window_count"], 1);
        let rows = script["sentences"].as_array().unwrap();
        assert!(!rows.is_empty());
        assert!(
            rows.iter()
                .all(|row| row["device_process_id"] == script["device_pid"])
        );
        assert!(rows.iter().all(|row| row["model_api_calls"] == 0));
        assert!(rows.windows(2).all(|pair| pair[0]["epoch"].as_u64().unwrap() < pair[1]["epoch"].as_u64().unwrap()));

        let end = script["elapsed_ms"].as_u64().unwrap() as u128;
        let first = report["device_first_played_script_ms"].as_u64().unwrap() as u128;
        assert!(observed >= end);
        reports.push(serde_json::json!({"request":request,"epoch":done.epoch,"submit_first_sample_upper_ms":observed-end+first,"report":report,"script":script}));
    }
    assert_ne!(reports[0]["epoch"], reports[1]["epoch"]);
    assert_eq!(
        reports[0]["report"]["asset_identity"],
        reports[1]["report"]["asset_identity"]
    );
    assert_eq!(
        reports[0]["report"]["pcm_sha256"],
        reports[1]["report"]["pcm_sha256"]
    );
    c.play_prepared(text.clone()).unwrap();
    let started = Instant::now();
    loop {
        let v = c.status();
        if v.kind == Kind::Starting as i32 {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(5));
    }
    // Let actual device-clock-selected frames advance, rather than only
    // cancelling admission. The failure record below must prove this.
    std::thread::sleep(Duration::from_millis(1000));
    let stop = Instant::now();
    c.stop();
    let stopped = wait(&c, Kind::Stopped, 5);
    let stop_ms = stop.elapsed().as_millis();
    assert_eq!(stopped.kind, Kind::Stopped as i32);
    let stopped_output: serde_json::Value = serde_json::from_slice(
        &std::fs::read(artifacts.join("play-5/sentence-1/failure.json")).unwrap(),
    )
    .unwrap();
    assert!(
        stopped_output["shown_frames"].as_u64().unwrap() >= 2,
        "must have advanced actual device-clock-selected frames before stop"
    );
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(c.status().kind, Kind::Stopped as i32);
    c.play_prepared(text.clone()).unwrap();
    let resumed = wait(&c, Kind::Completed, 46);
    assert_eq!(resumed.kind, Kind::Completed as i32, "{}", resumed.code);
    c.play_prepared("这是修改后的内容。".into()).unwrap();
    let rejected = wait(&c, Kind::Failed, 5);
    assert_eq!(rejected.code, "CONTENT_NOT_PREPARED");
    c.shutdown_wait().unwrap();
    serde_json::to_writer_pretty(std::fs::File::create_new(artifacts.join("native-summary.json")).unwrap(),&serde_json::json!({"scope":"session_private_prepared_asset_actual_device_not_live_generation","reports":reports,"stop_ms":stop_ms,"post_stop_epoch":resumed.epoch,"changed_text_rejected":true,"replay_model_calls":0,"total_model_calls":reports[0]["script"]["sentences"].as_array().unwrap().len()+1,"worker_pid":prepared.worker_pid,"shutdown":true,"external_obs_attachment_wait_ms":settle_ms})).unwrap();
}
