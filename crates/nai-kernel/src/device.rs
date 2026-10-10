//! Device admission contract (V3.7 §34.4, §1469, §35.3, INV-23, §8.5; registry O04).
//!
//! §1424 names the gap plainly: there is "no abstraction for devices and
//! hardware access, and no physical safety interlock". This module is that
//! abstraction's **judging** half. Device capabilities are *declared*
//! (`device.sense@1` / `device.control@1` declare an action enum, a value
//! range, a rate limit and a safety interlock, §1469); actuation additionally
//! requires a safety interlock and a local manual override (§1497); physical
//! execution is `risk_3`, disabled by default and needs a local confirmation
//! and an independent switch (§1469, §34.4); a driver or SDK is never declared
//! *in the kernel process* (§34.4); hotplug, disconnect, duplicate and timeout
//! each need an explicit disposition that is written to the decision log
//! (§1497, §1498); a duplicate audio device never opens a second live capture
//! path (§8.5); and actual use never exceeds the declaration and the user's
//! authorization (INV-23).
//!
//! Nothing here opens a device, enumerates hardware, routes WASAPI or mixes
//! PCM. [`audit_host`] is a pure function of a declared [`DeviceHost`], so
//! every rule is directly constructible in a test.
use crate::golden::Finding;

/// A `device.sense@1` (read) or `device.control@1` (actuate) channel (§1469).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeviceChannel {
    /// Read-only sensing. It has no action to interlock and no execution to
    /// override, so the actuation governance below does not apply to it.
    Sense,
    /// Actuation. It needs a safety interlock and a local manual override.
    Control,
}

impl DeviceChannel {
    pub fn label(self) -> &'static str {
        match self {
            DeviceChannel::Sense => "sense",
            DeviceChannel::Control => "control",
        }
    }
}

/// §35.3: effective risk = max(declared, derived). §1469: control defaults to
/// `risk_2`; physical execution defaults to `risk_3`. The variants are ordered
/// so `max` picks the more severe grade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RiskClass {
    /// Read-only sensing.
    Risk1,
    /// Control.
    Risk2,
    /// Physical execution.
    Risk3,
}

impl RiskClass {
    pub fn label(self) -> &'static str {
        match self {
            RiskClass::Risk1 => "risk_1",
            RiskClass::Risk2 => "risk_2",
            RiskClass::Risk3 => "risk_3",
        }
    }
}

/// §1469: a declared value range, in thousandths so the core stays integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueRange {
    pub min_milli: i64,
    pub max_milli: i64,
}

impl ValueRange {
    pub fn new(min_milli: i64, max_milli: i64) -> Self {
        Self {
            min_milli,
            max_milli,
        }
    }

    /// A range is well formed only when it is not empty.
    pub fn is_well_formed(&self) -> bool {
        self.min_milli <= self.max_milli
    }
}

/// §1469: one declared action of a device capability. The action enum, its
/// value range and its rate limit are all part of the declaration; a missing
/// range or rate limit is reported rather than assumed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceAction {
    pub name: String,
    pub range: Option<ValueRange>,
    pub rate_limit_per_min: Option<u32>,
}

impl DeviceAction {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            range: None,
            rate_limit_per_min: None,
        }
    }

    pub fn with_range(mut self, range: ValueRange) -> Self {
        self.range = Some(range);
        self
    }

    pub fn with_rate_limit(mut self, per_min: u32) -> Self {
        self.rate_limit_per_min = Some(per_min);
        self
    }
}

/// §34.4: device drivers and SDKs sit in a subprocess or an edge node; they
/// never enter the kernel process. This is a *placement* boundary, not a
/// statement about where the judging code lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceHostPlacement {
    Subprocess,
    EdgeNode,
    /// Forbidden: a driver or SDK inside the kernel process.
    KernelProcess,
}

/// One declared device capability (`device.sense@1` / `device.control@1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceDeclaration {
    pub device_ref: String,
    pub channel: DeviceChannel,
    /// §1469: physical execution is `risk_3`, disabled by default and needs a
    /// local confirmation and an independent switch.
    pub physical_execution: bool,
    /// §34.4: physical execution devices are disabled by default; enabling one
    /// needs an administrator's local confirmation.
    pub enabled_by_default: bool,
    /// §1469 / §34.4: enabling or executing needs a local confirmation.
    pub local_confirmation: bool,
    /// §1469: physical execution needs an independent switch.
    pub independent_switch: bool,
    pub actions: Vec<DeviceAction>,
    /// §1469/§1497: the safety interlock. Required for `Control`.
    pub safety_interlock: Option<String>,
    /// §1497: the local manual override. Required for `Control`.
    pub local_manual_override: bool,
    /// §35.3: the risk the declarer asserts.
    pub declared_risk: RiskClass,
}

