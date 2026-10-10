//! Mobile monitoring surface contract (V3.7 §3.1 L5, §10.2, §37.3; registry O03).
//!
//! The L5 experience layer only sends commands and views state; it never holds
//! a media clock or a secret (§3.1). That boundary is expressed here as a
//! **white-list projection** — [`MobileView`] simply has no field for a media
//! clock, a secret, raw media or broadcast control — rather than as a black
//! list that a later refactor can silently miss. [`audit_surface`] is a pure
//! function of a declared [`MobileSurface`], so every rule is directly
//! constructible in a test.
use crate::golden::Finding;

/// Capabilities a mobile monitoring package may offer (§10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MobileCapability {
    /// Status board (状态看板).
    Board,
    /// Approval (审批).
    Approval,
    /// Live takeover (接管).
    Takeover,
    /// Emergency mute (紧急静音).
    EmergencyMute,
    /// Quick phrase insertion (快捷话术插播).
    QuickPhrase,
    /// Alerts (告警).
    Alert,
    /// Knowledge quick patch (知识速补), which enters the review/publish flow.
    KnowledgePatch,
    /// On-demand preview (预览).
    Preview,
    /// Push notifications (推送).
    Push,
}

impl MobileCapability {
    pub const ALL: [MobileCapability; 9] = [
        MobileCapability::Board,
        MobileCapability::Approval,
        MobileCapability::Takeover,
        MobileCapability::EmergencyMute,
        MobileCapability::QuickPhrase,
        MobileCapability::Alert,
        MobileCapability::KnowledgePatch,
        MobileCapability::Preview,
        MobileCapability::Push,
    ];

    /// §10.2 / §663: the minimum form is the board, approval, takeover and
    /// emergency mute. Preview and push may be deferred.
    pub const MINIMUM_FORM: [MobileCapability; 4] = [
        MobileCapability::Board,
        MobileCapability::Approval,
        MobileCapability::Takeover,
        MobileCapability::EmergencyMute,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MobileCapability::Board => "board",
            MobileCapability::Approval => "approval",
            MobileCapability::Takeover => "takeover",
            MobileCapability::EmergencyMute => "emergency_mute",
            MobileCapability::QuickPhrase => "quick_phrase",
            MobileCapability::Alert => "alert",
            MobileCapability::KnowledgePatch => "knowledge_patch",
            MobileCapability::Preview => "preview",
            MobileCapability::Push => "push",
        }
    }
}

/// What may cross the L5 boundary (§3.1: only commands and state).
///
/// The white-listed variants name the ten state fields [`MobileView`] carries.
/// The forbidden variants are categories that must never reach a phone: a
/// media clock, a secret, raw (unencoded) media, and broadcast control
/// (§3.1, §10.2 "它不是手机开播", §742).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Exposure {
    // White-listed state the surface may show.
    Mode,
    CurrentUtterance,
    PendingSeconds,
    EventSource,
    ApiStatus,
    BudgetStatus,
    DeviceStatus,
    PendingApprovals,
    Alerts,
    EnableStatus,
    // Forbidden categories.
    MediaClock,
    Secret,
    RawAudio,
    RawVideo,
    BroadcastControl,
}

impl Exposure {
    pub const ALL: [Exposure; 15] = [
        Exposure::Mode,
        Exposure::CurrentUtterance,
        Exposure::PendingSeconds,
        Exposure::EventSource,
        Exposure::ApiStatus,
        Exposure::BudgetStatus,
        Exposure::DeviceStatus,
        Exposure::PendingApprovals,
        Exposure::Alerts,
        Exposure::EnableStatus,
        Exposure::MediaClock,
        Exposure::Secret,
        Exposure::RawAudio,
        Exposure::RawVideo,
        Exposure::BroadcastControl,
    ];

    pub const ALLOWED: [Exposure; 10] = [
        Exposure::Mode,
        Exposure::CurrentUtterance,
        Exposure::PendingSeconds,
        Exposure::EventSource,
        Exposure::ApiStatus,
        Exposure::BudgetStatus,
        Exposure::DeviceStatus,
        Exposure::PendingApprovals,
        Exposure::Alerts,
        Exposure::EnableStatus,
    ];

