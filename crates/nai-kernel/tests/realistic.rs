use nai_kernel::realistic::*;

// ------------------------------------------------------------------ helpers

fn rights_full() -> RightsEvidence {
    let mut rights = RightsEvidence::new();
    for kind in RightKind::ALL {
        rights.set(
            kind,
            RightStatus::Established {
                basis: format!("licence-{}", kind.label()),
            },
        );
    }
    rights.pipeline_obtainable = true;
    rights
}

fn correctness_aligned() -> CorrectnessEvidence {
    let mut correctness = CorrectnessEvidence::new();
    for stage in Stage::ALL {
        correctness.set(stage, StageAlignment::Aligned);
    }
    correctness.fixed_inputs = "fixed-input-set-a".to_owned();
    correctness
}

fn correctness_full() -> CorrectnessEvidence {
    let mut correctness = correctness_aligned();
    correctness.certify(Precision::Fp32);
    correctness
}

fn frame_rate_at(fps: u64) -> FrameRate {
    let mut rate = FrameRate::new();
    // Three windows, all at the same rate, so the floor is that rate.
    for _ in 0..3 {
        rate.record(fps, 1_000).unwrap();
    }
    rate
}

fn skew(value_ms: u64) -> nai_kernel::metrics::LatencySamples {
    let mut samples = nai_kernel::metrics::LatencySamples::new();
    for _ in 0..20 {
        samples.record(value_ms).unwrap();
    }
    samples
}

fn placement_full_gpu() -> PlacementReport {
    PlacementReport {
        cuda_ep_registered: true,
        node_placements: vec![
            NodePlacement {
                node: "encoder".to_owned(),
                device: Device::Gpu,
            },
            NodePlacement {
                node: "denoiser".to_owned(),
                device: Device::Gpu,
            },
        ],
        gpu_copies: 0,
        claims_full_gpu: true,
    }
}

fn realtime_full() -> RealtimeEvidence {
    RealtimeEvidence {
        target_machine: "win-cuda-box".to_owned(),
        concurrent_with_live_tool: true,
        frame_rate: Some(frame_rate_at(MIN_STABLE_FPS)),
        frame_skew: skew(40),
        vram_mb: Some(6_144),
        cpu_pct: Some(38),
        temperature_c: Some(64),
        placement: placement_full_gpu(),
    }
}

fn stability_full() -> StabilityEvidence {
    let mut stability = StabilityEvidence::new();
    for scenario in StabilityScenario::ALL {
        stability.set(
            scenario,
            ScenarioResult::Passed {
                evidence: format!("soak-{}", scenario.label()),
            },
        );
    }
    stability
}

fn report() -> GateReport {
    GateReport {
        profile: Profile::Standard,
        model_present: true,
        realistic_not_enabled: false,
        rights: rights_full(),
        correctness: correctness_full(),
        realtime: realtime_full(),
        stability: stability_full(),
        fallback: None,
        acceptance_claimed_via_fallback: false,
    }
}

fn tokens(report: &GateReport) -> Vec<&'static str> {
    audit_gate(report).into_iter().map(|f| f.token).collect()
}

// ------------------------------------------------------------- gates overall

#[test]
fn the_four_gates_have_stable_labels() {
    assert_eq!(
        Gate::ALL.map(Gate::label),
        [
            "g0_source_and_rights",
            "g1_correctness",
            "g2_realtime",
            "g3_stability",
        ]
    );
}

#[test]
fn a_fully_measured_report_is_accepted() {
    let report = report();
    for gate in Gate::ALL {
        assert_eq!(report.verdict(gate), GateVerdict::Passed, "{gate:?}");
    }
    assert_eq!(report.acceptance(), Acceptance::Accepted);
    assert!(audit_gate(&report).is_empty());
}

#[test]
fn one_unmeasured_gate_keeps_the_admission_out_of_accepted() {
    let mut report = report();
    // No temperature was read: the realtime gate is not measured.
    report.realtime.temperature_c = None;
    assert_eq!(report.verdict(Gate::Realtime), GateVerdict::Unmeasured);
    assert_eq!(report.verdict(Gate::SourceAndRights), GateVerdict::Passed);
    assert_eq!(report.verdict(Gate::Stability), GateVerdict::Passed);
    assert_eq!(report.acceptance(), Acceptance::Unmeasured);
}

