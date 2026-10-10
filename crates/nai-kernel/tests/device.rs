//! §34.4 / §1469 / §35.3 / INV-23 / §8.5 device admission (registry O04).
use nai_kernel::device::{
    DeviceAction, DeviceChannel, DeviceDeclaration, DeviceEvent, DeviceGrant, DeviceHost,
    DeviceHostPlacement, DeviceUse, EventDisposition, HostReadiness, RiskClass, ValueRange,
    audit_host,
};
use nai_kernel::golden::Finding;
use std::collections::BTreeSet;

fn governed_action(name: &str) -> DeviceAction {
    DeviceAction::new(name)
        .with_range(ValueRange::new(0, 1_000))
        .with_rate_limit(60)
}

/// A read-only sensor: no actuation, so no interlock or local override is owed.
fn sense_declaration(device: &str) -> DeviceDeclaration {
    DeviceDeclaration {
        device_ref: device.to_owned(),
        channel: DeviceChannel::Sense,
        physical_execution: false,
        enabled_by_default: false,
        local_confirmation: false,
        independent_switch: false,
        actions: vec![governed_action("read")],
        safety_interlock: None,
        local_manual_override: false,
        declared_risk: RiskClass::Risk1,
    }
}

/// An actuating device with the interlock and the local override it owes.
fn control_declaration(device: &str) -> DeviceDeclaration {
    DeviceDeclaration {
        device_ref: device.to_owned(),
        channel: DeviceChannel::Control,
        physical_execution: false,
        enabled_by_default: false,
        local_confirmation: false,
        independent_switch: false,
        actions: vec![governed_action("set_level")],
        safety_interlock: Some("interlock-a".to_owned()),
        local_manual_override: true,
        declared_risk: RiskClass::Risk2,
    }
}

/// A physical execution device with every governance flag it owes.
fn physical_declaration(device: &str) -> DeviceDeclaration {
    DeviceDeclaration {
        device_ref: device.to_owned(),
        channel: DeviceChannel::Control,
        physical_execution: true,
        enabled_by_default: false,
        local_confirmation: true,
        independent_switch: true,
        actions: vec![governed_action("move")],
        safety_interlock: Some("interlock-b".to_owned()),
        local_manual_override: true,
        declared_risk: RiskClass::Risk3,
    }
}

fn resolved_events() -> Vec<EventDisposition> {
    DeviceEvent::ALL
        .into_iter()
        .map(EventDisposition::resolved)
        .collect()
}

