//! What the explicit Unavailable connectors must be, and must never be.
//!
//! These tests are deliberately written so that the two failure modes that would
//! make them decorative are themselves covered: (a) "every duty fails" is paired
//! with a *verified* double that has a success path, and (b) the "no SDK is
//! linked" scan is exercised against a mutated manifest that it must catch.
use nai_contracts::v1::{ContractError, Envelope, ErrorCode, PlatformEvent};
use nai_platform_unavailable::UnavailablePlatform;
use nai_plugin_sdk::connector::{
    AdmissionNotes, ComplianceCheck, ConnectorAvailability, ConnectorIdentity, Delivered,
    Initialized, InterfaceCheck, PlatformConnector, Shutdown, TranslatedEvent,
    UNVERIFIED_EXTERNAL_DEPENDENCY, UnavailableReason,
};

// ---------------------------------------------------------------------------
// The connectors as declared
// ---------------------------------------------------------------------------

#[test]
fn two_platforms_share_a_directory_with_distinct_platform_ids() {
    let connectors = UnavailablePlatform::all();
    assert_eq!(connectors.len(), 2);

    let ids: Vec<&str> = connectors.iter().map(|c| c.platform_id()).collect();
    assert_eq!(ids, UnavailablePlatform::PLATFORM_IDS);
    // §9.4: 快手 and 视频号 sit in the same directory; distinct ids are what keeps
    // them from being confused for one another (or for a connector revision).
    assert_ne!(ids[0], ids[1]);
    let connector_ids: Vec<&str> = connectors.iter().map(|c| c.connector_id()).collect();
    assert_ne!(connector_ids[0], connector_ids[1]);

    // The declared identities are valid by the seam's own rule — checked through
    // the validating constructor rather than by trusting the literals.
    for connector in &connectors {
        let identity = connector.identity();
        assert_eq!(
            ConnectorIdentity::new(
                identity.connector_id.clone(),
                identity.platform_id.clone(),
                identity.platform_label.clone()
            ),
            Ok(identity)
        );
    }
    assert_eq!(UnavailablePlatform::kuaishou().platform_id(), "kuaishou");
    assert_eq!(
        UnavailablePlatform::wechat_channels().platform_id(),
        "wechat-channels"
    );
}

#[test]
fn availability_is_explicitly_unavailable_and_never_verified() {
    for connector in UnavailablePlatform::all() {
        let availability = connector.availability();
        assert!(!availability.is_available());
        assert_eq!(
            availability.reason(),
            Some(UnavailableReason::NoOfficialInterface)
        );
        // There is no interface revision to name, so no `Verified` claim can be
        // made by this connector even by accident.
        assert_eq!(availability.interface_revision(), None);
        assert!(matches!(
            availability,
            ConnectorAvailability::Unavailable { .. }
        ));
    }
}

#[test]
fn admission_notes_cover_every_required_check() {
    for connector in UnavailablePlatform::all() {
        let notes = connector.admission_notes();
        assert_eq!(notes, AdmissionNotes::complete());
        assert!(notes.is_complete());
        assert!(notes.missing().is_empty());
        assert_eq!(notes.compliance().len(), ComplianceCheck::ALL.len());
        assert_eq!(notes.interface().len(), InterfaceCheck::ALL.len());
        // §13.1's five items and §9.3's four are all still outstanding.
        assert_eq!(notes.missing_compliance(), Vec::<ComplianceCheck>::new());
        assert_eq!(notes.missing_interface(), Vec::<InterfaceCheck>::new());
    }

    // The completeness rule is not decorative: dropping one §13.1 check is
    // refused, by name.
    let dropped = ComplianceCheck::VirtualHostMarkOverlayForced;
    let error = AdmissionNotes::declared(
        ComplianceCheck::ALL.into_iter().filter(|c| *c != dropped),
        InterfaceCheck::ALL,
    )
    .unwrap_err();
    assert!(error.message.contains(dropped.label()), "{}", error.message);
    assert_eq!(
        error.details.get("reason").map(String::as_str),
        Some("ADMISSION_NOTES_INCOMPLETE")
    );
}