impl DeviceDeclaration {
    /// §35.3 / §1469: the risk that follows from the declaration itself.
    pub fn derived_risk(&self) -> RiskClass {
        if self.physical_execution {
            RiskClass::Risk3
        } else if self.channel == DeviceChannel::Control {
            RiskClass::Risk2
        } else {
            RiskClass::Risk1
        }
    }

    /// §35.3: effective risk = max(declared, derived). A declaration may raise
    /// its own risk (more conservative) but never lower it.
    pub fn effective_risk(&self) -> RiskClass {
        self.declared_risk.max(self.derived_risk())
    }

    pub fn declares_action(&self, action: &str) -> bool {
        self.actions.iter().any(|declared| declared.name == action)
    }
}

/// §1498: the four event classes that need an explicit disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeviceEvent {
    Hotplug,
    Disconnect,
    Duplicate,
    Timeout,
}

impl DeviceEvent {
    pub const ALL: [DeviceEvent; 4] = [
        DeviceEvent::Hotplug,
        DeviceEvent::Disconnect,
        DeviceEvent::Duplicate,
        DeviceEvent::Timeout,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DeviceEvent::Hotplug => "hotplug",
            DeviceEvent::Disconnect => "disconnect",
            DeviceEvent::Duplicate => "duplicate",
            DeviceEvent::Timeout => "timeout",
        }
    }
}

/// §1498 + §1497: how a host disposes of one event class, and whether that
/// disposition is written to the decision log. §8.5 / O04: a duplicate audio
/// device must not open a second live capture path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventDisposition {
    pub event: DeviceEvent,
    /// The host has an explicit disposition for this event class.
    pub handled: bool,
    /// The event is written to the decision log.
    pub logged: bool,
    /// A duplicate keeps a single live capture path (only meaningful for
    /// [`DeviceEvent::Duplicate`]).
    pub keeps_single_capture_path: bool,
}

impl EventDisposition {
    /// A sound disposition for `event`: handled, logged, and — for a duplicate
    /// — keeping a single capture path.
    pub fn resolved(event: DeviceEvent) -> Self {
        Self {
            event,
            handled: true,
            logged: true,
            keeps_single_capture_path: true,
        }
    }
}

/// INV-23: one device action the host actually used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceUse {
    pub device_ref: String,
    pub action: String,
}

impl DeviceUse {
    pub fn new(device_ref: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            device_ref: device_ref.into(),
            action: action.into(),
        }
    }
}

/// INV-23: the actions the user has authorized for one device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceGrant {
    pub device_ref: String,
    pub actions: Vec<String>,
}

impl DeviceGrant {
    pub fn new(device_ref: impl Into<String>, actions: Vec<String>) -> Self {
        Self {
            device_ref: device_ref.into(),
            actions,
        }
    }

    pub fn authorizes(&self, device_ref: &str, action: &str) -> bool {
        self.device_ref == device_ref && self.actions.iter().any(|name| name == action)
    }
}

/// A declared device host: the declarations, the user's authorization, the
/// actual use, the event dispositions and the placement. The audit judges this
/// descriptor; it does not open a device.
#[derive(Debug, Clone)]
pub struct DeviceHost {
    pub declarations: Vec<DeviceDeclaration>,
    pub granted: Vec<DeviceGrant>,
    pub used: Vec<DeviceUse>,
    pub events: Vec<EventDisposition>,
    pub placement: DeviceHostPlacement,
}

impl DeviceHost {
    /// A host with no device integration declared at all. It audits empty and
    /// reads as [`HostReadiness::Blocked`], the honest O04 state.
    pub fn empty() -> Self {
        Self {
            declarations: Vec::new(),
            granted: Vec::new(),
            used: Vec::new(),
            events: Vec::new(),
            placement: DeviceHostPlacement::Subprocess,
        }
    }

    pub fn declaration(&self, device_ref: &str) -> Option<&DeviceDeclaration> {
        self.declarations
            .iter()
            .find(|declared| declared.device_ref == device_ref)
    }

    /// `Defective` takes precedence: a not-yet-integrated host must not hide a
    /// boundary violation.
    pub fn readiness(&self) -> HostReadiness {
        if !audit_host(self).is_empty() {
            return HostReadiness::Defective;
        }
        if self.declarations.is_empty() {
            return HostReadiness::Blocked;
        }
        HostReadiness::Ready
    }
}

