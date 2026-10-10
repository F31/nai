//! Cloud disconnect, local autonomy and low-cloud capacity (A10).
use nai_contracts::v1::ErrorCode;
use nai_upgrade_core::{
    Availability, CapabilityGate, CapabilityMode, Capacity, CapacityBudget, CapacityVerdict,
    CloudLink, LinkGraph, LowCloudProfile, NodeClass, Sizing, ThrottlePolicy, assess,
    audit_local_autonomy, audit_low_cloud, availability,
};

/// The nodes A10 names directly: the local real-time path and the local mute.
const LOCAL_CRITICAL: &[&str] = &["session.control", "media.clock", "media.local_mute"];

fn local_graph() -> LinkGraph {
    let mut graph = LinkGraph::new();
    graph.add_local("session.control").unwrap();
    graph.add_local("media.clock").unwrap();
    graph.add_local("media.local_mute").unwrap();
    graph.add_cloud("cloud.gateway").unwrap();
    graph.add_cloud("cloud.tts").unwrap();
    graph.depends_on("media.local_mute", "media.clock").unwrap();
    graph.depends_on("media.clock", "session.control").unwrap();
    graph
}

// --- local critical path -----------------------------------------------------

#[test]
fn local_critical_path_without_cloud_is_local() {
    let graph = local_graph();
    let audit = audit_local_autonomy(&graph, LOCAL_CRITICAL);
    assert!(
        audit.is_local(),
        "unexpected violations: {:?}",
        audit.violations
    );
    assert_eq!(audit.critical.len(), LOCAL_CRITICAL.len());
    assert!(audit.critical.iter().all(|(_, reach)| reach.is_empty()));
}

#[test]
fn direct_cloud_dependency_on_critical_node_is_a_violation() {
    let mut graph = local_graph();
    graph
        .depends_on("media.local_mute", "cloud.gateway")
        .unwrap();
    let audit = audit_local_autonomy(&graph, LOCAL_CRITICAL);
    assert!(!audit.is_local());
    assert!(audit.violations.iter().any(|v| {
        v.token == "local critical node depends on cloud"
            && v.subject.starts_with("media.local_mute")
    }));
}

#[test]
fn transitive_cloud_dependency_is_caught() {
    // media.local_mute -> media.clock -> cloud.tts: two hops away, still a bug.
    let mut graph = local_graph();
    graph.depends_on("media.clock", "cloud.tts").unwrap();
    let audit = audit_local_autonomy(&graph, LOCAL_CRITICAL);
    assert!(audit.violations.iter().any(|v| {
        v.token == "local critical node depends on cloud" && v.subject.starts_with("media.clock")
    }));
    // The mute reaches the cloud only through the clock, and that shows up too.
    assert!(
        audit
            .violations
            .iter()
            .any(|v| v.subject.starts_with("media.local_mute"))
    );
}

#[test]
fn undeclared_critical_node_is_reported_separately() {
    let graph = local_graph();
    let audit = audit_local_autonomy(&graph, &["media.local_mute", "media.missing"]);
    assert!(
        audit
            .violations
            .iter()
            .any(|v| v.token == "local critical node is undefined" && v.subject == "media.missing")
    );
}

#[test]
fn critical_node_declared_cloud_is_reported() {
    let graph = local_graph();
    let audit = audit_local_autonomy(&graph, &["cloud.gateway"]);
    assert!(
        audit
            .violations
            .iter()
            .any(|v| v.token == "local critical node is not local" && v.subject == "cloud.gateway")
    );
}

