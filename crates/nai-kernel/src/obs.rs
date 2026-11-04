//! OBS detection layers (V4.9 / V5.1 一键准备; v49 1.3.31).
//!
//! Detection distinguishes process, scene/source, window capture, audio
//! routing and actual signal. Unobserved is [`LayerState::Unknown`]. A running
//! process or an install path must not be reported as connected. A window
//! capture or a virtual camera does not prove public audio or platform
//! go-live. [`audit_obs`] is a pure function of a declared [`ObsSurface`]: it
//! opens no process, reads no websocket and starts no recording.
use crate::golden::Finding;

/// Public window title the window-capture layer must name when Present.
pub const PUBLIC_WINDOW_TITLE: &str = "nai · 公共输出";

/// Closed set of finding tokens. Tests pin the contents, not only the count.
pub const FINDING_TOKENS: &[&str] = &[
    "present_without_evidence",
    "install_claimed_as_process",
    "process_claimed_as_connected",
    "connected_without_all_layers",
    "obs_claimed_as_platform_live",
    "virtual_camera_claimed_as_audio",
    "window_capture_claimed_as_audio",
    "window_capture_not_public_window",
];

/// One of the five detection layers named by the design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind {
    Process,
    SceneSource,
    WindowCapture,
    AudioRouting,
    Signal,
}

impl LayerKind {
    pub const ALL: [Self; 5] = [
        Self::Process,
        Self::SceneSource,
        Self::WindowCapture,
        Self::AudioRouting,
        Self::Signal,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Process => "process",
            Self::SceneSource => "scene_source",
            Self::WindowCapture => "window_capture",
            Self::AudioRouting => "audio_routing",
            Self::Signal => "signal",
        }
    }
}

/// Observed state of a non-process layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerState {
    Present,
    Absent,
    Unknown,
}

impl LayerState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Absent => "absent",
            Self::Unknown => "unknown",
        }
    }
}

/// A scene/source, window-capture, audio-routing or signal observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    pub state: LayerState,
    pub evidence: String,
}

impl Layer {
    pub fn unknown() -> Self {
        Self {
            state: LayerState::Unknown,
            evidence: String::new(),
        }
    }

    pub fn absent(evidence: impl Into<String>) -> Self {
        Self {
            state: LayerState::Absent,
            evidence: evidence.into(),
        }
    }

    pub fn present(evidence: impl Into<String>) -> Self {
        Self {
            state: LayerState::Present,
            evidence: evidence.into(),
        }
    }
}

/// Process layer: an install path is weaker than a running process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessObservation {
    Unknown,
    Absent,
    /// Binary on disk. Not a running process.
    Binary {
        path: String,
    },
    Running {
        pid: u32,
        image: String,
    },
}

impl ProcessObservation {
    pub fn running(pid: u32, image: impl Into<String>) -> Self {
        Self::Running {
            pid,
            image: image.into(),
        }
    }

    pub fn binary(path: impl Into<String>) -> Self {
        Self::Binary { path: path.into() }
    }

    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running { pid, .. } if *pid != 0)
    }
}

/// Declared OBS observation. The audit judges this descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObsSurface {
    pub process: ProcessObservation,
    pub scene_source: Layer,
    pub window_capture: Layer,
    pub audio_routing: Layer,
    pub signal: Layer,
    /// Caller claims OBS is connected / admitted.
    pub claims_connected: bool,
    /// Caller claims the livestream platform is live.
    pub claims_platform_live: bool,
    /// Caller treats a virtual camera as an audio path.
    pub virtual_camera_as_audio: bool,
    /// Caller treats window capture as public, product or platform audio.
    pub window_capture_as_audio: bool,
}

impl ObsSurface {
    /// Nothing observed. Audits empty and reads [`ObsReadiness::Blocked`].
    pub fn empty() -> Self {
        Self {
            process: ProcessObservation::Unknown,
            scene_source: Layer::unknown(),
            window_capture: Layer::unknown(),
            audio_routing: Layer::unknown(),
            signal: Layer::unknown(),
            claims_connected: false,
            claims_platform_live: false,
            virtual_camera_as_audio: false,
            window_capture_as_audio: false,
        }
    }

