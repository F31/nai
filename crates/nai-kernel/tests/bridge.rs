//! §27.1 / §347 / §349 / §119 / §1328 platform-bridge lifecycle (registry P01).
use nai_kernel::bridge::{
    BridgeCondition, BridgeDescriptor, BridgeReadiness, BridgeScope, BridgeTransition,
    BroadcastAuthority, CallbackPolicy, CapabilityGrant, CapabilityState, ConditionDisposition,
    CurrentConnection, Degradation, Feature, SdkAvailability, SdkTrust, TransitionDisposition,
    audit_bridge,
};
use nai_kernel::golden::Finding;
use std::collections::BTreeSet;

/// Every lifecycle transition disposed, clearing the authority and logging.
fn resolved_transitions() -> Vec<TransitionDisposition> {
    BridgeTransition::ALL
        .into_iter()
        .map(TransitionDisposition::resolved)
        .collect()
}

/// Every condition class of §347 disposed and logged.
fn resolved_conditions() -> Vec<ConditionDisposition> {
    BridgeCondition::ALL
        .into_iter()
        .map(ConditionDisposition::resolved)
        .collect()
}

/// A fully declared bridge with nothing violated: reads as Ready.
fn sound_bridge() -> BridgeDescriptor {
    BridgeDescriptor {
        scope: BridgeScope::complete(),
        transitions: resolved_transitions(),
        connection: CurrentConnection::new("room-1", 4),
        authority: Some(BroadcastAuthority::new("room-1", 4)),
        capabilities: vec![CapabilityGrant::new(
            "platform.events@1",
            CapabilityState::Granted,
        )],
        features: vec![Feature::active("auto_reply", "platform.events@1")],
        degradations: Vec::new(),
        conditions: resolved_conditions(),
        sdk_trust: SdkTrust::complete(),
        sdk_availability: SdkAvailability::Verified,
        uses_fake_connector_claimed_real: false,
        callback: CallbackPolicy::decoupled(),
    }
}

/// Audit `bridge` and collect its finding tokens.
fn tokens(bridge: &BridgeDescriptor) -> Vec<&'static str> {
    audit_bridge(bridge).into_iter().map(|f| f.token).collect()
}

fn has(bridge: &BridgeDescriptor, token: &str) -> bool {
    tokens(bridge).contains(&token)
}

fn subjects_of(bridge: &BridgeDescriptor, token: &str) -> Vec<String> {
    audit_bridge(bridge)
        .into_iter()
        .filter(|f| f.token == token)
        .map(|f| f.subject)
        .collect()
}

// ---------------------------------------------------------------------------
// §27.1 the bridge only does four things
// ---------------------------------------------------------------------------

#[test]
fn the_four_declared_duties_are_not_reported() {
    let bridge = sound_bridge();
    assert!(!has(&bridge, "duty_outside_bridge_scope"));
    assert_eq!(BridgeScope::complete().duties.len(), 4);
}

#[test]
fn a_duty_outside_the_bridge_is_named() {
    let mut bridge = sound_bridge();
    bridge.scope.extra_duties = vec!["compose_reply".to_owned()];
    assert_eq!(
        subjects_of(&bridge, "duty_outside_bridge_scope"),
        vec!["compose_reply".to_owned()]
    );
}

// ---------------------------------------------------------------------------
// §27.1 / §1075 SDK handshake
// ---------------------------------------------------------------------------

