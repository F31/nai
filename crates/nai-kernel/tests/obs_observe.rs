//! v49 2.4.2.1 OBS destination observation mapper.
use nai_kernel::obs::{LayerState, ObsReadiness, PUBLIC_WINDOW_TITLE, ProcessObservation};
use nai_kernel::obs_dest::{DestKind, DestReadiness};
use nai_kernel::obs_observe::{
    KEEPER_IMAGE, OBS_IMAGE, ObservationInput, ObservedProcess, observe, observe_view,
};

fn obs64(pid: u32) -> ObservedProcess {
    ObservedProcess {
        pid,
        name: OBS_IMAGE.into(),
        path: r"E:\projects\nai\target-windows\obs-isolated-v49\bin\64bit\obs64.exe".into(),
    }
}

fn keeper(pid: u32) -> ObservedProcess {
    ObservedProcess {
        pid,
        name: KEEPER_IMAGE.into(),
        path: r"E:\projects\nai\target-windows\live-auto-runtime\nai-public-output-keeper.exe"
            .into(),
    }
}

fn public_scene() -> String {
    format!(
        r#"{{"name":"nai-v49-device-clock","sources":[{{"id":"window_capture","name":"nai-public-window-only","settings":{{"window":"{PUBLIC_WINDOW_TITLE}:minifb_window:{KEEPER_IMAGE}","capture_audio":true}}}}]}}"#
    )
}

fn other_scene() -> String {
    r#"{"name":"other","sources":[{"id":"window_capture","name":"notepad","settings":{"window":"Untitled - Notepad:Notepad:notepad.exe"}}]}"#.into()
}

#[test]
fn an_empty_input_stays_unknown_and_window_public() {
    let observation = observe(&ObservationInput::default());
    assert!(matches!(
        observation.obs.process,
        ProcessObservation::Unknown
    ));
    assert_eq!(observation.obs.scene_source.state, LayerState::Unknown);
    assert_eq!(observation.obs.window_capture.state, LayerState::Unknown);
    assert_eq!(observation.obs.audio_routing.state, LayerState::Unknown);
    assert_eq!(observation.obs.signal.state, LayerState::Unknown);
    assert_eq!(
        observation.dest.window.connection.state,
        LayerState::Unknown
    );
    assert_eq!(
        observation.dest.virtual_camera.signal.state,
        LayerState::Unknown
    );
    assert_eq!(observation.dest.public_via, DestKind::Window);
    assert!(!observation.obs.claims_connected);
    assert!(!observation.dest.claims_virtual_camera_available);
    assert_eq!(observation.obs.readiness(), ObsReadiness::Blocked);
    assert_eq!(observation.dest.readiness(), DestReadiness::WindowPublic);
}

#[test]
fn a_failed_command_ignores_leftover_processes_and_files() {
    let observation = observe(&ObservationInput {
        enumerated: false,
        processes: vec![obs64(44740), keeper(1001)],
        config_observed: true,
        scene_json: public_scene(),
        scene_path: "leftover.json".into(),
        log_text: "Virtual camera started\nWASAPI: Device '[VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK]' initialized (source: nai-public-window-only)\n".into(),
        log_path: "leftover.txt".into(),
    });
    assert!(matches!(
        observation.obs.process,
        ProcessObservation::Unknown
    ));
    assert_eq!(observation.obs.window_capture.state, LayerState::Unknown);
    assert_eq!(observation.obs.audio_routing.state, LayerState::Unknown);
    assert_eq!(
        observation.dest.virtual_camera.connection.state,
        LayerState::Unknown
    );
    assert_eq!(
        observation.dest.virtual_camera.signal.state,
        LayerState::Unknown
    );
}

