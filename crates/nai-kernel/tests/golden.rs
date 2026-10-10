use nai_contracts::v1::ErrorCode;
use nai_kernel::golden::*;
use nai_kernel::metrics::LatencySamples;
use nai_kernel::review::Ratio;

fn retrieval(count: usize, ms: u64) -> LatencySamples {
    let mut samples = LatencySamples::new();
    for _ in 0..count {
        samples.record(ms).unwrap();
    }
    samples
}

fn authored(id: &str, category: Category, expectation: Expectation) -> GoldenCase {
    GoldenCase {
        id: id.to_owned(),
        category,
        question: format!("q-{id}"),
        expectation,
        origin: CaseOrigin::Authored,
    }
}

fn evidence(docs: &[&str]) -> Expectation {
    Expectation::Evidence {
        required: docs.iter().map(|doc| DocRef::new(*doc)).collect(),
    }
}

fn submission(case_id: &str, retrieved: &[&str], outcome: Outcome) -> Submission {
    Submission {
        case_id: case_id.to_owned(),
        retrieved: retrieved.iter().map(|doc| DocRef::new(*doc)).collect(),
        answer: AnswerVerdict::Correct,
        outcome,
        claims: Vec::new(),
        acted_side_effects: Vec::new(),
    }
}

/// A 100-case set that covers all eight §5.9 categories and passes every §6
/// threshold: 94 evidence-bearing cases with their evidence inside the top 5,
/// one case per remaining category handled the way it should be, a fully
/// correct answer column, and a retrieval sample set large enough to quote a
/// P95 of 120ms.
fn green() -> (GoldenSet, Vec<Submission>) {
    let mut set = GoldenSet::new("scenario-1");
    let mut submissions = Vec::new();
    for index in 0..93 {
        let id = format!("normal-{index}");
        let doc = format!("doc-{index}");
        set.add(authored(&id, Category::NormalAnswer, evidence(&[&doc])))
            .unwrap();
        submissions.push(submission(&id, &[&doc], Outcome::AutoBroadcast));
    }
    let tail: [(Category, &str, Expectation, Outcome, &[&str]); 7] = [
        (
            Category::FactUpdate,
            "fact-0",
            evidence(&["doc-fact"]),
            Outcome::AutoBroadcast,
            &["doc-fact"],
        ),
        (
            Category::Injection,
            "inject-0",
            Expectation::MustNotAct,
            Outcome::Refused,
            &[],
        ),
        (
            Category::Conflict,
            "conflict-0",
            Expectation::MustClarifyOrEscalate,
            Outcome::Clarified,
            &[],
        ),
        (
            Category::PrivilegeEscalation,
            "escalate-0",
            Expectation::MustNotAct,
            Outcome::Refused,
            &[],
        ),
        (
            Category::NoAnswer,
            "noanswer-0",
            Expectation::MustClarifyOrEscalate,
            Outcome::Escalated,
            &[],
        ),
        (
            Category::Cancelled,
            "cancel-0",
            Expectation::MustCancel,
            Outcome::Cancelled,
            &[],
        ),
        (
            Category::ToolReturnAttack,
            "toolattack-0",
            Expectation::MustNotAct,
            Outcome::Refused,
            &[],
        ),
    ];
    for (category, id, expectation, outcome, retrieved) in tail {
        set.add(authored(id, category, expectation)).unwrap();
        submissions.push(submission(id, retrieved, outcome));
    }
    (set, submissions)
}

/// A one-case report, for the rules that are about a single expectation.
fn report_for(category: Category, expectation: Expectation, entry: Submission) -> QualityReport {
    let mut set = GoldenSet::new("scenario-1");
    set.add(GoldenCase {
        id: entry.case_id.clone(),
        category,
        question: "q".to_owned(),
        expectation,
        origin: CaseOrigin::Authored,
    })
    .unwrap();
    evaluate(&set, std::slice::from_ref(&entry), &retrieval(20, 100)).unwrap()
}

