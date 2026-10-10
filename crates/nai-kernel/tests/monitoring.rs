//! §3.1 L5 / §10.2 mobile monitoring surface (registry O03).
use nai_kernel::golden::Finding;
use nai_kernel::monitoring::{
    CommandRisk, ConnectionRoute, EncryptionClaim, Exposure, LiveState, MediaClock,
    MobileCapability, MobileSurface, MobileView, PreviewPolicy, RemoteCommand, Role,
    SurfaceReadiness, audit_surface,
};
use std::collections::BTreeSet;

fn command(name: &str) -> RemoteCommand {
    RemoteCommand {
        name: name.to_owned(),
        risk: CommandRisk::Low,
        min_role: Role::Operator,
        carries_identity: true,
        carries_session_epoch: true,
        carries_sequence: true,
        carries_ttl: true,
        returns_actual_ack: true,
    }
}

/// The honest baseline: every capability, only white-listed state, route
/// undecided, relay-terminated TLS disclosed without claiming end-to-end.
fn clean_surface() -> MobileSurface {
    MobileSurface {
        capabilities: MobileCapability::ALL.to_vec(),
        exposures: Exposure::ALLOWED.to_vec(),
        route: ConnectionRoute::Undecided,
        encryption: EncryptionClaim {
            relay_terminates_tls: true,
            claims_end_to_end: false,
            protocol_reviewed: false,
        },
        preview: PreviewPolicy {
            enabled: true,
            on_demand: true,
            truly_encoded_low_bitrate: true,
            releases_encoder_and_bandwidth_on_close: true,
            may_preempt_official_output: false,
        },
        commands: vec![command("takeover")],
        emergency_mute_reachable: true,
        enable_status_provided: true,
        knowledge_patch_routes_to_review: true,
    }
}

fn tokens(findings: &[Finding]) -> Vec<&'static str> {
    findings.iter().map(|finding| finding.token).collect()
}

fn has(findings: &[Finding], token: &str, subject: &str) -> bool {
    findings
        .iter()
        .any(|finding| finding.token == token && finding.subject == subject)
}

// ---------------------------------------------------------------- projection

#[test]
fn exposure_partition_is_exact() {
    assert_eq!(
        Exposure::ALL.len(),
        Exposure::ALLOWED.len() + Exposure::FORBIDDEN.len()
    );
    assert_eq!(
        &Exposure::ALL[..Exposure::ALLOWED.len()],
        &Exposure::ALLOWED[..]
    );
    assert_eq!(
        &Exposure::ALL[Exposure::ALLOWED.len()..],
        &Exposure::FORBIDDEN[..]
    );

    for allowed in Exposure::ALLOWED {
        assert!(!allowed.is_forbidden(), "{allowed:?}");
    }
    for forbidden in Exposure::FORBIDDEN {
        assert!(forbidden.forbidden_token().is_some(), "{forbidden:?}");
    }

    let unique: BTreeSet<Exposure> = Exposure::ALL.into_iter().collect();
    assert_eq!(unique.len(), Exposure::ALL.len());
}

#[test]
fn view_fields_agree_with_the_allow_list() {
    let view: BTreeSet<&str> = MobileView::field_names().into_iter().collect();
    let allowed: BTreeSet<&str> = Exposure::ALLOWED.iter().map(|e| e.label()).collect();
    let exposed: BTreeSet<&str> = MobileView::exposed().iter().map(|e| e.label()).collect();
    assert_eq!(view, allowed);
    assert_eq!(exposed, allowed);
}

#[test]
fn the_view_has_no_place_for_a_forbidden_field() {
    let sensitive: BTreeSet<&str> = LiveState::sensitive_field_names().into_iter().collect();
    let forbidden: BTreeSet<&str> = Exposure::FORBIDDEN.iter().map(|e| e.label()).collect();
    assert_eq!(sensitive, forbidden);

    for name in MobileView::field_names() {
        assert!(!sensitive.contains(name), "{name} must not be a view field");
    }
}

