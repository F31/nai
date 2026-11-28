//! Maps collected OBS process and portable-config facts onto [`ObsSurface`]
//! and [`DestSurface`].
//!
//! The mapper is a pure function of [`ObservationInput`]. It opens no process,
//! enumerates no DirectShow device, reads no websocket and starts no recording.
//! A command failure is [`enumerated`](ObservationInput::enumerated) = false:
//! every layer stays unknown. A module file on disk is not a destination
//! signal. Independent tracks stay unknown. Public picture stays the window.
use super::obs::{Layer, LayerState, ObsSurface, PUBLIC_WINDOW_TITLE, ProcessObservation};
use super::obs_dest::{AUDIO_TRACKS, AudioTrackReport, DestKind, DestReport, DestSurface};
use serde_json::{Value, json};

/// OBS image this producer recognises.
pub const OBS_IMAGE: &str = "obs64.exe";
/// Public-output keeper image this producer recognises.
pub const KEEPER_IMAGE: &str = "nai-public-output-keeper.exe";

/// One process row from a host enumerator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedProcess {
    pub pid: u32,
    pub name: String,
    pub path: String,
}

/// Host-collected facts. File fields are ignored unless enumerated and `config_observed`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObservationInput {
    /// Process list was collected. False means the command failed.
    pub enumerated: bool,
    pub processes: Vec<ObservedProcess>,
    /// Portable OBS config directory was found beside a running obs64.
    pub config_observed: bool,
    pub scene_json: String,
    pub scene_path: String,
    pub log_text: String,
    pub log_path: String,
}

/// Surfaces produced from an observation. Claims stay false.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub obs: ObsSurface,
    pub dest: DestSurface,
}

/// Map collected facts onto the two audit surfaces.
pub fn observe(input: &ObservationInput) -> Observation {
    let processes = if input.enumerated {
        input.processes.as_slice()
    } else {
        &[]
    };
    let config_live = input.enumerated && input.config_observed;
    let scene_json = if config_live {
        input.scene_json.as_str()
    } else {
        ""
    };
    let log_text = if config_live {
        input.log_text.as_str()
    } else {
        ""
    };

    let obs_proc = find_image(processes, OBS_IMAGE);
    let keeper_proc = find_image(processes, KEEPER_IMAGE);
    let scene = parse_scene(scene_json, config_live, &input.scene_path);

    let process = if !input.enumerated {
        ProcessObservation::Unknown
    } else if let Some(proc) = obs_proc {
        ProcessObservation::running(proc.pid, OBS_IMAGE)
    } else {
        ProcessObservation::Absent
    };

    let window_capture = scene.window_capture;
    let audio_routing = last_audio_routing(
        log_text,
        window_capture.state == LayerState::Present,
        scene.source_name.as_deref(),
        &input.log_path,
    );

    let obs = ObsSurface {
        process,
        scene_source: scene.scene_source,
        window_capture: window_capture.clone(),
        audio_routing,
        signal: Layer::unknown(),
        claims_connected: false,
        claims_platform_live: false,
        virtual_camera_as_audio: false,
        window_capture_as_audio: false,
    };

    let dest = DestSurface {
        window: DestReport {
            connection: process_connection(input.enumerated, keeper_proc, KEEPER_IMAGE),
            source: window_capture.clone(),
            signal: Layer::unknown(),
        },
        virtual_camera: DestReport {
            connection: process_connection(input.enumerated, obs_proc, OBS_IMAGE),
            source: window_capture,
            signal: last_vcam_signal(log_text, &input.log_path),
        },
        audio_tracks: AUDIO_TRACKS.map(|name| AudioTrackReport {
            name,
            report: DestReport::unknown(),
        }),
        public_via: DestKind::Window,
        claims_virtual_camera_available: false,
        claims_virtual_camera_carries_audio: false,
        claims_independent_audio_is_picture: false,
        claims_track_available: None,
    };

    Observation { obs, dest }
}

/// JSON view matching the workbench `ObsDetection` / `DestSurface` field names.
pub fn observe_view(input: &ObservationInput) -> Value {
    let Observation { obs, dest } = observe(input);
    json!({
        "obs": {
            "process": process_label(&obs.process),
            "sceneSource": layer_label(&obs.scene_source),
            "windowCapture": layer_label(&obs.window_capture),
            "audioRouting": layer_label(&obs.audio_routing),
            "signal": layer_label(&obs.signal),
        },
        "dest": {
            "window": dest_report_view(&dest.window),
            "virtualCamera": dest_report_view(&dest.virtual_camera),
            "tracks": {
                "mix": dest_report_view(&dest.audio_tracks[0].report),
                "tts": dest_report_view(&dest.audio_tracks[1].report),
                "talent": dest_report_view(&dest.audio_tracks[2].report),
            },
            "publicVia": "window",
        },
        "obsReadiness": match obs.readiness() {
            super::obs::ObsReadiness::Blocked => "blocked",
            super::obs::ObsReadiness::Defective => "defective",
            super::obs::ObsReadiness::Observed => "observed",
        },
        "destReadiness": match dest.readiness() {
            super::obs_dest::DestReadiness::Defective => "defective",
            super::obs_dest::DestReadiness::WindowPublic => "window_public",
        },
    })
}