#[test]
fn a_verified_sdk_without_every_handshake_check_is_reported() {
    let mut bridge = sound_bridge();
    bridge.sdk_trust = SdkTrust::default();
    let mut subjects = subjects_of(&bridge, "sdk_loaded_without_trust_check");
    subjects.sort();
    assert_eq!(
        subjects,
        vec!["nonce", "path_pinned", "signature", "token"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_verified_sdk_with_every_check_is_not_reported() {
    let bridge = sound_bridge();
    assert!(bridge.sdk_trust.is_complete());
    assert!(!has(&bridge, "sdk_loaded_without_trust_check"));
}

#[test]
fn an_unverified_sdk_is_not_judged_for_handshake_checks() {
    let mut bridge = sound_bridge();
    bridge.sdk_availability = SdkAvailability::UnverifiedExternalDependency;
    bridge.sdk_trust = SdkTrust::default();
    assert!(!has(&bridge, "sdk_loaded_without_trust_check"));
}

#[test]
fn a_fake_connector_claimed_real_is_reported() {
    let mut bridge = sound_bridge();
    bridge.sdk_availability = SdkAvailability::UnverifiedExternalDependency;
    bridge.uses_fake_connector_claimed_real = true;
    assert!(has(&bridge, "fake_connector_claimed_as_real_capability"));
}

#[test]
fn an_honest_unverified_dependency_is_not_reported() {
    let mut bridge = sound_bridge();
    bridge.sdk_availability = SdkAvailability::UnverifiedExternalDependency;
    bridge.uses_fake_connector_claimed_real = false;
    assert!(!has(&bridge, "fake_connector_claimed_as_real_capability"));
}

// ---------------------------------------------------------------------------
// §27.1 callback policy
// ---------------------------------------------------------------------------

#[test]
fn a_callback_that_awaits_the_llm_or_disk_is_reported() {
    let mut bridge = sound_bridge();
    bridge.callback.awaits_llm_or_disk = true;
    assert!(has(&bridge, "callback_blocks_on_llm_or_disk"));
}

#[test]
fn a_callback_that_only_enqueues_is_not_reported() {
    let bridge = sound_bridge();
    assert!(!has(&bridge, "callback_blocks_on_llm_or_disk"));
}

// ---------------------------------------------------------------------------
// §1328 lifecycle transitions
// ---------------------------------------------------------------------------

#[test]
fn every_transition_class_is_disposed() {
    let bridge = sound_bridge();
    assert!(!has(&bridge, "transition_undisposed"));
    assert_eq!(resolved_transitions().len(), 4);
}

#[test]
fn a_missing_transition_class_is_named() {
    let mut bridge = sound_bridge();
    bridge
        .transitions
        .retain(|d| d.transition != BridgeTransition::RoomSwitched);
    assert_eq!(
        subjects_of(&bridge, "transition_undisposed"),
        vec!["room_switched".to_owned()]
    );
}

#[test]
fn a_transition_that_keeps_the_broadcast_authority_is_reported() {
    let mut bridge = sound_bridge();
    for disposition in &mut bridge.transitions {
        if disposition.transition == BridgeTransition::Reconnected {
            disposition.clears_broadcast_authority = false;
        }
    }
    assert_eq!(
        subjects_of(&bridge, "transition_keeps_stale_broadcast"),
        vec!["reconnected".to_owned()]
    );
}

#[test]
fn a_transition_that_is_not_logged_is_reported() {
    let mut bridge = sound_bridge();
    for disposition in &mut bridge.transitions {
        if disposition.transition == BridgeTransition::Exited {
            disposition.logged = false;
        }
    }
    assert_eq!(
        subjects_of(&bridge, "transition_not_logged"),
        vec!["exited".to_owned()]
    );
}

#[test]
fn no_bridge_declared_means_no_transition_defects() {
    // An undeclared bridge has no transitions to dispose of; it must not be
    // read as four defects.
    let bridge = BridgeDescriptor::empty();
    assert!(!has(&bridge, "transition_undisposed"));
    assert!(!has(&bridge, "transition_keeps_stale_broadcast"));
    assert!(!has(&bridge, "transition_not_logged"));
    assert!(audit_bridge(&bridge).is_empty());
}

#[test]
fn a_reconnect_that_restores_the_old_auto_authority_is_reported() {
    let mut bridge = sound_bridge();
    for disposition in &mut bridge.transitions {
        if disposition.transition == BridgeTransition::Reconnected {
            disposition.restores_previous_auto_authority = true;
        }
    }
    assert_eq!(
        subjects_of(&bridge, "reconnect_restores_previous_auto_authority"),
        vec!["reconnected".to_owned()]
    );
}

// ---------------------------------------------------------------------------
// §1328 stale-room broadcast suppression
// ---------------------------------------------------------------------------

#[test]
fn a_stale_room_authority_is_reported_and_never_broadcasts() {
    let mut bridge = sound_bridge();
    bridge.connection = CurrentConnection::new("room-2", 5);
    bridge.authority = Some(BroadcastAuthority::new("room-1", 4));
    assert_eq!(
        subjects_of(&bridge, "stale_room_broadcast_permitted"),
        vec!["room-1".to_owned()]
    );
    assert!(!bridge.may_broadcast());
}

#[test]
fn an_authority_for_the_current_room_but_an_older_generation_is_stale() {
    let mut bridge = sound_bridge();
    bridge.connection = CurrentConnection::new("room-1", 9);
    bridge.authority = Some(BroadcastAuthority::new("room-1", 8));
    assert!(has(&bridge, "stale_room_broadcast_permitted"));
    assert!(!bridge.may_broadcast());
}

#[test]
fn an_authority_covering_the_current_connection_may_broadcast() {
    let bridge = sound_bridge();
    assert!(!has(&bridge, "stale_room_broadcast_permitted"));
    assert!(bridge.may_broadcast());
}

#[test]
fn no_authority_never_broadcasts() {
    let mut bridge = sound_bridge();
    bridge.authority = None;
    assert!(!bridge.may_broadcast());
    assert!(!has(&bridge, "stale_room_broadcast_permitted"));
}

// ---------------------------------------------------------------------------
// §347 exit leaves the engine in standby
// ---------------------------------------------------------------------------

#[test]
fn an_exit_that_does_not_leave_the_engine_in_standby_is_reported() {
    let mut bridge = sound_bridge();
    for disposition in &mut bridge.transitions {
        if disposition.transition == BridgeTransition::Exited {
            disposition.leaves_engine_in_standby = false;
        }
    }
    assert_eq!(
        subjects_of(&bridge, "exit_without_standby"),
        vec!["exited".to_owned()]
    );
}

#[test]
fn an_exit_that_leaves_the_engine_in_standby_is_not_reported() {
    let bridge = sound_bridge();
    assert!(!has(&bridge, "exit_without_standby"));
}

#[test]
fn an_undisposed_exit_is_only_reported_as_undisposed() {
    // A missing exit disposition is one defect, not two.
    let mut bridge = sound_bridge();
    bridge
        .transitions
        .retain(|d| d.transition != BridgeTransition::Exited);
    assert!(has(&bridge, "transition_undisposed"));
    assert!(!has(&bridge, "exit_without_standby"));
}

// ---------------------------------------------------------------------------
// §349 capability snapshot and feature switches
// ---------------------------------------------------------------------------

#[test]
fn an_unknown_capability_read_as_granted_is_reported() {
    let mut bridge = sound_bridge();
    bridge.capabilities = vec![CapabilityGrant {
        capability: "data.query@1".to_owned(),
        state: CapabilityState::Unknown,
        read_as_granted: true,
    }];
    assert_eq!(
        subjects_of(&bridge, "unknown_capability_treated_as_granted"),
        vec!["data.query@1".to_owned()]
    );
}

#[test]
fn an_unknown_capability_read_strictly_is_not_reported() {
    let mut bridge = sound_bridge();
    bridge.capabilities = vec![CapabilityGrant::new(
        "data.query@1",
        CapabilityState::Unknown,
    )];
    assert!(!has(&bridge, "unknown_capability_treated_as_granted"));
}

#[test]
fn a_feature_active_without_its_capability_is_reported() {
    for state in [CapabilityState::Denied, CapabilityState::Unknown] {
        let mut bridge = sound_bridge();
        bridge.capabilities = vec![CapabilityGrant::new("data.query@1", state)];
        bridge.features = vec![Feature::active("live_query", "data.query@1")];
        assert_eq!(
            subjects_of(&bridge, "feature_active_without_capability"),
            vec!["live_query".to_owned()],
            "state {state:?} is not Granted, so the feature must be closed"
        );
    }
}

#[test]
fn a_feature_stopped_despite_a_granted_capability_is_reported() {
    let mut bridge = sound_bridge();
    bridge.features = vec![Feature::stopped_by_capability_change(
        "live_query",
        "platform.events@1",
    )];
    assert_eq!(
        subjects_of(&bridge, "feature_stopped_despite_capability"),
        vec!["live_query".to_owned()]
    );
}

#[test]
fn a_feature_open_on_a_granted_capability_is_not_reported() {
    let bridge = sound_bridge();
    assert!(!has(&bridge, "feature_active_without_capability"));
    assert!(!has(&bridge, "feature_stopped_despite_capability"));
}

#[test]
fn a_feature_stopped_by_an_operator_is_not_reported() {
    // A feature a user closed by hand was not closed *by* a capability change,
    // so it never conflicts with "close only the matching feature".
    let mut bridge = sound_bridge();
    bridge.features = vec![Feature {
        name: "live_query".to_owned(),
        requires: "platform.events@1".to_owned(),
        active: false,
        stopped_by_capability_change: false,
    }];
    assert!(!has(&bridge, "feature_stopped_despite_capability"));
}

// ---------------------------------------------------------------------------
// §119 platform interruption
// ---------------------------------------------------------------------------

#[test]
fn an_interruption_that_keeps_auto_replies_is_reported() {
    let mut bridge = sound_bridge();
    let mut degradation = Degradation::interruption("sdk_lost");
    degradation.auto_reply_revoked = false;
    bridge.degradations = vec![degradation];
    assert_eq!(
        subjects_of(&bridge, "platform_interruption_keeps_auto_reply"),
        vec!["sdk_lost".to_owned()]
    );
}

#[test]
fn an_interruption_that_disables_manual_or_replay_is_reported() {
    let mut bridge = sound_bridge();
    let mut degradation = Degradation::interruption("sdk_lost");
    degradation.manual_available = false;
    degradation.replay_available = false;
    bridge.degradations = vec![degradation];
    assert_eq!(
        subjects_of(&bridge, "platform_interruption_disables_manual"),
        vec!["sdk_lost".to_owned()]
    );
    assert_eq!(
        subjects_of(&bridge, "platform_interruption_disables_replay"),
        vec!["sdk_lost".to_owned()]
    );
}

#[test]
fn an_interruption_that_is_not_logged_is_reported() {
    let mut bridge = sound_bridge();
    let mut degradation = Degradation::interruption("sdk_lost");
    degradation.logged = false;
    bridge.degradations = vec![degradation];
    assert_eq!(
        subjects_of(&bridge, "degradation_not_logged"),
        vec!["sdk_lost".to_owned()]
    );
}

#[test]
fn a_sound_interruption_is_not_reported() {
    let mut bridge = sound_bridge();
    bridge.degradations = vec![Degradation::interruption("sdk_lost")];
    assert!(!has(&bridge, "platform_interruption_keeps_auto_reply"));
    assert!(!has(&bridge, "platform_interruption_disables_manual"));
    assert!(!has(&bridge, "platform_interruption_disables_replay"));
    assert!(!has(&bridge, "degradation_not_logged"));
}

#[test]
fn a_degradation_that_is_not_a_platform_interruption_is_not_reported() {
    // A Provider outage is not a platform-event interruption; it owes none of
    // §119's three-state rule.
    let mut bridge = sound_bridge();
    bridge.degradations = vec![Degradation {
        cause: "provider_unavailable".to_owned(),
        bridge_lost: false,
        auto_reply_revoked: false,
        manual_available: false,
        replay_available: false,
        logged: false,
    }];
    assert!(!has(&bridge, "platform_interruption_keeps_auto_reply"));
    assert!(!has(&bridge, "platform_interruption_disables_manual"));
    assert!(!has(&bridge, "platform_interruption_disables_replay"));
    assert!(!has(&bridge, "degradation_not_logged"));
}

// ---------------------------------------------------------------------------
// §347 the seven condition classes
// ---------------------------------------------------------------------------

#[test]
fn every_condition_class_is_disposed() {
    let bridge = sound_bridge();
    assert!(!has(&bridge, "condition_undisposed"));
    assert!(!has(&bridge, "condition_not_logged"));
    assert_eq!(resolved_conditions().len(), 7);
}

#[test]
fn a_missing_condition_class_is_named() {
    let mut bridge = sound_bridge();
    bridge
        .conditions
        .retain(|d| d.condition != BridgeCondition::TimeDrift);
    assert_eq!(
        subjects_of(&bridge, "condition_undisposed"),
        vec!["time_drift".to_owned()]
    );
}

#[test]
fn an_unhandled_condition_class_is_reported() {
    let mut bridge = sound_bridge();
    for disposition in &mut bridge.conditions {
        if disposition.condition == BridgeCondition::PortConflict {
            disposition.handled = false;
        }
    }
    assert_eq!(
        subjects_of(&bridge, "condition_undisposed"),
        vec!["port_conflict".to_owned()]
    );
}

#[test]
fn a_condition_disposition_that_is_not_logged_is_reported() {
    let mut bridge = sound_bridge();
    for disposition in &mut bridge.conditions {
        if disposition.condition == BridgeCondition::PermissionRevoked {
            disposition.logged = false;
        }
    }
    assert_eq!(
        subjects_of(&bridge, "condition_not_logged"),
        vec!["permission_revoked".to_owned()]
    );
}

#[test]
fn no_bridge_declared_means_no_condition_defects() {
    let bridge = BridgeDescriptor::empty();
    assert!(!has(&bridge, "condition_undisposed"));
    assert!(!has(&bridge, "condition_not_logged"));
}

// ---------------------------------------------------------------------------
// readiness
// ---------------------------------------------------------------------------

#[test]
fn an_undeclared_bridge_is_blocked_and_audits_empty() {
    let bridge = BridgeDescriptor::empty();
    assert!(audit_bridge(&bridge).is_empty());
    assert_eq!(bridge.readiness(), BridgeReadiness::Blocked);
}

#[test]
fn a_defect_outranks_the_undeclared_bridge() {
    // No bridge is declared, but a grant for a room already left is still in
    // force: the honest "not implemented yet" must not hide it.
    let mut bridge = BridgeDescriptor::empty();
    bridge.authority = Some(BroadcastAuthority::new("room-1", 1));
    assert!(has(&bridge, "stale_room_broadcast_permitted"));
    assert_eq!(bridge.readiness(), BridgeReadiness::Defective);
}

#[test]
fn a_second_defect_outranks_the_undeclared_bridge() {
    // Independently of the lifecycle gate: a forged substitute is a defect
    // even before any bridge is declared.
    let mut bridge = BridgeDescriptor::empty();
    bridge.uses_fake_connector_claimed_real = true;
    assert!(has(&bridge, "fake_connector_claimed_as_real_capability"));
    assert_eq!(bridge.readiness(), BridgeReadiness::Defective);
}

#[test]
fn a_declared_bridge_without_violations_is_ready() {
    let bridge = sound_bridge();
    assert!(audit_bridge(&bridge).is_empty());
    assert_eq!(bridge.readiness(), BridgeReadiness::Ready);
}

// ---------------------------------------------------------------------------
// the audit is a pure function of the descriptor
// ---------------------------------------------------------------------------

#[test]
fn a_clean_descriptor_audits_empty() {
    let bridge = sound_bridge();
    assert_eq!(audit_bridge(&bridge), Vec::<Finding>::new());
}

#[test]
fn every_finding_token_is_constructible() {
    let expected: BTreeSet<&str> = [
        "duty_outside_bridge_scope",
        "sdk_loaded_without_trust_check",
        "fake_connector_claimed_as_real_capability",
        "callback_blocks_on_llm_or_disk",
        "transition_undisposed",
        "transition_keeps_stale_broadcast",
        "transition_not_logged",
        "reconnect_restores_previous_auto_authority",
        "exit_without_standby",
        "condition_undisposed",
        "condition_not_logged",
        "platform_interruption_keeps_auto_reply",
        "platform_interruption_disables_manual",
        "platform_interruption_disables_replay",
        "degradation_not_logged",
        "stale_room_broadcast_permitted",
        "unknown_capability_treated_as_granted",
        "feature_active_without_capability",
        "feature_stopped_despite_capability",
    ]
    .into_iter()
    .collect();
    assert_eq!(expected.len(), 19);

    // One bridge that violates everything reachable through the verified-SDK
    // path, plus a second through the unverified-SDK path (the two are
    // mutually exclusive). Their union must be exactly the token set above:
    // every rule is constructible, and no unregistered token exists.
    let mut verified = BridgeDescriptor::empty();
    verified.scope = BridgeScope {
        extra_duties: vec!["compose_reply".to_owned()],
        ..BridgeScope::complete()
    };
    verified.sdk_availability = SdkAvailability::Verified;
    verified.sdk_trust = SdkTrust::default();
    verified.callback.awaits_llm_or_disk = true;
    verified.transitions = vec![
        // Missing the room-switch disposition entirely.
        TransitionDisposition {
            transition: BridgeTransition::Started,
            clears_broadcast_authority: false,
            logged: false,
            leaves_engine_in_standby: false,
            restores_previous_auto_authority: false,
        },
        TransitionDisposition {
            transition: BridgeTransition::Reconnected,
            clears_broadcast_authority: true,
            logged: true,
            leaves_engine_in_standby: false,
            restores_previous_auto_authority: true,
        },
        TransitionDisposition {
            transition: BridgeTransition::Exited,
            clears_broadcast_authority: true,
            logged: true,
            leaves_engine_in_standby: false,
            restores_previous_auto_authority: false,
        },
    ];
    verified.conditions = vec![
        ConditionDisposition {
            condition: BridgeCondition::MultipleStarts,
            handled: true,
            logged: false,
        },
        ConditionDisposition {
            condition: BridgeCondition::PortConflict,
            handled: false,
            logged: true,
        },
    ];
    verified.degradations = vec![Degradation {
        cause: "sdk_lost".to_owned(),
        bridge_lost: true,
        auto_reply_revoked: false,
        manual_available: false,
        replay_available: false,
        logged: false,
    }];
    verified.connection = CurrentConnection::new("room-2", 5);
    verified.authority = Some(BroadcastAuthority::new("room-1", 4));
    verified.capabilities = vec![
        CapabilityGrant {
            capability: "data.query@1".to_owned(),
            state: CapabilityState::Unknown,
            read_as_granted: true,
        },
        CapabilityGrant::new("auto_reply@1", CapabilityState::Denied),
        CapabilityGrant::new("show.plan@1", CapabilityState::Granted),
    ];
    verified.features = vec![
        Feature::active("live_query", "data.query@1"),
        Feature::stopped_by_capability_change("show_plan", "show.plan@1"),
    ];

    let mut unverified = BridgeDescriptor::empty();
    unverified.sdk_availability = SdkAvailability::UnverifiedExternalDependency;
    unverified.uses_fake_connector_claimed_real = true;

    let mut union: BTreeSet<&str> = BTreeSet::new();
    for bridge in [&verified, &unverified] {
        for finding in audit_bridge(bridge) {
            union.insert(finding.token);
        }
    }

    assert_eq!(union, expected);
}