#[test]
fn projection_drops_the_media_clock_and_secret() {
    let state = LiveState {
        mode: "auto".to_owned(),
        pending_seconds: 12,
        secret: "sk-live-must-not-leak".to_owned(),
        media_clock: MediaClock {
            epoch: 7,
            tick_ms: 4242,
        },
        raw_audio: vec![1, 2, 3],
        raw_video: vec![9, 9, 9],
        broadcast_control: Some(nai_kernel::monitoring::BroadcastHandle {
            room: "room-1".to_owned(),
        }),
        ..LiveState::default()
    };

    let view = MobileView::project(&state);
    assert_eq!(view.mode, "auto");
    assert_eq!(view.pending_seconds, 12);

    for exposure in MobileView::exposed() {
        assert!(!exposure.is_forbidden());
    }
    let rendered = format!("{view:?}");
    for name in LiveState::sensitive_field_names() {
        assert!(!rendered.contains(name), "view leaked {name}");
    }
    assert!(!rendered.contains("sk-live-must-not-leak"));
}

// ------------------------------------------------------------- minimum form

#[test]
fn minimum_form_is_the_four_named_capabilities() {
    assert_eq!(MobileCapability::MINIMUM_FORM.len(), 4);
    for capability in MobileCapability::MINIMUM_FORM {
        assert!(
            MobileCapability::ALL.contains(&capability),
            "{capability:?}"
        );
    }
    let unique: BTreeSet<MobileCapability> = MobileCapability::ALL.into_iter().collect();
    assert_eq!(unique.len(), MobileCapability::ALL.len());
}

#[test]
fn a_missing_minimum_capability_is_named() {
    let mut surface = clean_surface();
    surface
        .capabilities
        .retain(|capability| *capability != MobileCapability::EmergencyMute);

    let findings = audit_surface(&surface);
    assert!(has(&findings, "minimum_form_incomplete", "emergency_mute"));
    assert!(!surface.minimum_form_complete());
    assert!(
        surface
            .missing_minimum()
            .contains(&MobileCapability::EmergencyMute)
    );
}

#[test]
fn a_missing_capability_does_not_also_report_unreachable() {
    let mut surface = clean_surface();
    surface
        .capabilities
        .retain(|capability| *capability != MobileCapability::EmergencyMute);
    surface.emergency_mute_reachable = false;

    let findings = audit_surface(&surface);
    assert!(has(&findings, "minimum_form_incomplete", "emergency_mute"));
    assert!(
        !findings
            .iter()
            .any(|f| f.token == "emergency_mute_unreachable")
    );
}

#[test]
fn deferred_preview_and_push_are_not_missing_capabilities() {
    let mut surface = clean_surface();
    surface.capabilities.retain(|capability| {
        *capability != MobileCapability::Preview && *capability != MobileCapability::Push
    });

    assert!(surface.minimum_form_complete());
    assert!(audit_surface(&surface).is_empty());
}

// ----------------------------------------------------------------- exposure

#[test]
fn each_forbidden_exposure_class_is_reported() {
    let cases: [(Exposure, &str); 5] = [
        (Exposure::MediaClock, "exposes_media_clock"),
        (Exposure::Secret, "exposes_secret"),
        (Exposure::RawAudio, "exposes_raw_media"),
        (Exposure::RawVideo, "exposes_raw_media"),
        (Exposure::BroadcastControl, "exposes_broadcast_control"),
    ];
    for (exposure, token) in cases {
        let mut surface = clean_surface();
        surface.exposures.push(exposure);
        let findings = audit_surface(&surface);
        assert!(has(&findings, token, exposure.label()), "{exposure:?}");
    }
}

#[test]
fn a_duplicate_forbidden_exposure_is_reported_once() {
    let mut surface = clean_surface();
    surface.exposures.push(Exposure::Secret);
    surface.exposures.push(Exposure::Secret);

    let count = audit_surface(&surface)
        .iter()
        .filter(|finding| finding.token == "exposes_secret")
        .count();
    assert_eq!(count, 1);
}

