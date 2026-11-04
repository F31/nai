//! v49 1.3.31 OBS five-layer detection.
use nai_kernel::golden::Finding;
use nai_kernel::obs::{
    FINDING_TOKENS, Layer, LayerKind, ObsReadiness, ObsSurface, PUBLIC_WINDOW_TITLE,
    ProcessObservation, audit_obs,
};
use std::collections::BTreeSet;

fn tokens(findings: &[Finding]) -> Vec<&'static str> {
    findings.iter().map(|finding| finding.token).collect()
}

fn has(findings: &[Finding], token: &str, subject: &str) -> bool {
    findings
        .iter()
        .any(|finding| finding.token == token && finding.subject == subject)
}

fn observed() -> ObsSurface {
    ObsSurface {
        process: ProcessObservation::running(44740, "obs64.exe"),
        scene_source: Layer::present("scene:nai-public"),
        window_capture: Layer::present(format!("window_capture {PUBLIC_WINDOW_TITLE}")),
        audio_routing: Layer::present("wasapi_process:nai-public-output-keeper"),
        signal: Layer::present("video_active=true audio_peak_q16=1200"),
        claims_connected: true,
        claims_platform_live: false,
        virtual_camera_as_audio: false,
        window_capture_as_audio: false,
    }
}

#[test]
fn an_empty_surface_audits_empty_and_is_blocked() {
    let surface = ObsSurface::empty();
    assert!(audit_obs(&surface).is_empty());
    assert_eq!(surface.readiness(), ObsReadiness::Blocked);
    assert!(!surface.all_observed());
}

#[test]
fn a_running_process_alone_is_not_connected() {
    let mut surface = ObsSurface::process_only(44740, "obs64.exe");
    assert!(audit_obs(&surface).is_empty());
    assert_eq!(surface.readiness(), ObsReadiness::Blocked);
    assert!(!surface.all_observed());

    surface.claims_connected = true;
    let findings = audit_obs(&surface);
    assert!(has(
        &findings,
        "process_claimed_as_connected",
        LayerKind::Process.label()
    ));
    assert_eq!(surface.readiness(), ObsReadiness::Defective);
}

#[test]
fn an_install_path_is_not_a_running_process() {
    let surface = ObsSurface {
        process: ProcessObservation::binary(r"D:\obs-studio\bin\64bit\obs64.exe"),
        claims_connected: true,
        ..ObsSurface::empty()
    };
    let findings = audit_obs(&surface);
    assert!(has(
        &findings,
        "install_claimed_as_process",
        LayerKind::Process.label()
    ));
    assert!(!tokens(&findings).contains(&"process_claimed_as_connected"));
    assert_eq!(surface.readiness(), ObsReadiness::Defective);
}

#[test]
fn unknown_layers_stay_unknown_without_a_claim() {
    let surface = ObsSurface {
        process: ProcessObservation::running(44740, "obs64.exe"),
        scene_source: Layer::unknown(),
        window_capture: Layer::unknown(),
        audio_routing: Layer::unknown(),
        signal: Layer::unknown(),
        ..ObsSurface::empty()
    };
    assert!(audit_obs(&surface).is_empty());
    assert_eq!(
        surface.scene_source.state,
        nai_kernel::obs::LayerState::Unknown
    );
    assert_eq!(surface.readiness(), ObsReadiness::Blocked);
}

#[test]
fn present_without_evidence_is_a_defect() {
    let surface = ObsSurface {
        process: ProcessObservation::running(44740, "obs64.exe"),
        scene_source: Layer::present(""),
        ..ObsSurface::empty()
    };
    let findings = audit_obs(&surface);
    assert!(has(
        &findings,
        "present_without_evidence",
        LayerKind::SceneSource.label()
    ));
}

#[test]
fn a_zero_pid_is_present_without_evidence() {
    let findings = audit_obs(&ObsSurface {
        process: ProcessObservation::running(0, "obs64.exe"),
        ..ObsSurface::empty()
    });
    assert!(has(
        &findings,
        "present_without_evidence",
        LayerKind::Process.label()
    ));
}

#[test]
fn window_capture_must_name_the_public_window() {
    let mut surface = observed();
    surface.window_capture = Layer::present("window_capture Untitled");
    let findings = audit_obs(&surface);
    assert!(has(
        &findings,
        "window_capture_not_public_window",
        LayerKind::WindowCapture.label()
    ));
    assert!(!surface.all_observed());
}

#[test]
fn five_layers_with_evidence_are_observed_and_still_not_platform_live() {
    let surface = observed();
    assert!(audit_obs(&surface).is_empty());
    assert_eq!(surface.readiness(), ObsReadiness::Observed);
    assert!(surface.all_observed());

    let mut live = surface.clone();
    live.claims_platform_live = true;
    let findings = audit_obs(&live);
    assert!(has(&findings, "obs_claimed_as_platform_live", "platform"));
    assert_eq!(live.readiness(), ObsReadiness::Defective);
}

#[test]
fn virtual_camera_is_not_audio() {
    let mut surface = observed();
    surface.virtual_camera_as_audio = true;
    assert!(has(
        &audit_obs(&surface),
        "virtual_camera_claimed_as_audio",
        LayerKind::AudioRouting.label()
    ));
}

#[test]
fn window_capture_is_not_audio() {
    let mut surface = observed();
    surface.window_capture_as_audio = true;
    assert!(has(
        &audit_obs(&surface),
        "window_capture_claimed_as_audio",
        LayerKind::WindowCapture.label()
    ));
}

#[test]
fn connected_without_process_uses_the_generic_token() {
    let mut surface = observed();
    surface.process = ProcessObservation::Unknown;
    surface.claims_connected = true;
    let findings = audit_obs(&surface);
    assert!(has(&findings, "connected_without_all_layers", "connected"));
    assert!(!tokens(&findings).contains(&"process_claimed_as_connected"));
}

#[test]
fn finding_tokens_are_exactly_the_closed_set() {
    assert_eq!(LayerKind::ALL.len(), 5);
    let mut constructed = BTreeSet::new();

    let mut present = ObsSurface::empty();
    present.scene_source = Layer::present("");
    constructed.extend(tokens(&audit_obs(&present)));

    let mut install = ObsSurface::empty();
    install.process = ProcessObservation::binary(r"D:\obs-studio\bin\64bit\obs64.exe");
    install.claims_connected = true;
    constructed.extend(tokens(&audit_obs(&install)));

    let mut process = ObsSurface::process_only(44740, "obs64.exe");
    process.claims_connected = true;
    constructed.extend(tokens(&audit_obs(&process)));

    let mut missing = observed();
    missing.process = ProcessObservation::Unknown;
    missing.claims_connected = true;
    constructed.extend(tokens(&audit_obs(&missing)));

    let mut platform = observed();
    platform.claims_platform_live = true;
    constructed.extend(tokens(&audit_obs(&platform)));

    let mut cam = observed();
    cam.virtual_camera_as_audio = true;
    constructed.extend(tokens(&audit_obs(&cam)));

    let mut capture_audio = observed();
    capture_audio.window_capture_as_audio = true;
    constructed.extend(tokens(&audit_obs(&capture_audio)));

    let mut wrong_window = observed();
    wrong_window.window_capture = Layer::present("other");
    constructed.extend(tokens(&audit_obs(&wrong_window)));

    let expected: BTreeSet<&str> = FINDING_TOKENS.iter().copied().collect();
    assert_eq!(constructed, expected);
}
