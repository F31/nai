//! Platform-bridge lifecycle and stale-room broadcast suppression
//! (V3.7 §27.1, §347, §349, §119, §1328; registry P01).
//!
//! [`crate::platform`] judges whether one platform *event* may enter. This
//! module judges the *process lifecycle* around it, which the specification
//! states with a dense set of negative clauses:
//!
//! - The bridge "only does" four things — SDK init, official event and
//!   permission translation, the authenticated IPC to the engine, and exit
//!   (§27.1).
//! - Partner start, exit, reconnect and room switch each need an explicit
//!   disposition, every transition clears the broadcast authority, and a room
//!   that is no longer current never broadcasts (§1328).
//! - Partner exit leaves the engine in standby and a reconnect never restores
//!   the previous automatic authority (§347, §119).
//! - A capability snapshot takes the *strictest* intersection — `unknown` is
//!   read as not granted — and a missing capability closes only its own
//!   feature, never synthesising events (§349).
//! - A platform-event interruption revokes automatic replies while manual and
//!   replay stay available (§119).
//! - Seven condition classes each need a disposition (§347).
//! - The official SDK is loaded only through a pinned, trusted path (§27.1,
//!   §1075); with no official SDK the honest answer is
//!   `UNVERIFIED_EXTERNAL_DEPENDENCY`, never a fake connector passed off as a
//!   real capability; and an SDK callback never blocks on the LLM or disk.
//!
//! Nothing here starts a partner, loads an SDK, opens a socket or pushes a
//! stream. [`audit_bridge`] is a pure function of a declared
//! [`BridgeDescriptor`], so every rule is directly constructible in a test.
use crate::golden::Finding;

/// §27.1: the bridge "only does" these four things.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BridgeDuty {
    SdkInit,
    EventPermissionTranslation,
    AuthenticatedIpc,
    Exit,
}

impl BridgeDuty {
    pub const ALL: [BridgeDuty; 4] = [
        BridgeDuty::SdkInit,
        BridgeDuty::EventPermissionTranslation,
        BridgeDuty::AuthenticatedIpc,
        BridgeDuty::Exit,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BridgeDuty::SdkInit => "sdk_init",
            BridgeDuty::EventPermissionTranslation => "event_permission_translation",
            BridgeDuty::AuthenticatedIpc => "authenticated_ipc",
            BridgeDuty::Exit => "exit",
        }
    }
}

/// §27.1: what the bridge declares it does. `extra_duties` is anything beyond
/// the closed set of four, reported one by one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeScope {
    pub duties: Vec<BridgeDuty>,
    pub extra_duties: Vec<String>,
}

impl BridgeScope {
    /// The four declared duties and nothing else.
    pub fn complete() -> Self {
        Self {
            duties: BridgeDuty::ALL.to_vec(),
            extra_duties: Vec::new(),
        }
    }

    /// A declared bridge exists. Until one is declared there are no lifecycle
    /// transitions and no condition classes to dispose of.
    pub fn is_declared(&self) -> bool {
        !self.duties.is_empty()
    }
}

/// §1328: the four partner lifecycle transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BridgeTransition {
    /// Partner started and bridged to the engine.
    Started,
    /// Partner exited; the engine goes to standby.
    Exited,
    /// Partner reconnected; it never restores the previous automatic authority.
    Reconnected,
    /// The room/occasion changed.
    RoomSwitched,
}

impl BridgeTransition {
    pub const ALL: [BridgeTransition; 4] = [
        BridgeTransition::Started,
        BridgeTransition::Exited,
        BridgeTransition::Reconnected,
        BridgeTransition::RoomSwitched,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BridgeTransition::Started => "started",
            BridgeTransition::Exited => "exited",
            BridgeTransition::Reconnected => "reconnected",
            BridgeTransition::RoomSwitched => "room_switched",
        }
    }
}