#[test]
fn allowed_exposures_are_not_findings() {
    let surface = clean_surface();
    assert_eq!(surface.forbidden_exposures().len(), 0);
    assert!(audit_surface(&surface).is_empty());
}

// ------------------------------------------------------------ operational

#[test]
fn unreachable_emergency_mute_is_reported() {
    let mut surface = clean_surface();
    surface.emergency_mute_reachable = false;
    let findings = audit_surface(&surface);
    assert!(has(
        &findings,
        "emergency_mute_unreachable",
        "emergency_mute"
    ));
}

#[test]
fn a_missing_enable_status_is_reported() {
    let mut surface = clean_surface();
    surface.enable_status_provided = false;
    let findings = audit_surface(&surface);
    assert!(has(&findings, "enable_status_missing", "enable_status"));
}

#[test]
fn knowledge_patch_bypassing_review_is_reported() {
    let mut surface = clean_surface();
    surface.knowledge_patch_routes_to_review = false;
    let findings = audit_surface(&surface);
    assert!(has(
        &findings,
        "knowledge_patch_without_review_flow",
        "knowledge_patch"
    ));
}

#[test]
fn an_absent_knowledge_patch_is_not_audited() {
    let mut surface = clean_surface();
    surface
        .capabilities
        .retain(|capability| *capability != MobileCapability::KnowledgePatch);
    surface.knowledge_patch_routes_to_review = false;
    assert!(audit_surface(&surface).is_empty());
}

// ------------------------------------------------------------------ preview

#[test]
fn enabled_preview_conditions_are_each_reported() {
    let mut surface = clean_surface();
    surface.preview.on_demand = false;
    surface.preview.truly_encoded_low_bitrate = false;
    surface.preview.releases_encoder_and_bandwidth_on_close = false;
    surface.preview.may_preempt_official_output = true;

    let tokens = tokens(&audit_surface(&surface));
    assert!(tokens.contains(&"preview_not_on_demand"));
    assert!(tokens.contains(&"preview_not_encoded"));
    assert!(tokens.contains(&"preview_does_not_release_on_close"));
    assert!(tokens.contains(&"preview_may_preempt_output"));
}

#[test]
fn a_disabled_preview_is_not_a_defect() {
    let mut surface = clean_surface();
    surface.preview = PreviewPolicy {
        enabled: false,
        on_demand: false,
        truly_encoded_low_bitrate: false,
        releases_encoder_and_bandwidth_on_close: false,
        may_preempt_official_output: true,
    };
    assert!(audit_surface(&surface).is_empty());
    assert!(!PreviewPolicy::default().enabled);
}

// -------------------------------------------------------------------- route

#[test]
fn route_a_requires_a_secure_context() {
    let mut surface = clean_surface();
    surface.route = ConnectionRoute::RelayPwa {
        https_domain: false,
        certificate: true,
    };
    assert!(has(
        &audit_surface(&surface),
        "relay_pwa_without_secure_context",
        "route_a"
    ));

    surface.route = ConnectionRoute::RelayPwa {
        https_domain: true,
        certificate: false,
    };
    assert!(has(
        &audit_surface(&surface),
        "relay_pwa_without_secure_context",
        "route_a"
    ));
}

#[test]
fn a_complete_route_a_is_ready() {
    let mut surface = clean_surface();
    surface.route = ConnectionRoute::RelayPwa {
        https_domain: true,
        certificate: true,
    };
    assert!(audit_surface(&surface).is_empty());
    assert_eq!(surface.readiness(), SurfaceReadiness::Ready);
}

#[test]
fn route_b_requires_distribution_and_verified_tauri() {
    let mut surface = clean_surface();
    surface.route = ConnectionRoute::NativeShellLan {
        app_distribution: false,
        tauri_maturity_verified: false,
    };
    let findings = audit_surface(&surface);
    assert!(has(
        &findings,
        "native_shell_without_distribution",
        "route_b"
    ));
    assert!(has(
        &findings,
        "native_shell_tauri_maturity_unverified",
        "route_b"
    ));

    surface.route = ConnectionRoute::NativeShellLan {
        app_distribution: true,
        tauri_maturity_verified: true,
    };
    assert!(audit_surface(&surface).is_empty());
    assert_eq!(surface.readiness(), SurfaceReadiness::Ready);
}