fn process_label(process: &ProcessObservation) -> &'static str {
    match process {
        ProcessObservation::Unknown => "unknown",
        ProcessObservation::Absent => "absent",
        ProcessObservation::Binary { .. } => "binary",
        ProcessObservation::Running { .. } => "running",
    }
}

fn layer_label(layer: &Layer) -> &'static str {
    layer.state.label()
}

fn dest_report_view(report: &DestReport) -> Value {
    json!({
        "connection": layer_label(&report.connection),
        "source": layer_label(&report.source),
        "signal": layer_label(&report.signal),
    })
}

fn find_image<'a>(processes: &'a [ObservedProcess], image: &str) -> Option<&'a ObservedProcess> {
    processes
        .iter()
        .find(|proc| proc.pid != 0 && proc.name.eq_ignore_ascii_case(image))
}

fn process_connection(enumerated: bool, proc: Option<&ObservedProcess>, image: &str) -> Layer {
    if !enumerated {
        return Layer::unknown();
    }
    match proc {
        Some(proc) => {
            let path = if proc.path.is_empty() {
                String::new()
            } else {
                format!(" path={}", proc.path)
            };
            Layer::present(format!("{image} pid={}{path}", proc.pid))
        }
        None => Layer::absent(format!("process list had no {image}")),
    }
}

struct SceneFacts {
    scene_source: Layer,
    window_capture: Layer,
    source_name: Option<String>,
}

fn parse_scene(scene_json: &str, config_observed: bool, scene_path: &str) -> SceneFacts {
    if !config_observed {
        return SceneFacts {
            scene_source: Layer::unknown(),
            window_capture: Layer::unknown(),
            source_name: None,
        };
    }
    if scene_json.trim().is_empty() {
        return SceneFacts {
            scene_source: Layer::absent("portable config had no scene json"),
            window_capture: Layer::absent("portable config had no scene json"),
            source_name: None,
        };
    }
    let Ok(value) = serde_json::from_str::<Value>(scene_json) else {
        return SceneFacts {
            scene_source: Layer::unknown(),
            window_capture: Layer::unknown(),
            source_name: None,
        };
    };
    let sources = match value.get("sources").and_then(Value::as_array) {
        Some(sources) => sources,
        None => {
            return SceneFacts {
                scene_source: Layer::absent("scene json had no sources"),
                window_capture: Layer::absent("scene json had no sources"),
                source_name: None,
            };
        }
    };
    if sources.is_empty() {
        return SceneFacts {
            scene_source: Layer::absent("scene json sources were empty"),
            window_capture: Layer::absent("scene json sources were empty"),
            source_name: None,
        };
    }
    let scene_name = value
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("unnamed");
    let path = if scene_path.is_empty() {
        String::new()
    } else {
        format!(" path={scene_path}")
    };
    let scene_source = Layer::present(format!(
        "scene:{scene_name} sources={}{path}",
        sources.len()
    ));
    let mut window_capture = Layer::absent("no window_capture named the public window");
    let mut source_name = None;
    for source in sources {
        let id = source.get("id").and_then(Value::as_str).unwrap_or("");
        if id != "window_capture" {
            continue;
        }
        let window = source
            .pointer("/settings/window")
            .and_then(Value::as_str)
            .unwrap_or("");
        if window.contains(PUBLIC_WINDOW_TITLE) {
            let name = source
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("window_capture");
            window_capture = Layer::present(format!("window_capture {window}"));
            source_name = Some(name.to_string());
            break;
        }
    }
    SceneFacts {
        scene_source,
        window_capture,
        source_name,
    }
}

fn last_vcam_signal(log: &str, log_path: &str) -> Layer {
    if log.is_empty() {
        return Layer::unknown();
    }
    let mut last = Layer::unknown();
    for line in log.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("virtual camera started") {
            last = Layer::present(with_log_path(line.trim(), log_path));
        } else if lower.contains("virtual camera stopped") {
            last = Layer::absent(with_log_path(line.trim(), log_path));
        }
    }
    last
}

fn last_audio_routing(
    log: &str,
    public_capture_present: bool,
    source_name: Option<&str>,
    log_path: &str,
) -> Layer {
    if log.is_empty() {
        return Layer::unknown();
    }
    let mut last = Layer::unknown();
    for line in log.lines() {
        let loopback = line.contains("PROCESS_LOOPBACK")
            || line.contains("WASAPISource")
            || (line.contains("WASAPI:") && source_name.is_some_and(|name| line.contains(name)));
        if !loopback {
            continue;
        }
        let names_source = source_name.is_some_and(|name| line.contains(name));
        let window_event = line.contains("Failed to find window")
            || line.contains("window disappeared")
            || line.contains("failed to start");
        let relevant = names_source
            || window_event
            || line.contains(PUBLIC_WINDOW_TITLE)
            || public_capture_present;
        if !relevant {
            continue;
        }
        if line.contains("initialized") {
            last = Layer::present(with_log_path(line.trim(), log_path));
        } else if window_event {
            last = Layer::absent(with_log_path(line.trim(), log_path));
        }
    }
    last
}

fn with_log_path(line: &str, log_path: &str) -> String {
    if log_path.is_empty() {
        line.to_string()
    } else {
        format!("{line} path={log_path}")
    }
}
