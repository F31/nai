#![cfg(windows)]
//! Bounded real experience capture. Performance is assessed separately from integrity.
use nai_contracts::v561_live::{Event, Kind};
use nai_workbench_host::v561_live::Controller;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn wait(c: &Controller, expected: Kind, seconds: u64) -> Event {
    let started = Instant::now();
    loop {
        let view = c.status();
        if !view.busy && matches!(Kind::try_from(view.kind), Ok(Kind::Failed | Kind::Stopped)) {
            return view;
        }
        if view.kind == expected as i32 && !view.busy {
            return view;
        }
        assert!(
            started.elapsed() < Duration::from_secs(seconds),
            "waiting {expected:?}: {view:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "explicit four local syntheses, real GPU/window/audio; no cloud or streaming broadcast"]
fn actual_four_sentence_experience() {
    let runtime = PathBuf::from(std::env::var_os("NAI_LIVE_RUNTIME").expect("runtime"));
    let profile = PathBuf::from(std::env::var_os("NAI_LIVE_PROFILE").expect("profile"));
    let artifacts = PathBuf::from(std::env::var_os("NAI_LIVE_ARTIFACTS").expect("artifacts"));
    let controller = Controller::new(
        runtime.join("v561_live_control.exe"),
        runtime.join("nai-v561-stream-audio-output.exe"),
        profile,
        artifacts.clone(),
    )
    .unwrap();
    let preparing = Instant::now();
    controller.prepare().unwrap();
    let prepared = wait(&controller, Kind::Prepared, 245);
    let prepare_ms = preparing.elapsed().as_millis();
    serde_json::to_writer_pretty(
        std::fs::File::create_new(artifacts.join("preparation-observation.json")).unwrap(),
        &serde_json::json!({"prepare_ms":prepare_ms,"terminal_kind":prepared.kind,
            "terminal_code":prepared.code,"worker_pid":prepared.worker_pid}),
    )
    .unwrap();
    assert_eq!(prepared.kind, Kind::Prepared as i32, "{}", prepared.code);
    assert!(prepared.visual_prepared && prepared.tts_prepared);
    let warm: serde_json::Value = serde_json::from_slice(
        &std::fs::read(artifacts.join("prepare-1/tts-warmup.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(warm["status"], "completed");
    assert_eq!(warm["model_api_calls"], 1);
    assert_eq!(warm["audio_discarded"], true);
    let submitted = Instant::now();
    controller
        .start(
            include_str!("../../../tests/v561/experience-script.txt")
                .trim()
                .into(),
        )
        .unwrap();
    let completed = wait(&controller, Kind::Completed, 46);
    let elapsed = submitted.elapsed().as_millis();
    serde_json::to_writer_pretty(std::fs::File::create_new(artifacts.join("submission-observation.json")).unwrap(),
        &serde_json::json!({"input_mode":"prepared_text","llm_included":false,"prepared_before_submit":true,
            "prepare_ms":prepare_ms,"submit_to_completed_ms":elapsed,"terminal_kind":completed.kind,
            "terminal_code":completed.code,"worker_pid":completed.worker_pid})).unwrap();
    let shutdown = controller.shutdown_wait();
    assert_eq!(completed.kind, Kind::Completed as i32, "{}", completed.code);
    assert_eq!(completed.samples, completed.played_samples);
    assert_eq!(prepared.worker_pid, completed.worker_pid);
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(artifacts.join("play-2/script.json")).unwrap())
            .unwrap();
    assert_eq!(report["model_api_calls"], 4);
    assert_eq!(report["completed_sentences"], 4);
    assert_eq!(report["output_window_creations"], 1);
    assert_eq!(report["device_open_count"], 1);
    shutdown.unwrap();
}