#[test]
fn route_b_with_unverified_tauri_is_reported() {
    let mut surface = clean_surface();
    surface.route = ConnectionRoute::NativeShellLan {
        app_distribution: true,
        tauri_maturity_verified: false,
    };
    let findings = audit_surface(&surface);
    assert!(has(
        &findings,
        "native_shell_tauri_maturity_unverified",
        "route_b"
    ));
    assert!(
        !findings
            .iter()
            .any(|f| f.token == "native_shell_without_distribution")
    );
}

// --------------------------------------------------------------- encryption

#[test]
fn a_relay_that_terminates_tls_must_not_claim_end_to_end() {
    let mut surface = clean_surface();
    surface.encryption = EncryptionClaim {
        relay_terminates_tls: true,
        claims_end_to_end: true,
        protocol_reviewed: true,
    };
    assert!(has(
        &audit_surface(&surface),
        "e2e_encryption_claimed_with_relay_terminated_tls",
        "encryption"
    ));
}

#[test]
fn an_honest_relay_disclosure_is_clean() {
    // The baseline discloses relay-terminated TLS without claiming E2E.
    let surface = clean_surface();
    assert!(surface.encryption.relay_terminates_tls);
    assert!(!surface.encryption.claims_end_to_end);
    assert!(audit_surface(&surface).is_empty());
}

#[test]
fn end_to_end_without_a_reviewed_protocol_is_reported() {
    let mut surface = clean_surface();
    surface.encryption = EncryptionClaim {
        relay_terminates_tls: false,
        claims_end_to_end: true,
        protocol_reviewed: false,
    };
    let findings = audit_surface(&surface);
    assert!(has(
        &findings,
        "e2e_encryption_claimed_without_reviewed_protocol",
        "encryption"
    ));
    assert!(
        !findings
            .iter()
            .any(|f| f.token == "e2e_encryption_claimed_with_relay_terminated_tls")
    );
}

#[test]
fn a_reviewed_end_to_end_is_clean() {
    let mut surface = clean_surface();
    surface.encryption = EncryptionClaim {
        relay_terminates_tls: false,
        claims_end_to_end: true,
        protocol_reviewed: true,
    };
    assert!(audit_surface(&surface).is_empty());
}

// ----------------------------------------------------------------- commands

#[test]
fn role_ordering_is_operator_over_observer() {
    assert!(Role::Operator.at_least(Role::Operator));
    assert!(Role::Operator.at_least(Role::Observer));
    assert!(Role::Observer.at_least(Role::Observer));
    assert!(!Role::Observer.at_least(Role::Operator));
}

#[test]
fn each_missing_command_contract_field_is_reported() {
    let mut mutations: [(&str, RemoteCommand); 5] = [
        ("identity", command("c")),
        ("session_epoch", command("c")),
        ("sequence", command("c")),
        ("ttl", command("c")),
        ("ack", command("c")),
    ];
    mutations[0].1.carries_identity = false;
    mutations[1].1.carries_session_epoch = false;
    mutations[2].1.carries_sequence = false;
    mutations[3].1.carries_ttl = false;
    mutations[4].1.returns_actual_ack = false;

    for (field, remote) in mutations {
        assert!(!remote.contract_complete(), "{field}");
        let mut surface = clean_surface();
        surface.commands = vec![remote];
        let findings = audit_surface(&surface);
        assert!(
            has(&findings, "command_contract_incomplete", "c"),
            "{field}"
        );
    }
}

#[test]
fn a_high_risk_command_open_to_an_observer_is_reported() {
    let mut remote = command("start_broadcast");
    remote.risk = CommandRisk::High;
    remote.min_role = Role::Observer;

    let mut surface = clean_surface();
    surface.commands = vec![remote];
    let findings = audit_surface(&surface);
    assert!(has(
        &findings,
        "high_risk_command_open_to_observer",
        "start_broadcast"
    ));
}

