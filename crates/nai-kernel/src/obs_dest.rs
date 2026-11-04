//! Optional OBS destinations (v49 2.4.2).
//!
//! Window, virtual camera and independent audio tracks each report
//! connection, source and real signal. Unobserved stays
//! [`super::obs::LayerState::Unknown`]. The public picture stays the window.
//! A virtual camera is video-only. Independent tracks are audio-only.
//! [`audit_dest`] is a pure function of a declared [`DestSurface`]: it opens
//! no process, enumerates no DirectShow device and starts no recording.
use super::obs::{Layer, LayerState};
use crate::golden::Finding;

/// Closed set of destination finding tokens. Tests pin the contents.
pub const DEST_FINDING_TOKENS: &[&str] = &[
    "present_without_evidence",
    "virtual_camera_replaces_window",
    "independent_audio_replaces_window",
    "virtual_camera_claimed_as_audio",
    "independent_audio_claimed_as_picture",
    "virtual_camera_claimed_available",
    "track_claimed_available",
];

/// Independent OBS audio tracks. Mix, TTS and talent stay separate.
pub const AUDIO_TRACKS: [&str; 3] = ["mix", "tts", "talent"];

/// Connection, source and signal for one destination.
pub const DEST_LAYERS: [&str; 3] = ["connection", "source", "signal"];

/// Kind of optional destination. Public picture is always [`DestKind::Window`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestKind {
    Window,
    VirtualCamera,
    IndependentAudio,
}

impl DestKind {
    pub const ALL: [Self; 3] = [Self::Window, Self::VirtualCamera, Self::IndependentAudio];

    pub fn label(self) -> &'static str {
        match self {
            Self::Window => "window",
            Self::VirtualCamera => "virtual_camera",
            Self::IndependentAudio => "independent_audio",
        }
    }
}

/// Connection, source and signal for one destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestReport {
    pub connection: Layer,
    pub source: Layer,
    pub signal: Layer,
}

impl DestReport {
    pub fn unknown() -> Self {
        Self {
            connection: Layer::unknown(),
            source: Layer::unknown(),
            signal: Layer::unknown(),
        }
    }

    /// All three layers Present with evidence. This is a destination
    /// observation, not a playback permit and not a public-path change.
    pub fn observed(&self) -> bool {
        self.layers()
            .iter()
            .all(|(_, state, evidence)| *state == LayerState::Present && !evidence.is_empty())
    }

    fn layers(&self) -> [(&'static str, LayerState, &str); 3] {
        [
            (
                DEST_LAYERS[0],
                self.connection.state,
                self.connection.evidence.as_str(),
            ),
            (
                DEST_LAYERS[1],
                self.source.state,
                self.source.evidence.as_str(),
            ),
            (
                DEST_LAYERS[2],
                self.signal.state,
                self.signal.evidence.as_str(),
            ),
        ]
    }
}

/// One named independent audio track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioTrackReport {
    pub name: &'static str,
    pub report: DestReport,
}

/// Declared destination surface. The audit judges this descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestSurface {
    pub window: DestReport,
    pub virtual_camera: DestReport,
    pub audio_tracks: [AudioTrackReport; 3],
    pub public_via: DestKind,
    pub claims_virtual_camera_available: bool,
    pub claims_virtual_camera_carries_audio: bool,
    pub claims_independent_audio_is_picture: bool,
    pub claims_track_available: Option<&'static str>,
}

impl DestSurface {
    /// Nothing observed. Public picture is still the window.
    pub fn empty() -> Self {
        Self {
            window: DestReport::unknown(),
            virtual_camera: DestReport::unknown(),
            audio_tracks: AUDIO_TRACKS.map(|name| AudioTrackReport {
                name,
                report: DestReport::unknown(),
            }),
            public_via: DestKind::Window,
            claims_virtual_camera_available: false,
            claims_virtual_camera_carries_audio: false,
            claims_independent_audio_is_picture: false,
            claims_track_available: None,
        }
    }

    pub fn readiness(&self) -> DestReadiness {
        if audit_dest(self).is_empty() {
            DestReadiness::WindowPublic
        } else {
            DestReadiness::Defective
        }
    }
}

/// Readiness of the declared destination surface. `WindowPublic` is not a
/// playback permit and is not platform go-live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestReadiness {
    /// A caller replaced the window or claimed an unobserved destination.
    Defective,
    /// Public picture stays the window. Optional dests may still be unknown.
    WindowPublic,
}

/// Audit a declared destination surface. A pure function of the descriptor.
pub fn audit_dest(surface: &DestSurface) -> Vec<Finding> {
    let mut findings = Vec::new();

    push_present_without_evidence(&mut findings, DestKind::Window.label(), &surface.window);
    push_present_without_evidence(
        &mut findings,
        DestKind::VirtualCamera.label(),
        &surface.virtual_camera,
    );
    for track in &surface.audio_tracks {
        push_present_without_evidence(
            &mut findings,
            &format!("track.{}", track.name),
            &track.report,
        );
    }

    match surface.public_via {
        DestKind::Window => {}
        DestKind::VirtualCamera => {
            findings.push(Finding::new("virtual_camera_replaces_window", "public"));
        }
        DestKind::IndependentAudio => {
            findings.push(Finding::new("independent_audio_replaces_window", "public"));
        }
    }

    if surface.claims_virtual_camera_carries_audio {
        findings.push(Finding::new(
            "virtual_camera_claimed_as_audio",
            DestKind::VirtualCamera.label(),
        ));
    }
    if surface.claims_independent_audio_is_picture {
        findings.push(Finding::new(
            "independent_audio_claimed_as_picture",
            DestKind::IndependentAudio.label(),
        ));
    }
    if surface.claims_virtual_camera_available && !surface.virtual_camera.observed() {
        findings.push(Finding::new(
            "virtual_camera_claimed_available",
            DestKind::VirtualCamera.label(),
        ));
    }
    if let Some(name) = surface.claims_track_available {
        let observed = surface
            .audio_tracks
            .iter()
            .find(|track| track.name == name)
            .is_some_and(|track| track.report.observed());
        if !observed {
            findings.push(Finding::new(
                "track_claimed_available",
                format!("track.{name}"),
            ));
        }
    }

    findings
}

fn push_present_without_evidence(findings: &mut Vec<Finding>, prefix: &str, report: &DestReport) {
    for (layer, state, evidence) in report.layers() {
        if state == LayerState::Present && evidence.is_empty() {
            findings.push(Finding::new(
                "present_without_evidence",
                format!("{prefix}.{layer}"),
            ));
        }
    }
}
