//! Residency, idle reclamation and the tier migration graph (A08 / A09).
use nai_contracts::v1::ErrorCode;
use nai_security::Operation;
use nai_upgrade_core::{
    CapabilityResidency, IdlePolicy, PRO_ADDITIONS, Residency, ResidencyLedger, ResourceClaim,
    ResourceKind, STANDARD_ADDITIONS, Tier, audit_residency, tier_plan,
};
use std::time::Duration;

fn op() -> Operation {
    Operation::new(Duration::from_secs(30)).unwrap()
}

fn claim(kind: ResourceKind, name: &str, amount: u64) -> ResourceClaim {
    ResourceClaim {
        kind,
        name: name.to_owned(),
        amount,
    }
}

/// The four families the acceptance criteria name, all declared by one avatar.
fn footprint() -> Vec<ResourceClaim> {
    vec![
        claim(ResourceKind::Process, "render-worker", 1),
        claim(ResourceKind::Port, "tcp/8765", 1),
        claim(ResourceKind::Vram, "model/avatar-2d", 4096),
        claim(ResourceKind::CloudRequest, "api/tts", 60),
    ]
}

fn ledger_with_avatar() -> ResidencyLedger {
    let mut ledger = ResidencyLedger::new(IdlePolicy::default());
    ledger
        .install(&op(), "avatar.realistic", footprint())
        .unwrap();
    ledger
}

fn tokens(audit: &nai_upgrade_core::ResidencyAudit) -> Vec<&'static str> {
    audit
        .violations
        .iter()
        .map(|violation| violation.token)
        .collect()
}

fn residency(
    capability: &str,
    held: Vec<ResourceClaim>,
    sessions: usize,
    idle_since_ms: Option<i64>,
) -> CapabilityResidency {
    CapabilityResidency {
        capability: capability.to_owned(),
        declared: held.clone(),
        held,
        enabled: true,
        sessions,
        idle_since_ms,
    }
}

#[test]
fn install_without_enable_holds_nothing() {
    let ledger = ledger_with_avatar();
    assert!(ledger.held().is_empty());
    assert_eq!(ledger.total(ResourceKind::Vram), 0);
    assert_eq!(ledger.total(ResourceKind::CloudRequest), 0);
    assert_eq!(
        ledger.residency_of("avatar.realistic"),
        Some(Residency::Disabled)
    );
    assert!(ledger.audit(0).is_clean());
}

#[test]
fn enabling_materialises_the_declared_footprint() {
    let mut ledger = ledger_with_avatar();
    ledger.enable(&op(), "avatar.realistic", 1_000).unwrap();
    assert_eq!(ledger.total(ResourceKind::Vram), 4096);
    assert_eq!(ledger.total(ResourceKind::CloudRequest), 60);
    assert_eq!(ledger.held().len(), 4);
    assert_eq!(
        ledger.residency_of("avatar.realistic"),
        Some(Residency::Resident { since_ms: 1_000 })
    );
    assert!(ledger.audit(1_000).is_clean());
}

#[test]
fn exclusive_resource_conflict_is_refused() {
    let mut ledger = ledger_with_avatar();
    ledger
        .install(
            &op(),
            "output.preview",
            vec![claim(ResourceKind::Port, "tcp/8765", 1)],
        )
        .unwrap();
    ledger.enable(&op(), "avatar.realistic", 1_000).unwrap();

    let refusal = ledger.enable(&op(), "output.preview", 1_000).unwrap_err();
    assert_eq!(refusal.code, ErrorCode::Conflict);
    // the refusal changed nothing: the port still belongs to the first holder
    assert_eq!(
        ledger.residency_of("output.preview"),
        Some(Residency::Disabled)
    );
    assert!(ledger.audit(1_000).is_clean());
}

#[test]
fn shared_families_are_not_exclusive() {
    let mut ledger = ledger_with_avatar();
    ledger
        .install(
            &op(),
            "avatar.third-party",
            vec![claim(ResourceKind::Vram, "model/avatar-2d", 512)],
        )
        .unwrap();
    ledger.enable(&op(), "avatar.realistic", 1_000).unwrap();
    ledger.enable(&op(), "avatar.third-party", 1_000).unwrap();
    // two capabilities may load the same model; only processes and ports collide
    assert_eq!(ledger.total(ResourceKind::Vram), 4096 + 512);
    assert!(ledger.audit(1_000).is_clean());
}

#[test]
fn ending_a_session_does_not_unload_a_model() {
    let mut ledger = ledger_with_avatar();
    ledger.enable(&op(), "avatar.realistic", 1_000).unwrap();
    ledger.session_started(&op(), "avatar.realistic").unwrap();
    assert_eq!(
        ledger.residency_of("avatar.realistic"),
        Some(Residency::Serving { sessions: 1 })
    );
    ledger
        .session_ended(&op(), "avatar.realistic", 2_000)
        .unwrap();

    // the worker's TTL has elapsed, the model's has not: release is staged
    let report = ledger.reclaim(&op(), 2_000 + 60_000).unwrap();
    assert_eq!(report.released.len(), 1);
    assert_eq!(report.released[0].capability, "avatar.realistic");
    assert_eq!(report.released[0].claims.len(), 3);
    assert_eq!(report.held_back, 1);
    assert_eq!(
        ledger.total(ResourceKind::Vram),
        4096,
        "model stays resident"
    );
    assert_eq!(ledger.total(ResourceKind::Process), 0);
    assert_eq!(ledger.total(ResourceKind::CloudRequest), 0);
    // reclaiming is not disabling
    assert_eq!(
        ledger.residency_of("avatar.realistic"),
        Some(Residency::Resident { since_ms: 2_000 })
    );
    assert!(ledger.audit(2_000 + 60_000).is_clean());
}

