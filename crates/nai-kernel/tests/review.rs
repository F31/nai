use nai_contracts::v1::ErrorCode;
use nai_kernel::metrics::{LatencySamples, Percentile};
use nai_kernel::review::*;
use std::collections::BTreeMap;

fn latency() -> LatencySamples {
    let mut samples = LatencySamples::new();
    for _ in 0..20 {
        samples.record(100).unwrap();
    }
    for _ in 0..20 {
        samples.record(200).unwrap();
    }
    samples
}

fn entries() -> SessionEntries {
    SessionEntries {
        interactions: 1_000,
        admitted: 800,
        answered: 700,
        approved: 600,
        modified: 100,
        rejected: 100,
        reject_reasons: BTreeMap::from([
            ("stale_fact".to_owned(), 60),
            ("no_evidence".to_owned(), 40),
        ]),
        guard_hits: BTreeMap::from([
            ((GuardStage::Input, GuardVerdict::Pass), 500),
            ((GuardStage::Input, GuardVerdict::Block), 30),
            ((GuardStage::Output, GuardVerdict::Rewrite), 12),
        ]),
        faults: 2,
        degradations: 1,
        cost_cent: None,
        budget_cent: None,
    }
}

fn clean() -> SessionReview {
    SessionReview::build(&entries(), &latency()).unwrap()
}

fn tokens(review: &SessionReview) -> Vec<&'static str> {
    audit_review(review)
        .into_iter()
        .map(|violation| violation.token)
        .collect()
}

#[test]
fn a_report_keeps_every_ratio_with_its_denominator() {
    let review = clean();
    let admitted = review.admitted_ratio.unwrap();
    assert_eq!((admitted.numerator, admitted.denominator), (800, 1_000));
    assert_eq!(admitted.permille(), Some(800));

    let coverage = review.coverage_ratio.unwrap();
    assert_eq!((coverage.numerator, coverage.denominator), (700, 800));
    assert_eq!(coverage.permille(), Some(875));

    assert_eq!(review.approval_ratio.unwrap().permille(), Some(750));
    assert_eq!(review.modification_ratio.unwrap().permille(), Some(125));
    assert_eq!(review.rejection_ratio.unwrap().permille(), Some(125));
    assert_eq!(review.faults, 2);
    assert_eq!(review.degradations, 1);
}

#[test]
fn incoherent_counters_are_refused() {
    let cases = [
        (
            "admissions exceed interactions",
            SessionEntries {
                interactions: 10,
                admitted: 11,
                ..entries()
            },
        ),
        (
            "answers exceed admissions",
            SessionEntries {
                answered: 801,
                ..entries()
            },
        ),
        (
            "decisions exceed admissions",
            SessionEntries {
                approved: 900,
                ..entries()
            },
        ),
        (
            "rejections unexplained",
            SessionEntries {
                rejected: 90,
                ..entries()
            },
        ),
    ];
    for (label, broken) in cases {
        let error = SessionReview::build(&broken, &latency()).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidArgument, "{label}");
    }
}

#[test]
fn a_session_with_no_interactions_has_no_ratios() {
    let quiet = SessionEntries {
        interactions: 0,
        admitted: 0,
        answered: 0,
        approved: 0,
        modified: 0,
        rejected: 0,
        reject_reasons: BTreeMap::new(),
        guard_hits: BTreeMap::new(),
        ..entries()
    };
    let review = SessionReview::build(&quiet, &LatencySamples::new()).unwrap();
    assert!(review.admitted_ratio.is_none());
    assert!(review.coverage_ratio.is_none());
    assert!(review.approval_ratio.is_none());
    assert!(review.unmeasured.contains(&FIELD_ADMITTED_RATIO));
    assert!(review.unmeasured.contains(&FIELD_COVERAGE_RATIO));
    // Latency was never measured either, and is named rather than zeroed.
    assert!(review.latency.is_empty());
    assert!(review.unmeasured.contains(&FIELD_LATENCY));
    assert_eq!(audit_review(&review), Vec::new());
}

