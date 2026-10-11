//! Platform connector seam: the explicit-Unavailable surface for platforms
//! whose official interface is not yet verified (V3.7 §9.4, §17.2, §27.1, §1344).
//!
//! The specification asks for three things of a connector that has no official
//! interface, and this module is the seam that makes all three checkable:
//!
//! - Its **availability is part of what it declares** (§9.4 "保留显式
//!   Unavailable 实现，不宣称可上线"). `Verified` is a claim *with a basis* —
//!   the interface revision it was verified against — not an adjective.
//! - It **only keeps contracts, admission notes and an explicit Unavailable
//!   implementation — no fake SDK** (§17.2). The notes live here as a closed
//!   requirement set, so "which §13.1 check is missing" is answerable.
//! - An unavailable connector **fails by name and never fabricates success**
//!   (§1344 "不得用 mock 通过真实验收"). This is why the four §27.1 duties are
//!   *provided* methods that pass [`require_available`] before delegating to an
//!   `*_impl` hook: "returned Ok but did nothing" has no representation here.
//!
//! Implementing [`PlatformConnector`] needs no kernel type, which is A03's
//! statement ("新增连接器不改内核") expressed as a property of the seam rather
//! than a promise — `xtask check-deps` enforces both directions.
//!
//! Nothing here loads an SDK, opens a socket, starts a companion process or
//! pushes a stream. Real interfaces, a debug-build companion PoC and the
//! platform's own admission decision are external truths and stay absent.
use nai_contracts::v1::{ContractError, Envelope, ErrorCode, PlatformEvent};
use std::collections::BTreeMap;

/// §27.1: the specification's own term for "there is no official interface or
/// SDK". Shared verbatim with
/// `nai_kernel::bridge::SdkAvailability::UnverifiedExternalDependency`; the two
/// must not drift into two names for one thing.
pub const UNVERIFIED_EXTERNAL_DEPENDENCY: &str = "UNVERIFIED_EXTERNAL_DEPENDENCY";

/// A connector that reports itself available but does not implement a duty.
/// Distinct from [`UNVERIFIED_EXTERNAL_DEPENDENCY`] on purpose: "no interface
/// yet" and "interface claimed, hook missing" are different situations.
pub const NOT_IMPLEMENTED: &str = "NOT_IMPLEMENTED";

fn fail(
    reason_token: &str,
    unavailable_reason: Option<UnavailableReason>,
    duty: &str,
    message: String,
) -> ContractError {
    let mut details = BTreeMap::from([
        ("reason".to_string(), reason_token.to_string()),
        ("duty".to_string(), duty.to_string()),
    ]);
    if let Some(reason) = unavailable_reason {
        details.insert("unavailable_reason".to_string(), reason.label().to_string());
    }
    ContractError {
        code: ErrorCode::CapabilityNotReady as i32,
        category: "platform".into(),
        // A readiness condition, not a permanent refusal: it clears when the
        // official interface is verified and the platform admits the use.
        retryable: true,
        message,
        details,
        ..Default::default()
    }
}

/// §27.1: the honest error for a connector with no verified official interface.
pub fn unverified_external_dependency(reason: UnavailableReason, duty: &str) -> ContractError {
    fail(
        UNVERIFIED_EXTERNAL_DEPENDENCY,
        Some(reason),
        duty,
        format!(
            "platform connector unavailable ({}) for {duty}",
            reason.label()
        ),
    )
}

fn not_implemented(duty: &str) -> ContractError {
    fail(
        NOT_IMPLEMENTED,
        None,
        duty,
        format!("platform connector reports available but does not implement {duty}"),
    )
}

/// §9.4 / §17.2: why a connector is not available. A **closed** set: an open
/// string would make "why not" free text that no test or gate can tell apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnavailableReason {
    /// §9.4: the official interface has not been verified yet.
    NoOfficialInterface,
    /// §9.3 / §13.1: the platform has not confirmed the digital-host use.
    PlatformAdmissionPending,
    /// The connector is not ported to this target OS.
    NotPortedToTargetOs,
    /// The account credentials the platform requires are absent.
    CredentialAbsent,
    /// Only a debug-build companion exists; that is not a live authorisation.
    DebugBuildOnly,
}