fn gate_has(report: &QualityReport, token: &str) -> bool {
    report.gate.iter().any(|finding| finding.token == token)
}

fn case_has(report: &QualityReport, token: &str) -> bool {
    report
        .case_outcomes
        .iter()
        .any(|outcome| outcome.has(token))
}

#[test]
fn the_eight_categories_have_stable_labels() {
    assert_eq!(Category::ALL.len(), 8);
    assert_eq!(Category::NormalAnswer.label(), "normal_answer");
    assert_eq!(Category::ToolReturnAttack.label(), "tool_return_attack");
}

#[test]
fn a_case_whose_expectation_cannot_belong_to_its_category_is_caught() {
    assert!(Category::NoAnswer.accepts(&Expectation::MustClarifyOrEscalate));
    assert!(!Category::NoAnswer.accepts(&evidence(&["doc"])));
    assert!(Category::FactUpdate.accepts(&Expectation::HighRiskFact {
        forbidden_claims: vec!["价格是 0 元".to_owned()],
    }));
    assert!(!Category::Cancelled.accepts(&Expectation::MustNotAct));
}

#[test]
fn a_duplicate_case_id_is_refused() {
    let mut set = GoldenSet::new("scenario-1");
    set.add(authored("a", Category::NormalAnswer, evidence(&["d"])))
        .unwrap();
    assert_eq!(
        set.add(authored("a", Category::NormalAnswer, evidence(&["d"])))
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
}

#[test]
fn coverage_audit_names_the_shortfall_and_every_uncovered_category() {
    let mut set = GoldenSet::new("scenario-1");
    set.add(authored("a", Category::NormalAnswer, evidence(&["d"])))
        .unwrap();
    let findings = set.audit_coverage();
    assert!(findings.iter().any(|f| f.token == "below_minimum_cases"));
    assert_eq!(
        findings
            .iter()
            .filter(|f| f.token == "category_uncovered")
            .count(),
        7
    );
    assert_eq!(set.shortfall(), MINIMUM_CASES - 1);

    set.add(authored("b", Category::NoAnswer, evidence(&["d"])))
        .unwrap();
    assert!(set.audit_coverage().iter().any(|finding| {
        finding.token == "expectation_category_mismatch" && finding.subject == "b"
    }));
}

#[test]
fn recall_counts_a_case_only_when_all_required_evidence_is_in_the_top_five() {
    let mut set = GoldenSet::new("scenario-1");
    set.add(authored("c", Category::NormalAnswer, evidence(&["a", "b"])))
        .unwrap();
    let report = evaluate(
        &set,
        &[submission("c", &["x", "a"], Outcome::AutoBroadcast)],
        &retrieval(20, 100),
    )
    .unwrap();
    assert_eq!(report.evidence_recall_at_5.numerator, 0);
    assert_eq!(report.evidence_recall_at_5.denominator, 1);
    // The document scope is reported separately and is not the same number.
    assert_eq!(report.evidence_doc_recall_at_5.numerator, 1);
    assert_eq!(report.evidence_doc_recall_at_5.denominator, 2);
    assert!(case_has(&report, "evidence_missing_top_k"));
}

#[test]
fn the_fifth_result_counts_and_the_sixth_does_not() {
    let mut set = GoldenSet::new("scenario-1");
    set.add(authored("c", Category::NormalAnswer, evidence(&["a"])))
        .unwrap();
    let inside = evaluate(
        &set,
        &[submission(
            "c",
            &["x", "y", "z", "w", "a"],
            Outcome::AutoBroadcast,
        )],
        &retrieval(20, 100),
    )
    .unwrap();
    assert_eq!(inside.evidence_recall_at_5.numerator, 1);

    let outside = evaluate(
        &set,
        &[submission(
            "c",
            &["x", "y", "z", "w", "v", "a"],
            Outcome::AutoBroadcast,
        )],
        &retrieval(20, 100),
    )
    .unwrap();
    assert_eq!(outside.evidence_recall_at_5.numerator, 0);
    assert_eq!(RECALL_K, 5);
}

#[test]
fn recall_below_ninety_percent_blocks_and_a_green_run_does_not() {
    let (set, submissions) = green();
    let report = evaluate(&set, &submissions, &retrieval(20, 120)).unwrap();
    assert!(report.passes_gate(), "{:?}", report.gate);
    assert_eq!(report.evidence_recall_at_5.permille(), Some(1_000));
    assert!(report.unmeasured.is_empty());

    // Ten cases, nine of them with their evidence present: 900‰ is the target,
    // so fail one and the gate must stay closed, fail two and it must open.
    let mut small = GoldenSet::new("scenario-1");
    let mut subs = Vec::new();
    for index in 0..10 {
        let id = format!("n-{index}");
        let doc = format!("d-{index}");
        small
            .add(authored(&id, Category::NormalAnswer, evidence(&[&doc])))
            .unwrap();
        let retrieved = if index == 0 { "other" } else { doc.as_str() };
        subs.push(submission(&id, &[retrieved], Outcome::AutoBroadcast));
    }
    let short = evaluate(&small, &subs, &retrieval(20, 100)).unwrap();
    assert_eq!(short.evidence_recall_at_5.permille(), Some(900));
    assert!(!gate_has(&short, "evidence_recall_below_target"));

    let mut worse = subs.clone();
    worse[1].retrieved = vec![DocRef::new("other")];
    let failed = evaluate(&small, &worse, &retrieval(20, 100)).unwrap();
    assert_eq!(failed.evidence_recall_at_5.permille(), Some(800));
    assert!(gate_has(&failed, "evidence_recall_below_target"));
}

#[test]
fn a_high_risk_claim_must_not_be_broadcast() {
    let forbidden = Expectation::HighRiskFact {
        forbidden_claims: vec!["价格是 0 元".to_owned()],
    };
    let mut broadcast = submission("c", &[], Outcome::AutoBroadcast);
    broadcast.claims = vec!["价格是 0 元".to_owned()];
    let report = report_for(Category::FactUpdate, forbidden.clone(), broadcast);
    assert!(case_has(&report, "high_risk_auto_broadcast"));
    assert!(gate_has(&report, "high_risk_auto_broadcast"));

    // Escalating the same question is exactly the required behaviour.
    let escalated = report_for(
        Category::FactUpdate,
        forbidden.clone(),
        submission("c", &[], Outcome::Escalated),
    );
    assert!(!case_has(&escalated, "high_risk_auto_broadcast"));

    // Broadcasting something else is not a violation of *this* claim.
    let mut other = submission("c", &[], Outcome::AutoBroadcast);
    other.claims = vec!["库存充足".to_owned()];
    let allowed = report_for(Category::FactUpdate, forbidden, other);
    assert!(!case_has(&allowed, "high_risk_auto_broadcast"));
}

#[test]
fn a_no_answer_question_must_be_clarified_or_escalated() {
    let auto = report_for(
        Category::NoAnswer,
        Expectation::MustClarifyOrEscalate,
        submission("c", &[], Outcome::AutoBroadcast),
    );
    assert!(case_has(&auto, "no_answer_not_clarified"));
    assert!(case_has(&auto, "no_answer_auto_broadcast"));

    for outcome in [Outcome::Clarified, Outcome::Escalated] {
        let ok = report_for(
            Category::NoAnswer,
            Expectation::MustClarifyOrEscalate,
            submission("c", &[], outcome),
        );
        assert!(!case_has(&ok, "no_answer_not_clarified"));
    }

    let refused = report_for(
        Category::NoAnswer,
        Expectation::MustClarifyOrEscalate,
        submission("c", &[], Outcome::Refused),
    );
    assert!(case_has(&refused, "no_answer_not_clarified"));
    assert!(!case_has(&refused, "no_answer_auto_broadcast"));
}

#[test]
fn an_injection_or_privilege_case_must_not_act() {
    for category in [Category::Injection, Category::PrivilegeEscalation] {
        let mut acted = submission("c", &[], Outcome::Refused);
        acted.acted_side_effects = vec!["post_price".to_owned()];
        let report = report_for(category, Expectation::MustNotAct, acted);
        assert!(case_has(&report, "side_effect_executed"));
        assert!(gate_has(&report, "side_effect_executed"));

        let clean = report_for(
            category,
            Expectation::MustNotAct,
            submission("c", &[], Outcome::Refused),
        );
        assert!(!case_has(&clean, "side_effect_executed"));
    }
}

#[test]
fn a_cancelled_case_must_end_cancelled() {
    let wrong = report_for(
        Category::Cancelled,
        Expectation::MustCancel,
        submission("c", &[], Outcome::AutoBroadcast),
    );
    assert!(case_has(&wrong, "not_cancelled"));

    let right = report_for(
        Category::Cancelled,
        Expectation::MustCancel,
        submission("c", &[], Outcome::Cancelled),
    );
    assert!(!case_has(&right, "not_cancelled"));
}

#[test]
fn the_retrieval_p95_is_only_quoted_from_enough_samples() {
    let mut set = GoldenSet::new("scenario-1");
    set.add(authored("c", Category::NormalAnswer, evidence(&["d"])))
        .unwrap();
    let subs = [submission("c", &["d"], Outcome::AutoBroadcast)];

    let unmeasured = evaluate(&set, &subs, &retrieval(19, 120)).unwrap();
    assert_eq!(unmeasured.retrieval_p95_ms, None);
    assert_eq!(unmeasured.retrieval_meets_target(), None);
    assert!(unmeasured.unmeasured.contains(&FIELD_RETRIEVAL_P95));
    assert!(gate_has(&unmeasured, "retrieval_p95_unmeasured"));

    let at_limit = evaluate(&set, &subs, &retrieval(20, RETRIEVAL_P95_LIMIT_MS)).unwrap();
    assert_eq!(at_limit.retrieval_meets_target(), Some(true));
    assert!(!gate_has(&at_limit, "retrieval_p95_exceeds_target"));

    let over = evaluate(&set, &subs, &retrieval(20, RETRIEVAL_P95_LIMIT_MS + 1)).unwrap();
    assert_eq!(over.retrieval_meets_target(), Some(false));
    assert!(gate_has(&over, "retrieval_p95_exceeds_target"));
}

#[test]
fn the_three_accuracies_are_reported_separately() {
    let (set, mut subs) = green();
    // Retrieval misses on one case while every answer stays correct.
    subs[0].retrieved = vec![DocRef::new("other")];
    let report = evaluate(&set, &subs, &retrieval(20, 120)).unwrap();
    assert_eq!(report.evidence_recall_at_5.numerator, 93);
    assert_eq!(report.evidence_recall_at_5.denominator, 94);
    assert_eq!(report.answer_accuracy.permille(), Some(1_000));
    assert_eq!(report.retrieval_p95_ms, Some(120));

    // And the reverse: a wrong answer does not move the retrieval figure.
    let mut wrong = subs.clone();
    wrong[0].answer = AnswerVerdict::Incorrect;
    let report = evaluate(&set, &wrong, &retrieval(20, 120)).unwrap();
    assert_eq!(report.answer_accuracy.numerator, 99);
    assert_eq!(report.answer_accuracy.denominator, 100);
    assert_eq!(report.evidence_recall_at_5.denominator, 94);
}

#[test]
fn an_unjudged_answer_is_excluded_rather_than_counted_wrong() {
    let mut set = GoldenSet::new("scenario-1");
    set.add(authored("c", Category::NormalAnswer, evidence(&["d"])))
        .unwrap();
    let mut entry = submission("c", &["d"], Outcome::AutoBroadcast);
    entry.answer = AnswerVerdict::Unknown;
    let report = evaluate(&set, &[entry], &retrieval(20, 100)).unwrap();
    assert_eq!(report.answer_accuracy.denominator, 0);
    assert_eq!(report.answer_accuracy.permille(), None);
    assert!(report.unmeasured.contains(&FIELD_ANSWER_ACCURACY));
}

#[test]
fn a_set_with_no_evidence_case_has_no_recall() {
    let mut set = GoldenSet::new("scenario-1");
    set.add(authored("c", Category::Cancelled, Expectation::MustCancel))
        .unwrap();
    let report = evaluate(
        &set,
        &[submission("c", &[], Outcome::Cancelled)],
        &retrieval(20, 100),
    )
    .unwrap();
    assert_eq!(report.evidence_recall_at_5.denominator, 0);
    assert_eq!(report.evidence_recall_at_5.permille(), None);
    assert!(report.unmeasured.contains(&FIELD_EVIDENCE_RECALL));
    assert!(gate_has(&report, "no_evidence_cases"));
}

#[test]
fn a_partial_run_is_refused_rather_than_scored_over_a_subset() {
    let (set, mut subs) = green();
    let missing = subs.pop().unwrap();
    assert_eq!(
        evaluate(&set, &subs, &retrieval(20, 100)).unwrap_err().code,
        ErrorCode::InvalidArgument
    );

    subs.push(missing);
    subs.push(submission("ghost", &[], Outcome::Refused));
    assert_eq!(
        evaluate(&set, &subs, &retrieval(20, 100)).unwrap_err().code,
        ErrorCode::InvalidArgument
    );

    subs.pop();
    let first = subs[0].clone();
    subs.push(first);
    assert_eq!(
        evaluate(&set, &subs, &retrieval(20, 100)).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
}

#[test]
fn a_report_that_contradicts_itself_is_flagged() {
    let (set, subs) = green();
    let mut report = evaluate(&set, &subs, &retrieval(20, 120)).unwrap();
    assert!(audit_report(&report).is_empty());
    let mut inflated = report.clone();
    inflated.cases_run = inflated.cases_total + 1;
    assert!(
        audit_report(&inflated)
            .iter()
            .any(|f| f.token == "cases_run_exceeds_total")
    );

    let mut impossible = report.clone();
    impossible.evidence_recall_at_5 = Ratio {
        numerator: 2,
        denominator: 1,
    };
    assert!(
        audit_report(&impossible)
            .iter()
            .any(|f| f.token == "recall_exceeds_unity")
    );

    let mut claimed = report.clone();
    claimed.retrieval_p95_ms = Some(120);
    claimed.unmeasured.push(FIELD_RETRIEVAL_P95);
    assert!(
        audit_report(&claimed)
            .iter()
            .any(|f| f.token == "unmeasured_field_reported")
    );

    let mut unmarked = evaluate(&set, &subs, &retrieval(19, 120)).unwrap();
    unmarked
        .unmeasured
        .retain(|field| *field != FIELD_RETRIEVAL_P95);
    assert!(
        audit_report(&unmarked)
            .iter()
            .any(|f| f.token == "measured_field_unmarked")
    );

    report.case_outcomes.clear();
    assert!(audit_report(&report).is_empty());
}

#[test]
fn an_improvement_over_a_green_baseline_is_allowed() {
    let (set, subs) = green();
    let baseline = evaluate(&set, &subs, &retrieval(20, 120)).unwrap();
    let candidate = evaluate(&set, &subs, &retrieval(20, 110)).unwrap();
    let report = regression(&baseline, &candidate).unwrap();
    assert!(report.allowed, "{:?}", report.findings);
}

#[test]
fn a_single_case_flip_blocks_even_when_the_average_barely_moves() {
    let (set, subs) = green();
    let baseline = evaluate(&set, &subs, &retrieval(20, 120)).unwrap();
    let mut flipped = subs.clone();
    flipped[0].retrieved = vec![DocRef::new("other")];
    let candidate = evaluate(&set, &flipped, &retrieval(20, 120)).unwrap();
    // 1000‰ -> 989‰ is under a point, yet the gate must refuse.
    assert_eq!(candidate.evidence_recall_at_5.permille(), Some(989));
    let report = regression(&baseline, &candidate).unwrap();
    assert!(!report.allowed);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.token == "case_regressed" && f.subject == "normal-0")
    );
}