#[test]
fn reclaim_after_the_model_ttl_releases_every_family() {
    let mut ledger = ledger_with_avatar();
    ledger.enable(&op(), "avatar.realistic", 1_000).unwrap();
    ledger.session_started(&op(), "avatar.realistic").unwrap();
    ledger
        .session_ended(&op(), "avatar.realistic", 2_000)
        .unwrap();

    let report = ledger.reclaim(&op(), 2_000 + 300_000).unwrap();
    assert_eq!(report.released[0].claims.len(), 4);
    assert_eq!(report.held_back, 0);
    assert!(ledger.held().is_empty());
    assert!(ledger.audit(2_000 + 300_000).is_clean());
}

#[test]
fn busy_capability_is_never_reclaimed() {
    let mut ledger = ledger_with_avatar();
    ledger.enable(&op(), "avatar.realistic", 1_000).unwrap();
    ledger.session_started(&op(), "avatar.realistic").unwrap();

    let report = ledger.reclaim(&op(), 1_000 + 86_400_000).unwrap();
    assert!(report.is_empty());
    assert_eq!(
        report.held_back, 0,
        "a busy capability is not a held-back one"
    );
    assert_eq!(ledger.held().len(), 4);
    assert!(ledger.audit(1_000 + 86_400_000).is_clean());
}

#[test]
fn reclaim_ignores_disabled_capabilities() {
    let mut ledger = ledger_with_avatar();
    let report = ledger.reclaim(&op(), 1_000 + 86_400_000).unwrap();
    assert!(report.is_empty());
    assert!(ledger.held().is_empty());
}

#[test]
fn disable_refuses_while_serving_then_releases() {
    let mut ledger = ledger_with_avatar();
    ledger.enable(&op(), "avatar.realistic", 1_000).unwrap();
    ledger.session_started(&op(), "avatar.realistic").unwrap();
    assert_eq!(
        ledger.disable(&op(), "avatar.realistic").unwrap_err().code,
        ErrorCode::Conflict
    );

    ledger
        .session_ended(&op(), "avatar.realistic", 2_000)
        .unwrap();
    ledger.disable(&op(), "avatar.realistic").unwrap();
    assert!(ledger.held().is_empty());
    assert_eq!(
        ledger.residency_of("avatar.realistic"),
        Some(Residency::Disabled)
    );
    // re-enabling is possible: disabling retires resources, not the capability
    ledger.enable(&op(), "avatar.realistic", 3_000).unwrap();
    assert_eq!(ledger.total(ResourceKind::Vram), 4096);
}