impl UnavailableReason {
    pub const ALL: [UnavailableReason; 5] = [
        UnavailableReason::NoOfficialInterface,
        UnavailableReason::PlatformAdmissionPending,
        UnavailableReason::NotPortedToTargetOs,
        UnavailableReason::CredentialAbsent,
        UnavailableReason::DebugBuildOnly,
    ];

    pub fn label(self) -> &'static str {
        match self {
            UnavailableReason::NoOfficialInterface => "no_official_interface",
            UnavailableReason::PlatformAdmissionPending => "platform_admission_pending",
            UnavailableReason::NotPortedToTargetOs => "not_ported_to_target_os",
            UnavailableReason::CredentialAbsent => "credential_absent",
            UnavailableReason::DebugBuildOnly => "debug_build_only",
        }
    }
}

/// §9.4: what a connector declares about itself. `Verified` carries the
/// official interface revision it was verified against — an empty revision is
/// not a basis, so it is rejected at construction rather than reported later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectorAvailability {
    Verified { interface_revision: String },
    Unavailable { reason: UnavailableReason },
}

impl ConnectorAvailability {
    /// A verified connector, or a named refusal when no basis is given.
    pub fn verified(interface_revision: impl Into<String>) -> Result<Self, ContractError> {
        let interface_revision = interface_revision.into();
        if interface_revision.trim().is_empty() {
            return Err(fail(
                "VERIFIED_WITHOUT_INTERFACE_REVISION",
                None,
                "availability",
                "a verified connector must name the official interface revision it was verified against"
                    .into(),
            ));
        }
        Ok(ConnectorAvailability::Verified { interface_revision })
    }

    /// An explicitly unavailable connector (§9.4, §17.2).
    pub fn unavailable(reason: UnavailableReason) -> Self {
        ConnectorAvailability::Unavailable { reason }
    }

    pub fn is_available(&self) -> bool {
        matches!(self, ConnectorAvailability::Verified { .. })
    }

    /// The interface revision, when `Verified`.
    pub fn interface_revision(&self) -> Option<&str> {
        match self {
            ConnectorAvailability::Verified { interface_revision } => Some(interface_revision),
            ConnectorAvailability::Unavailable { .. } => None,
        }
    }

    /// The reason, when `Unavailable`.
    pub fn reason(&self) -> Option<UnavailableReason> {
        match self {
            ConnectorAvailability::Unavailable { reason } => Some(*reason),
            ConnectorAvailability::Verified { .. } => None,
        }
    }
}

/// §13.1: the platform compliance checks that must be cleared before the system
/// may leave drill mode. Every one is required; a connector that drops one is
/// not "less compliant", it is unjudgeable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ComplianceCheck {
    /// §13.1 item 1 — the account's identity verification and category are done.
    AccountIdentityAndCategoryConfirmed,
    /// §13.1 item 2 — the virtual-host mark is overlaid by the keeper (INV-20).
    VirtualHostMarkOverlayForced,
    /// §13.1 item 3 — an operator can take over, and automatic interaction is
    /// only allowed while the permission state is `granted`.
    OperatorOnSiteAndAutoOnlyWhenGranted,
    /// §13.1 item 4 — responses vary; nothing repeats a fixed script for long.
    ResponseVariety,
    /// §13.1 item 5 — interaction and approval records are exportable.
    InteractionRecordsExportable,
}

