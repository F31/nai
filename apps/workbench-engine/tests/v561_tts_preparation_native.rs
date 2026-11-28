#![cfg(windows)]
//! Real private control with an explicit CPU HTTP fault fixture; no model/GPU/audio.
use nai_contracts::v561_live::Kind;
use nai_workbench_host::v561_live::Controller;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

#[test]
#[ignore = "explicit native control fault fixture; zero synthesis/GPU/device calls"]
fn native_tts_failure_and_busy_stop_do_not_prepare_visual() {
    let runtime = PathBuf::from(std::env::var_os("NAI_LIVE_RUNTIME").expect("runtime"));
    let profile = PathBuf::from(std::env::var_os("NAI_LIVE_PROFILE").expect("profile"));
    let artifacts = PathBuf::from(std::env::var_os("NAI_LIVE_ARTIFACTS").expect("artifacts"));
    let mut outcomes = Vec::new();
    for busy in [false, true] {
        let dir = artifacts.join(if busy { "busy-stop" } else { "unavailable" });
        std::fs::create_dir_all(&dir).unwrap();
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let address = server.server_addr().to_string();
        let done = Arc::new(AtomicBool::new(false));
        let stop = done.clone();
        let health = Arc::new(AtomicU64::new(0));
        let seen = health.clone();
        let models = Arc::new(AtomicU64::new(0));
        let calls = models.clone();
        let serving = std::thread::spawn(move || {
            while !stop.load(Ordering::Acquire) {
                if let Some(request) = server.recv_timeout(Duration::from_millis(20)).unwrap() {
                    if request.url() == "/health" {
                        seen.fetch_add(1, Ordering::AcqRel);
                        let response = if busy {
                            tiny_http::Response::from_string(r#"{"status":"ok","busy":true}"#)
                        } else {
                            tiny_http::Response::from_string("unavailable").with_status_code(503)
                        };
                        let _ = request.respond(response);
                    } else {
                        calls.fetch_add(1, Ordering::AcqRel);
                        let _ = request.respond(tiny_http::Response::empty(500));
                    }
                }
            }
        });
        let mut p: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&profile).unwrap()).unwrap();
        let mut t: serde_json::Value =
            serde_json::from_slice(&std::fs::read(p["tts_config"].as_str().unwrap()).unwrap())
                .unwrap();
        t["endpoint"] = format!("http://{address}/inference_zero_shot").into();
        let tts = dir.join("tts.json");
        std::fs::write(&tts, serde_json::to_vec(&t).unwrap()).unwrap();
        p["tts_config"] = tts.to_string_lossy().as_ref().into();
        let fixture = dir.join("profile.json");
        std::fs::write(&fixture, serde_json::to_vec(&p).unwrap()).unwrap();
        let c = Controller::new(
            runtime.join("v561_live_control.exe"),
            runtime.join("nai-v561-stream-audio-output.exe"),
            fixture,
            dir.join("session"),
        )
        .unwrap();
        c.prepare().unwrap();
        let began = Instant::now();
        let mut cancelled = None;
        let terminal = loop {
            let state = c.status();
            if busy && cancelled.is_none() && health.load(Ordering::Acquire) > 0 {
                c.stop();
                cancelled = Some(Instant::now());
            }
            if !state.busy
                && state.kind
                    == if busy {
                        Kind::Stopped as i32
                    } else {
                        Kind::Failed as i32
                    }
            {
                break state;
            }
            assert!(began.elapsed() < Duration::from_secs(8), "{state:?}");
            std::thread::sleep(Duration::from_millis(5));
        };
        let stop_ms = cancelled.map(|at| at.elapsed().as_millis());
        c.shutdown_wait().unwrap();
        done.store(true, Ordering::Release);
        serving.join().unwrap();
        assert!(!terminal.visual_prepared && !terminal.tts_prepared);
        assert_eq!(terminal.worker_pid, 0);
        assert!(c.start("不得开始。".into()).is_err());
        if busy {
            assert!(stop_ms.unwrap() < 500);
        } else {
            assert_eq!(terminal.code, "TTS_PREPARATION_FAILED");
        }
        assert_eq!(models.load(Ordering::Acquire), 0);
        let warm: serde_json::Value = serde_json::from_slice(
            &std::fs::read(dir.join("session/prepare-1/tts-warmup.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(warm["synthesis_attempts"], 0);
        outcomes.push(serde_json::json!({"busy":busy,"terminal_code":terminal.code,"stop_ms":stop_ms,
            "synthesis_calls":0,"worker_pid":terminal.worker_pid,"tts_prepared":terminal.tts_prepared}));
    }
    serde_json::to_writer_pretty(
        std::fs::File::create_new(artifacts.join("fault-summary.json")).unwrap(),
        &outcomes,
    )
    .unwrap();
}