#[test]
fn a_single_failed_gate_is_not_accepted() {
    let mut report = report();
    report.stability.set(
        StabilityScenario::Oom,
        ScenarioResult::Failed {
            reason: "did not mute".to_owned(),
        },
    );
    assert_eq!(report.verdict(Gate::Stability), GateVerdict::Failed);
    assert_eq!(report.acceptance(), Acceptance::NotAccepted);
}

#[test]
fn a_missing_model_is_blocked() {
    let mut report = report();
    report.model_present = false;
    assert_eq!(report.acceptance(), Acceptance::Blocked);
}

#[test]
fn a_gate_that_passed_with_no_model_is_audited() {
    let mut report = report();
    report.model_present = false;
    let tokens = tokens(&report);
    assert!(
        tokens.contains(&"gates_evaluated_without_model"),
        "{tokens:?}"
    );
}

#[test]
fn lite_declaring_realistic_disabled_is_not_applicable() {
    let mut report = report();
    report.profile = Profile::Lite;
    report.realistic_not_enabled = true;
    assert_eq!(report.acceptance(), Acceptance::NotApplicable);
    assert!(audit_gate(&report).is_empty());
}

#[test]
fn standard_claiming_not_applicable_is_audited() {
    let mut report = report();
    report.profile = Profile::Standard;
    report.realistic_not_enabled = true;
    assert_ne!(report.acceptance(), Acceptance::NotApplicable);
    assert!(tokens(&report).contains(&"not_applicable_for_target_capability"));
}

// ------------------------------------------------------------------- G0

#[test]
fn all_five_rights_are_required() {
    assert_eq!(RightKind::ALL.len(), 5);
    assert_eq!(rights_full().verdict(), GateVerdict::Passed);
}

#[test]
fn a_missing_right_fails_the_gate() {
    let mut rights = rights_full();
    rights.set(RightKind::Portrait, RightStatus::Absent);
    assert_eq!(rights.verdict(), GateVerdict::Failed);
    assert_eq!(rights.missing(), vec![RightKind::Portrait]);
}

#[test]
fn a_blank_right_basis_is_not_established() {
    let mut rights = rights_full();
    rights.set(
        RightKind::Voice,
        RightStatus::Established {
            basis: "   ".to_owned(),
        },
    );
    assert!(!rights.established(RightKind::Voice));
    assert_eq!(rights.verdict(), GateVerdict::Unmeasured);

    let mut report = report();
    report.rights = rights;
    assert!(tokens(&report).contains(&"established_right_without_basis"));
}

#[test]
fn a_component_left_in_an_undeclared_service_fails_the_gate() {
    let mut rights = rights_full();
    rights
        .undeclared_components
        .push("vae-decode (python)".to_owned());
    assert_eq!(rights.verdict(), GateVerdict::Failed);

    let mut report = report();
    report.rights = rights;
    assert!(tokens(&report).contains(&"undeclared_component"));
}

#[test]
fn an_unobtainable_pipeline_fails_the_gate() {
    let mut rights = rights_full();
    rights.pipeline_obtainable = false;
    assert_eq!(rights.verdict(), GateVerdict::Failed);
}

// ------------------------------------------------------------------- G1

#[test]
fn all_four_stages_must_align() {
    assert_eq!(Stage::ALL.len(), 4);
    assert_eq!(correctness_full().verdict(), GateVerdict::Passed);

    let mut correctness = correctness_full();
    correctness.set(
        Stage::Generation,
        StageAlignment::Diverged { max_abs_delta: 3 },
    );
    assert_eq!(correctness.verdict(), GateVerdict::Failed);
}

#[test]
fn a_stage_not_run_is_unmeasured() {
    let mut correctness = correctness_full();
    correctness.set(Stage::Postprocess, StageAlignment::NotRun);
    assert_eq!(correctness.verdict(), GateVerdict::Unmeasured);
}

#[test]
fn fixed_inputs_must_be_named() {
    let mut correctness = correctness_full();
    correctness.fixed_inputs = String::new();
    assert_eq!(correctness.verdict(), GateVerdict::Unmeasured);
}

#[test]
fn fp16_certified_without_fp32_fails() {
    let mut correctness = correctness_aligned();
    correctness.certify(Precision::Fp16);
    assert_eq!(correctness.verdict(), GateVerdict::Failed);

    let mut report = report();
    report.correctness = correctness;
    assert!(tokens(&report).contains(&"precision_without_fp32_baseline"));
}

