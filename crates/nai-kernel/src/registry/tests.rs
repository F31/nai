use super::*;

fn call() -> CallContext {
    CallContext::new(Duration::from_secs(30)).unwrap()
}
fn registry() -> Registry {
    Registry::new("test-boot", Limits::default()).unwrap()
}
fn registration(id: &str, caps: &[&str], requires: &[&str]) -> PluginRegistration {
    PluginRegistration {
        plugin_id: id.into(),
        plugin_version: "0.1.0".into(),
        artifact_hash: "a".repeat(64),
        manifest_hash: "b".repeat(64),
        validated_kernel_version: "0.1.0".into(),
        protocol_ranges: vec![ProtocolRange {
            major: 1,
            min_minor: 0,
            max_minor: 0,
        }],
        contracts: caps.iter().map(|s| (*s).into()).collect(),
        supported_hosts: vec![HostKind::Inproc as i32, HostKind::Subprocess as i32],
        requires: requires.iter().map(|s| (*s).into()).collect(),
        license_ref: "test-license-not-real".into(),
        resource_profile_ref: "test-resource".into(),
        certification_ref: "test-cert-not-real".into(),
        ..Default::default()
    }
}
fn admission(registration: &PluginRegistration, scope: &str) -> Admission {
    Admission {
        provider: BoundProvider {
            plugin_id: registration.plugin_id.clone(),
            plugin_version: registration.plugin_version.clone(),
            artifact_hash: registration.artifact_hash.clone(),
            manifest_hash: registration.manifest_hash.clone(),
            host: 1,
            policy_scope_ref: scope.into(),
        },
        review_ref: "test-review".into(),
        license_ref: registration.license_ref.clone(),
        resource_profile_ref: registration.resource_profile_ref.clone(),
        certification_ref: registration.certification_ref.clone(),
        host_boundary_ref: "test-boundary".into(),
        compiled_inproc: true,
    }
}
fn permission(caps: &[&str], state: i32) -> PermissionSnapshot {
    PermissionSnapshot {
        revision: "test-permission-v1".into(),
        authority_ref: "test-authority".into(),
        valid_until_utc_ms: Some(call().now_utc_ms() + 60000),
        capabilities: caps.iter().map(|c| ((*c).into(), state)).collect(),
    }
}
fn install(registry: &mut Registry, id: &str, caps: &[&str], requires: &[&str], scope: &str) {
    let registration = registration(id, caps, requires);
    registry
        .admit(&call(), admission(&registration, scope))
        .unwrap();
    registry
        .register(&call(), registration, HostKind::Inproc)
        .unwrap();
}
fn ready(registry: &mut Registry, id: &str, caps: &[&str], requires: &[&str]) {
    install(registry, id, caps, requires, "test-scope");
    registry
        .health(
            &call(),
            id,
            HealthResponse {
                ready: true,
                revision: "test-health".into(),
                ..Default::default()
            },
            call().now_utc_ms() + 60000,
        )
        .unwrap();
    registry
        .permissions(&call(), "session-A", id, permission(caps, 1))
        .unwrap();
}
fn plan(caps: &[(&str, bool)]) -> BindingPlan {
    BindingPlan {
        session_id: "session-A".into(),
        mode: 1,
        requirements: caps
            .iter()
            .map(|(c, r)| BindingRequirement {
                capability_id: (*c).into(),
                required: *r,
            })
            .collect(),
        ..Default::default()
    }
}
fn choice(cap: &str, primary: &str, fallbacks: &[&str], scope: &str) -> BindingSelection {
    BindingSelection {
        capability_id: cap.into(),
        primary_plugin_id: primary.into(),
        fallback_plugin_ids: fallbacks.iter().map(|s| (*s).into()).collect(),
        policy_scope_ref: scope.into(),
    }
}

