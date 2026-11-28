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
        let view = c.status();
        if view.kind == kind as i32
            && (view.busy == matches!(kind, Kind::Preparing | Kind::Starting | Kind::Stopping))
        {
            return view;
        }
        assert_ne!(
            view.kind,
            Kind::Failed as i32,
            "native failed: {}",
            view.code
        );
        assert!(
            began.elapsed() < Duration::from_secs(seconds),
            "waiting {kind:?}: {view:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[test]
#[ignore = "explicit real GPU/local TTS; one silent warmup plus one sentence"]
fn actual_private_host_one_sentence_diagnostic() {
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
    c.prepare().unwrap();
    wait(&c, Kind::Prepared, 245);
    c.start("你好，这是数字人直播。".into()).unwrap();
    let began = Instant::now();
    let terminal = loop {
        let event = c.status();
        if matches!(
            Kind::try_from(event.kind),
            Ok(Kind::Completed | Kind::Failed | Kind::Stopped)
        ) {
            break event;
        }
        assert!(began.elapsed() < Duration::from_secs(48));
        std::thread::sleep(Duration::from_millis(20));
    };
    c.shutdown_wait().unwrap();
    std::fs::create_dir_all(&artifacts).unwrap();
    serde_json::to_writer_pretty(std::fs::File::create_new(artifacts.join("one-sentence-summary.json")).unwrap(),
        &serde_json::json!({"scope":"real_one_sentence_diagnostic","kind":terminal.kind,
            "code":terminal.code,"samples":terminal.samples,"played_samples":terminal.played_samples,
            "model_api_calls_max":2,"shutdown":true})).unwrap();
    assert_eq!(terminal.kind, Kind::Completed as i32, "{}", terminal.code);
    assert!(terminal.played_samples.unwrap_or(0) > 0);
    assert_eq!(terminal.samples, terminal.played_samples);
}
#[test]
#[ignore = "explicit visual startup failure injection; one local TTS warmup, no GPU"]
fn actual_private_host_classified_prepare_failure() {
    let runtime = PathBuf::from(std::env::var_os("NAI_LIVE_RUNTIME").expect("runtime"));
    let profile = PathBuf::from(std::env::var_os("NAI_LIVE_PROFILE").expect("profile"));
    let artifacts = PathBuf::from(std::env::var_os("NAI_LIVE_ARTIFACTS").expect("artifacts"));
    std::fs::create_dir_all(&artifacts).unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(profile).unwrap()).unwrap();
    value["worker"] = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/v561_prepare_failure/flashhead_stream_worker.py")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .as_ref()
        .into();
    let injected = artifacts.join("test-only-profile.json");
    std::fs::write(&injected, serde_json::to_vec(&value).unwrap()).unwrap();
    let c = Controller::new(
        runtime.join("v561_live_control.exe"),
        runtime.join("nai-v561-stream-audio-output.exe"),
        injected,
        artifacts.join("session"),
    )
    .unwrap();
    c.prepare().unwrap();
    let began = Instant::now();
    loop {
        let event = c.status();
        assert!(!event.visual_prepared);
        if event.kind == Kind::Failed as i32 {
            assert_eq!(event.code, "HOST_MEMORY_EXHAUSTED");
            assert!(!event.busy);
            break;
        }
        assert!(began.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(c.start("must not play".into()).is_err());
    c.shutdown_wait().unwrap();
}
#[test]
#[ignore = "explicit normal GPU preparation; one silent local TTS warmup"]
fn actual_private_host_prepare_only_shutdown() {
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
    c.prepare().unwrap();
    let ready = wait(&c, Kind::Prepared, 245);
    assert!(ready.visual_prepared);
    assert!(ready.worker_pid > 0);
    c.shutdown_wait().unwrap();
    std::fs::create_dir_all(&artifacts).unwrap();
    serde_json::to_writer_pretty(
        std::fs::File::create_new(artifacts.join("prepare-summary.json")).unwrap(),
        &serde_json::json!({"scope":"normal_private_host_preparation","prepared":true,
        "worker_pid":ready.worker_pid,"model_requests":1,"shutdown":true}),
    )
    .unwrap();
}
#[test]
#[ignore = "explicit real GPU/local TTS/device; max four model requests including prepare cancellation"]
fn actual_private_host_prepare_play_stop_shutdown() {
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
    assert!(c.start("你好。".into()).is_err());
    c.prepare().unwrap();
    wait(&c, Kind::Preparing, 5);
    c.stop();
    let cancelled = wait(&c, Kind::Stopped, 8);
    assert!(!cancelled.visual_prepared);
    c.prepare().unwrap();
    let prepared = wait(&c, Kind::Prepared, 245);
    assert!(prepared.visual_prepared);
    c.start("你好，这是数字人直播。".into()).unwrap();
    let completed = wait(&c, Kind::Completed, 46);
    assert_eq!(completed.samples, completed.played_samples);
    assert!(completed.samples.unwrap() > 0);
    assert_eq!(prepared.worker_pid, completed.worker_pid);
    c.start("你好，这是数字人直播。".into()).unwrap();
    wait(&c, Kind::Starting, 5);
    std::thread::sleep(Duration::from_millis(2200));
    c.stop();
    let stopped = wait(&c, Kind::Stopped, 8);
    c.shutdown_wait().unwrap();
    std::fs::create_dir_all(&artifacts).unwrap();
    serde_json::to_writer_pretty(std::fs::File::create_new(artifacts.join("native-summary.json")).unwrap(),&serde_json::json!({"scope":"real_host_private_ipc_gpu_tts_device","prepare_cancelled":true,"completed_samples":completed.played_samples,"worker_pid":completed.worker_pid,"stop_kind":stopped.kind,"shutdown":true,"model_requests_max":4})).unwrap();
}
