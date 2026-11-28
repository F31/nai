//! Collects running OBS / public-keeper process facts and portable OBS files.
//!
//! Enumeration is a bounded PowerShell CIM command. A command failure is
//! `enumerated = false`: leftover files are ignored. The collector never
//! guesses `%APPDATA%`, never enumerates DirectShow devices, never talks to
//! OBS websocket, and never starts a recording or virtual camera.
use nai_kernel::obs::PUBLIC_WINDOW_TITLE;
use nai_kernel::obs_observe::{
    KEEPER_IMAGE, OBS_IMAGE, ObservationInput, ObservedProcess, observe_view,
};
use serde_json::Value;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime},
};

const COMMAND_DEADLINE: Duration = Duration::from_secs(4);
const MAX_LOG_BYTES: u64 = 256 * 1024;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Host observation used by [`workbench_obs_observe`].
pub fn collect() -> ObservationInput {
    #[cfg(windows)]
    {
        collect_windows()
    }
    #[cfg(not(windows))]
    {
        ObservationInput {
            enumerated: false,
            ..ObservationInput::default()
        }
    }
}

#[tauri::command]
pub async fn workbench_obs_observe() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(|| observe_view(&collect()).to_string())
        .await
        .map_err(|_| "WB_WORKER_LOST".to_string())
}

#[cfg(windows)]
fn collect_windows() -> ObservationInput {
    let Some(processes) = enumerate_processes() else {
        return ObservationInput {
            enumerated: false,
            ..ObservationInput::default()
        };
    };
    let obs = processes.iter().find(|proc| {
        proc.pid != 0 && proc.name.eq_ignore_ascii_case(OBS_IMAGE) && !proc.path.is_empty()
    });
    let (config_observed, scene_json, scene_path, log_text, log_path) = match obs {
        Some(proc) => match portable_root(Path::new(&proc.path)) {
            Some(config) => {
                let (scene_json, scene_path) = preferred_scene(&config, PUBLIC_WINDOW_TITLE);
                let (log_text, log_path) = latest_log(&config, MAX_LOG_BYTES);
                (true, scene_json, scene_path, log_text, log_path)
            }
            None => (
                false,
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ),
        },
        None => (
            false,
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ),
    };
    ObservationInput {
        enumerated: true,
        processes,
        config_observed,
        scene_json,
        scene_path,
        log_text,
        log_path,
    }
}

#[cfg(windows)]
fn enumerate_processes() -> Option<Vec<ObservedProcess>> {
    use std::os::windows::process::CommandExt;
    let mut child = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-CimInstance Win32_Process -Filter \"Name = 'obs64.exe' OR Name = 'nai-public-output-keeper.exe'\" | Select-Object ProcessId,Name,ExecutablePath | ConvertTo-Json -Compress",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .ok()?;
    let deadline = Instant::now() + COMMAND_DEADLINE;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                break;
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => return None,
        }
    }
    let mut stdout = child.stdout.take()?;
    let mut bytes = Vec::new();
    if stdout.read_to_end(&mut bytes).is_err() {
        return None;
    }
    processes_from_json(&String::from_utf8_lossy(&bytes)).ok()
}

/// Parse CIM JSON. Empty stdout is an empty process list. Malformed JSON is
/// a command failure for the caller.
pub fn processes_from_json(raw: &str) -> Result<Vec<ObservedProcess>, ()> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return Ok(Vec::new());
    }
    let value: Value = serde_json::from_str(trimmed).map_err(|_| ())?;
    let rows = match value {
        Value::Array(rows) => rows,
        Value::Object(_) => vec![value],
        _ => return Err(()),
    };
    let mut processes = Vec::new();
    for row in rows {
        let pid = json_u32(&row, "ProcessId").unwrap_or(0);
        let name = json_string(&row, "Name");
        let path = json_string(&row, "ExecutablePath");
        if pid != 0 && !name.is_empty() {
            processes.push(ObservedProcess { pid, name, path });
        }
    }
    Ok(processes)
}

fn json_u32(row: &Value, key: &str) -> Option<u32> {
    let value = row.get(key)?;
    value
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .or_else(|| value.as_i64().and_then(|n| u32::try_from(n).ok()))
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}