/// The honest baseline: one governed sensor and one governed actuator, every
/// event class dispositioned and logged, drivers in a subprocess, and no use
/// beyond what is declared and authorized.
fn clean_host() -> DeviceHost {
    DeviceHost {
        declarations: vec![sense_declaration("mic"), control_declaration("light")],
        granted: vec![DeviceGrant::new("light", vec!["set_level".to_owned()])],
        used: vec![DeviceUse::new("light", "set_level")],
        events: resolved_events(),
        placement: DeviceHostPlacement::Subprocess,
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

// ------------------------------------------------------- declaration / actions

#[test]
fn no_declared_actions_is_reported() {
    let mut host = clean_host();
    host.declarations = vec![DeviceDeclaration {
        actions: Vec::new(),
        ..control_declaration("arm")
    }];
    let findings = audit_host(&host);
    assert!(has(&findings, "no_declared_actions", "arm"));
}

#[test]
fn an_action_without_a_value_range_is_reported_and_named() {
    let mut host = clean_host();
    host.declarations = vec![DeviceDeclaration {
        actions: vec![DeviceAction::new("read").with_rate_limit(60)],
        ..sense_declaration("mic")
    }];
    let findings = audit_host(&host);
    assert!(has(&findings, "action_without_value_range", "mic/read"));
    assert!(!has(&findings, "action_without_rate_limit", "mic/read"));
}

#[test]
fn an_action_without_a_rate_limit_is_reported_and_named() {
    let mut host = clean_host();
    host.declarations = vec![DeviceDeclaration {
        actions: vec![DeviceAction::new("read").with_range(ValueRange::new(0, 10))],
        ..sense_declaration("mic")
    }];
    let findings = audit_host(&host);
    assert!(has(&findings, "action_without_rate_limit", "mic/read"));
    assert!(!has(&findings, "action_without_value_range", "mic/read"));
}

#[test]
fn a_fully_declared_action_is_not_reported() {
    let host = clean_host();
    let findings = audit_host(&host);
    assert!(tokens(&findings).is_empty());
    assert!(ValueRange::new(0, 1_000).is_well_formed());
    assert!(!ValueRange::new(1_000, 0).is_well_formed());
}

// ---------------------------------------------------- interlock / local override

#[test]
fn control_without_an_interlock_or_a_local_override_is_reported() {
    let mut host = clean_host();
    host.declarations = vec![DeviceDeclaration {
        safety_interlock: None,
        local_manual_override: false,
        ..control_declaration("light")
    }];
    let findings = audit_host(&host);
    assert!(has(&findings, "control_without_safety_interlock", "light"));
    assert!(has(&findings, "control_without_local_override", "light"));
}

#[test]
fn a_pure_sense_declaration_is_not_reported_for_actuation_governance() {
    // §1469/§1497's interlock and local override interlock an *action* and
    // cover an *execution*; a read-only sensor has neither. "Not applicable"
    // is not "non-compliant".
    let mut host = clean_host();
    host.declarations = vec![sense_declaration("mic")];
    host.granted = Vec::new();
    host.used = Vec::new();
    let findings = audit_host(&host);
    assert!(tokens(&findings).is_empty());
    assert!(!has(&findings, "control_without_safety_interlock", "mic"));
    assert!(!has(&findings, "control_without_local_override", "mic"));
}

#[test]
fn a_control_declaration_with_both_is_clean() {
    let mut host = clean_host();
    host.declarations = vec![control_declaration("light")];
    let findings = audit_host(&host);
    assert!(tokens(&findings).is_empty());
}

// ------------------------------------------------------------ physical execution

#[test]
fn physical_execution_enabled_by_default_is_reported() {
    let mut host = clean_host();
    host.declarations = vec![DeviceDeclaration {
        enabled_by_default: true,
        ..physical_declaration("arm")
    }];
    let findings = audit_host(&host);
    assert!(has(&findings, "physical_enabled_by_default", "arm"));
}

#[test]
fn physical_execution_without_a_local_confirmation_is_reported() {
    let mut host = clean_host();
    host.declarations = vec![DeviceDeclaration {
        local_confirmation: false,
        ..physical_declaration("arm")
    }];
    let findings = audit_host(&host);
    assert!(has(&findings, "physical_without_local_confirmation", "arm"));
    assert!(!has(
        &findings,
        "physical_without_independent_switch",
        "arm"
    ));
}

#[test]
fn physical_execution_without_an_independent_switch_is_reported() {
    let mut host = clean_host();
    host.declarations = vec![DeviceDeclaration {
        independent_switch: false,
        ..physical_declaration("arm")
    }];
    let findings = audit_host(&host);
    assert!(has(&findings, "physical_without_independent_switch", "arm"));
    assert!(!has(
        &findings,
        "physical_without_local_confirmation",
        "arm"
    ));
}

#[test]
fn a_sense_channel_claiming_physical_execution_is_reported() {
    let mut host = clean_host();
    host.declarations = vec![DeviceDeclaration {
        physical_execution: true,
        local_confirmation: true,
        independent_switch: true,
        declared_risk: RiskClass::Risk3,
        ..sense_declaration("mic")
    }];
    let findings = audit_host(&host);
    assert!(has(
        &findings,
        "sense_channel_with_physical_execution",
        "mic"
    ));
}

#[test]
fn a_fully_governed_physical_device_is_clean() {
    let mut host = clean_host();
    host.declarations = vec![physical_declaration("arm")];
    host.granted = Vec::new();
    host.used = Vec::new();
    let findings = audit_host(&host);
    assert!(tokens(&findings).is_empty());
}

// ------------------------------------------------------------------- risk class

#[test]
fn derived_risk_follows_channel_and_physical_execution() {
    assert_eq!(sense_declaration("mic").derived_risk(), RiskClass::Risk1);
    assert_eq!(
        control_declaration("light").derived_risk(),
        RiskClass::Risk2
    );
    // §1469: physical execution is risk_3 regardless of the declared channel.
    assert_eq!(physical_declaration("arm").derived_risk(), RiskClass::Risk3);
    let physical_sense = DeviceDeclaration {
        physical_execution: true,
        ..sense_declaration("mic")
    };
    assert_eq!(physical_sense.derived_risk(), RiskClass::Risk3);
}

#[test]
fn a_declared_risk_below_the_derived_risk_is_reported_and_effective_risk_is_max() {
    // §35.3: a physical execution device declared risk_2 derives risk_3.
    let understated = DeviceDeclaration {
        declared_risk: RiskClass::Risk2,
        ..physical_declaration("arm")
    };
    assert_eq!(understated.effective_risk(), RiskClass::Risk3);
    let mut host = clean_host();
    host.declarations = vec![understated];
    host.granted = Vec::new();
    host.used = Vec::new();
    let findings = audit_host(&host);
    assert!(has(&findings, "declared_risk_below_derived", "arm"));
}

#[test]
fn a_declared_risk_above_the_derived_risk_is_accepted() {
    // §35.3: a declaration may raise its own risk; max keeps the higher one.
    let conservative = DeviceDeclaration {
        declared_risk: RiskClass::Risk3,
        ..control_declaration("light")
    };
    assert_eq!(conservative.effective_risk(), RiskClass::Risk3);
    let mut host = clean_host();
    host.declarations = vec![conservative];
    let findings = audit_host(&host);
    assert!(!has(&findings, "declared_risk_below_derived", "light"));
}

// ------------------------------------------------------------------- placement

#[test]
fn a_driver_in_the_kernel_process_is_reported() {
    let mut host = clean_host();
    host.placement = DeviceHostPlacement::KernelProcess;
    let findings = audit_host(&host);
    assert!(has(
        &findings,
        "device_driver_in_kernel_process",
        "placement"
    ));
}

#[test]
fn a_subprocess_or_edge_node_placement_is_not_reported() {
    for placement in [
        DeviceHostPlacement::Subprocess,
        DeviceHostPlacement::EdgeNode,
    ] {
        let mut host = clean_host();
        host.placement = placement;
        let findings = audit_host(&host);
        assert!(!has(
            &findings,
            "device_driver_in_kernel_process",
            "placement"
        ));
    }
}

// ------------------------------------------------------- events / decision log

#[test]
fn an_undisposed_event_class_is_reported_and_named() {
    let mut host = clean_host();
    host.events = resolved_events()
        .into_iter()
        .filter(|disposition| disposition.event != DeviceEvent::Timeout)
        .collect();
    let findings = audit_host(&host);
    assert!(has(&findings, "device_event_undisposed", "timeout"));
}

#[test]
fn an_unhandled_disposition_is_reported() {
    let mut host = clean_host();
    host.events = DeviceEvent::ALL
        .into_iter()
        .map(|event| EventDisposition {
            handled: event != DeviceEvent::Hotplug,
            ..EventDisposition::resolved(event)
        })
        .collect();
    let findings = audit_host(&host);
    assert!(has(&findings, "device_event_undisposed", "hotplug"));
}

#[test]
fn an_event_not_written_to_the_decision_log_is_reported_and_named() {
    let mut host = clean_host();
    host.events = DeviceEvent::ALL
        .into_iter()
        .map(|event| EventDisposition {
            logged: event != DeviceEvent::Disconnect,
            ..EventDisposition::resolved(event)
        })
        .collect();
    let findings = audit_host(&host);
    assert!(has(&findings, "device_event_not_logged", "disconnect"));
}

#[test]
fn a_duplicate_device_opening_a_second_capture_path_is_reported() {
    // §8.5 / O04: 音频设备变更不重复收音.
    let mut host = clean_host();
    host.events = DeviceEvent::ALL
        .into_iter()
        .map(|event| EventDisposition {
            keeps_single_capture_path: event != DeviceEvent::Duplicate,
            ..EventDisposition::resolved(event)
        })
        .collect();
    let findings = audit_host(&host);
    assert!(has(
        &findings,
        "duplicate_event_opens_second_capture_path",
        "duplicate"
    ));
}

#[test]
fn a_duplicate_device_keeping_a_single_capture_path_is_not_reported() {
    let host = clean_host();
    let findings = audit_host(&host);
    assert!(!has(
        &findings,
        "duplicate_event_opens_second_capture_path",
        "duplicate"
    ));
}

// ------------------------------------------------------- use vs declaration

#[test]
fn using_an_undeclared_device_action_is_reported_and_named() {
    // INV-23: the actual permission never exceeds the declared list.
    let mut host = clean_host();
    host.used = vec![DeviceUse::new("mic", "write")];
    let findings = audit_host(&host);
    assert!(has(&findings, "used_undeclared_device_action", "mic/write"));
}

#[test]
fn using_an_action_beyond_authorization_is_reported_and_named() {
    let mut host = clean_host();
    host.granted = Vec::new();
    host.used = vec![DeviceUse::new("light", "set_level")];
    let findings = audit_host(&host);
    assert!(has(
        &findings,
        "used_beyond_authorization",
        "light/set_level"
    ));
}

#[test]
fn an_undeclared_and_unauthorized_use_is_reported_once() {
    // A use that is not declared is reported once — not also as "beyond
    // authorization".
    let mut host = clean_host();
    host.granted = Vec::new();
    host.used = vec![DeviceUse::new("ghost", "open")];
    let findings = audit_host(&host);
    assert!(has(
        &findings,
        "used_undeclared_device_action",
        "ghost/open"
    ));
    assert!(!has(&findings, "used_beyond_authorization", "ghost/open"));
}

#[test]
fn a_use_within_declaration_and_authorization_is_not_reported() {
    let host = clean_host();
    let findings = audit_host(&host);
    assert!(!has(
        &findings,
        "used_undeclared_device_action",
        "light/set_level"
    ));
    assert!(!has(
        &findings,
        "used_beyond_authorization",
        "light/set_level"
    ));
}

// ------------------------------------------------------------------- readiness

#[test]
fn a_host_with_no_declaration_is_blocked_and_audits_empty() {
    let host = DeviceHost::empty();
    assert!(tokens(&audit_host(&host)).is_empty());
    assert_eq!(host.readiness(), HostReadiness::Blocked);
}

#[test]
fn a_defect_outranks_the_undeclared_host() {
    let mut host = DeviceHost::empty();
    host.placement = DeviceHostPlacement::KernelProcess;
    assert_eq!(host.readiness(), HostReadiness::Defective);
}

#[test]
fn a_declared_host_with_no_violation_is_ready() {
    let host = clean_host();
    assert_eq!(host.readiness(), HostReadiness::Ready);
    assert!(host.declaration("light").is_some());
    assert!(host.declaration("ghost").is_none());
}

// ------------------------------------------------------------------ audit purity

#[test]
fn a_clean_host_audits_empty() {
    let findings = audit_host(&clean_host());
    assert!(
        findings.is_empty(),
        "unexpected findings: {:?}",
        tokens(&findings)
    );
}

#[test]
fn every_finding_token_is_constructible() {
    let expected: BTreeSet<&str> = [
        "device_driver_in_kernel_process",
        "no_declared_actions",
        "action_without_value_range",
        "action_without_rate_limit",
        "control_without_safety_interlock",
        "control_without_local_override",
        "declared_risk_below_derived",
        "sense_channel_with_physical_execution",
        "physical_enabled_by_default",
        "physical_without_local_confirmation",
        "physical_without_independent_switch",
        "device_event_undisposed",
        "device_event_not_logged",
        "duplicate_event_opens_second_capture_path",
        "used_undeclared_device_action",
        "used_beyond_authorization",
    ]
    .into_iter()
    .collect();

    // One maximally violating host that exercises every rule at once.
    let mut host = DeviceHost::empty();
    host.placement = DeviceHostPlacement::KernelProcess;
    host.declarations = vec![
        // A sensor claiming physical execution, with an ungoverned action.
        DeviceDeclaration {
            physical_execution: true,
            enabled_by_default: true,
            local_confirmation: false,
            independent_switch: false,
            actions: vec![DeviceAction::new("read")],
            declared_risk: RiskClass::Risk1,
            ..sense_declaration("cam")
        },
        // An actuator with no actions, no interlock and no local override.
        DeviceDeclaration {
            actions: Vec::new(),
            safety_interlock: None,
            local_manual_override: false,
            ..control_declaration("arm")
        },
    ];
    host.events = vec![
        EventDisposition {
            logged: false,
            ..EventDisposition::resolved(DeviceEvent::Hotplug)
        },
        EventDisposition {
            handled: false,
            ..EventDisposition::resolved(DeviceEvent::Disconnect)
        },
        EventDisposition {
            keeps_single_capture_path: false,
            ..EventDisposition::resolved(DeviceEvent::Duplicate)
        },
        // Timeout is absent entirely.
    ];
    host.granted = Vec::new();
    host.used = vec![DeviceUse::new("arm", "fire"), DeviceUse::new("cam", "read")];

    let actual: BTreeSet<&str> = tokens(&audit_host(&host)).into_iter().collect();
    assert_eq!(actual, expected);
}