#[test]
fn a_high_risk_command_requiring_the_operator_is_clean() {
    let mut remote = command("takeover");
    remote.risk = CommandRisk::High;
    remote.min_role = Role::Operator;

    let mut surface = clean_surface();
    surface.commands = vec![remote];
    assert!(audit_surface(&surface).is_empty());
}

#[test]
fn a_low_risk_command_may_be_open_to_an_observer() {
    let mut remote = command("view_board");
    remote.risk = CommandRisk::Low;
    remote.min_role = Role::Observer;

    let mut surface = clean_surface();
    surface.commands = vec![remote];
    assert!(audit_surface(&surface).is_empty());
}

// ---------------------------------------------------------------- readiness

#[test]
fn an_undecided_route_is_not_a_defect() {
    let surface = clean_surface();
    assert_eq!(surface.route, ConnectionRoute::Undecided);
    assert!(audit_surface(&surface).is_empty());
    assert_eq!(surface.readiness(), SurfaceReadiness::AwaitingRouteDecision);
}

#[test]
fn a_defect_outranks_an_undecided_route() {
    let mut surface = clean_surface();
    surface.exposures.push(Exposure::MediaClock);
    assert_eq!(surface.route, ConnectionRoute::Undecided);
    assert_eq!(surface.readiness(), SurfaceReadiness::Defective);
}

#[test]
fn every_finding_token_is_constructible() {
    let expected: BTreeSet<&str> = [
        "exposes_media_clock",
        "exposes_secret",
        "exposes_raw_media",
        "exposes_broadcast_control",
        "minimum_form_incomplete",
        "emergency_mute_unreachable",
        "enable_status_missing",
        "knowledge_patch_without_review_flow",
        "preview_not_on_demand",
        "preview_not_encoded",
        "preview_does_not_release_on_close",
        "preview_may_preempt_output",
        "relay_pwa_without_secure_context",
        "native_shell_without_distribution",
        "native_shell_tauri_maturity_unverified",
        "e2e_encryption_claimed_with_relay_terminated_tls",
        "e2e_encryption_claimed_without_reviewed_protocol",
        "command_contract_incomplete",
        "high_risk_command_open_to_observer",
    ]
    .into_iter()
    .collect();

    let maximally_violating = MobileSurface {
        capabilities: vec![
            MobileCapability::EmergencyMute,
            MobileCapability::KnowledgePatch,
        ],
        exposures: Exposure::ALL.to_vec(),
        route: ConnectionRoute::Undecided,
        encryption: EncryptionClaim {
            relay_terminates_tls: true,
            claims_end_to_end: true,
            protocol_reviewed: false,
        },
        preview: PreviewPolicy {
            enabled: true,
            on_demand: false,
            truly_encoded_low_bitrate: false,
            releases_encoder_and_bandwidth_on_close: false,
            may_preempt_official_output: true,
        },
        commands: vec![RemoteCommand {
            name: "risky".to_owned(),
            risk: CommandRisk::High,
            min_role: Role::Observer,
            carries_identity: false,
            carries_session_epoch: false,
            carries_sequence: false,
            carries_ttl: false,
            returns_actual_ack: false,
        }],
        emergency_mute_reachable: false,
        enable_status_provided: false,
        knowledge_patch_routes_to_review: false,
    };

    let mut route_a = clean_surface();
    route_a.route = ConnectionRoute::RelayPwa {
        https_domain: false,
        certificate: false,
    };
    let mut route_b = clean_surface();
    route_b.route = ConnectionRoute::NativeShellLan {
        app_distribution: false,
        tauri_maturity_verified: false,
    };

    let mut found: BTreeSet<&str> = BTreeSet::new();
    for surface in [maximally_violating, route_a, route_b] {
        for finding in audit_surface(&surface) {
            found.insert(finding.token);
        }
    }
    assert_eq!(found, expected);
}

#[test]
fn a_clean_surface_audits_empty() {
    assert!(audit_surface(&clean_surface()).is_empty());
}