#[test]
fn graph_rejects_self_loops_and_unknown_edges() {
    let mut graph = local_graph();
    assert_eq!(
        graph
            .depends_on("media.clock", "media.clock")
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(
        graph.depends_on("media.clock", "nowhere").unwrap_err().code,
        ErrorCode::CapabilityAbsent
    );
}

// --- availability ------------------------------------------------------------

#[test]
fn a_local_capability_is_available_under_either_link() {
    assert_eq!(
        availability(CloudLink::Unreachable, NodeClass::Local),
        Availability::Available
    );
    assert_eq!(
        availability(CloudLink::Reachable, NodeClass::Local),
        Availability::Available
    );
}

#[test]
fn a_cloud_capability_fails_closed_when_the_link_drops() {
    assert_eq!(
        availability(CloudLink::Reachable, NodeClass::Cloud),
        Availability::Available
    );
    assert_eq!(
        availability(CloudLink::Unreachable, NodeClass::Cloud),
        Availability::Unavailable {
            token: "cloud link unreachable"
        }
    );
}

#[test]
fn the_local_mute_stays_available_while_the_cloud_link_is_down() {
    let graph = local_graph();
    let class = graph.class_of("media.local_mute").unwrap();
    assert_eq!(
        availability(CloudLink::Unreachable, class),
        Availability::Available
    );
    assert!(audit_local_autonomy(&graph, LOCAL_CRITICAL).is_local());
}

// --- gate --------------------------------------------------------------------

#[test]
fn disconnect_deactivates_only_cloud_capabilities() {
    let mut gate = CapabilityGate::new();
    gate.register("media.local_mute", NodeClass::Local).unwrap();
    gate.register("cloud.tts", NodeClass::Cloud).unwrap();
    gate.activate("media.local_mute").unwrap();
    gate.activate("cloud.tts").unwrap();

    let transition = gate.disconnect();
    assert_eq!(transition.deactivated, vec!["cloud.tts".to_owned()]);
    assert_eq!(gate.mode("media.local_mute"), Some(CapabilityMode::Active));
    assert_eq!(gate.mode("cloud.tts"), Some(CapabilityMode::Disabled));
}

#[test]
fn reconnect_parks_cloud_at_needs_prepare_not_active() {
    let mut gate = CapabilityGate::new();
    gate.register("media.local_mute", NodeClass::Local).unwrap();
    gate.register("cloud.tts", NodeClass::Cloud).unwrap();
    gate.activate("media.local_mute").unwrap();
    gate.activate("cloud.tts").unwrap();

    gate.disconnect();
    let transition = gate.reconnect();
    assert_eq!(transition.needs_prepare, vec!["cloud.tts".to_owned()]);
    assert_eq!(
        gate.mode("cloud.tts"),
        Some(CapabilityMode::NeedsPrepare),
        "recovery must not resume the old Auto state (§27.3)"
    );
    assert_eq!(gate.mode("media.local_mute"), Some(CapabilityMode::Active));
    assert!(transition.deactivated.is_empty());
}

#[test]
fn a_parked_cloud_capability_needs_an_explicit_activation() {
    let mut gate = CapabilityGate::new();
    gate.register("cloud.tts", NodeClass::Cloud).unwrap();
    gate.activate("cloud.tts").unwrap();
    gate.disconnect();
    gate.reconnect();
    assert_eq!(gate.mode("cloud.tts"), Some(CapabilityMode::NeedsPrepare));
    gate.activate("cloud.tts").unwrap();
    assert_eq!(gate.mode("cloud.tts"), Some(CapabilityMode::Active));
}

#[test]
fn a_transition_never_touches_a_local_capability() {
    let mut gate = CapabilityGate::new();
    gate.register("media.local_mute", NodeClass::Local).unwrap();
    gate.activate("media.local_mute").unwrap();
    let down = gate.disconnect();
    let up = gate.reconnect();
    assert!(down.deactivated.is_empty());
    assert!(up.needs_prepare.is_empty());
    assert_eq!(gate.mode("media.local_mute"), Some(CapabilityMode::Active));
}

#[test]
fn registering_a_capability_twice_is_a_conflict() {
    let mut gate = CapabilityGate::new();
    gate.register("cloud.tts", NodeClass::Cloud).unwrap();
    assert_eq!(
        gate.register("cloud.tts", NodeClass::Cloud)
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
}

// --- low-cloud capacity ------------------------------------------------------

fn measured(connections: u64, memory_mib: u64, latency_ms: u64) -> Capacity {
    Capacity::declare(
        connections,
        memory_mib,
        latency_ms,
        Sizing::Measured(nai_upgrade_core::Measurement {
            instances: 3,
            load_pct: 60,
        }),
    )
    .unwrap()
}

#[test]
fn unmeasured_capacity_is_refused() {
    let error = Capacity::declare(10, 512, 40, Sizing::Unmeasured).unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityNotReady);
    assert_eq!(error.reason, "capacity is unmeasured");
}

#[test]
fn a_measurement_must_name_instances_and_load() {
    for sizing in [
        Sizing::Measured(nai_upgrade_core::Measurement {
            instances: 0,
            load_pct: 50,
        }),
        Sizing::Measured(nai_upgrade_core::Measurement {
            instances: 1,
            load_pct: 0,
        }),
        Sizing::Measured(nai_upgrade_core::Measurement {
            instances: 1,
            load_pct: 101,
        }),
    ] {
        assert_eq!(
            Capacity::declare(10, 512, 40, sizing).unwrap_err().code,
            ErrorCode::InvalidArgument
        );
    }
}

#[test]
fn measured_capacity_keeps_its_basis() {
    let capacity = measured(10, 512, 40);
    assert_eq!(capacity.measurement.instances, 3);
    assert_eq!(capacity.measurement.load_pct, 60);
}

#[test]
fn within_budget_is_within() {
    let budget = CapacityBudget {
        max_connections: 20,
        max_memory_mib: 1024,
        max_latency_ms: 80,
    };
    assert_eq!(
        assess(&measured(10, 512, 40), &budget),
        CapacityVerdict::Within
    );
}

#[test]
fn over_budget_reports_every_breach() {
    let budget = CapacityBudget {
        max_connections: 8,
        max_memory_mib: 256,
        max_latency_ms: 30,
    };
    let CapacityVerdict::Throttled { breaches } = assess(&measured(10, 512, 40), &budget) else {
        panic!("expected throttling");
    };
    assert_eq!(
        breaches.len(),
        3,
        "every ceiling is reported, not just the first"
    );
    for token in [
        "connections over budget",
        "memory over budget",
        "latency over budget",
    ] {
        assert!(breaches.contains(&token), "missing {token}");
    }
}

#[test]
fn absorbing_a_breach_is_a_violation_but_throttling_is_clean() {
    let budget = CapacityBudget {
        max_connections: 8,
        max_memory_mib: 256,
        max_latency_ms: 30,
    };
    let capacity = measured(10, 512, 40);
    let absorbing = LowCloudProfile {
        capacity,
        budget,
        policy: ThrottlePolicy::Absorb,
    };
    assert_eq!(
        audit_low_cloud(&absorbing)
            .iter()
            .map(|violation| violation.token)
            .collect::<Vec<_>>(),
        vec!["budget breach absorbed instead of throttled"]
    );
    let throttling = LowCloudProfile {
        capacity,
        budget,
        policy: ThrottlePolicy::RateLimit,
    };
    assert!(audit_low_cloud(&throttling).is_empty());
}

#[test]
fn a_profile_within_budget_is_clean_under_either_policy() {
    let budget = CapacityBudget {
        max_connections: 20,
        max_memory_mib: 1024,
        max_latency_ms: 80,
    };
    let capacity = measured(10, 512, 40);
    for policy in [ThrottlePolicy::RateLimit, ThrottlePolicy::Absorb] {
        assert!(
            audit_low_cloud(&LowCloudProfile {
                capacity,
                budget,
                policy,
            })
            .is_empty()
        );
    }
}