impl ComplianceCheck {
    pub const ALL: [ComplianceCheck; 5] = [
        ComplianceCheck::AccountIdentityAndCategoryConfirmed,
        ComplianceCheck::VirtualHostMarkOverlayForced,
        ComplianceCheck::OperatorOnSiteAndAutoOnlyWhenGranted,
        ComplianceCheck::ResponseVariety,
        ComplianceCheck::InteractionRecordsExportable,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ComplianceCheck::AccountIdentityAndCategoryConfirmed => {
                "account_identity_and_category_confirmed"
            }
            ComplianceCheck::VirtualHostMarkOverlayForced => "virtual_host_mark_overlay_forced",
            ComplianceCheck::OperatorOnSiteAndAutoOnlyWhenGranted => {
                "operator_on_site_and_auto_only_when_granted"
            }
            ComplianceCheck::ResponseVariety => "response_variety",
            ComplianceCheck::InteractionRecordsExportable => "interaction_records_exportable",
        }
    }
}

/// §9.3 (and §9.2's "产物配置不能变更"): the access-pace items standing between
/// "a debug companion answered once" and "the platform admitted us".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InterfaceCheck {
    /// §9.3 — official interface verified: start parameters, asset channel, one
    /// test audio/video push, simulated interactions, exit and reconnect,
    /// resolution and channel constraints.
    OfficialInterfaceVerified,
    /// §9.3 — the debug-build companion PoC is done. Debug success is **not**
    /// go-live authorisation.
    DebugCompanionPoc,
    /// §9.3 — platform admission confirmed (digital-host use, artifact type,
    /// permissions and fees).
    PlatformAdmissionConfirmed,
    /// §9.2 — the artifact configuration is final before applying, because it
    /// cannot change after review or go-live.
    ArtifactConfigFinalBeforeApplication,
}

impl InterfaceCheck {
    pub const ALL: [InterfaceCheck; 4] = [
        InterfaceCheck::OfficialInterfaceVerified,
        InterfaceCheck::DebugCompanionPoc,
        InterfaceCheck::PlatformAdmissionConfirmed,
        InterfaceCheck::ArtifactConfigFinalBeforeApplication,
    ];

    pub fn label(self) -> &'static str {
        match self {
            InterfaceCheck::OfficialInterfaceVerified => "official_interface_verified",
            InterfaceCheck::DebugCompanionPoc => "debug_companion_poc",
            InterfaceCheck::PlatformAdmissionConfirmed => "platform_admission_confirmed",
            InterfaceCheck::ArtifactConfigFinalBeforeApplication => {
                "artifact_config_final_before_application"
            }
        }
    }
}

/// §17.2: the admission notes a not-yet-verified connector keeps alongside its
/// contracts and its explicit Unavailable implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionNotes {
    compliance: Vec<ComplianceCheck>,
    interface: Vec<InterfaceCheck>,
}

impl AdmissionNotes {
    /// The full §13.1 + §9.3 set — what an unverified connector owes.
    pub fn complete() -> Self {
        Self {
            compliance: ComplianceCheck::ALL.to_vec(),
            interface: InterfaceCheck::ALL.to_vec(),
        }
    }

    /// Declare notes and **reject an incomplete set by name**. Silently dropping
    /// a §13.1 check would make the notes look satisfied while the gate is open.
    pub fn declared(
        compliance: impl IntoIterator<Item = ComplianceCheck>,
        interface: impl IntoIterator<Item = InterfaceCheck>,
    ) -> Result<Self, ContractError> {
        let notes = Self {
            compliance: compliance.into_iter().collect(),
            interface: interface.into_iter().collect(),
        };
        let missing: Vec<&'static str> =
            notes.missing().iter().map(|check| check.label()).collect();
        if !missing.is_empty() {
            return Err(fail(
                "ADMISSION_NOTES_INCOMPLETE",
                None,
                "admission_notes",
                format!(
                    "admission notes miss required checks: {}",
                    missing.join(", ")
                ),
            ));
        }
        Ok(notes)
    }

    pub fn compliance(&self) -> &[ComplianceCheck] {
        &self.compliance
    }

    pub fn interface(&self) -> &[InterfaceCheck] {
        &self.interface
    }