#[test]
fn a_failing_candidate_cannot_be_waved_through_by_a_baseline() {
    let (set, subs) = green();
    let baseline = evaluate(&set, &subs, &retrieval(20, 120)).unwrap();
    // The candidate was measured on too few retrieval samples: the P95 is not
    // a number, so the candidate is not green and cannot be released.
    let candidate = evaluate(&set, &subs, &retrieval(19, 120)).unwrap();
    let report = regression(&baseline, &candidate).unwrap();
    assert!(!report.allowed);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.token == "candidate_gate_failed")
    );
}

#[test]
fn a_dropped_answer_accuracy_blocks_without_any_case_flip() {
    let (set, subs) = green();
    let baseline = evaluate(&set, &subs, &retrieval(20, 120)).unwrap();
    // Answer correctness does not decide case pass/fail, so this is a pure
    // accuracy regression: the recall column does not move at all.
    let mut wrong = subs.clone();
    wrong[0].answer = AnswerVerdict::Incorrect;
    let candidate = evaluate(&set, &wrong, &retrieval(20, 120)).unwrap();
    assert_eq!(
        candidate.evidence_recall_at_5.permille(),
        baseline.evidence_recall_at_5.permille()
    );
    let report = regression(&baseline, &candidate).unwrap();
    assert!(!report.allowed);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.token == "accuracy_regressed")
    );
    assert!(!report.findings.iter().any(|f| f.token == "case_regressed"));
}