    /// This machine's 2026-11-01 observation: obs64 is running, websocket
    /// timed out, no public window capture, no audio routing, no signal.
    pub fn process_only(pid: u32, image: impl Into<String>) -> Self {
        Self {
            process: ProcessObservation::running(pid, image),
            ..Self::empty()
        }
    }

    fn layers(&self) -> [(LayerKind, LayerState, &str); 4] {
        [
            (
                LayerKind::SceneSource,
                self.scene_source.state,
                self.scene_source.evidence.as_str(),
            ),
            (
                LayerKind::WindowCapture,
                self.window_capture.state,
                self.window_capture.evidence.as_str(),
            ),
            (
                LayerKind::AudioRouting,
                self.audio_routing.state,
                self.audio_routing.evidence.as_str(),
            ),
            (
                LayerKind::Signal,
                self.signal.state,
                self.signal.evidence.as_str(),
            ),
        ]
    }

    /// All five layers have positive evidence. This is OBS observed, not
    /// platform go-live and not a playback permit.
    pub fn all_observed(&self) -> bool {
        self.process.is_running()
            && self
                .layers()
                .iter()
                .all(|(_, state, evidence)| *state == LayerState::Present && !evidence.is_empty())
            && window_capture_names_public_window(&self.window_capture)
    }

    pub fn readiness(&self) -> ObsReadiness {
        if !audit_obs(self).is_empty() {
            return ObsReadiness::Defective;
        }
        if self.all_observed() {
            return ObsReadiness::Observed;
        }
        ObsReadiness::Blocked
    }
}

/// Readiness of the declared OBS surface. `Observed` is not a playback
/// permit and is not platform go-live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObsReadiness {
    /// Layers are unknown or incomplete, and no false claim was made.
    Blocked,
    /// A caller claimed connected, platform live, or substituted a capture.
    Defective,
    /// All five layers Present with evidence.
    Observed,
}

/// Audit a declared OBS surface. A pure function of the descriptor.
pub fn audit_obs(surface: &ObsSurface) -> Vec<Finding> {
    let mut findings = Vec::new();

    match &surface.process {
        ProcessObservation::Running { pid, image } if *pid == 0 || image.is_empty() => {
            findings.push(Finding::new(
                "present_without_evidence",
                LayerKind::Process.label(),
            ));
        }
        ProcessObservation::Binary { path } if path.is_empty() => {
            findings.push(Finding::new(
                "present_without_evidence",
                LayerKind::Process.label(),
            ));
        }
        _ => {}
    }

    for (kind, state, evidence) in surface.layers() {
        if state == LayerState::Present && evidence.is_empty() {
            findings.push(Finding::new("present_without_evidence", kind.label()));
        }
    }

    if surface.window_capture.state == LayerState::Present
        && !surface.window_capture.evidence.is_empty()
        && !window_capture_names_public_window(&surface.window_capture)
    {
        findings.push(Finding::new(
            "window_capture_not_public_window",
            LayerKind::WindowCapture.label(),
        ));
    }

    if surface.claims_connected && !surface.all_observed() {
        match &surface.process {
            ProcessObservation::Binary { .. } => {
                findings.push(Finding::new(
                    "install_claimed_as_process",
                    LayerKind::Process.label(),
                ));
            }
            ProcessObservation::Running { .. } => {
                findings.push(Finding::new(
                    "process_claimed_as_connected",
                    LayerKind::Process.label(),
                ));
            }
            ProcessObservation::Unknown | ProcessObservation::Absent => {
                findings.push(Finding::new("connected_without_all_layers", "connected"));
            }
        }
    }

    if surface.claims_platform_live {
        findings.push(Finding::new("obs_claimed_as_platform_live", "platform"));
    }
    if surface.virtual_camera_as_audio {
        findings.push(Finding::new(
            "virtual_camera_claimed_as_audio",
            LayerKind::AudioRouting.label(),
        ));
    }
    if surface.window_capture_as_audio {
        findings.push(Finding::new(
            "window_capture_claimed_as_audio",
            LayerKind::WindowCapture.label(),
        ));
    }

    findings
}

fn window_capture_names_public_window(layer: &Layer) -> bool {
    layer.state == LayerState::Present && layer.evidence.contains(PUBLIC_WINDOW_TITLE)
}