/// Readiness of the declared device host. `Blocked` is the honest state for
/// O04 while no real device integration exists (§1261: an unrun acceptance is
/// marked Blocked, never rendered as a pass).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostReadiness {
    /// No device integration declared yet.
    Blocked,
    /// A declaration exists but violates a boundary.
    Defective,
    /// Declared, and nothing is violated.
    Ready,
}

/// Audit a declared device host. A pure function of the descriptor: every
/// finding is directly constructible, and no real device is touched.
pub fn audit_host(host: &DeviceHost) -> Vec<Finding> {
    let mut findings = Vec::new();

    // §34.4: a driver or SDK never sits in the kernel process.
    if host.placement == DeviceHostPlacement::KernelProcess {
        findings.push(Finding::new("device_driver_in_kernel_process", "placement"));
    }

    for declaration in &host.declarations {
        let device = declaration.device_ref.clone();

        // §1469: the action enum, its value range and its rate limit.
        if declaration.actions.is_empty() {
            findings.push(Finding::new("no_declared_actions", device.clone()));
        }
        for action in &declaration.actions {
            let subject = format!("{}/{}", device, action.name);
            if action.range.is_none() {
                findings.push(Finding::new("action_without_value_range", subject.clone()));
            }
            if action.rate_limit_per_min.is_none() {
                findings.push(Finding::new("action_without_rate_limit", subject.clone()));
            }
        }

        // §1469/§1497: actuation needs a safety interlock and a local override.
        // A pure sense channel has no action to interlock and no execution to
        // override, so it is not reported for either.
        if declaration.channel == DeviceChannel::Control {
            if declaration.safety_interlock.is_none() {
                findings.push(Finding::new(
                    "control_without_safety_interlock",
                    device.clone(),
                ));
            }
            if !declaration.local_manual_override {
                findings.push(Finding::new(
                    "control_without_local_override",
                    device.clone(),
                ));
            }
        }

        // §35.3: a declared risk below the derived risk is refused.
        if declaration.declared_risk < declaration.derived_risk() {
            findings.push(Finding::new("declared_risk_below_derived", device.clone()));
        }

        // §1469/§34.4: physical execution governance.
        if declaration.physical_execution {
            if declaration.channel == DeviceChannel::Sense {
                findings.push(Finding::new(
                    "sense_channel_with_physical_execution",
                    device.clone(),
                ));
            }
            if declaration.enabled_by_default {
                findings.push(Finding::new("physical_enabled_by_default", device.clone()));
            }
            if !declaration.local_confirmation {
                findings.push(Finding::new(
                    "physical_without_local_confirmation",
                    device.clone(),
                ));
            }
            if !declaration.independent_switch {
                findings.push(Finding::new(
                    "physical_without_independent_switch",
                    device.clone(),
                ));
            }
        }
    }

    // §1498 + §1497: every event class has an explicit, logged disposition.
    // §8.5 / O04: a duplicate never opens a second live capture path. A host
    // that declares no device has no device events to dispose of, so this
    // block is judged only once a device is declared — the not-yet-integrated
    // host is Blocked, not Defective.
    if !host.declarations.is_empty() {
        for event in DeviceEvent::ALL {
            let disposition = host
                .events
                .iter()
                .find(|disposition| disposition.event == event);
            let Some(disposition) = disposition else {
                findings.push(Finding::new("device_event_undisposed", event.label()));
                continue;
            };
            if !disposition.handled {
                findings.push(Finding::new("device_event_undisposed", event.label()));
            }
            if !disposition.logged {
                findings.push(Finding::new("device_event_not_logged", event.label()));
            }
            if event == DeviceEvent::Duplicate && !disposition.keeps_single_capture_path {
                findings.push(Finding::new(
                    "duplicate_event_opens_second_capture_path",
                    event.label(),
                ));
            }
        }
    }

    // INV-23: actual use stays within the declaration and the authorization.
    // A use that is not declared at all is reported once, not twice.
    for used in &host.used {
        let subject = format!("{}/{}", used.device_ref, used.action);
        let declared = host
            .declarations
            .iter()
            .any(|declaration| declaration.declares_action_of(&used.device_ref, &used.action));
        if !declared {
            findings.push(Finding::new("used_undeclared_device_action", subject));
            continue;
        }
        let authorized = host
            .granted
            .iter()
            .any(|grant| grant.authorizes(&used.device_ref, &used.action));
        if !authorized {
            findings.push(Finding::new("used_beyond_authorization", subject));
        }
    }

    findings
}

impl DeviceDeclaration {
    fn declares_action_of(&self, device_ref: &str, action: &str) -> bool {
        self.device_ref == device_ref && self.declares_action(action)
    }
}