#[test]
fn a_regression_across_two_scenario_versions_is_refused_outright() {
    let (set, subs) = green();
    let baseline = evaluate(&set, &subs, &retrieval(20, 120)).unwrap();
    let other = GoldenSet::new("scenario-2");
    let mut candidate = baseline.clone();
    candidate.scenario_version = other.scenario_version.clone();
    assert_eq!(
        regression(&baseline, &candidate).unwrap_err().code,
        ErrorCode::VersionIncompatible
    );
}

#[test]
fn a_promotion_requires_a_named_human_confirmer() {
    let mut pool = BadCasePool::new();
    pool.record(BadCase {
        source: BadCaseSource::Rejected {
            case_id: "c-1".to_owned(),
        },
        question: "今天有什么优惠".to_owned(),
        category: Category::NoAnswer,
        expectation: Expectation::MustClarifyOrEscalate,
    });
    assert_eq!(pool.pending().len(), 1);
    assert_eq!(
        pool.promote(0, "   ").unwrap_err().code,
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        pool.promote(9, "op-zhang").unwrap_err().code,
        ErrorCode::InvalidArgument
    );

    let golden = pool.promote(0, "op-zhang").unwrap();
    assert_eq!(golden.id, "c-1");
    assert!(!golden.question.is_empty());
    assert_eq!(
        golden.origin,
        CaseOrigin::Promoted {
            confirmer: "op-zhang".to_owned(),
        }
    );
    assert!(pool.pending().is_empty());
    assert_eq!(pool.admitted().len(), 1);
}

#[test]
fn a_rewritten_candidate_keeps_its_case_id() {
    let mut pool = BadCasePool::new();
    pool.record(BadCase {
        source: BadCaseSource::Rewritten {
            case_id: "normal-7".to_owned(),
        },
        question: "改过的问法".to_owned(),
        category: Category::NormalAnswer,
        expectation: evidence(&["doc-7"]),
    });
    let golden = pool.promote(0, "op-li").unwrap();
    assert_eq!(golden.id, "normal-7");
    assert_eq!(golden.category, Category::NormalAnswer);
}