/// §1328: how the bridge disposes of one lifecycle transition. Every
/// transition clears the broadcast authority — that is the mechanism behind
/// "no stale-room broadcast". A partner exit leaves the engine in standby
/// (§347); a reconnect additionally never restores the previous automatic
/// authority (§347, §119).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionDisposition {
    pub transition: BridgeTransition,
    /// The transition clears the broadcast authority bound to the old room.
    pub clears_broadcast_authority: bool,
    /// The transition is written to the decision log.
    pub logged: bool,
    /// Only meaningful for [`BridgeTransition::Exited`]: the partner exit
    /// leaves the engine in standby.
    pub leaves_engine_in_standby: bool,
    /// Only meaningful for [`BridgeTransition::Reconnected`]: the disposition
    /// claims to restore the previous automatic authority, which is forbidden.
    pub restores_previous_auto_authority: bool,
}

impl TransitionDisposition {
    /// A sound disposition: clears the authority, logs, leaves the engine in
    /// standby on exit, and restores nothing.
    pub fn resolved(transition: BridgeTransition) -> Self {
        Self {
            transition,
            clears_broadcast_authority: true,
            logged: true,
            leaves_engine_in_standby: transition == BridgeTransition::Exited,
            restores_previous_auto_authority: false,
        }
    }
}

/// The room and connection generation the bridge is currently attached to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentConnection {
    pub room: String,
    pub connection_generation: u64,
}

impl CurrentConnection {
    pub fn new(room: impl Into<String>, connection_generation: u64) -> Self {
        Self {
            room: room.into(),
            connection_generation,
        }
    }
}

/// §1328: the authority to broadcast, bound to the room and connection
/// generation it was granted for. A grant that no longer covers the current
/// connection must never be in force — that is the stale-room suppression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BroadcastAuthority {
    pub room: String,
    pub connection_generation: u64,
}

impl BroadcastAuthority {
    pub fn new(room: impl Into<String>, connection_generation: u64) -> Self {
        Self {
            room: room.into(),
            connection_generation,
        }
    }

    /// The authority still applies to `connection`.
    pub fn covers(&self, connection: &CurrentConnection) -> bool {
        self.room == connection.room
            && self.connection_generation == connection.connection_generation
    }
}

/// §349: a capability snapshot takes granted / denied / unknown and uses the
/// strictest intersection. `unknown` is read as not granted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityState {
    Granted,
    Denied,
    Unknown,
}

impl CapabilityState {
    pub fn label(self) -> &'static str {
        match self {
            CapabilityState::Granted => "granted",
            CapabilityState::Denied => "denied",
            CapabilityState::Unknown => "unknown",
        }
    }
}

/// §349: one platform capability of the snapshot. `read_as_granted` records
/// how the snapshot was read: recording `unknown` and reading it as granted is
/// the defect the strictest-intersection rule exists to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityGrant {
    pub capability: String,
    pub state: CapabilityState,
    pub read_as_granted: bool,
}

impl CapabilityGrant {
    /// A snapshot read honestly: `granted` is read as granted, anything else is
    /// not read as granted.
    pub fn new(capability: impl Into<String>, state: CapabilityState) -> Self {
        Self {
            capability: capability.into(),
            state,
            read_as_granted: matches!(state, CapabilityState::Granted),
        }
    }
}

/// §349: one feature and the platform capability it depends on. A missing
/// capability must close only this feature; a capability that is still granted
/// must not have closed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feature {
    pub name: String,
    /// The capability this feature depends on.
    pub requires: String,
    /// The feature is still active.
    pub active: bool,
    /// The feature was stopped *because* a platform capability changed. Only
    /// such a stop can be "beyond the capability".
    pub stopped_by_capability_change: bool,
}

impl Feature {
    /// A feature that is still running on `requires`.
    pub fn active(name: impl Into<String>, requires: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            requires: requires.into(),
            active: true,
            stopped_by_capability_change: false,
        }
    }

    /// A feature that was closed because a platform capability changed.
    pub fn stopped_by_capability_change(
        name: impl Into<String>,
        requires: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            requires: requires.into(),
            active: false,
            stopped_by_capability_change: true,
        }
    }
}

/// §347: the seven condition classes the bridge must handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BridgeCondition {
    MultipleStarts,
    PortConflict,
    SdkLost,
    DuplicateMessages,
    StaleRoomEvent,
    TimeDrift,
    PermissionRevoked,
}