#[test]
fn install_rejects_duplicates_and_non_canonical_footprints() {
    let mut ledger = ResidencyLedger::new(IdlePolicy::default());
    ledger.install(&op(), "cap.a", footprint()).unwrap();

    assert_eq!(
        ledger
            .install(&op(), "cap.a", footprint())
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    // the same port declared twice inside one capability is ambiguous
    assert_eq!(
        ledger
            .install(
                &op(),
                "cap.b",
                vec![
                    claim(ResourceKind::Port, "tcp/1", 1),
                    claim(ResourceKind::Port, "tcp/1", 1),
                ],
            )
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
    // a zero-amount claim is not a claim
    assert_eq!(
        ledger
            .install(&op(), "cap.c", vec![claim(ResourceKind::Vram, "m", 0)])
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(
        ledger.enable(&op(), "cap.missing", 0).unwrap_err().code,
        ErrorCode::CapabilityAbsent
    );
}

#[test]
fn uninstall_removes_the_capability() {
    let mut ledger = ledger_with_avatar();
    ledger.enable(&op(), "avatar.realistic", 1_000).unwrap();
    ledger.uninstall(&op(), "avatar.realistic").unwrap();
    assert_eq!(ledger.residency_of("avatar.realistic"), None);
    assert!(ledger.held().is_empty());
}

#[test]
fn session_guards_reject_impossible_transitions() {
    let mut ledger = ledger_with_avatar();
    // not enabled yet
    assert_eq!(
        ledger
            .session_started(&op(), "avatar.realistic")
            .unwrap_err()
            .code,
        ErrorCode::CapabilityNotReady
    );
    ledger.enable(&op(), "avatar.realistic", 1_000).unwrap();
    // no session to end
    assert_eq!(
        ledger
            .session_ended(&op(), "avatar.realistic", 2_000)
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    // enabling twice
    assert_eq!(
        ledger
            .enable(&op(), "avatar.realistic", 2_000)
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
}

#[test]
fn audit_cannot_hide_a_leak() {
    let policy = IdlePolicy::default();
    // a resource past its TTL that nobody reclaimed is the reachable leak
    let leaked = vec![residency(
        "avatar.realistic",
        vec![claim(ResourceKind::Vram, "model", 4096)],
        0,
        Some(0),
    )];
    assert_eq!(
        tokens(&audit_residency(&leaked, &policy, 300_000)),
        ["idle resource past its ttl"]
    );
    // ...and the same state under its TTL is clean, which is what makes the
    // check a leak detector rather than a blanket ban on being resident
    assert!(audit_residency(&leaked, &policy, 299_999).is_clean());

    // a disabled holder would be a broken ledger
    let mut disabled = residency(
        "cap.a",
        vec![claim(ResourceKind::Port, "tcp/1", 1)],
        0,
        None,
    );
    disabled.enabled = false;
    assert_eq!(
        tokens(&audit_residency(&[disabled], &policy, 0)),
        ["disabled capability holds resources"]
    );

    // holding something never declared
    let mut undeclared = residency(
        "cap.a",
        vec![claim(ResourceKind::Port, "tcp/1", 1)],
        0,
        Some(0),
    );
    undeclared.declared.clear();
    assert_eq!(
        tokens(&audit_residency(&[undeclared], &policy, 0)),
        ["enabled capability holds undeclared resource"]
    );

    // the same exclusive resource claimed by two capabilities
    let twice = vec![
        residency(
            "cap.a",
            vec![claim(ResourceKind::Port, "tcp/1", 1)],
            0,
            Some(0),
        ),
        residency(
            "cap.b",
            vec![claim(ResourceKind::Port, "tcp/1", 1)],
            0,
            Some(0),
        ),
    ];
    assert_eq!(
        tokens(&audit_residency(&twice, &policy, 0)),
        ["exclusive resource held twice"]
    );

    // clocks that disagree with the session count
    let no_clock = vec![residency("cap.a", vec![], 0, None)];
    assert_eq!(
        tokens(&audit_residency(&no_clock, &policy, 0)),
        ["idle capability has no idle clock"]
    );
    let serving_clocks = vec![residency("cap.a", vec![], 2, Some(0))];
    assert_eq!(
        tokens(&audit_residency(&serving_clocks, &policy, 0)),
        ["serving capability has an idle clock"]
    );
}

#[test]
fn idle_policy_refuses_a_negative_ttl() {
    let mut policy = IdlePolicy::default();
    assert_eq!(policy.ttl_ms(ResourceKind::Vram), 300_000);
    assert_eq!(policy.ttl_ms(ResourceKind::Process), 30_000);
    assert_eq!(
        policy.set(ResourceKind::Vram, -1).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    policy.set(ResourceKind::Vram, 0).unwrap();
    assert_eq!(policy.ttl_ms(ResourceKind::Vram), 0);
}

#[test]
fn tier_plan_uses_one_graph_without_the_intermediate_tier() {
    let single_hop = tier_plan(Tier::Lite, Tier::Standard);
    assert_eq!(single_hop.install, STANDARD_ADDITIONS.to_vec());
    assert!(single_hop.remove.is_empty());

    let second_hop = tier_plan(Tier::Standard, Tier::Pro);
    assert_eq!(second_hop.install, PRO_ADDITIONS.to_vec());

    // skipping the intermediate tier is the union of both hops, not a third path
    let skipped = tier_plan(Tier::Lite, Tier::Pro);
    assert_eq!(
        skipped.install.len(),
        STANDARD_ADDITIONS.len() + PRO_ADDITIONS.len()
    );
    assert!(
        STANDARD_ADDITIONS
            .iter()
            .all(|cap| skipped.install.contains(cap))
    );
    assert!(
        PRO_ADDITIONS
            .iter()
            .all(|cap| skipped.install.contains(cap))
    );
    assert!(skipped.remove.is_empty());

    // and going back down removes exactly what going up installed
    let down = tier_plan(Tier::Pro, Tier::Lite);
    assert!(down.install.is_empty());
    assert_eq!(down.remove, skipped.install);
}

#[test]
fn tier_plan_never_touches_the_kernel() {
    for from in [Tier::Lite, Tier::Standard, Tier::Pro] {
        for to in [Tier::Lite, Tier::Standard, Tier::Pro] {
            let plan = tier_plan(from, to);
            for capability in plan.install.iter().chain(plan.remove.iter()) {
                assert!(
                    !capability.contains("kernel"),
                    "{from:?} -> {to:?} would reinstall the kernel via {capability}"
                );
            }
            assert_eq!(plan.is_empty(), from == to, "{from:?} -> {to:?}");
        }
    }
}

#[test]
fn diagnostics_are_stable_tokens_not_os_strings() {
    let audit = audit_residency(
        &[residency("cap.a", vec![], 1, Some(0))],
        &IdlePolicy::default(),
        0,
    );
    assert_eq!(audit.violations[0].subject, "cap.a");
    assert_eq!(ResourceKind::CloudRequest.as_str(), "cloud_request");
    assert_eq!(Tier::Standard.as_str(), "standard");
}