    /// The §13.1 checks absent from these notes, in the specified order.
    pub fn missing_compliance(&self) -> Vec<ComplianceCheck> {
        ComplianceCheck::ALL
            .into_iter()
            .filter(|check| !self.compliance.contains(check))
            .collect()
    }

    /// The §9.3 items absent from these notes, in the specified order.
    pub fn missing_interface(&self) -> Vec<InterfaceCheck> {
        InterfaceCheck::ALL
            .into_iter()
            .filter(|check| !self.interface.contains(check))
            .collect()
    }

    pub fn is_complete(&self) -> bool {
        self.missing().is_empty()
    }

    /// Every admission check these notes do not carry, §13.1 items first.
    pub fn missing(&self) -> Vec<MissingCheck> {
        self.missing_compliance()
            .into_iter()
            .map(MissingCheck::Compliance)
            .chain(
                self.missing_interface()
                    .into_iter()
                    .map(MissingCheck::Interface),
            )
            .collect()
    }
}

/// One missing admission check, so [`AdmissionNotes::missing`] can report both
/// kinds in the order the specification lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingCheck {
    Compliance(ComplianceCheck),
    Interface(InterfaceCheck),
}

impl MissingCheck {
    pub fn label(self) -> &'static str {
        match self {
            MissingCheck::Compliance(check) => check.label(),
            MissingCheck::Interface(check) => check.label(),
        }
    }
}

/// A connector's stable identity. Two platforms in one directory MUST have
/// distinct `platform_id`s (§9.4 "快手、视频号放入同一平台插件目录").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorIdentity {
    pub connector_id: String,
    pub platform_id: String,
    pub platform_label: String,
}

impl ConnectorIdentity {
    pub fn new(
        connector_id: impl Into<String>,
        platform_id: impl Into<String>,
        platform_label: impl Into<String>,
    ) -> Result<Self, ContractError> {
        let identity = Self {
            connector_id: connector_id.into(),
            platform_id: platform_id.into(),
            platform_label: platform_label.into(),
        };
        if [
            &identity.connector_id,
            &identity.platform_id,
            &identity.platform_label,
        ]
        .into_iter()
        .any(|field| field.trim().is_empty())
        {
            return Err(fail(
                "CONNECTOR_IDENTITY_INCOMPLETE",
                None,
                "identity",
                "connector id, platform id and platform label must all be non-blank".into(),
            ));
        }
        Ok(identity)
    }
}

/// §27.1 SDK init succeeded, against the named official interface revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Initialized {
    pub interface_revision: String,
}

/// §27.1 official event converted into the internal event shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslatedEvent {
    pub event: PlatformEvent,
}

/// §27.1 authenticated IPC to the engine. Carries the envelope it delivered and
/// whether the platform's protocol ACK arrived — never an empty acknowledgement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivered {
    pub envelope_id: String,
    pub acknowledged: bool,
}

/// §27.1 exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shutdown;

/// The seam's single gate to every success type (§1344). An unavailable
/// connector never gets past it.
pub fn require_available(
    availability: &ConnectorAvailability,
    duty: &str,
) -> Result<(), ContractError> {
    match availability {
        ConnectorAvailability::Verified { .. } => Ok(()),
        ConnectorAvailability::Unavailable { reason } => {
            Err(unverified_external_dependency(*reason, duty))
        }
    }
}

/// §27.1: the four things a platform bridge does, and nothing else. Implementing
/// this trait requires no kernel type — that is A03 ("新增连接器不改内核") as a
/// property of the seam, enforced by `xtask check-deps`.
///
/// The four duties are **provided** methods: each passes [`require_available`]
/// before delegating to its `*_impl` hook. An unavailable connector therefore
/// cannot return success from any duty, and "Ok but nothing happened" is not
/// representable. The hooks default to a named [`NOT_IMPLEMENTED`] failure, so
/// forgetting to implement one fails closed.
pub trait PlatformConnector: Send + Sync {
    /// The stable identity of this connector.
    fn identity(&self) -> ConnectorIdentity;

