#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
//! Current single-room AV entry. No legacy DB or audio-only fallback.
use nai_contracts::v561_live::{Event, Kind};
use nai_workbench_host::v561_live::Controller;
use prost::Message;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
struct Live {
    controller: Mutex<Option<Arc<Controller>>>,
    runtime: PathBuf,
    error: Mutex<String>,
}
fn controller(live: &Live) -> Result<Arc<Controller>, String> {
    live.controller
        .lock()
        .map_err(|_| "STATE_FAILED")?
        .clone()
        .ok_or_else(|| {
            live.error
                .lock()
                .map(|v| v.clone())
                .unwrap_or("STATE_FAILED".into())
        })
}
#[tauri::command]
fn v561_live_status(live: tauri::State<Live>) -> Vec<u8> {
    controller(&live)
        .map(|c| c.status())
        .unwrap_or_else(|code| Event {
            version: 1,
            kind: Kind::Unprepared as i32,
            code,
            ..Default::default()
        })
        .encode_to_vec()
}
#[tauri::command]
fn v561_live_prepare(live: tauri::State<Live>) -> Result<(), String> {
    controller(&live)?.prepare()
}
#[tauri::command]
fn v561_live_start(text: String, live: tauri::State<Live>) -> Result<(), String> {
    controller(&live)?.start(text)
}
#[tauri::command]
fn v561_live_prepare_content(text: String, live: tauri::State<Live>) -> Result<(), String> {
    controller(&live)?.prepare_content(text)
}
#[tauri::command]
fn v561_live_play_prepared(
    text: String,
    obs: Option<bool>,
    live: tauri::State<Live>,
) -> Result<(), String> {
    let controller = controller(&live)?;
    if obs.unwrap_or(false) {
        controller.play_prepared_obs(text)
    } else {
        controller.play_prepared(text)
    }
}
#[tauri::command]
fn v561_live_ask(question: String, live: tauri::State<Live>) -> Result<(), String> {
    controller(&live)?.ask(question)
}
#[tauri::command]
fn v561_live_stop(live: tauri::State<Live>) {
    if let Ok(c) = controller(&live) {
        c.stop();
    }
}
fn load(live: &Live, profile: PathBuf) -> Result<(), String> {
    if controller(live).is_ok_and(|c| c.status().busy) {
        return Err("BUSY".into());
    }
    if let Ok(old) = controller(live) {
        old.shutdown_wait()?;
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "CLOCK_FAILED")?
        .as_nanos();
    let next = Controller::new(
        live.runtime.join("v561_live_control.exe"),
        live.runtime.join("nai-v561-stream-audio-output.exe"),
        profile,
        live.runtime
            .join("operator-artifacts")
            .join(format!("session-{stamp}")),
    )?;
    *live.controller.lock().map_err(|_| "STATE_FAILED")? = Some(Arc::new(next));
    *live.error.lock().map_err(|_| "STATE_FAILED")? = String::new();
    Ok(())
}
fn find_root(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    for _ in 0..8 {
        if dir.join("scripts/flashhead_stream_worker.py").is_file()
            && dir.join("docs/jjf.jpg").is_file()
        {
            return Some(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}
/// Pick the first Python that can import the worker's base dependencies:
/// `google.protobuf` (wire), `numpy` and `soxr`. Missing any one makes the
/// worker die before it can write a startup-failure record, so Prepare stalls
/// for the whole maintenance window with no usable error. Heavier model
/// dependencies (CUDA torch / flash_head) are a separate readiness check and
/// are reported by the engine as MODEL_PREPARATION_FAILED. Candidates: explicit
/// env override, repo-local venv, then PATH.
fn python_executable(root: &Path) -> Result<PathBuf, String> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(value) = std::env::var_os("NAI_V561_PYTHON") {
        candidates.push(PathBuf::from(value));
    }
    candidates.push(root.join("target-windows/v561-worker-python/Scripts/python.exe"));
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join("python.exe");
            if !candidates.contains(&candidate) {
                candidates.push(candidate);
            }
        }
    }
    let mut found_any = false;
    for candidate in candidates {
        if !candidate.is_file() {
            continue;
        }
        found_any = true;
        let usable = std::process::Command::new(&candidate)
            .arg("-c")
            .arg("import google.protobuf.internal.builder, numpy, soxr")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if usable {
            return Ok(candidate);
        }
    }
    let code = if found_any {
        "PYTHON_DEPS_MISSING"
    } else {
        "PYTHON_MISSING"
    };
    Err(code.into())
}
fn write_json(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| "DEFAULT_SERIALIZE_FAILED")?;
    fs::write(path, bytes).map_err(|_| "DEFAULT_WRITE_FAILED".into())
}
fn reference_wav(path: &Path, source: &Path) -> Result<(), String> {
    if source.is_file() {
        fs::copy(source, path).map_err(|_| "DEFAULT_WRITE_FAILED")?;
        return Ok(());
    }
    // One second of 24 kHz mono silence; a technical reference, not a licensed voice.
    let samples = 24_000u32;
    let data_len = samples * 2;
    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&samples.to_le_bytes());
    wav.extend_from_slice(&(samples * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend(vec![0u8; data_len as usize]);
    fs::write(path, wav).map_err(|_| "DEFAULT_WRITE_FAILED".into())
}
fn default_config(live: &Live) -> Result<PathBuf, String> {
    use sha2::{Digest, Sha256};
    let root = find_root(&live.runtime).ok_or("DEFAULT_ASSETS_MISSING")?;
    let python = python_executable(&root)?;
    let runtime = &live.runtime;
    fs::create_dir_all(runtime).map_err(|_| "DEFAULT_WRITE_FAILED")?;

    let model_dir = root.join("target-windows/models/SoulX-FlashHead-1_3B/Model_Lite");
    let wav2vec_dir = root.join("target-windows/models/wav2vec2-base-960h");
    let portrait = root.join("docs/jjf.jpg");
    // Resident::start requires this exact file name (avatar_stream wire protocol).
    let worker = root.join("scripts/flashhead_stream_worker.py");
    let scripts = root.join("scripts");
    let contracts = root.join("packages/contracts-python/src/generated");
    let source_wav = root.join("target-windows/v561-qwen-restart-check.wav");
    for (path, code) in [
        (&model_dir, "MODEL_MISSING"),
        (&wav2vec_dir, "WAV2VEC_MISSING"),
        (&portrait, "PORTRAIT_MISSING"),
        (&worker, "WORKER_MISSING"),
        (&scripts, "WORKER_DEPS_MISSING"),
        (&contracts, "CONTRACTS_MISSING"),
    ] {
        if !path.exists() {
            return Err(code.into());
        }
    }
    let digest = Sha256::digest(fs::read(&portrait).map_err(|_| "PORTRAIT_UNREADABLE")?);
    let portrait_hash = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    let avatar_config = runtime.join("default-avatar.json");
    let tts_config = runtime.join("default-tts.json");
    let mapping_reader = runtime.join("default-mapping.json");
    let temp_dir = runtime.join("avatar-temp");
    let reference_file = runtime.join("default-reference.wav");
    fs::create_dir_all(&temp_dir).map_err(|_| "DEFAULT_WRITE_FAILED")?;
    fs::write(&mapping_reader, b"{}").map_err(|_| "DEFAULT_WRITE_FAILED")?;
    reference_wav(&reference_file, &source_wav)?;

    write_json(
        &avatar_config,
        &serde_json::json!({
            "safetensors_backend": "mmap",
            "python": python,
            "upstream_dir": scripts,
            "dependency_dirs": [scripts, contracts],
            "model_dir": model_dir,
            "wav2vec_dir": wav2vec_dir,
            "portrait": portrait,
            "portrait_sha256": portrait_hash,
            "portrait_authorized": true,
            "mapping_reader": mapping_reader,
            "temp_dir": temp_dir,
            "width": 768,
            "height": 768,
            "frame_num": 49,
            "seed": 42,
            "face_ratio": 2.0,
            "timeout_ms": 45000,
            "maintenance_timeout_ms": 240000,
            "warmup_samples": 48000,
            "sample_steps": 4
        }),
    )?;
    write_json(
        &tts_config,
        &serde_json::json!({
            "endpoint": "http://127.0.0.1:50000/inference_zero_shot",
            "reference_file": reference_file,
            "prompt_text": "你好，这是数字人直播。<|endofprompt|>",
            "deadline_ms": 30000,
            "max_samples": 144000
        }),
    )?;

    let profile = runtime.join("live-session.json");
    write_json(
        &profile,
        &serde_json::json!({
            "avatar_config": avatar_config,
            "tts_config": tts_config,
            "worker": worker,
            "prefill_samples": 2400,
            "tts_warmup_text": "你好，这是数字人直播。",
            "prepared_cache": {
                "max_bytes": 67108864,
                "ttl_ms": 600000,
                "max_entries": 4,
                "max_total_bytes": 268435456,
                "prefetch_max_bytes": 33554432
            }
        }),
    )?;
    Ok(profile)
}
#[tauri::command]
fn v561_live_default_config(live: tauri::State<'_, Live>) -> Result<(), String> {
    let profile = default_config(&live)?;
    load(&live, profile)
}
#[tauri::command]
async fn v561_live_config(live: tauri::State<'_, Live>) -> Result<(), String> {
    if controller(&live).is_ok_and(|c| c.status().busy) {
        return Err("BUSY".into());
    }
    if let Some(path) = rfd::AsyncFileDialog::new()
        .set_title("选择本机音画装配配置")
        .add_filter("音画配置", &["json"])
        .pick_file()
        .await
    {
        load(&live, path.path().to_path_buf())?;
    }
    Ok(())
}
fn setup() -> Live {
    let runtime = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_default();
    let mut live = Live {
        controller: Mutex::new(None),
        runtime,
        error: Mutex::new("CONFIG_REQUIRED".into()),
    };
    let mut profile = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(key) = args.next() {
        let Some(value) = args.next() else { break };
        match key.to_str() {
            Some("--runtime-dir") => live.runtime = value.into(),
            Some("--profile") => profile = Some(PathBuf::from(value)),
            _ => {
                *live.error.lock().unwrap() = "ARGUMENTS_INVALID".into();
                return live;
            }
        }
    }
    if profile.is_none() {
        let default = live.runtime.join("live-session.json");
        if default.is_file() {
            profile = Some(default);
        } else {
            match default_config(&live) {
                Ok(path) => profile = Some(path),
                Err(code) => *live.error.lock().unwrap() = code,
            }
        }
    }
    if let Some(profile) = profile
        && let Err(e) = load(&live, profile)
    {
        *live.error.lock().unwrap() = e;
    }
    live
}
fn main() {
    use tauri::Manager;
    let mut context = tauri::generate_context!("v561-voice/tauri.conf.json");
    let profile = std::env::current_exe()
        .expect("executable")
        .parent()
        .expect("runtime")
        .join("v561-voice-webview");
    std::fs::create_dir_all(&profile).expect("webview directory");
    for window in &mut context.config_mut().app.windows {
        window.data_directory = Some(profile.clone());
    }
    tauri::Builder::default()
        .manage(setup())
        .invoke_handler(tauri::generate_handler![
            v561_live_status,
            v561_live_prepare,
            v561_live_start,
            v561_live_prepare_content,
            v561_live_play_prepared,
            v561_live_ask,
            v561_live_stop,
            v561_live_config,
            v561_live_default_config
        ])
        .build(context)
        .expect("AV workbench initialization")
        .run(|app, event| {
            if matches!(
                event,
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
            ) && let Ok(c) = controller(&app.state::<Live>())
            {
                c.stop();
            }
        });
}