// ---------------------------------------------------------------------------
// Every duty fails by name — and the rule is not vacuous
// ---------------------------------------------------------------------------

fn assert_unverified(duty: &str, error: ContractError) {
    assert_eq!(error.code, ErrorCode::CapabilityNotReady as i32, "{duty}");
    assert!(error.retryable, "{duty}");
    assert_eq!(
        error.details.get("reason").map(String::as_str),
        Some(UNVERIFIED_EXTERNAL_DEPENDENCY),
        "{duty}"
    );
    assert_eq!(
        error.details.get("unavailable_reason").map(String::as_str),
        Some("no_official_interface"),
        "{duty}"
    );
    // §27.1: the specification's own term, not a locally invented one.
    assert_eq!(
        error.details.get("duty").map(String::as_str),
        Some(duty),
        "{duty}"
    );
}

#[test]
fn every_duty_of_an_unavailable_connector_fails_by_name() {
    for connector in UnavailablePlatform::all() {
        // §27.1's four duties, none of which may fabricate a success (§1344).
        assert_unverified("sdk_init", connector.initialize().unwrap_err());
        assert_unverified(
            "event_translation",
            connector
                .translate_event(PlatformEvent::default())
                .unwrap_err(),
        );
        assert_unverified(
            "authenticated_ipc",
            connector
                .deliver_to_engine(Envelope::default())
                .unwrap_err(),
        );
        assert_unverified("exit", connector.shutdown().unwrap_err());
    }
}

/// A verified connector, test-only. It exists to prove the four duties *can*
/// succeed when a connector is available — without it, "everything fails" would
/// also be satisfied by a seam that always fails.
///
/// §17.5/§721 permit mocks in tests explicitly. This double claims no platform:
/// its identity says so, and it lives in `tests/`, never in `src/`.
struct VerifiedDouble;

const TEST_ONLY_REVISION: &str = "test-only-not-a-real-interface-revision";

impl PlatformConnector for VerifiedDouble {
    fn identity(&self) -> ConnectorIdentity {
        ConnectorIdentity::new("test-double", "test-platform", "test only")
            .expect("static test identity")
    }

    fn availability(&self) -> ConnectorAvailability {
        ConnectorAvailability::verified(TEST_ONLY_REVISION).expect("non-blank test revision")
    }

    fn admission_notes(&self) -> AdmissionNotes {
        AdmissionNotes::complete()
    }

    fn initialize_impl(&self) -> Result<Initialized, ContractError> {
        Ok(Initialized {
            interface_revision: TEST_ONLY_REVISION.into(),
        })
    }

    fn translate_event_impl(&self, event: PlatformEvent) -> Result<TranslatedEvent, ContractError> {
        Ok(TranslatedEvent { event })
    }

    fn deliver_to_engine_impl(&self, envelope: Envelope) -> Result<Delivered, ContractError> {
        Ok(Delivered {
            envelope_id: envelope.message_id,
            acknowledged: true,
        })
    }

    fn shutdown_impl(&self) -> Result<Shutdown, ContractError> {
        Ok(Shutdown)
    }
}