    /// §9.4: what this connector declares about its own availability.
    fn availability(&self) -> ConnectorAvailability;

    /// §17.2: the admission notes this connector keeps.
    fn admission_notes(&self) -> AdmissionNotes;

    /// §27.1 SDK init.
    fn initialize(&self) -> Result<Initialized, ContractError> {
        require_available(&self.availability(), "sdk_init")?;
        self.initialize_impl()
    }

    /// §27.1 official event and permission translation.
    fn translate_event(&self, event: PlatformEvent) -> Result<TranslatedEvent, ContractError> {
        require_available(&self.availability(), "event_translation")?;
        self.translate_event_impl(event)
    }

    /// §27.1 authenticated IPC to the engine.
    fn deliver_to_engine(&self, envelope: Envelope) -> Result<Delivered, ContractError> {
        require_available(&self.availability(), "authenticated_ipc")?;
        self.deliver_to_engine_impl(envelope)
    }

    /// §27.1 exit.
    fn shutdown(&self) -> Result<Shutdown, ContractError> {
        require_available(&self.availability(), "exit")?;
        self.shutdown_impl()
    }

    /// The real SDK-init work, only reachable when available.
    fn initialize_impl(&self) -> Result<Initialized, ContractError> {
        Err(not_implemented("sdk_init"))
    }

    /// The real translation work, only reachable when available.
    fn translate_event_impl(
        &self,
        _event: PlatformEvent,
    ) -> Result<TranslatedEvent, ContractError> {
        Err(not_implemented("event_translation"))
    }

    /// The real IPC work, only reachable when available.
    fn deliver_to_engine_impl(&self, _envelope: Envelope) -> Result<Delivered, ContractError> {
        Err(not_implemented("authenticated_ipc"))
    }

    /// The real exit work, only reachable when available.
    fn shutdown_impl(&self) -> Result<Shutdown, ContractError> {
        Err(not_implemented("exit"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verified_requires_a_basis() {
        assert!(ConnectorAvailability::verified("2026-10-11").is_ok());
        for empty in ["", "  ", "\t"] {
            let error = ConnectorAvailability::verified(empty).unwrap_err();
            assert_eq!(
                error.details.get("reason").map(String::as_str),
                Some("VERIFIED_WITHOUT_INTERFACE_REVISION")
            );
        }
    }

    #[test]
    fn identity_rejects_blank_fields() {
        assert!(ConnectorIdentity::new("c", "p", "P").is_ok());
        for (connector, platform, label) in [("", "p", "P"), ("c", " ", "P"), ("c", "p", "")] {
            assert!(ConnectorIdentity::new(connector, platform, label).is_err());
        }
    }

    #[test]
    fn incomplete_admission_notes_are_rejected_by_name() {
        let missing = ComplianceCheck::ResponseVariety;
        let error = AdmissionNotes::declared(
            ComplianceCheck::ALL
                .into_iter()
                .filter(|check| *check != missing),
            InterfaceCheck::ALL,
        )
        .unwrap_err();
        let message = error.message.clone();
        assert!(message.contains(missing.label()), "{message}");
        assert_eq!(
            error.details.get("reason").map(String::as_str),
            Some("ADMISSION_NOTES_INCOMPLETE")
        );
        assert!(AdmissionNotes::complete().is_complete());
    }

    #[test]
    fn the_gate_is_the_only_way_to_the_success_types() {
        let unavailable =
            ConnectorAvailability::unavailable(UnavailableReason::NoOfficialInterface);
        let error = require_available(&unavailable, "sdk_init").unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityNotReady as i32);
        assert!(error.retryable);
        assert_eq!(
            error.details.get("reason").map(String::as_str),
            Some(UNVERIFIED_EXTERNAL_DEPENDENCY)
        );
        assert_eq!(
            error.details.get("unavailable_reason").map(String::as_str),
            Some("no_official_interface")
        );
        assert!(
            require_available(&ConnectorAvailability::verified("1.0").unwrap(), "sdk_init").is_ok()
        );
    }
}
