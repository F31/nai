#![cfg(windows)]
//! Explicit local GPU/device/recording run; all replays reuse the same content.
use nai_contracts::v561_live::{Event, Kind};
use nai_workbench_host::v561_live::Controller;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn wait(c: &Controller, kind: Kind, request: u64, seconds: u64) -> Event {
    let began = Instant::now();
    loop {
        let e = c.status();
        if e.request_sequence == request
            && (!e.busy || kind == Kind::Starting)
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
fn script(dir: &Path, request: u64) -> serde_json::Value {
    let value: serde_json::Value = serde_json::from_slice(
        &std::fs::read(dir.join(format!("play-{request}/script.json"))).unwrap(),
    )
    .unwrap();
    assert_eq!(value["status"], "completed");
    assert_eq!(value["model_api_calls"], 0);
    assert_eq!(value["samples"], value["played_samples"]);
    for row in value["sentences"].as_array().unwrap() {
        assert_eq!(row["standby_visible_after_play"], true);
        assert!(row["standby_paints"].as_u64().unwrap() > 0);
        assert_eq!(row["samples"], row["played_samples"]);
    }
    value
}
fn mark(dir: &Path, timeline: &mut Vec<serde_json::Value>, began: Instant, name: &str) {
    timeline.push(serde_json::json!({"stage":name,"elapsed_ms":began.elapsed().as_millis()}));
    std::fs::write(
        dir.join("timeline.json"),
        serde_json::to_vec_pretty(timeline).unwrap(),
    )
    .unwrap();
}
fn visual_gate(dir: &Path, name: &str) {
    if std::env::var("NAI_STANDBY_VISUAL_GATE").is_ok_and(|v| v == "1") {
        let began = Instant::now();
        while !dir.join(format!("checked-{name}")).is_file() {
            assert!(
                began.elapsed() < Duration::from_secs(90),
                "window observation timed out: {name}"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
#[test]
#[ignore = "explicit real local models: one warmup plus four sentences; no model retries"]
fn actual_standby_natural_stop_restore_and_worker_loss() {
    use std::os::windows::process::CommandExt;
    let runtime = PathBuf::from(std::env::var_os("NAI_LIVE_RUNTIME").expect("runtime"));
    let profile = PathBuf::from(std::env::var_os("NAI_LIVE_PROFILE").expect("profile"));
    let dir = PathBuf::from(std::env::var_os("NAI_LIVE_ARTIFACTS").expect("artifacts"));
    let text = std::env::var("NAI_LIVE_TEST_TEXT").expect("explicit four-sentence text");
    let began = Instant::now();
    let unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let c = Controller::new(
        runtime.join("v561_live_control.exe"),
        runtime.join("nai-v561-stream-audio-output.exe"),
        profile,
        dir.clone(),
    )
    .unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("native-start.json"),
        serde_json::to_vec(&serde_json::json!({"unix_ms":unix_ms})).unwrap(),
    )
    .unwrap();
    let mut timeline = Vec::new();
    c.prepare().unwrap();
    let ready = wait(&c, Kind::Prepared, 1, 245);
    assert_eq!(ready.kind, Kind::Prepared as i32, "{}", ready.code);
    c.prepare_content(text.clone()).unwrap();
    let prepared = wait(&c, Kind::ContentPrepared, 2, 46);
    assert_eq!(
        prepared.kind,
        Kind::ContentPrepared as i32,
        "{}",
        prepared.code
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("content-2/standby.json")).unwrap())
            .unwrap();
    assert_eq!(
        metadata["scope"],
        "session_private_static_standby_not_media_authority"
    );
    mark(&dir, &mut timeline, began, "content_ready");
    visual_gate(&dir, "content_ready");
    if std::env::var("NAI_OBS_ATTACH_GATE").is_ok_and(|v| v == "1") {
        let attach = Instant::now();
        while !dir.join("obs-attached").is_file() {
            assert!(
                attach.elapsed() < Duration::from_secs(90),
                "external OBS attachment timeout"
            );
            assert_eq!(
                c.status().kind,
                Kind::ContentPrepared as i32,
                "root changed while waiting for recorder"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        mark(&dir, &mut timeline, began, "obs_attached");
    }
    std::thread::sleep(Duration::from_secs(4));
    let mut reports = Vec::new();
    for request in [3, 4] {
        mark(&dir, &mut timeline, began, &format!("play_{request}"));
        let submitted = Instant::now();
        c.play_prepared(text.clone()).unwrap();
        let done = wait(&c, Kind::Completed, request, 46);
        assert_eq!(done.kind, Kind::Completed as i32, "{}", done.code);
        let observed = submitted.elapsed().as_millis();
        let s = script(&dir, request);
        let first = observed - s["elapsed_ms"].as_u64().unwrap() as u128
            + s["sentences"][0]["device_first_played_script_ms"]
                .as_u64()
                .unwrap() as u128;
        reports.push(serde_json::json!({"script":s,"submit_first_sample_upper_ms":first}));
        mark(&dir, &mut timeline, began, &format!("completed_{request}"));
        if request == 3 {
            visual_gate(&dir, "natural");
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    mark(&dir, &mut timeline, began, "idle_stop_requested");
    let stop = Instant::now();
    c.stop();
    assert_eq!(wait(&c, Kind::Stopped, 5, 5).kind, Kind::Stopped as i32);
    let idle_stop_ms = stop.elapsed().as_millis();
    mark(&dir, &mut timeline, began, "idle_stopped");
    visual_gate(&dir, "idle_stopped");
    std::thread::sleep(Duration::from_secs(1));
    c.play_prepared(text.clone()).unwrap();
    assert_eq!(wait(&c, Kind::Starting, 6, 5).kind, Kind::Starting as i32);
    let progress = Instant::now();
    let first_path = dir.join("play-6/sentence-1/session.json");
    let consumed = loop {
        if let Ok(bytes) = std::fs::read(&first_path)
            && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes)
        {
            break value;
        }
        assert!(
            progress.elapsed() < Duration::from_secs(15),
            "no actual first-sentence consumption"
        );
        assert_eq!(c.status().kind, Kind::Starting as i32);
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(consumed["shown_frames"].as_u64().unwrap() >= 2);
    assert_eq!(consumed["samples"], consumed["played_samples"]);
    assert_eq!(
        c.status().kind,
        Kind::Starting as i32,
        "plan completed before stop"
    );
    mark(&dir, &mut timeline, began, "playing_stop_requested");
    let stop = Instant::now();
    c.stop();
    assert_eq!(wait(&c, Kind::Stopped, 7, 5).kind, Kind::Stopped as i32);
    let playing_stop_ms = stop.elapsed().as_millis();
    mark(&dir, &mut timeline, began, "playing_stopped");
    for leaf in std::fs::read_dir(dir.join("play-6")).unwrap() {
        let failure = leaf.unwrap().path().join("failure.json");
        if failure.is_file() {
            let value: serde_json::Value =
                serde_json::from_slice(&std::fs::read(failure).unwrap()).unwrap();
            assert_eq!(value["standby_visible"], false);
        }
    }
    std::thread::sleep(Duration::from_secs(1));
    c.play_prepared(text).unwrap();
    let resumed = wait(&c, Kind::Completed, 8, 46);
    assert_eq!(resumed.kind, Kind::Completed as i32, "{}", resumed.code);
    let resumed_script = script(&dir, 8);
    let model_api_calls = reports[0]["script"]["sentences"].as_array().unwrap().len() + 1;
    assert!(resumed.epoch > reports[1]["script"]["epoch"].as_u64().unwrap());
    mark(&dir, &mut timeline, began, "restored_completed");
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(c.status().worker_pid, ready.worker_pid);
    let killed = std::process::Command::new("taskkill.exe")
        .args(["/PID", &ready.worker_pid.to_string(), "/T", "/F"])
        .creation_flags(0x08000000)
        .output()
        .unwrap();
    assert!(killed.status.success());
    mark(&dir, &mut timeline, began, "worker_killed");
    let lost = wait(&c, Kind::Unprepared, 8, 5);
    assert!(!lost.visual_prepared);
    assert!(!lost.content_prepared);
    mark(&dir, &mut timeline, began, "worker_unavailable");
    visual_gate(&dir, "worker_unavailable");
    std::thread::sleep(Duration::from_secs(1));
    c.shutdown_wait().unwrap();
    std::fs::write(dir.join("native-summary.json"),serde_json::to_vec_pretty(&serde_json::json!({"scope":"static_standby_local_native_not_soak", "reports":reports,"resumed_script":resumed_script,
        "idle_stop_ms":idle_stop_ms,"playing_stop_ms":playing_stop_ms,"worker_loss_observed":true,"replay_model_calls":0,"model_api_calls":model_api_calls,"shutdown":true})).unwrap()).unwrap();
}
