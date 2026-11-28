#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
//! Current single-room AV entry. No legacy DB or audio-only fallback.
use nai_contracts::v561_live::{Event, Kind};
use nai_workbench_host::v561_live::Controller;
use prost::Message;
use std::{
    path::PathBuf,
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
            v561_live_config
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