    /// §3.1: never hold a media clock or a key. §10.2: not a broadcast channel.
    pub const FORBIDDEN: [Exposure; 5] = [
        Exposure::MediaClock,
        Exposure::Secret,
        Exposure::RawAudio,
        Exposure::RawVideo,
        Exposure::BroadcastControl,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Exposure::Mode => "mode",
            Exposure::CurrentUtterance => "current_utterance",
            Exposure::PendingSeconds => "pending_seconds",
            Exposure::EventSource => "event_source",
            Exposure::ApiStatus => "api_status",
            Exposure::BudgetStatus => "budget_status",
            Exposure::DeviceStatus => "device_status",
            Exposure::PendingApprovals => "pending_approvals",
            Exposure::Alerts => "alerts",
            Exposure::EnableStatus => "enable_status",
            Exposure::MediaClock => "media_clock",
            Exposure::Secret => "secret",
            Exposure::RawAudio => "raw_audio",
            Exposure::RawVideo => "raw_video",
            Exposure::BroadcastControl => "broadcast_control",
        }
    }

    /// Stable finding token for a forbidden category, or `None` for state the
    /// surface is allowed to carry.
    pub fn forbidden_token(self) -> Option<&'static str> {
        Some(match self {
            Exposure::MediaClock => "exposes_media_clock",
            Exposure::Secret => "exposes_secret",
            Exposure::RawAudio | Exposure::RawVideo => "exposes_raw_media",
            Exposure::BroadcastControl => "exposes_broadcast_control",
            _ => return None,
        })
    }

    pub fn is_forbidden(self) -> bool {
        self.forbidden_token().is_some()
    }
}

/// §3.1: the media clock never crosses to the L5 surface.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MediaClock {
    pub epoch: u64,
    pub tick_ms: u64,
}

/// §10.2: authority to start broadcasting. It never crosses to a phone
/// ("它不是手机开播"; §742 mobile is monitor-only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BroadcastHandle {
    pub room: String,
}

/// The full live state. Only a [`MobileView`] projection crosses the L5
/// boundary; the sensitive fields below have no place on that view.
#[derive(Debug, Clone, Default)]
pub struct LiveState {
    // White-listed state.
    pub mode: String,
    pub current_utterance: String,
    pub pending_seconds: u32,
    pub event_source: String,
    pub api_status: String,
    pub budget_status: String,
    pub device_status: String,
    pub pending_approvals: Vec<String>,
    pub alerts: Vec<String>,
    pub enable_status: String,
    // Forbidden to cross the L5 boundary.
    pub media_clock: MediaClock,
    pub secret: String,
    pub raw_audio: Vec<u8>,
    pub raw_video: Vec<u8>,
    pub broadcast_control: Option<BroadcastHandle>,
}

impl LiveState {
    /// The forbidden field names, aligned with [`Exposure::FORBIDDEN`] labels.
    /// The consistency test asserts none of these is a [`MobileView`] field.
    pub fn sensitive_field_names() -> [&'static str; 5] {
        [
            "media_clock",
            "secret",
            "raw_audio",
            "raw_video",
            "broadcast_control",
        ]
    }
}

/// What a phone actually sees (§3.1 L5). It carries only white-listed state;
/// there is no field for a media clock, a secret, raw media or broadcast
/// control, so a projection cannot leak them even by accident.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MobileView {
    pub mode: String,
    pub current_utterance: String,
    pub pending_seconds: u32,
    pub event_source: String,
    pub api_status: String,
    pub budget_status: String,
    pub device_status: String,
    pub pending_approvals: Vec<String>,
    pub alerts: Vec<String>,
    pub enable_status: String,
}

impl MobileView {
    /// The view's own field names. The consistency test asserts this set equals
    /// [`Exposure::ALLOWED`]: adding a field without registering it fails.
    pub fn field_names() -> [&'static str; 10] {
        [
            "mode",
            "current_utterance",
            "pending_seconds",
            "event_source",
            "api_status",
            "budget_status",
            "device_status",
            "pending_approvals",
            "alerts",
            "enable_status",
        ]
    }

    /// The exposures this view can carry — exactly the white list.
    pub fn exposed() -> [Exposure; 10] {
        Exposure::ALLOWED
    }

    pub fn project(state: &LiveState) -> Self {
        Self {
            mode: state.mode.clone(),
            current_utterance: state.current_utterance.clone(),
            pending_seconds: state.pending_seconds,
            event_source: state.event_source.clone(),
            api_status: state.api_status.clone(),
            budget_status: state.budget_status.clone(),
            device_status: state.device_status.clone(),
            pending_approvals: state.pending_approvals.clone(),
            alerts: state.alerts.clone(),
            enable_status: state.enable_status.clone(),
        }
    }
}