#[test]
fn the_same_duties_have_a_success_path_when_the_connector_is_available() {
    let verified = VerifiedDouble;
    assert!(verified.availability().is_available());

    assert_eq!(
        verified.initialize().unwrap().interface_revision,
        TEST_ONLY_REVISION
    );
    let event = PlatformEvent {
        event_id: "e1".into(),
        ..Default::default()
    };
    assert_eq!(
        verified.translate_event(event.clone()).unwrap().event,
        event
    );
    let delivered = verified
        .deliver_to_engine(Envelope {
            message_id: "m1".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(delivered.envelope_id, "m1");
    assert!(delivered.acknowledged);
    assert_eq!(verified.shutdown().unwrap(), Shutdown);
}

#[test]
fn the_connectors_are_usable_through_the_seam_object() {
    // A03: a connector is consumed through the seam alone — no kernel type is
    // named here, and the trait is object-safe so callers can hold a box.
    let boxes: Vec<Box<dyn PlatformConnector>> = UnavailablePlatform::all()
        .into_iter()
        .map(|connector| Box::new(connector) as Box<dyn PlatformConnector>)
        .collect();
    assert_eq!(boxes.len(), 2);
    for connector in boxes {
        assert!(!connector.availability().is_available());
        assert!(connector.initialize().is_err());
        assert!(connector.admission_notes().is_complete());
    }
}

// ---------------------------------------------------------------------------
// §17.2: no fake SDK — read from this crate's own manifest
// ---------------------------------------------------------------------------

/// Dependency names declared in a package manifest. A line-level parse on
/// purpose: adding a TOML crate to prove the absence of dependencies would be
/// self-defeating. Anything outside `[dependencies]`-shaped sections is not
/// looked at, and the caller asserts the entry count so the parse cannot quietly
/// see nothing.
fn manifest_dependencies(manifest: &str) -> Vec<String> {
    let mut in_dependencies = false;
    let mut names = Vec::new();
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_dependencies = line == "[dependencies]"
                || line == "[dev-dependencies]"
                || line == "[build-dependencies]"
                || (line.starts_with("[target.") && line.ends_with("dependencies]"));
            continue;
        }
        if !in_dependencies || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let name = line.split(['=', '.']).next().unwrap_or_default().trim();
        if !name.is_empty() {
            names.push(name.to_owned());
        }
    }
    names
}

/// Vendor and FFI shapes. `sdk` on its own is not listed: the plugin seam this
/// connector is *supposed* to depend on is named `nai-plugin-sdk`, and the exact
/// allowlist below is what rules out an `*-sdk` addition.
const SDK_SHAPES: [&str; 9] = [
    "douyin",
    "kuaishou",
    "wechat",
    "tencent",
    "bytedance",
    "-sys",
    "bindings",
    "ffi",
    "native-",
];

fn sdk_shaped(names: &[String]) -> Vec<String> {
    names
        .iter()
        .filter(|name| {
            let lowered = name.to_ascii_lowercase();
            SDK_SHAPES.iter().any(|shape| lowered.contains(shape))
        })
        .cloned()
        .collect()
}

const MANIFEST: &str = include_str!("../Cargo.toml");

#[test]
fn the_connector_depends_on_the_contracts_crate_and_the_seam_only() {
    let dependencies = manifest_dependencies(MANIFEST);

    // Non-vacuity: a parse that saw nothing would satisfy every assertion below.
    assert_eq!(dependencies.len(), 2, "manifest parse saw {dependencies:?}");
    assert_eq!(dependencies, ["nai-contracts", "nai-plugin-sdk"]);

    // A03: the connector reaches the system through the seam, so it needs no
    // kernel type — `xtask check-deps` enforces the other direction.
    assert!(!dependencies.iter().any(|name| name == "nai-kernel"));
    assert!(sdk_shaped(&dependencies).is_empty());
}

#[test]
fn the_sdk_shape_scan_catches_an_injected_vendor_dependency() {
    // The scan above is only worth having if it can fail. Feed it the real
    // manifest with one line added, and require it to name that line.
    let mutated = MANIFEST.replace("[dependencies]", "[dependencies]\ndouyin-sdk = \"=1.0.0\"");
    assert_ne!(mutated, MANIFEST, "mutation applied");
    let dependencies = manifest_dependencies(&mutated);
    assert_eq!(dependencies.len(), 3);
    assert_eq!(sdk_shaped(&dependencies), ["douyin-sdk"]);

    // And an FFI binding added later would be caught too, not just a vendor name.
    let ffi = MANIFEST.replace(
        "[dependencies]",
        "[dependencies]\nplatform-bindings = \"=1\"",
    );
    assert_eq!(
        sdk_shaped(&manifest_dependencies(&ffi)),
        ["platform-bindings"]
    );
}

#[test]
fn every_declared_reason_is_usable_and_labelled() {
    // The reason set is closed; nothing in this crate may pick a reason outside
    // it, and every reason is printable for diagnostics.
    for reason in UnavailableReason::ALL {
        assert!(!reason.label().is_empty());
        assert!(
            ConnectorAvailability::unavailable(reason)
                .reason()
                .is_some_and(|picked| picked == reason)
        );
    }
}