impl BridgeCondition {
    pub const ALL: [BridgeCondition; 7] = [
        BridgeCondition::MultipleStarts,
        BridgeCondition::PortConflict,
        BridgeCondition::SdkLost,
        BridgeCondition::DuplicateMessages,
        BridgeCondition::StaleRoomEvent,
        BridgeCondition::TimeDrift,
        BridgeCondition::PermissionRevoked,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BridgeCondition::MultipleStarts => "multiple_starts",
            BridgeCondition::PortConflict => "port_conflict",
            BridgeCondition::SdkLost => "sdk_lost",
            BridgeCondition::DuplicateMessages => "duplicate_messages",
            BridgeCondition::StaleRoomEvent => "stale_room_event",
            BridgeCondition::TimeDrift => "time_drift",
            BridgeCondition::PermissionRevoked => "permission_revoked",
        }
    }
}

/// §347: how the bridge disposes of one condition class, and whether that
/// disposition is written to the decision log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionDisposition {
    pub condition: BridgeCondition,
    pub handled: bool,
    pub logged: bool,
}

impl ConditionDisposition {
    /// A sound disposition: handled and logged.
    pub fn resolved(condition: BridgeCondition) -> Self {
        Self {
            condition,
            handled: true,
            logged: true,
        }
    }
}

/// §119: how the bridge degrades when the platform link is interrupted. A
/// platform-event interruption revokes automatic replies while manual and
/// replay stay available. A degradation that is *not* a platform interruption
/// owes none of this, so `bridge_lost` gates the whole rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Degradation {
    /// What degraded, as a decision-log subject (e.g. a condition label).
    pub cause: String,
    /// This is a platform-event interruption.
    pub bridge_lost: bool,
    /// Automatic replies were revoked.
    pub auto_reply_revoked: bool,
    /// Manual operation is still available.
    pub manual_available: bool,
    /// Replay is still available.
    pub replay_available: bool,
    /// The degradation is written to the decision log.
    pub logged: bool,
}

impl Degradation {
    /// An interruption that behaves as §119 requires.
    pub fn interruption(cause: impl Into<String>) -> Self {
        Self {
            cause: cause.into(),
            bridge_lost: true,
            auto_reply_revoked: true,
            manual_available: true,
            replay_available: true,
            logged: true,
        }
    }
}

/// §27.1 / §1075: the handshake an officially-verified SDK must pass. The SDK
/// library is never loaded from an arbitrary path.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SdkTrust {
    pub path_pinned: bool,
    pub signature_checked: bool,
    pub nonce_checked: bool,
    pub token_valid: bool,
}

impl SdkTrust {
    /// Every handshake check passed.
    pub fn complete() -> Self {
        Self {
            path_pinned: true,
            signature_checked: true,
            nonce_checked: true,
            token_valid: true,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.missing().is_empty()
    }

    /// The handshake checks that are missing, as stable subjects.
    pub fn missing(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if !self.path_pinned {
            missing.push("path_pinned");
        }
        if !self.signature_checked {
            missing.push("signature");
        }
        if !self.nonce_checked {
            missing.push("nonce");
        }
        if !self.token_valid {
            missing.push("token");
        }
        missing
    }
}

/// §27.1: with no official interface documentation or SDK, the honest answer is
/// `UNVERIFIED_EXTERNAL_DEPENDENCY`; drills run manual and replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdkAvailability {
    Verified,
    UnverifiedExternalDependency,
}

/// §27.1: an SDK callback enqueues quickly and never waits on the LLM or a
/// large disk transaction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CallbackPolicy {
    pub enqueues_without_blocking: bool,
    pub awaits_llm_or_disk: bool,
}

impl CallbackPolicy {
    /// A callback that only enqueues.
    pub fn decoupled() -> Self {
        Self {
            enqueues_without_blocking: true,
            awaits_llm_or_disk: false,
        }
    }
}