#[test]
fn int8_without_calibration_fails() {
    let mut correctness = correctness_full();
    correctness.certify(Precision::Int8);
    assert_eq!(correctness.verdict(), GateVerdict::Failed);

    let mut report = report();
    report.correctness = correctness;
    assert!(tokens(&report).contains(&"int8_without_calibration"));
}

#[test]
fn int8_certified_with_fp32_and_calibration_passes() {
    let mut correctness = correctness_full();
    correctness.certify(Precision::Int8);
    correctness.int8_calibration_report = Some("calibration-report-1".to_owned());
    assert_eq!(correctness.verdict(), GateVerdict::Passed);
}

// ------------------------------------------------------------------- G2

#[test]
fn realtime_must_be_measured_on_the_target_machine() {
    let mut realtime = realtime_full();
    realtime.target_machine = "  ".to_owned();
    assert_eq!(realtime.verdict(), GateVerdict::Unmeasured);
}

#[test]
fn realtime_must_be_concurrent_with_the_live_tool() {
    let mut realtime = realtime_full();
    realtime.concurrent_with_live_tool = false;
    assert_eq!(realtime.verdict(), GateVerdict::Failed);
}

#[test]
fn a_frame_rate_below_twenty_five_fails_and_exactly_twenty_five_passes() {
    let mut realtime = realtime_full();
    realtime.frame_rate = Some(frame_rate_at(MIN_STABLE_FPS - 1));
    assert_eq!(realtime.verdict(), GateVerdict::Failed);

    let mut realtime = realtime_full();
    realtime.frame_rate = Some(frame_rate_at(MIN_STABLE_FPS));
    assert_eq!(realtime.verdict(), GateVerdict::Passed);
}

#[test]
fn the_frame_rate_floor_is_what_counts_not_the_average() {
    let mut rate = FrameRate::new();
    rate.record(30, 1_000).unwrap();
    rate.record(30, 1_000).unwrap();
    rate.record(15, 1_000).unwrap();
    assert_eq!(rate.min_fps_permille(), Some(15_000));
    assert_eq!(rate.meets_min_fps(), Some(false));
}

#[test]
fn frame_skew_p95_above_eighty_fails_and_exactly_eighty_passes() {
    let mut realtime = realtime_full();
    realtime.frame_skew = skew(FRAME_SKEW_P95_LIMIT_MS + 1);
    assert_eq!(realtime.verdict(), GateVerdict::Failed);

    let mut realtime = realtime_full();
    realtime.frame_skew = skew(FRAME_SKEW_P95_LIMIT_MS);
    assert_eq!(realtime.verdict(), GateVerdict::Passed);
}

#[test]
fn a_frame_skew_with_too_few_samples_is_unmeasured() {
    let mut realtime = realtime_full();
    let mut samples = nai_kernel::metrics::LatencySamples::new();
    for _ in 0..10 {
        samples.record(40).unwrap();
    }
    realtime.frame_skew = samples;
    assert_eq!(realtime.verdict(), GateVerdict::Unmeasured);
}

#[test]
fn a_missing_realtime_sample_is_unmeasured() {
    let mut realtime = realtime_full();
    realtime.cpu_pct = None;
    assert_eq!(realtime.verdict(), GateVerdict::Unmeasured);

    let mut realtime = realtime_full();
    realtime.vram_mb = None;
    assert_eq!(realtime.verdict(), GateVerdict::Unmeasured);
}

#[test]
fn cuda_ep_registration_is_not_placement() {
    let placement = PlacementReport {
        cuda_ep_registered: true,
        node_placements: Vec::new(),
        gpu_copies: 0,
        claims_full_gpu: true,
    };
    assert!(!placement.is_full_gpu());
    assert!(placement.falsely_claims_full_gpu());

    let mut realtime = realtime_full();
    realtime.placement = placement;
    assert_eq!(realtime.verdict(), GateVerdict::Failed);

    let mut report = report();
    report.realtime = realtime;
    let tokens = tokens(&report);
    assert!(
        tokens.contains(&"cuda_ep_registered_without_placement"),
        "{tokens:?}"
    );
    assert!(
        tokens.contains(&"full_gpu_claimed_without_placement_records"),
        "{tokens:?}"
    );
}