fn json_string(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// Walk up from the running `obs64.exe` looking for `portable_mode.txt` with
/// a sibling `config/obs-studio`. A marker under `bin/64bit` without that
/// config directory is skipped.
pub fn portable_root(exe: &Path) -> Option<PathBuf> {
    let mut dir = exe.parent()?;
    loop {
        let marker = dir.join("portable_mode.txt");
        let config = dir.join("config").join("obs-studio");
        if marker.is_file() && config.is_dir() {
            return Some(config);
        }
        dir = dir.parent()?;
    }
}

/// Newest log in `config/obs-studio/logs`, last `max_bytes` only.
pub fn latest_log(config: &Path, max_bytes: u64) -> (String, String) {
    let logs = config.join("logs");
    let Some(path) = newest_file(&logs, "txt") else {
        return (String::new(), String::new());
    };
    let Ok(bytes) = fs::read(&path) else {
        return (String::new(), path.display().to_string());
    };
    let slice = if bytes.len() as u64 > max_bytes {
        let start = bytes.len() - max_bytes as usize;
        &bytes[start..]
    } else {
        bytes.as_slice()
    };
    let lossy = String::from_utf8_lossy(slice);
    let text = if bytes.len() as u64 > max_bytes {
        match lossy.find('\n') {
            Some(index) => lossy[index + 1..].to_string(),
            None => lossy.into_owned(),
        }
    } else {
        lossy.into_owned()
    };
    (text, path.display().to_string())
}

/// Scene JSON that names the public window, otherwise the newest `.json`.
pub fn preferred_scene(config: &Path, public_title: &str) -> (String, String) {
    let scenes = config.join("basic").join("scenes");
    let Ok(entries) = fs::read_dir(&scenes) else {
        return (String::new(), String::new());
    };
    let mut named = Vec::new();
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) else {
            continue;
        };
        let text = fs::read_to_string(&path).unwrap_or_default();
        files.push((modified, path.clone(), text.clone()));
        if text.contains(public_title) {
            named.push((modified, path, text));
        }
    }
    let chosen = if !named.is_empty() { named } else { files };
    chosen
        .into_iter()
        .max_by_key(|(modified, _, _)| *modified)
        .map(|(_, path, text)| (text, path.display().to_string()))
        .unwrap_or_default()
}

fn newest_file(dir: &Path, extension: &str) -> Option<PathBuf> {
    let mut newest: Option<(SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some(extension) {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) else {
            continue;
        };
        if newest.as_ref().is_none_or(|(stamp, _)| modified > *stamp) {
            newest = Some((modified, path));
        }
    }
    newest.map(|(_, path)| path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nai_kernel::obs_dest::DestKind;
    use nai_kernel::obs_observe::observe;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "nai-obs-{}-{}-{name}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    #[test]
    fn empty_json_is_an_empty_process_list() {
        assert_eq!(processes_from_json("").unwrap(), Vec::new());
        assert_eq!(processes_from_json("null").unwrap(), Vec::new());
    }

    #[test]
    fn a_single_cim_object_is_one_process() {
        let processes = processes_from_json(
            r#"{"ProcessId":22,"Name":"obs64.exe","ExecutablePath":"E:\\obs\\obs64.exe"}"#,
        )
        .unwrap();
        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].pid, 22);
        assert_eq!(processes[0].name, "obs64.exe");
    }

    #[test]
    fn a_cim_array_keeps_both_images() {
        let processes = processes_from_json(
            r#"[{"ProcessId":22,"Name":"obs64.exe","ExecutablePath":"E:\\obs\\obs64.exe"},{"ProcessId":33,"Name":"nai-public-output-keeper.exe","ExecutablePath":"E:\\keeper.exe"}]"#,
        )
        .unwrap();
        assert_eq!(processes.len(), 2);
        assert_eq!(processes[1].name, KEEPER_IMAGE);
    }

    #[test]
    fn malformed_json_is_a_command_failure() {
        assert!(processes_from_json("{").is_err());
        assert!(processes_from_json("\"obs64.exe\"").is_err());
    }

    #[test]
    fn portable_root_skips_the_bin_marker_without_config() {
        let root = scratch("portable");
        let bin = root.join("bin").join("64bit");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("obs64.exe"), []).unwrap();
        fs::write(bin.join("portable_mode.txt"), []).unwrap();
        fs::write(root.join("portable_mode.txt"), []).unwrap();
        fs::create_dir_all(root.join("config").join("obs-studio")).unwrap();
        let found = portable_root(&bin.join("obs64.exe")).unwrap();
        assert_eq!(found, root.join("config").join("obs-studio"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn latest_log_keeps_the_newest_tail() {
        let root = scratch("logs");
        let logs = root.join("logs");
        fs::create_dir_all(&logs).unwrap();
        fs::write(logs.join("old.txt"), "old line\n").unwrap();
        std::thread::sleep(Duration::from_millis(20));
        fs::write(logs.join("new.txt"), "Virtual camera started\n").unwrap();
        let (text, path) = latest_log(&root, 256 * 1024);
        assert!(path.ends_with("new.txt"));
        assert!(text.contains("Virtual camera started"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn preferred_scene_picks_the_public_window_over_mtime() {
        let root = scratch("scenes");
        let scenes = root.join("basic").join("scenes");
        fs::create_dir_all(&scenes).unwrap();
        fs::write(
            scenes.join("other.json"),
            r#"{"name":"other","sources":[]}"#,
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(20));
        fs::write(
            scenes.join("public.json"),
            format!(r#"{{"name":"public","sources":[{{"id":"window_capture","settings":{{"window":"{PUBLIC_WINDOW_TITLE}:minifb_window:{KEEPER_IMAGE}"}}}}]}}"#),
        )
        .unwrap();
        fs::write(
            scenes.join("newer-other.json"),
            r#"{"name":"newer","sources":[]}"#,
        )
        .unwrap();
        let (text, path) = preferred_scene(&root, PUBLIC_WINDOW_TITLE);
        assert!(path.ends_with("public.json"));
        assert!(text.contains(PUBLIC_WINDOW_TITLE));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn collect_maps_to_window_public_without_claims() {
        let observation = observe(&collect());
        assert_eq!(observation.dest.public_via, DestKind::Window);
        assert!(!observation.obs.claims_connected);
        assert!(!observation.dest.claims_virtual_camera_available);
        assert!(!observation.dest.claims_virtual_camera_carries_audio);
    }
}