/// A declared bridge: its scope, its lifecycle dispositions, the current
/// connection and broadcast authority, the capability snapshot, the feature
/// switches, the degradations, the condition dispositions, the SDK handshake
/// and the callback policy. The audit judges this descriptor; it starts no
/// partner and loads no SDK.
#[derive(Debug, Clone)]
pub struct BridgeDescriptor {
    pub scope: BridgeScope,
    pub transitions: Vec<TransitionDisposition>,
    pub connection: CurrentConnection,
    pub authority: Option<BroadcastAuthority>,
    pub capabilities: Vec<CapabilityGrant>,
    pub features: Vec<Feature>,
    pub degradations: Vec<Degradation>,
    pub conditions: Vec<ConditionDisposition>,
    pub sdk_trust: SdkTrust,
    pub sdk_availability: SdkAvailability,
    pub uses_fake_connector_claimed_real: bool,
    pub callback: CallbackPolicy,
}

impl BridgeDescriptor {
    /// A bridge with nothing declared at all. It audits empty and reads as
    /// [`BridgeReadiness::Blocked`], the honest P01 state.
    pub fn empty() -> Self {
        Self {
            scope: BridgeScope {
                duties: Vec::new(),
                extra_duties: Vec::new(),
            },
            transitions: Vec::new(),
            connection: CurrentConnection::new(String::new(), 0),
            authority: None,
            capabilities: Vec::new(),
            features: Vec::new(),
            degradations: Vec::new(),
            conditions: Vec::new(),
            sdk_trust: SdkTrust::default(),
            sdk_availability: SdkAvailability::UnverifiedExternalDependency,
            uses_fake_connector_claimed_real: false,
            callback: CallbackPolicy::default(),
        }
    }

    /// The recorded state of one platform capability, if the snapshot has it.
    pub fn capability_state(&self, capability: &str) -> Option<CapabilityState> {
        self.capabilities
            .iter()
            .find(|grant| grant.capability == capability)
            .map(|grant| grant.state)
    }

    /// §1328: the bridge may broadcast only while its authority still covers
    /// the current connection. A stale-room grant never broadcasts.
    pub fn may_broadcast(&self) -> bool {
        self.authority
            .as_ref()
            .is_some_and(|authority| authority.covers(&self.connection))
    }

    /// `Defective` takes precedence: a not-yet-implemented bridge must not hide
    /// a stale-room grant, a forged substitute or an over-reaching switch.
    pub fn readiness(&self) -> BridgeReadiness {
        if !audit_bridge(self).is_empty() {
            return BridgeReadiness::Defective;
        }
        if !self.scope.is_declared() {
            return BridgeReadiness::Blocked;
        }
        BridgeReadiness::Ready
    }
}

/// Readiness of the declared bridge. `Blocked` is the honest state for P01
/// while no official SDK or production host exists (§1261: an unrun acceptance
/// is marked Blocked, never rendered as a pass).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeReadiness {
    /// No bridge declared yet.
    Blocked,
    /// A bridge is declared but something violates a boundary.
    Defective,
    /// Declared, and nothing is violated.
    Ready,
}

