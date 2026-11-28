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
        if view.kind == kind as i32 && !view.busy {
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
#[ignore = "explicit real script/GPU/device; exactly three local syntheses"]
fn actual_three_sentence_single_slot_script() {
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
    let prepared = wait(&c, Kind::Prepared, 245);
    c.start("你好，这是数字人直播。现在介绍我们的产品。最后感谢大家的关注。".into())
        .unwrap();
    let completed = wait(&c, Kind::Completed, 46);
    assert_eq!(completed.epoch, 3);
    assert_eq!(completed.samples, completed.played_samples);
    assert_eq!(prepared.worker_pid, completed.worker_pid);
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(artifacts.join("play-2/script.json")).unwrap())
            .unwrap();
    assert_eq!(report["model_api_calls"], 3);
    assert_eq!(report["completed_sentences"], 3);
    assert_eq!(report["output_window_creations"], 1);
    assert_eq!(report["device_open_count"], 1);
    let device_pid = report["device_process_id"].as_u64().unwrap();
    assert!(device_pid > 0);
    let window = report["results"][0]["output_window_identity"]
        .as_str()
        .unwrap();
    assert!(!window.is_empty());
    for sentence in report["results"].as_array().unwrap() {
        assert_eq!(sentence["device_process_id"], device_pid);
        assert_eq!(sentence["output_window_identity"], window);
        assert!(sentence["source_wait_ms"].as_u64().is_some());
    }
    assert!(
        report["results"][1]["gap_from_previous_device_end_ms"]
            .as_u64()
            .is_some()
    );
    assert!(
        report["ready_before_previous_device_end_count"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(report["results"][0]["source_mode"], "streaming_first");
    assert_eq!(report["results"][1]["source_mode"], "streaming_future");
    assert_eq!(
        report["results"][1]["video_source_mode"],
        "incremental_gpu_stream"
    );
    assert_eq!(
        report["results"][2]["video_source_mode"],
        "incremental_gpu_stream"
    );
    assert!(
        report["results"]
            .as_array()
            .unwrap()
            .iter()
            .skip(1)
            .any(|row| row["gpu_first_consumed_frame_script_ms"]
                .as_u64()
                .zip(row["gpu_completed_script_ms"].as_u64())
                .is_some_and(|(first, done)| first < done)),
        "future frames must be consumed before RenderComplete"
    );
    let overlapped = report["results"]
        .as_array()
        .unwrap()
        .windows(2)
        .any(|pair| {
            pair[1]["gpu_prefetch_started_script_ms"]
                .as_u64()
                .zip(pair[0]["device_completed_script_ms"].as_u64())
                .is_some_and(|(start, end)| start < end)
        });
    assert!(
        overlapped,
        "future GPU must actually start before prior device completion"
    );
    c.shutdown_wait().unwrap();
    serde_json::to_writer_pretty(std::fs::File::create_new(artifacts.join("native-summary.json")).unwrap(),&serde_json::json!({"scope":"real_three_sentence_single_tts_script","samples":completed.samples,"worker_pid":completed.worker_pid,"model_api_calls":3,"epoch":3,"shutdown":true})).unwrap();
}