#[test]
fn enumerated_none_is_absent_not_unknown() {
    let observation = observe(&ObservationInput {
        enumerated: true,
        ..ObservationInput::default()
    });
    assert!(matches!(
        observation.obs.process,
        ProcessObservation::Absent
    ));
    assert_eq!(observation.dest.window.connection.state, LayerState::Absent);
    assert_eq!(
        observation.dest.virtual_camera.connection.state,
        LayerState::Absent
    );
    assert_eq!(observation.obs.scene_source.state, LayerState::Unknown);
}

#[test]
fn a_running_obs_without_portable_config_is_process_only() {
    let observation = observe(&ObservationInput {
        enumerated: true,
        processes: vec![obs64(44740)],
        config_observed: false,
        scene_json: public_scene(),
        log_text: "Virtual camera started\n".into(),
        ..ObservationInput::default()
    });
    match observation.obs.process {
        ProcessObservation::Running { pid, ref image } => {
            assert_eq!(pid, 44740);
            assert_eq!(image, OBS_IMAGE);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(observation.obs.scene_source.state, LayerState::Unknown);
    assert_eq!(observation.obs.window_capture.state, LayerState::Unknown);
    assert_eq!(
        observation.dest.virtual_camera.connection.state,
        LayerState::Present
    );
    assert_eq!(
        observation.dest.virtual_camera.source.state,
        LayerState::Unknown
    );
    assert_eq!(
        observation.dest.virtual_camera.signal.state,
        LayerState::Unknown
    );
    assert!(!observation.obs.claims_connected);
    assert!(!observation.obs.all_observed());
}

#[test]
fn a_public_window_capture_in_the_scene_is_present() {
    let observation = observe(&ObservationInput {
        enumerated: true,
        processes: vec![obs64(22), keeper(33)],
        config_observed: true,
        scene_json: public_scene(),
        scene_path: "nai-v49-device-clock.json".into(),
        ..ObservationInput::default()
    });
    assert_eq!(observation.obs.scene_source.state, LayerState::Present);
    assert!(
        observation
            .obs
            .window_capture
            .evidence
            .contains(PUBLIC_WINDOW_TITLE)
    );
    assert_eq!(observation.obs.window_capture.state, LayerState::Present);
    assert_eq!(observation.dest.window.source.state, LayerState::Present);
    assert_eq!(
        observation.dest.virtual_camera.source.state,
        LayerState::Present
    );
    assert_eq!(
        observation.dest.window.connection.state,
        LayerState::Present
    );
    assert_eq!(observation.obs.signal.state, LayerState::Unknown);
    assert_eq!(observation.dest.window.signal.state, LayerState::Unknown);
    assert_eq!(observation.dest.public_via, DestKind::Window);
}

#[test]
fn a_window_capture_of_another_title_is_absent() {
    let observation = observe(&ObservationInput {
        enumerated: true,
        processes: vec![obs64(22)],
        config_observed: true,
        scene_json: other_scene(),
        ..ObservationInput::default()
    });
    assert_eq!(observation.obs.scene_source.state, LayerState::Present);
    assert_eq!(observation.obs.window_capture.state, LayerState::Absent);
}

#[test]
fn wasapi_loopback_initialized_is_audio_present() {
    let observation = observe(&ObservationInput {
        enumerated: true,
        processes: vec![obs64(22)],
        config_observed: true,
        scene_json: public_scene(),
        log_text: "23:34:44.790: WASAPI: Device '[VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK]' [48000 Hz] initialized (source: nai-public-window-only (音频))\n".into(),
        ..ObservationInput::default()
    });
    assert_eq!(observation.obs.audio_routing.state, LayerState::Present);
    assert!(
        observation
            .obs
            .audio_routing
            .evidence
            .contains("PROCESS_LOOPBACK")
    );
}

#[test]
fn a_later_window_disappeared_line_is_audio_absent() {
    let observation = observe(&ObservationInput {
        enumerated: true,
        processes: vec![obs64(22)],
        config_observed: true,
        scene_json: public_scene(),
        log_text: "23:34:44.790: WASAPI: Device '[VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK]' [48000 Hz] initialized (source: nai-public-window-only (音频))\n23:34:50.873: [WASAPISource::ProcessCaptureData] window disappeared\n".into(),
        ..ObservationInput::default()
    });
    assert_eq!(observation.obs.audio_routing.state, LayerState::Absent);
    assert!(
        observation
            .obs
            .audio_routing
            .evidence
            .contains("window disappeared")
    );
}

#[test]
fn virtual_camera_started_is_vcam_signal_present() {
    let observation = observe(&ObservationInput {
        enumerated: true,
        processes: vec![obs64(22)],
        config_observed: true,
        scene_json: public_scene(),
        log_text: "23:40:00.001: Virtual camera started\n".into(),
        ..ObservationInput::default()
    });
    assert_eq!(
        observation.dest.virtual_camera.signal.state,
        LayerState::Present
    );
    assert!(observation.dest.virtual_camera.observed());
    assert_eq!(observation.dest.public_via, DestKind::Window);
    assert!(!observation.dest.claims_virtual_camera_available);
    assert_eq!(observation.obs.signal.state, LayerState::Unknown);
}

#[test]
fn a_later_virtual_camera_stopped_line_is_absent() {
    let observation = observe(&ObservationInput {
        enumerated: true,
        processes: vec![obs64(22)],
        config_observed: true,
        scene_json: public_scene(),
        log_text: "Virtual camera started\n23:41:00.000: Virtual camera stopped\n".into(),
        ..ObservationInput::default()
    });
    assert_eq!(
        observation.dest.virtual_camera.signal.state,
        LayerState::Absent
    );
    assert!(!observation.dest.virtual_camera.observed());
}

#[test]
fn a_module_file_is_not_a_virtual_camera_signal() {
    let observation = observe(&ObservationInput {
        enumerated: true,
        processes: vec![obs64(22)],
        config_observed: true,
        scene_json: public_scene(),
        log_text: "23:00:00.000: [win-dshow] loaded module 'obs-virtualcam-module64.dll'\n23:00:00.001: LoadLibrary win-dshow.dll\n".into(),
        ..ObservationInput::default()
    });
    assert_eq!(
        observation.dest.virtual_camera.signal.state,
        LayerState::Unknown
    );
}

#[test]
fn independent_tracks_stay_unknown() {
    let observation = observe(&ObservationInput {
        enumerated: true,
        processes: vec![obs64(22), keeper(33)],
        config_observed: true,
        scene_json: public_scene(),
        log_text: "Virtual camera started\nWASAPI: Device '[VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK]' initialized (source: nai-public-window-only)\n".into(),
        ..ObservationInput::default()
    });
    assert!(
        observation
            .dest
            .audio_tracks
            .iter()
            .all(|track| track.report.connection.state == LayerState::Unknown
                && track.report.source.state == LayerState::Unknown
                && track.report.signal.state == LayerState::Unknown)
    );
}

#[test]
fn observe_view_uses_camel_case_and_keeps_public_via_window() {
    let view = observe_view(&ObservationInput {
        enumerated: true,
        processes: vec![obs64(22), keeper(33)],
        config_observed: true,
        scene_json: public_scene(),
        log_text: "Virtual camera started\n".into(),
        ..ObservationInput::default()
    });
    assert_eq!(view["obs"]["process"], "running");
    assert_eq!(view["obs"]["windowCapture"], "present");
    assert_eq!(view["obs"]["signal"], "unknown");
    assert_eq!(view["dest"]["window"]["connection"], "present");
    assert_eq!(view["dest"]["virtualCamera"]["signal"], "present");
    assert_eq!(view["dest"]["tracks"]["mix"]["signal"], "unknown");
    assert_eq!(view["dest"]["publicVia"], "window");
    assert_eq!(view["obsReadiness"], "blocked");
    assert_eq!(view["destReadiness"], "window_public");
}