#[test]
fn full_gpu_claimed_with_cpu_fallback_fails() {
    let placement = PlacementReport {
        cuda_ep_registered: true,
        node_placements: vec![NodePlacement {
            node: "postproc".to_owned(),
            device: Device::Cpu,
        }],
        gpu_copies: 2,
        claims_full_gpu: true,
    };
    assert_eq!(placement.cpu_fallbacks(), 1);
    assert!(placement.falsely_claims_full_gpu());

    let mut report = report();
    report.realtime.placement = placement;
    assert_eq!(report.verdict(Gate::Realtime), GateVerdict::Failed);
    assert!(tokens(&report).contains(&"full_gpu_claimed_with_cpu_fallback"));
}

#[test]
fn recorded_cpu_fallbacks_that_meet_the_targets_still_pass() {
    let mut report = report();
    report.realtime.placement = PlacementReport {
        cuda_ep_registered: true,
        node_placements: vec![
            NodePlacement {
                node: "denoiser".to_owned(),
                device: Device::Gpu,
            },
            NodePlacement {
                node: "postproc".to_owned(),
                device: Device::Cpu,
            },
        ],
        gpu_copies: 1,
        claims_full_gpu: false,
    };
    assert_eq!(report.verdict(Gate::Realtime), GateVerdict::Passed);
    assert!(audit_gate(&report).is_empty());
}

// ------------------------------------------------------------------- G3

#[test]
fn all_seven_stability_scenarios_are_required() {
    assert_eq!(StabilityScenario::ALL.len(), 7);
    assert_eq!(stability_full().verdict(), GateVerdict::Passed);
}

#[test]
fn a_failed_stability_scenario_fails_the_gate() {
    let mut stability = stability_full();
    stability.set(
        StabilityScenario::ProcessRestart,
        ScenarioResult::Failed {
            reason: "no re-authorization".to_owned(),
        },
    );
    assert_eq!(stability.verdict(), GateVerdict::Failed);
}

#[test]
fn a_stability_scenario_not_run_is_unmeasured() {
    let mut stability = stability_full();
    stability.set(
        StabilityScenario::AvatarSwitchAndOutputRecovery,
        ScenarioResult::NotRun,
    );
    assert_eq!(stability.verdict(), GateVerdict::Unmeasured);
}

#[test]
fn a_stability_pass_without_evidence_fails() {
    let mut stability = stability_full();
    stability.set(
        StabilityScenario::Cancel,
        ScenarioResult::Passed {
            evidence: "  ".to_owned(),
        },
    );
    assert_eq!(stability.verdict(), GateVerdict::Failed);

    let mut report = report();
    report.stability = stability;
    assert!(tokens(&report).contains(&"scenario_passed_without_evidence"));
}

// -------------------------------------------------------------- fallback

#[test]
fn fallback_does_not_rescue_a_gate() {
    let mut report = report();
    report.stability.set(
        StabilityScenario::Mute,
        ScenarioResult::Failed {
            reason: "channel stayed open".to_owned(),
        },
    );
    report.fallback = Some(FallbackRecord {
        reason: "gpu error".to_owned(),
        at_sentence_boundary: true,
        operator_authorized: true,
        logged: true,
    });
    assert_eq!(report.verdict(Gate::Stability), GateVerdict::Failed);
    assert_eq!(report.acceptance(), Acceptance::NotAccepted);
}

#[test]
fn fallback_claimed_as_acceptance_is_audited() {
    let mut report = report();
    report.acceptance_claimed_via_fallback = true;
    assert!(tokens(&report).contains(&"fallback_claimed_as_acceptance"));
    // The claim never turns the admission into an acceptance.
    assert_eq!(report.acceptance(), Acceptance::Accepted);
    assert!(!audit_gate(&report).is_empty());
}

#[test]
fn fallback_requires_authorization_boundary_and_log() {
    let mut report = report();
    report.fallback = Some(FallbackRecord {
        reason: "gpu error".to_owned(),
        at_sentence_boundary: false,
        operator_authorized: false,
        logged: false,
    });
    let tokens = tokens(&report);
    assert!(
        tokens.contains(&"fallback_without_authorization"),
        "{tokens:?}"
    );
    assert!(
        tokens.contains(&"fallback_outside_sentence_boundary"),
        "{tokens:?}"
    );
    assert!(tokens.contains(&"fallback_unlogged"), "{tokens:?}");
}

#[test]
fn a_compliant_fallback_alone_has_no_findings() {
    let mut report = report();
    report.fallback = Some(FallbackRecord {
        reason: "gpu driver reset".to_owned(),
        at_sentence_boundary: true,
        operator_authorized: true,
        logged: true,
    });
    assert!(audit_gate(&report).is_empty());
}