#[test]
fn empty_registry_is_not_ready_and_does_not_invent_plugins() {
    let registry = registry();
    let report = registry
        .prepare(&call(), &plan(&[("agent.decide@1", true)]))
        .unwrap();
    assert_eq!(report.readiness, 1);
    assert!(report.snapshot.is_none());
    assert!(report.capability_view.unwrap().capabilities.is_empty());
}
#[test]
fn independent_admission_and_compiled_review_are_mandatory() {
    let mut registry = registry();
    let registration = registration("guard", &["guard.check@1"], &[]);
    assert!(
        registry
            .register(&call(), registration.clone(), HostKind::Inproc)
            .is_err()
    );
    let mut proof = admission(&registration, "test-scope");
    proof.compiled_inproc = false;
    assert!(registry.admit(&call(), proof).is_err());
    registry
        .admit(&call(), admission(&registration, "test-scope"))
        .unwrap();
    let mut forged = registration.clone();
    forged.artifact_hash = "c".repeat(64);
    assert!(
        registry
            .register(&call(), forged, HostKind::Inproc)
            .is_err()
    );
    assert!(
        registry
            .register(&call(), registration, HostKind::Subprocess)
            .is_err()
    );
    assert!(registry.plugins.is_empty());
}
#[test]
fn target_protocol_and_unknown_formal_contract_are_rejected() {
    let mut registry = registry();
    let registration = registration("guard", &["guard.check@1"], &[]);
    registry
        .admit(&call(), admission(&registration, "test-scope"))
        .unwrap();
    for change in [0, 1, 2] {
        let mut invalid = registration.clone();
        match change {
            0 => invalid.validated_kernel_version = "2.0.0".into(),
            1 => invalid.protocol_ranges[0].major = 2,
            _ => invalid.contracts = vec!["not.registered@1".into()],
        };
        assert!(
            registry
                .register(&call(), invalid, HostKind::Inproc)
                .is_err()
        );
    }
}
#[test]
fn dependency_cycle_rejection_is_atomic() {
    let mut registry = registry();
    install(
        &mut registry,
        "agent",
        &["agent.decide@1"],
        &["skill.invoke@1"],
        "test-scope",
    );
    let revision = registry.revision;
    let skill = registration("skill", &["skill.invoke@1"], &["agent.decide@1"]);
    registry
        .admit(&call(), admission(&skill, "test-scope"))
        .unwrap();
    assert_eq!(
        registry
            .register(&call(), skill, HostKind::Inproc)
            .unwrap_err()
            .message,
        "DEPENDENCY_CYCLE"
    );
    assert_eq!(registry.plugins.len(), 1);
    assert_eq!(registry.revision, revision + 1);
}
#[test]
fn hard_dependency_cycles_cannot_be_hidden_in_manifest() {
    let mut registry = registry();
    let guard = registration("guard", &["guard.check@1"], &["output.deliver@1"]);
    registry
        .admit(&call(), admission(&guard, "test-scope"))
        .unwrap();
    registry.register(&call(), guard, HostKind::Inproc).unwrap();
    let output = registration("output", &["output.deliver@1"], &[]);
    registry
        .admit(&call(), admission(&output, "test-scope"))
        .unwrap();
    assert!(
        registry
            .register(&call(), output, HostKind::Inproc)
            .is_err()
    );
}
#[test]
fn single_provider_ready_and_complete_transitive_closure() {
    let mut registry = registry();
    ready(
        &mut registry,
        "agent",
        &["agent.decide@1"],
        &["skill.invoke@1"],
    );
    ready(
        &mut registry,
        "skill",
        &["skill.invoke@1"],
        &["storage.persist@1"],
    );
    ready(&mut registry, "storage", &["storage.persist@1"], &[]);
    let report = registry
        .prepare(&call(), &plan(&[("agent.decide@1", true)]))
        .unwrap();
    assert_eq!(report.readiness, 2);
    assert_eq!(report.snapshot.unwrap().bindings.len(), 3);
    assert!(report.issues.is_empty());
}
#[test]
fn optional_missing_degrades_without_requiring_llm_or_knowledge() {
    let mut registry = registry();
    ready(&mut registry, "agent", &["agent.decide@1"], &[]);
    let report = registry
        .prepare(
            &call(),
            &plan(&[("agent.decide@1", true), ("memory.access@1", false)]),
        )
        .unwrap();
    assert_eq!(report.readiness, 3);
    assert_eq!(report.snapshot.unwrap().bindings.len(), 1);
    assert!(!report.issues[0].required);
}
#[test]
fn external_output_cannot_omit_guard_and_avatar_profile() {
    let mut registry = registry();
    ready(&mut registry, "output", &["output.deliver@1"], &[]);
    assert_eq!(
        registry
            .prepare(&call(), &plan(&[("output.deliver@1", true)]))
            .unwrap()
            .readiness,
        1
    );
    ready(&mut registry, "guard", &["guard.check@1"], &[]);
    assert_eq!(
        registry
            .prepare(&call(), &plan(&[("output.deliver@1", true)]))
            .unwrap()
            .readiness,
        2
    );
    ready(&mut registry, "avatar", &["avatar.render@1"], &[]);
    assert_eq!(
        registry
            .prepare(&call(), &plan(&[("avatar.render@1", true)]))
            .unwrap()
            .readiness,
        1
    );
    ready(&mut registry, "profile", &["avatar.profile@1"], &[]);
    assert_eq!(
        registry
            .prepare(&call(), &plan(&[("avatar.render@1", true)]))
            .unwrap()
            .readiness,
        2
    );
}
#[test]
fn declared_permission_is_not_granted_and_other_session_cannot_inherit() {
    let mut registry = registry();
    install(
        &mut registry,
        "agent",
        &["agent.decide@1"],
        &[],
        "test-scope",
    );
    registry
        .health(
            &call(),
            "agent",
            HealthResponse {
                ready: true,
                revision: "test-health".into(),
                ..Default::default()
            },
            call().now_utc_ms() + 60000,
        )
        .unwrap();
    let plan = plan(&[("agent.decide@1", true)]);
    assert_eq!(registry.prepare(&call(), &plan).unwrap().readiness, 1);
    registry
        .permissions(
            &call(),
            "session-A",
            "agent",
            permission(&["agent.decide@1"], 1),
        )
        .unwrap();
    assert_eq!(registry.prepare(&call(), &plan).unwrap().readiness, 2);
    let mut other = plan.clone();
    other.session_id = "session-B".into();
    assert_eq!(registry.prepare(&call(), &other).unwrap().readiness, 1);
    for state in [0, 2, 77] {
        registry
            .permissions(
                &call(),
                "session-A",
                "agent",
                permission(&["agent.decide@1"], state),
            )
            .unwrap();
        let report = registry.prepare(&call(), &plan).unwrap();
        assert_eq!(report.readiness, 1);
        assert_ne!(
            report.capability_view.unwrap().capabilities[0].permission,
            1
        );
    }
}
#[test]
fn permission_source_and_expiry_are_rechecked() {
    let mut registry = registry();
    ready(&mut registry, "agent", &["agent.decide@1"], &[]);
    let plan = plan(&[("agent.decide@1", true)]);
    for change in [0, 1, 2] {
        let mut invalid = permission(&["agent.decide@1"], 1);
        match change {
            0 => invalid.authority_ref.clear(),
            1 => invalid.valid_until_utc_ms = Some(1),
            _ => invalid.revision.clear(),
        };
        registry
            .permissions(&call(), "session-A", "agent", invalid)
            .unwrap();
        assert_eq!(registry.prepare(&call(), &plan).unwrap().readiness, 1);
    }
}
#[test]
fn health_missing_failed_and_expired_do_not_become_ready() {
    let mut registry = registry();
    install(
        &mut registry,
        "agent",
        &["agent.decide@1"],
        &[],
        "test-scope",
    );
    registry
        .permissions(
            &call(),
            "session-A",
            "agent",
            permission(&["agent.decide@1"], 1),
        )
        .unwrap();
    let plan = plan(&[("agent.decide@1", true)]);
    assert_eq!(registry.prepare(&call(), &plan).unwrap().readiness, 1);
    for (healthy, until) in [(false, call().now_utc_ms() + 60000), (true, 1)] {
        registry
            .health(
                &call(),
                "agent",
                HealthResponse {
                    ready: healthy,
                    revision: "test-health".into(),
                    ..Default::default()
                },
                until,
            )
            .unwrap();
        assert_eq!(registry.prepare(&call(), &plan).unwrap().readiness, 1);
    }
}
#[test]
fn multiple_providers_need_explicit_choice_even_if_only_one_is_healthy() {
    let mut registry = registry();
    ready(&mut registry, "agent-a", &["agent.decide@1"], &[]);
    install(
        &mut registry,
        "agent-b",
        &["agent.decide@1"],
        &[],
        "test-scope",
    );
    let mut plan = plan(&[("agent.decide@1", true)]);
    assert_eq!(
        registry.prepare(&call(), &plan).unwrap().issues[0].reason_code,
        "BINDING_AMBIGUOUS"
    );
    plan.selections
        .push(choice("agent.decide@1", "agent-a", &[], "test-scope"));
    assert_eq!(registry.prepare(&call(), &plan).unwrap().readiness, 2);
}
#[test]
fn explicit_fallback_requires_its_complete_dependency_closure() {
    let mut registry = registry();
    ready(
        &mut registry,
        "primary",
        &["agent.decide@1"],
        &["memory.access@1"],
    );
    ready(&mut registry, "backup", &["agent.decide@1"], &[]);
    let mut plan = plan(&[("agent.decide@1", true)]);
    plan.selections.push(choice(
        "agent.decide@1",
        "primary",
        &["backup"],
        "test-scope",
    ));
    let report = registry.prepare(&call(), &plan).unwrap();
    assert_eq!(report.readiness, 3);
    let snapshot = report.snapshot.unwrap();
    assert_eq!(snapshot.bindings.len(), 1);
    assert_eq!(
        snapshot.bindings[0].selected.as_ref().unwrap().plugin_id,
        "backup"
    );
    assert_eq!(snapshot.bindings[0].primary_plugin_id, "backup");
}
#[test]
fn fallback_chain_does_not_cross_data_or_upload_policy_scope() {
    let mut registry = registry();
    ready(&mut registry, "primary", &["agent.decide@1"], &[]);
    install(
        &mut registry,
        "backup",
        &["agent.decide@1"],
        &[],
        "other-knowledge-domain",
    );
    let mut plan = plan(&[("agent.decide@1", true)]);
    plan.selections.push(choice(
        "agent.decide@1",
        "primary",
        &["backup"],
        "test-scope",
    ));
    assert_eq!(
        registry.prepare(&call(), &plan).unwrap().issues[0].reason_code,
        "FALLBACK_POLICY_SCOPE_MISMATCH"
    );
}
#[test]
fn frozen_snapshot_is_versioned_immutable_and_blocks_hot_replacement() {
    let mut registry = registry();
    ready(&mut registry, "agent", &["agent.decide@1"], &[]);
    let plan = plan(&[("agent.decide@1", true)]);
    let snapshot = registry.freeze(&call(), &plan).unwrap().snapshot.unwrap();
    assert_eq!(
        snapshot.bindings[0]
            .selected
            .as_ref()
            .unwrap()
            .plugin_version,
        "0.1.0"
    );
    registry.recheck(&call(), &snapshot).unwrap();
    assert!(
        registry
            .register(
                &call(),
                registration("agent", &["agent.decide@1"], &[]),
                HostKind::Inproc
            )
            .is_err()
    );
    assert!(registry.remove(&call(), "agent").is_err());
    assert!(registry.freeze(&call(), &plan).is_err());
    let mut forged = snapshot.clone();
    forged.bindings[0].selected.as_mut().unwrap().artifact_hash = "c".repeat(64);
    assert!(registry.recheck(&call(), &forged).is_err());
    forged = snapshot.clone();
    forged.session_id = "session-B".into();
    assert!(registry.recheck(&call(), &forged).is_err());
    registry.release(&call(), "session-A").unwrap();
    registry.remove(&call(), "agent").unwrap();
    assert!(registry.recheck(&call(), &snapshot).is_err());
}
#[test]
fn live_revocation_invalidates_but_does_not_rewrite_frozen_selection() {
    let mut registry = registry();
    ready(&mut registry, "primary", &["agent.decide@1"], &[]);
    ready(&mut registry, "backup", &["agent.decide@1"], &[]);
    let mut plan = plan(&[("agent.decide@1", true)]);
    plan.selections.push(choice(
        "agent.decide@1",
        "primary",
        &["backup"],
        "test-scope",
    ));
    let snapshot = registry.freeze(&call(), &plan).unwrap().snapshot.unwrap();
    registry
        .permissions(
            &call(),
            "session-A",
            "primary",
            permission(&["agent.decide@1"], 2),
        )
        .unwrap();
    assert!(registry.recheck(&call(), &snapshot).is_err());
    assert_eq!(registry.frozen["session-A"].snapshot, snapshot);
    assert_eq!(
        registry
            .prepare(&call(), &plan)
            .unwrap()
            .snapshot
            .unwrap()
            .bindings[0]
            .selected
            .as_ref()
            .unwrap()
            .plugin_id,
        "backup"
    );
    registry.revoke_admission(&call(), "backup").unwrap();
    assert_eq!(registry.prepare(&call(), &plan).unwrap().readiness, 1);
}
#[test]
fn automatic_mode_requires_independent_platform_supervision_and_budget() {
    let mut registry = registry();
    for (id, cap) in [
        ("agent", "agent.decide@1"),
        ("guard", "guard.check@1"),
        ("knowledge", "knowledge.retrieve@1"),
        ("events", "platform.events@1"),
        ("capabilities", "platform.capabilities@1"),
        ("lifecycle", "platform.lifecycle@1"),
    ] {
        ready(&mut registry, id, &[cap], &[]);
    }
    let mut plan = plan(&[("agent.decide@1", true)]);
    plan.mode = 3;
    assert_eq!(registry.prepare(&call(), &plan).unwrap().readiness, 1);
    let good = AutomaticReadiness {
        platform_permissions: Some(permission(&["auto.supervised"], 1)),
        supervision_lease_ref: "test-not-real-lease".into(),
        supervision_expires_at_utc_ms: call().now_utc_ms() + 60000,
        budget_admission_ref: "test-not-real-budget".into(),
    };
    for change in [0, 1, 2, 3] {
        let mut bad = good.clone();
        match change {
            0 => bad
                .platform_permissions
                .as_mut()
                .unwrap()
                .capabilities
                .insert("auto.supervised".into(), 0),
            1 => {
                bad.supervision_expires_at_utc_ms = 1;
                None
            }
            2 => {
                bad.supervision_lease_ref.clear();
                None
            }
            _ => {
                bad.budget_admission_ref.clear();
                None
            }
        };
        registry
            .automatic_conditions(&call(), "session-A", bad)
            .unwrap();
        assert_eq!(registry.prepare(&call(), &plan).unwrap().readiness, 1);
    }
    registry
        .automatic_conditions(&call(), "session-A", good)
        .unwrap();
    assert_eq!(registry.prepare(&call(), &plan).unwrap().readiness, 2);
}
#[test]
fn bounded_calls_and_inventory_do_not_mutate_on_rejected_work() {
    let mut registry = Registry::new(
        "test",
        Limits {
            plugins: 1,
            sessions: 1,
            ..Limits::default()
        },
    )
    .unwrap();
    ready(&mut registry, "agent", &["agent.decide@1"], &[]);
    let initial = registry.revision;
    let cancelled = call();
    cancelled.cancel();
    assert_eq!(
        registry.remove(&cancelled, "agent").unwrap_err().code,
        ErrorCode::Cancelled as i32
    );
    assert_eq!(registry.revision, initial);
    assert!(
        registry
            .prepare(
                &CallContext::new(Duration::ZERO).unwrap(),
                &plan(&[("agent.decide@1", true)])
            )
            .is_err()
    );
    let other = registration("other", &["memory.access@1"], &[]);
    assert!(
        registry
            .admit(&call(), admission(&other, "test-scope"))
            .is_err()
    );
    registry
        .freeze(&call(), &plan(&[("agent.decide@1", true)]))
        .unwrap();
    let mut other = plan(&[("agent.decide@1", true)]);
    other.session_id = "session-B".into();
    assert!(registry.freeze(&call(), &other).is_err());
}
