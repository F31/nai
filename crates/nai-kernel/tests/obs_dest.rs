//! v49 2.4.2 optional OBS destinations.
use nai_kernel::golden::Finding;
use nai_kernel::obs::Layer;
use nai_kernel::obs_dest::{
    audit_dest, DestKind, DestReadiness, DestReport, DestSurface, AUDIO_TRACKS,
    DEST_FINDING_TOKENS, DEST_LAYERS,
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

fn present_report() -> DestReport {
    DestReport {
        connection: Layer::present("plugin:obs-virtualcam"),
        source: Layer::present("source:nai-virtual-camera"),
        signal: Layer::present("video_active=true"),
    }
}

fn present_audio() -> DestReport {
    DestReport {
        connection: Layer::present("wasapi:nai-mix"),
        source: Layer::present("track:mix"),
        signal: Layer::present("audio_peak_q16=800"),
    }
}

#[test]
fn an_empty_surface_audits_empty_and_keeps_window_public() {
    let surface = DestSurface::empty();
    assert!(audit_dest(&surface).is_empty());
    assert_eq!(surface.readiness(), DestReadiness::WindowPublic);
    assert_eq!(surface.public_via, DestKind::Window);
    assert!(!surface.window.observed());
    assert!(!surface.virtual_camera.observed());
    assert!(surface
        .audio_tracks
        .iter()
        .all(|track| !track.report.observed()));
    assert_eq!(
        surface
            .audio_tracks
            .iter()
            .map(|track| track.name)
            .collect::<Vec<_>>(),
        AUDIO_TRACKS
    );
}

#[test]
fn unobserved_layers_stay_unknown() {
    let surface = DestSurface::empty();
    assert_eq!(
        surface.window.connection.state,
        nai_kernel::obs::LayerState::Unknown
    );
    assert_eq!(
        surface.virtual_camera.signal.state,
        nai_kernel::obs::LayerState::Unknown
    );
    assert_eq!(
        surface.audio_tracks[0].report.source.state,
        nai_kernel::obs::LayerState::Unknown
    );
}

#[test]
fn present_without_evidence_is_a_defect() {
    let mut surface = DestSurface::empty();
    surface.window.connection = Layer::present("");
    let findings = audit_dest(&surface);
    assert!(has(
        &findings,
        "present_without_evidence",
        "window.connection"
    ));
    assert_eq!(surface.readiness(), DestReadiness::Defective);
}

#[test]
fn virtual_camera_does_not_replace_the_window() {
    let mut surface = DestSurface::empty();
    surface.virtual_camera = present_report();
    assert!(surface.virtual_camera.observed());
    assert!(audit_dest(&surface).is_empty());
    assert_eq!(surface.public_via, DestKind::Window);
    assert_eq!(surface.readiness(), DestReadiness::WindowPublic);

    surface.public_via = DestKind::VirtualCamera;
    let findings = audit_dest(&surface);
    assert!(has(&findings, "virtual_camera_replaces_window", "public"));
    assert_eq!(surface.readiness(), DestReadiness::Defective);
}

#[test]
fn independent_audio_does_not_replace_the_window() {
    let mut surface = DestSurface::empty();
    surface.audio_tracks[0].report = present_audio();
    surface.public_via = DestKind::IndependentAudio;
    let findings = audit_dest(&surface);
    assert!(has(
        &findings,
        "independent_audio_replaces_window",
        "public"
    ));
    assert_eq!(surface.readiness(), DestReadiness::Defective);
}

#[test]
fn virtual_camera_is_not_audio() {
    let mut surface = DestSurface::empty();
    surface.virtual_camera = present_report();
    surface.claims_virtual_camera_carries_audio = true;
    assert!(has(
        &audit_dest(&surface),
        "virtual_camera_claimed_as_audio",
        DestKind::VirtualCamera.label()
    ));
}

#[test]
fn independent_audio_is_not_picture() {
    let mut surface = DestSurface::empty();
    surface.claims_independent_audio_is_picture = true;
    assert!(has(
        &audit_dest(&surface),
        "independent_audio_claimed_as_picture",
        DestKind::IndependentAudio.label()
    ));
}

#[test]
fn unobserved_virtual_camera_is_not_available() {
    let mut surface = DestSurface::empty();
    surface.claims_virtual_camera_available = true;
    assert!(has(
        &audit_dest(&surface),
        "virtual_camera_claimed_available",
        DestKind::VirtualCamera.label()
    ));

    surface.virtual_camera = present_report();
    assert!(audit_dest(&surface).is_empty());
}

#[test]
fn unobserved_track_is_not_available() {
    let mut surface = DestSurface::empty();
    surface.claims_track_available = Some("mix");
    assert!(has(
        &audit_dest(&surface),
        "track_claimed_available",
        "track.mix"
    ));

    surface.audio_tracks[0].report = present_audio();
    assert!(audit_dest(&surface).is_empty());
}

#[test]
fn finding_tokens_are_exactly_the_closed_set() {
    assert_eq!(DestKind::ALL.len(), 3);
    assert_eq!(DEST_LAYERS.len(), 3);
    assert_eq!(AUDIO_TRACKS, ["mix", "tts", "talent"]);
    let mut constructed = BTreeSet::new();

    let mut present = DestSurface::empty();
    present.window.signal = Layer::present("");
    constructed.extend(tokens(&audit_dest(&present)));

    let mut cam = DestSurface::empty();
    cam.public_via = DestKind::VirtualCamera;
    constructed.extend(tokens(&audit_dest(&cam)));

    let mut audio = DestSurface::empty();
    audio.public_via = DestKind::IndependentAudio;
    constructed.extend(tokens(&audit_dest(&audio)));

    let mut cam_audio = DestSurface::empty();
    cam_audio.claims_virtual_camera_carries_audio = true;
    constructed.extend(tokens(&audit_dest(&cam_audio)));

    let mut picture = DestSurface::empty();
    picture.claims_independent_audio_is_picture = true;
    constructed.extend(tokens(&audit_dest(&picture)));

    let mut available = DestSurface::empty();
    available.claims_virtual_camera_available = true;
    constructed.extend(tokens(&audit_dest(&available)));

    let mut track = DestSurface::empty();
    track.claims_track_available = Some("talent");
    constructed.extend(tokens(&audit_dest(&track)));

    let expected: BTreeSet<&str> = DEST_FINDING_TOKENS.iter().copied().collect();
    assert_eq!(constructed, expected);
}