/// §10.2: the connection route is decided after the M0b PoC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionRoute {
    /// Not yet decided. The honest, expected state — not a defect.
    Undecided,
    /// Route A: relay + PWA, which requires a valid HTTPS domain and
    /// certificate (a Service Worker needs a secure context).
    RelayPwa {
        https_domain: bool,
        certificate: bool,
    },
    /// Route B: native shell + LAN direct with certificate pinning, which
    /// requires app distribution and verified Tauri 2 maturity.
    NativeShellLan {
        app_distribution: bool,
        tauri_maturity_verified: bool,
    },
}

/// §10.2: a relay that terminates TLS can read business content, so it must
/// not be advertised as end-to-end encrypted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptionClaim {
    /// The relay terminates TLS and can therefore read business content.
    pub relay_terminates_tls: bool,
    /// The surface advertises end-to-end encryption.
    pub claims_end_to_end: bool,
    /// The end-to-end protocol in use has been reviewed.
    pub protocol_reviewed: bool,
}

/// §10.2: preview is turned on on demand, uses a truly encoded low-bitrate
/// stream, releases the encoder and bandwidth immediately on close, and must
/// not squeeze the official output. These fields are only judged while
/// `enabled`; a deferred preview is not a defect. The derived default is that
/// deferred state (all flags false).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PreviewPolicy {
    pub enabled: bool,
    pub on_demand: bool,
    pub truly_encoded_low_bitrate: bool,
    pub releases_encoder_and_bandwidth_on_close: bool,
    pub may_preempt_official_output: bool,
}

/// §636 S09: a remote command's risk grade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CommandRisk {
    Low,
    High,
}

/// A role on the monitoring surface. `Operator` outranks `Observer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Observer,
    Operator,
}

impl Role {
    /// `Operator` outranks `Observer`.
    pub fn at_least(self, other: Role) -> bool {
        self >= other
    }
}

/// §86 INV-14 / §636 S09: a remote control command carries identity, session
/// and epoch, a sequence number and a TTL, and returns an actual execution
/// ACK. High-risk commands must not be executable by an observer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteCommand {
    pub name: String,
    pub risk: CommandRisk,
    pub min_role: Role,
    pub carries_identity: bool,
    pub carries_session_epoch: bool,
    pub carries_sequence: bool,
    pub carries_ttl: bool,
    pub returns_actual_ack: bool,
}

impl RemoteCommand {
    /// §86 INV-14: identity, session/epoch, sequence, TTL, actual ACK.
    pub fn contract_complete(&self) -> bool {
        self.carries_identity
            && self.carries_session_epoch
            && self.carries_sequence
            && self.carries_ttl
            && self.returns_actual_ack
    }
}

/// A declared mobile monitoring surface. The audit judges this declaration;
/// it does not run a phone, a relay or a certificate authority.
#[derive(Debug, Clone)]
pub struct MobileSurface {
    pub capabilities: Vec<MobileCapability>,
    /// Categories of state the surface declares it sends to the phone.
    pub exposures: Vec<Exposure>,
    pub route: ConnectionRoute,
    pub encryption: EncryptionClaim,
    pub preview: PreviewPolicy,
    pub commands: Vec<RemoteCommand>,
    /// §10.2 minimum form / §37.3: emergency mute is reachable.
    pub emergency_mute_reachable: bool,
    /// §37.3: the mobile surface provides the enable status.
    pub enable_status_provided: bool,
    /// §10.2: knowledge quick patches enter the review/publish flow.
    pub knowledge_patch_routes_to_review: bool,
}

impl MobileSurface {
    pub fn minimum_form_complete(&self) -> bool {
        self.missing_minimum().is_empty()
    }

    /// The minimum-form capabilities that are absent (§10.2 / §663).
    pub fn missing_minimum(&self) -> Vec<MobileCapability> {
        MobileCapability::MINIMUM_FORM
            .into_iter()
            .filter(|capability| !self.capabilities.contains(capability))
            .collect()
    }

    /// The declared exposures that hit a forbidden category, de-duplicated.
    pub fn forbidden_exposures(&self) -> Vec<Exposure> {
        let mut seen: Vec<Exposure> = Vec::new();
        for exposure in &self.exposures {
            if exposure.is_forbidden() && !seen.contains(exposure) {
                seen.push(*exposure);
            }
        }
        seen
    }

    /// `Defective` takes precedence: an undecided route must not hide a
    /// boundary violation.
    pub fn readiness(&self) -> SurfaceReadiness {
        if !audit_surface(self).is_empty() {
            return SurfaceReadiness::Defective;
        }
        match self.route {
            ConnectionRoute::Undecided => SurfaceReadiness::AwaitingRouteDecision,
            _ => SurfaceReadiness::Ready,
        }
    }
}

