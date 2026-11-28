#![cfg(windows)]
use nai_contracts::v561_live::Kind;
use nai_workbench_host::v561_live::Controller;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
#[test]
#[ignore = "explicit GPU/local TTS; at most current and one future model request"]
fn stop_while_future_generation_started_and_current_audio_not_finished() {
    let runtime = PathBuf::from(std::env::var_os("NAI_LIVE_RUNTIME").unwrap());
    let profile = PathBuf::from(std::env::var_os("NAI_LIVE_PROFILE").unwrap());
    let artifacts = PathBuf::from(std::env::var_os("NAI_LIVE_ARTIFACTS").unwrap());
    let c = Controller::new(
        runtime.join("v561_live_control.exe"),
        runtime.join("nai-v561-stream-audio-output.exe"),
        profile,
        artifacts.clone(),
    )
    .unwrap();
    c.prepare().unwrap();
    let began = Instant::now();
    loop {
        let s = c.status();
        assert_ne!(s.kind, Kind::Failed as i32, "{}", s.code);
        if s.kind == Kind::Prepared as i32 {
            break;
        }
        assert!(began.elapsed() < Duration::from_secs(245));
        std::thread::sleep(Duration::from_millis(10));
    }
    c.start("你好，这是数字人直播。现在介绍我们的产品。最后感谢大家的关注。".into())
        .unwrap();
    let began = Instant::now();
    loop {
        let s = c.status();
        assert_ne!(s.kind, Kind::Failed as i32, "{}", s.code);
        if artifacts
            .join("play-2/gpu-activity/segment-2.json")
            .is_file()
        {
            break;
        }
        assert!(began.elapsed() < Duration::from_secs(45));
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !artifacts.join("play-2/sentence-1/session.json").exists(),
        "current audio must still be unfinished"
    );
    c.stop();
    let stopped = Instant::now();
    loop {
        let s = c.status();
        if s.kind == Kind::Stopped as i32 && !s.busy {
            break;
        }
        assert!(stopped.elapsed() < Duration::from_secs(8), "{s:?}");
        std::thread::sleep(Duration::from_millis(10));
    }
    c.shutdown_wait().unwrap();
    serde_json::to_writer_pretty(std::fs::File::create_new(artifacts.join("stop-summary.json")).unwrap(),
        &serde_json::json!({"future_stage_observed":true,"current_completion_not_reported":true,"stopped":true,"shutdown":true,"max_model_calls":2})).unwrap();
}
