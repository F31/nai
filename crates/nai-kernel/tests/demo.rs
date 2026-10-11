//! §15 / §615 / §612 demo package and availability goal (registry O01).
use nai_kernel::demo::{
    ConnectivityTest, CostDeclaration, CostLine, DemoAvatar, DemoPackage, DemoReadiness,
    DemoScenario, DrillRecord, FIRST_UTTERANCE_TARGET_MS, KnowledgeSeed, ProviderPreset,
    REAL_OUTPUT_TARGET_MS, audit_demo_package,
};
use nai_kernel::golden::Finding;
use std::collections::BTreeSet;

const INSTALL_READY: u64 = 1_000;

/// A complete, honest, freshly rehearsed package: it audits empty.
fn clean_package() -> DemoPackage {
    DemoPackage {
        avatar: Some(DemoAvatar::licensed("avatar.demo", "licence:demo-2026")),
        knowledge: vec![KnowledgeSeed::new("kb.demo.1")],
        scenario: Some(DemoScenario::new("scenario.demo")),
        provider_presets: vec![ProviderPreset::referencing(
            "preset.demo.text",
            "credential:demo.text",
        )],
        connectivity: Some(ConnectivityTest::live()),
        cost: Some(
            CostDeclaration::new()
                .with("cloud", CostLine::Known(1_200))
                .with("download", CostLine::Known(0))
                .with("storage", CostLine::Unknown("target volume not probed")),
        ),
        drill: Some(DrillRecord::complete(
            INSTALL_READY,
            INSTALL_READY + 9 * 60 * 1000,
            INSTALL_READY + 25 * 60 * 1000,
        )),
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

// ------------------------------------------------------------------ conditional rule

#[test]
fn an_undeclared_package_audits_empty() {
    let package = DemoPackage::empty();
    assert!(audit_demo_package(&package).is_empty());
    assert_eq!(package.readiness(), DemoReadiness::Blocked);
}

#[test]
fn a_half_declared_package_names_each_missing_part() {
    // Declared by shipping sample knowledge alone.
    let package = DemoPackage {
        knowledge: vec![KnowledgeSeed::new("kb.demo.1")],
        ..DemoPackage::empty()
    };
    let findings = audit_demo_package(&package);
    for token in [
        "demo_avatar_missing",
        "demo_scenario_missing",
        "demo_provider_presets_missing",
        "demo_connectivity_missing",
        "demo_cost_estimate_missing",
    ] {
        assert!(
            has(&findings, token, token_subject(token)),
            "{token} missing"
        );
    }
    // The part that *is* there is not reported.
    assert!(!tokens(&findings).contains(&"demo_knowledge_missing"));
    assert_eq!(package.readiness(), DemoReadiness::Defective);
}

/// The subject the audit uses for a whole-part finding is the part's own name.
fn token_subject(token: &str) -> &str {
    match token {
        "demo_avatar_missing" => "avatar",
        "demo_scenario_missing" => "scenario",
        "demo_provider_presets_missing" => "provider_presets",
        "demo_connectivity_missing" => "connectivity",
        "demo_cost_estimate_missing" => "cost",
        _ => token,
    }
}

// ------------------------------------------------------------------ avatar

#[test]
fn an_avatar_without_a_licence_is_a_defect() {
    let package = DemoPackage {
        avatar: Some(DemoAvatar::unlicensed("avatar.demo")),
        ..clean_package()
    };
    let findings = audit_demo_package(&package);
    assert!(has(&findings, "demo_avatar_without_license", "avatar.demo"));
    assert_eq!(package.readiness(), DemoReadiness::Defective);
}

#[test]
fn a_licensed_avatar_is_not_reported() {
    let findings = audit_demo_package(&clean_package());
    assert!(tokens(&findings).is_empty(), "{:?}", tokens(&findings));
}

// ------------------------------------------------------------------ provider presets

#[test]
fn a_preset_that_embeds_a_secret_is_a_defect() {
    let package = DemoPackage {
        provider_presets: vec![ProviderPreset::embedding_secret("preset.demo.text")],
        ..clean_package()
    };
    let findings = audit_demo_package(&package);
    assert!(has(
        &findings,
        "provider_preset_embeds_secret",
        "preset.demo.text"
    ));
    // An embedded secret is not *also* reported as a missing credential: the
    // preset does carry one, just the wrong way.
    assert!(!tokens(&findings).contains(&"provider_preset_without_credential"));
}

#[test]
fn a_preset_without_a_credential_at_all_is_a_defect() {
    let package = DemoPackage {
        provider_presets: vec![ProviderPreset::without_credential("preset.demo.bare")],
        ..clean_package()
    };
    let findings = audit_demo_package(&package);
    assert!(has(
        &findings,
        "provider_preset_without_credential",
        "preset.demo.bare"
    ));
}

// ------------------------------------------------------------------ connectivity

#[test]
fn a_constant_probe_is_a_stub() {
    let package = DemoPackage {
        connectivity: Some(ConnectivityTest::constant()),
        ..clean_package()
    };
    let findings = audit_demo_package(&package);
    assert!(has(&findings, "connectivity_probe_is_stub", "connectivity"));
    assert!(!tokens(&findings).contains(&"smoke_claims_acceptance"));
}

#[test]
fn a_smoke_result_claimed_as_acceptance_is_a_defect() {
    let package = DemoPackage {
        connectivity: Some(ConnectivityTest::live().claiming_acceptance()),
        ..clean_package()
    };
    let findings = audit_demo_package(&package);
    assert!(has(&findings, "smoke_claims_acceptance", "connectivity"));
    // A live probe that over-claims is still a live probe.
    assert!(!tokens(&findings).contains(&"connectivity_probe_is_stub"));
}

// ------------------------------------------------------------------ cost

#[test]
fn an_unknown_cost_component_must_name_its_reason() {
    let package = DemoPackage {
        cost: Some(
            CostDeclaration::new()
                .with("cloud", CostLine::Known(1_200))
                .with("download", CostLine::Unknown("   ")),
        ),
        ..clean_package()
    };
    let findings = audit_demo_package(&package);
    assert!(has(&findings, "cost_unknown_without_reason", "download"));
    // A component with a stated reason is not reported.
    assert!(!has(&findings, "cost_unknown_without_reason", "storage"));
}

// ------------------------------------------------------------------ drill

#[test]
fn an_unrun_drill_is_blocked_not_defective() {
    let package = DemoPackage {
        drill: None,
        ..clean_package()
    };
    assert!(audit_demo_package(&package).is_empty());
    assert_eq!(package.readiness(), DemoReadiness::Blocked);
}

#[test]
fn a_drill_beyond_the_first_utterance_target_is_a_defect() {
    let package = DemoPackage {
        drill: Some(DrillRecord::complete(
            INSTALL_READY,
            INSTALL_READY + FIRST_UTTERANCE_TARGET_MS + 1,
            INSTALL_READY + 20 * 60 * 1000,
        )),
        ..clean_package()
    };
    let findings = audit_demo_package(&package);
    assert!(has(
        &findings,
        "first_utterance_beyond_target",
        &(FIRST_UTTERANCE_TARGET_MS + 1).to_string()
    ));
    assert!(!tokens(&findings).contains(&"real_output_beyond_target"));
}

#[test]
fn a_drill_beyond_the_real_output_target_is_a_defect() {
    let package = DemoPackage {
        drill: Some(DrillRecord::complete(
            INSTALL_READY,
            INSTALL_READY + 5 * 60 * 1000,
            INSTALL_READY + REAL_OUTPUT_TARGET_MS + 1,
        )),
        ..clean_package()
    };
    let findings = audit_demo_package(&package);
    assert!(has(
        &findings,
        "real_output_beyond_target",
        &(REAL_OUTPUT_TARGET_MS + 1).to_string()
    ));
}

#[test]
fn a_drill_exactly_on_the_target_passes() {
    let package = DemoPackage {
        drill: Some(DrillRecord::complete(
            INSTALL_READY,
            INSTALL_READY + FIRST_UTTERANCE_TARGET_MS,
            INSTALL_READY + REAL_OUTPUT_TARGET_MS,
        )),
        ..clean_package()
    };
    assert!(audit_demo_package(&package).is_empty());
    assert_eq!(package.readiness(), DemoReadiness::Ready);
}

#[test]
fn a_drill_earlier_than_installation_is_inconsistent() {
    let package = DemoPackage {
        drill: Some(DrillRecord::complete(
            INSTALL_READY,
            INSTALL_READY - 1,
            INSTALL_READY + 20 * 60 * 1000,
        )),
        ..clean_package()
    };
    let findings = audit_demo_package(&package);
    assert!(has(
        &findings,
        "drill_timeline_inconsistent",
        "first_utterance"
    ));
    // An impossible timestamp is reported as inconsistent, not as "fast".
    assert!(!tokens(&findings).contains(&"first_utterance_beyond_target"));
}

#[test]
fn an_incomplete_drill_record_is_a_defect() {
    let package = DemoPackage {
        drill: Some(DrillRecord {
            install_ready_at_ms: INSTALL_READY,
            first_utterance_at_ms: Some(INSTALL_READY + 60 * 1000),
            real_output_verified_at_ms: None,
        }),
        ..clean_package()
    };
    let findings = audit_demo_package(&package);
    assert!(has(&findings, "drill_record_incomplete", "drill"));
    assert_eq!(package.readiness(), DemoReadiness::Defective);
}

// ------------------------------------------------------------------ readiness precedence

#[test]
fn defective_outranks_blocked() {
    // A boundary violation with no drill at all must not read as Blocked.
    let package = DemoPackage {
        provider_presets: vec![ProviderPreset::embedding_secret("preset.demo.text")],
        drill: None,
        ..clean_package()
    };
    assert_eq!(package.readiness(), DemoReadiness::Defective);
}

#[test]
fn a_clean_rehearsed_package_is_ready() {
    assert_eq!(clean_package().readiness(), DemoReadiness::Ready);
}

// ------------------------------------------------------------------ audit purity

#[test]
fn the_audit_is_a_pure_function_of_the_descriptor() {
    let package = DemoPackage {
        avatar: Some(DemoAvatar::unlicensed("avatar.demo")),
        connectivity: Some(ConnectivityTest::constant().claiming_acceptance()),
        ..clean_package()
    };
    let first = audit_demo_package(&package);
    let second = audit_demo_package(&package);
    assert_eq!(first, second);
    // The audit reports *all* hits, not just the first.
    assert!(tokens(&first).len() >= 3);
}

#[test]
fn every_finding_token_is_constructible() {
    let expected: BTreeSet<&str> = [
        "demo_avatar_missing",
        "demo_avatar_without_license",
        "demo_knowledge_missing",
        "demo_scenario_missing",
        "demo_provider_presets_missing",
        "provider_preset_without_credential",
        "provider_preset_embeds_secret",
        "demo_connectivity_missing",
        "connectivity_probe_is_stub",
        "smoke_claims_acceptance",
        "demo_cost_estimate_missing",
        "cost_unknown_without_reason",
        "drill_record_incomplete",
        "drill_timeline_inconsistent",
        "first_utterance_beyond_target",
        "real_output_beyond_target",
    ]
    .into_iter()
    .collect();

    // "Missing" and "defective" are alternatives for one part, so the union is
    // taken over a small set of descriptors rather than one maximally broken
    // package. Two directions are covered: parts absent, and parts present but
    // violating.
    let absent_avatar = DemoPackage {
        knowledge: vec![KnowledgeSeed::new("kb.demo.1")],
        ..DemoPackage::empty()
    };
    let absent_knowledge = DemoPackage {
        avatar: Some(DemoAvatar::licensed("avatar.demo", "licence:demo-2026")),
        ..DemoPackage::empty()
    };
    let violating = DemoPackage {
        avatar: Some(DemoAvatar::unlicensed("avatar.demo")),
        provider_presets: vec![
            ProviderPreset::embedding_secret("preset.demo.secret"),
            ProviderPreset::without_credential("preset.demo.bare"),
        ],
        connectivity: Some(ConnectivityTest::constant().claiming_acceptance()),
        cost: Some(CostDeclaration::new().with("cloud", CostLine::Unknown(""))),
        drill: Some(DrillRecord::complete(
            INSTALL_READY,
            INSTALL_READY - 1,
            INSTALL_READY + REAL_OUTPUT_TARGET_MS + 1,
        )),
        ..clean_package()
    };
    let late_first_utterance = DemoPackage {
        drill: Some(DrillRecord::complete(
            INSTALL_READY,
            INSTALL_READY + FIRST_UTTERANCE_TARGET_MS + 1,
            INSTALL_READY + 60 * 1000,
        )),
        ..clean_package()
    };
    let incomplete_drill = DemoPackage {
        drill: Some(DrillRecord {
            install_ready_at_ms: INSTALL_READY,
            first_utterance_at_ms: None,
            real_output_verified_at_ms: None,
        }),
        ..clean_package()
    };

    let actual: BTreeSet<&str> = [
        absent_avatar,
        absent_knowledge,
        violating,
        late_first_utterance,
        incomplete_drill,
    ]
    .iter()
    .flat_map(|package| tokens(&audit_demo_package(package)))
    .collect();

    assert_eq!(actual, expected);
}