/// Readiness of the declared surface. `AwaitingRouteDecision` is the honest
/// blocked state for O03, not a defect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceReadiness {
    /// Route A/B not yet decided (M0b PoC pending).
    AwaitingRouteDecision,
    /// A route is declared or not, but the declaration violates a boundary.
    Defective,
    /// A route is declared, its prerequisites hold, and nothing is violated.
    Ready,
}

/// Audit a declared surface against the L5 and §10.2 boundaries. A pure
/// function of the descriptor: every finding is directly constructible.
pub fn audit_surface(surface: &MobileSurface) -> Vec<Finding> {
    let mut findings = Vec::new();

    // §3.1 L5 (no media clock or key) and §10.2 (not a broadcast channel).
    for exposure in surface.forbidden_exposures() {
        if let Some(token) = exposure.forbidden_token() {
            findings.push(Finding::new(token, exposure.label()));
        }
    }

    // §10.2 / §663: the minimum form is four named capabilities.
    for capability in surface.missing_minimum() {
        findings.push(Finding::new("minimum_form_incomplete", capability.label()));
    }

    // §10.2 minimum form / §37.3: emergency mute must be reachable.
    if surface
        .capabilities
        .contains(&MobileCapability::EmergencyMute)
        && !surface.emergency_mute_reachable
    {
        findings.push(Finding::new("emergency_mute_unreachable", "emergency_mute"));
    }

    // §37.3: the mobile surface provides the enable status.
    if !surface.enable_status_provided {
        findings.push(Finding::new("enable_status_missing", "enable_status"));
    }

    // §10.2: knowledge quick patches enter the review/publish flow.
    if surface
        .capabilities
        .contains(&MobileCapability::KnowledgePatch)
        && !surface.knowledge_patch_routes_to_review
    {
        findings.push(Finding::new(
            "knowledge_patch_without_review_flow",
            "knowledge_patch",
        ));
    }

    // §10.2: preview conditions are judged only while preview is enabled.
    if surface.preview.enabled {
        if !surface.preview.on_demand {
            findings.push(Finding::new("preview_not_on_demand", "preview"));
        }
        if !surface.preview.truly_encoded_low_bitrate {
            findings.push(Finding::new("preview_not_encoded", "preview"));
        }
        if !surface.preview.releases_encoder_and_bandwidth_on_close {
            findings.push(Finding::new("preview_does_not_release_on_close", "preview"));
        }
        if surface.preview.may_preempt_official_output {
            findings.push(Finding::new("preview_may_preempt_output", "preview"));
        }
    }

    // §10.2: a declared route must carry its prerequisites.
    match &surface.route {
        ConnectionRoute::Undecided => {}
        ConnectionRoute::RelayPwa {
            https_domain,
            certificate,
        } => {
            if !(*https_domain && *certificate) {
                findings.push(Finding::new("relay_pwa_without_secure_context", "route_a"));
            }
        }
        ConnectionRoute::NativeShellLan {
            app_distribution,
            tauri_maturity_verified,
        } => {
            if !*app_distribution {
                findings.push(Finding::new("native_shell_without_distribution", "route_b"));
            }
            if !*tauri_maturity_verified {
                findings.push(Finding::new(
                    "native_shell_tauri_maturity_unverified",
                    "route_b",
                ));
            }
        }
    }

    // §10.2: relay-terminated TLS must not be advertised as end-to-end.
    if surface.encryption.relay_terminates_tls && surface.encryption.claims_end_to_end {
        findings.push(Finding::new(
            "e2e_encryption_claimed_with_relay_terminated_tls",
            "encryption",
        ));
    }
    if surface.encryption.claims_end_to_end && !surface.encryption.protocol_reviewed {
        findings.push(Finding::new(
            "e2e_encryption_claimed_without_reviewed_protocol",
            "encryption",
        ));
    }

    // §86 INV-14 / §636 S09: the remote command contract.
    for command in &surface.commands {
        if !command.contract_complete() {
            findings.push(Finding::new(
                "command_contract_incomplete",
                command.name.clone(),
            ));
        }
        if command.risk == CommandRisk::High && !command.min_role.at_least(Role::Operator) {
            findings.push(Finding::new(
                "high_risk_command_open_to_observer",
                command.name.clone(),
            ));
        }
    }

    findings
}