#[test]
fn an_unknown_cost_is_not_zero() {
    let review = clean();
    assert_eq!(review.cost, CostView::Unknown);
    assert!(review.unmeasured.contains(&FIELD_COST));
}

#[test]
fn a_known_cost_tracks_its_budget() {
    let priced = SessionEntries {
        cost_cent: Some(12_345),
        budget_cent: Some(20_000),
        ..entries()
    };
    let review = SessionReview::build(&priced, &latency()).unwrap();
    assert_eq!(
        review.cost,
        CostView::Known {
            cost_cent: 12_345,
            budget_cent: Some(20_000),
            remaining_cent: Some(7_655),
            over_budget: false,
        }
    );
    assert!(!review.unmeasured.contains(&FIELD_COST));

    let over = SessionReview::build(
        &SessionEntries {
            cost_cent: Some(25_000),
            budget_cent: Some(20_000),
            ..entries()
        },
        &latency(),
    )
    .unwrap();
    assert_eq!(
        over.cost,
        CostView::Known {
            cost_cent: 25_000,
            budget_cent: Some(20_000),
            remaining_cent: Some(0),
            over_budget: true,
        }
    );
}

#[test]
fn guard_hits_count_only_non_pass_verdicts() {
    let review = clean();
    assert_eq!(review.guard_hit_total(), 42);
    assert_eq!(
        review
            .guard_hits
            .get(&(GuardStage::Input, GuardVerdict::Pass)),
        Some(&500)
    );
    assert!(!GuardVerdict::Pass.is_hit());
    assert!(GuardVerdict::Escalate.is_hit());
}

#[test]
fn a_clean_report_has_no_violation() {
    assert!(audit_review(&clean()).is_empty());
}

#[test]
fn each_contradiction_is_reported_by_token() {
    assert_eq!(
        tokens(&SessionReview {
            admitted_ratio: Some(Ratio {
                numerator: 0,
                denominator: 0,
            }),
            ..clean()
        }),
        vec!["ratio_without_denominator"]
    );
    assert_eq!(
        tokens(&SessionReview {
            admitted_ratio: Some(Ratio {
                numerator: 1_001,
                denominator: 1_000,
            }),
            ..clean()
        }),
        vec!["ratio_exceeds_unity"]
    );
    assert_eq!(
        tokens(&SessionReview {
            unmeasured: vec![FIELD_ADMITTED_RATIO, FIELD_COST],
            ..clean()
        }),
        vec!["field_measured_but_marked_unmeasured"]
    );
    assert_eq!(
        tokens(&SessionReview {
            coverage_ratio: Some(Ratio {
                numerator: 850,
                denominator: 900,
            }),
            ..clean()
        }),
        vec!["coverage_exceeds_admission"]
    );
    assert_eq!(
        tokens(&SessionReview {
            reject_reasons: BTreeMap::new(),
            ..clean()
        }),
        vec!["rejections_without_reasons"]
    );
    assert_eq!(
        tokens(&SessionReview {
            reject_reasons: BTreeMap::from([("stale_fact".to_owned(), 50)]),
            ..clean()
        }),
        vec!["reasons_do_not_match_rejections"]
    );
    assert_eq!(
        tokens(&SessionReview {
            cost: CostView::Unknown,
            unmeasured: Vec::new(),
            ..clean()
        }),
        vec!["unknown_cost_not_declared"]
    );
    assert_eq!(
        tokens(&SessionReview {
            cost: CostView::Known {
                cost_cent: 0,
                budget_cent: None,
                remaining_cent: None,
                over_budget: false,
            },
            ..clean()
        }),
        vec!["unknown_cost_reported_as_zero"]
    );

    let mut short = clean();
    short.latency.remove(&Percentile::P99);
    assert_eq!(tokens(&short), vec!["partial_latency_not_declared"]);

    let mut unordered = clean();
    unordered.latency.insert(Percentile::P50, 300);
    assert_eq!(tokens(&unordered), vec!["percentiles_out_of_order"]);
}