/// Audit a declared bridge. A pure function of the descriptor: every finding is
/// directly constructible, and no partner or SDK is touched.
pub fn audit_bridge(bridge: &BridgeDescriptor) -> Vec<Finding> {
    let mut findings = Vec::new();

    // §27.1: the bridge does only four things; anything else is reported.
    for extra in &bridge.scope.extra_duties {
        findings.push(Finding::new("duty_outside_bridge_scope", extra.clone()));
    }

    // §27.1/§1075: an SDK the bridge declares as officially verified must pass
    // every handshake check. An honestly-unverified dependency is not judged
    // here — requiring it to check signatures it does not have would punish
    // the honest answer.
    if bridge.sdk_availability == SdkAvailability::Verified {
        for check in bridge.sdk_trust.missing() {
            findings.push(Finding::new("sdk_loaded_without_trust_check", check));
        }
    }

    // §27.1: no official SDK means UNVERIFIED_EXTERNAL_DEPENDENCY; a fake
    // connector is never passed off as a real capability.
    if bridge.sdk_availability == SdkAvailability::UnverifiedExternalDependency
        && bridge.uses_fake_connector_claimed_real
    {
        findings.push(Finding::new(
            "fake_connector_claimed_as_real_capability",
            "connector",
        ));
    }

    // Lifecycle is judged once a bridge is declared: a not-yet-implemented
    // bridge has no transitions and no condition classes to dispose of, and
    // must not be read as four-plus-seven defects.
    if bridge.scope.is_declared() {
        // §27.1: the callback enqueues; it never waits on the LLM or disk.
        if bridge.callback.awaits_llm_or_disk {
            findings.push(Finding::new("callback_blocks_on_llm_or_disk", "callback"));
        }

        // §1328: each of the four transitions is disposed, clears the broadcast
        // authority and is logged.
        for transition in BridgeTransition::ALL {
            let disposition = bridge
                .transitions
                .iter()
                .find(|disposition| disposition.transition == transition);
            let Some(disposition) = disposition else {
                findings.push(Finding::new("transition_undisposed", transition.label()));
                continue;
            };
            if !disposition.clears_broadcast_authority {
                findings.push(Finding::new(
                    "transition_keeps_stale_broadcast",
                    transition.label(),
                ));
            }
            if !disposition.logged {
                findings.push(Finding::new("transition_not_logged", transition.label()));
            }
            // §347: a partner exit leaves the engine in standby.
            if transition == BridgeTransition::Exited && !disposition.leaves_engine_in_standby {
                findings.push(Finding::new("exit_without_standby", transition.label()));
            }
            // §347/§119: a reconnect never restores the previous automatic
            // authority; it is granted only by an explicit operator action.
            if transition == BridgeTransition::Reconnected
                && disposition.restores_previous_auto_authority
            {
                findings.push(Finding::new(
                    "reconnect_restores_previous_auto_authority",
                    transition.label(),
                ));
            }
        }

        // §347: each of the seven condition classes is disposed and logged.
        for condition in BridgeCondition::ALL {
            let disposition = bridge
                .conditions
                .iter()
                .find(|disposition| disposition.condition == condition);
            let Some(disposition) = disposition else {
                findings.push(Finding::new("condition_undisposed", condition.label()));
                continue;
            };
            if !disposition.handled {
                findings.push(Finding::new("condition_undisposed", condition.label()));
            }
            if !disposition.logged {
                findings.push(Finding::new("condition_not_logged", condition.label()));
            }
        }

        // §119: a platform-event interruption revokes automatic replies while
        // manual and replay stay available. A degradation that is not a
        // platform interruption owes none of this.
        for degradation in &bridge.degradations {
            if !degradation.bridge_lost {
                continue;
            }
            if !degradation.auto_reply_revoked {
                findings.push(Finding::new(
                    "platform_interruption_keeps_auto_reply",
                    degradation.cause.clone(),
                ));
            }
            if !degradation.manual_available {
                findings.push(Finding::new(
                    "platform_interruption_disables_manual",
                    degradation.cause.clone(),
                ));
            }
            if !degradation.replay_available {
                findings.push(Finding::new(
                    "platform_interruption_disables_replay",
                    degradation.cause.clone(),
                ));
            }
            if !degradation.logged {
                findings.push(Finding::new(
                    "degradation_not_logged",
                    degradation.cause.clone(),
                ));
            }
        }
    }

    // §1328: a stale-room authority must never be in force. Judged on the
    // descriptor whether or not a bridge is declared, so a not-yet-implemented
    // bridge cannot hide a live grant for a room it has already left.
    if let Some(authority) = &bridge.authority
        && !authority.covers(&bridge.connection)
    {
        findings.push(Finding::new(
            "stale_room_broadcast_permitted",
            authority.room.clone(),
        ));
    }

    // §349: unknown is read as the strictest, never as granted.
    for grant in &bridge.capabilities {
        if grant.state == CapabilityState::Unknown && grant.read_as_granted {
            findings.push(Finding::new(
                "unknown_capability_treated_as_granted",
                grant.capability.clone(),
            ));
        }
    }

    // §349: a missing capability closes only its own feature — judged in both
    // directions, so a permission change neither under- nor over-reaches.
    for feature in &bridge.features {
        let granted = bridge.capability_state(&feature.requires) == Some(CapabilityState::Granted);
        if feature.active && !granted {
            findings.push(Finding::new(
                "feature_active_without_capability",
                feature.name.clone(),
            ));
        }
        if feature.stopped_by_capability_change && granted {
            findings.push(Finding::new(
                "feature_stopped_despite_capability",
                feature.name.clone(),
            ));
        }
    }

    findings
}
