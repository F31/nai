//! Session review report (§5.9, last bullet).
//!
//! §5.9 ends with the fields a 场次复盘报告 must contain: interaction volume and
//! admission rate, answer coverage, the approved / modified / rejected mix with
//! reasons, latency percentiles, faults and degradations, cost against budget,
//! and the guard-hit breakdown. §5.4 supplies the guard classification: four
//! stages (input / evidence / output / preplay) and four verdicts (pass /
//! rewrite / block / escalate).
//!
//! Two rules shape the type. First, **a report is a projection, not a
//! measurement**: it is built from counters handed in by the caller, and
//! [`SessionReview::build`] refuses an incoherent set (answers exceeding
//! admissions, rejections unexplained by their reasons) rather than printing
//! numbers that cannot all be true. Second, **an unmeasured field is not zero**
//! — a session with no interactions has *no* coverage ratio, not 0%, and an
//! unknown cost is `Unknown`, not ¥0. Unmeasured fields are named in
//! [`SessionReview::unmeasured`], and [`audit_review`] flags any report that
//! contradicts itself.
use crate::metrics::{LatencySamples, MetricsError, Percentile};
use nai_contracts::v1::ErrorCode;
use std::collections::BTreeMap;

fn error(code: ErrorCode, reason: &'static str) -> MetricsError {
    MetricsError { code, reason }
}

type Result<T> = std::result::Result<T, MetricsError>;

/// §5.4 guard stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GuardStage {
    Input,
    Evidence,
    Output,
    Preplay,
}

impl GuardStage {
    pub const ALL: [GuardStage; 4] = [
        GuardStage::Input,
        GuardStage::Evidence,
        GuardStage::Output,
        GuardStage::Preplay,
    ];

    pub fn label(self) -> &'static str {
        match self {
            GuardStage::Input => "input",
            GuardStage::Evidence => "evidence",
            GuardStage::Output => "output",
            GuardStage::Preplay => "preplay",
        }
    }
}

/// §5.4 guard verdicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GuardVerdict {
    Pass,
    Rewrite,
    Block,
    Escalate,
}

impl GuardVerdict {
    pub const ALL: [GuardVerdict; 4] = [
        GuardVerdict::Pass,
        GuardVerdict::Rewrite,
        GuardVerdict::Block,
        GuardVerdict::Escalate,
    ];

    pub fn label(self) -> &'static str {
        match self {
            GuardVerdict::Pass => "pass",
            GuardVerdict::Rewrite => "rewrite",
            GuardVerdict::Block => "block",
            GuardVerdict::Escalate => "escalate",
        }
    }

    /// A hit is any verdict that is not a plain pass; §5.9 asks for the
    /// breakdown, so both are reported.
    pub fn is_hit(self) -> bool {
        self != GuardVerdict::Pass
    }
}

/// Counters for one session, handed in by the caller.
#[derive(Debug, Clone, Default)]
pub struct SessionEntries {
    pub interactions: u64,
    pub admitted: u64,
    pub answered: u64,
    pub approved: u64,
    pub modified: u64,
    pub rejected: u64,
    /// Why each rejection happened. Must explain every rejection.
    pub reject_reasons: BTreeMap<String, u64>,
    pub guard_hits: BTreeMap<(GuardStage, GuardVerdict), u64>,
    pub faults: u64,
    pub degradations: u64,
    /// Cost is optional: an unpriced session is `Unknown`, never ¥0.
    pub cost_cent: Option<u64>,
    pub budget_cent: Option<u64>,
}

/// A ratio kept as its parts, so a report shows the denominator it came from
/// instead of a rounded percentage nobody can check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ratio {
    pub numerator: u64,
    pub denominator: u64,
}

impl Ratio {
    /// Parts-per-thousand, or `None` when the denominator is zero. Returning
    /// `None` keeps "no denominator" distinct from "zero per thousand".
    pub fn permille(&self) -> Option<u64> {
        if self.denominator == 0 {
            return None;
        }
        Some(self.numerator.saturating_mul(1_000) / self.denominator)
    }

    pub fn exceeds_unity(&self) -> bool {
        self.numerator > self.denominator
    }
}

/// Cost line of the report. `Unknown` is a distinct state, not a zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostView {
    Known {
        cost_cent: u64,
        budget_cent: Option<u64>,
        remaining_cent: Option<u64>,
        over_budget: bool,
    },
    Unknown,
}

/// The report itself. Fields are public so [`audit_review`] can be a pure
/// function of a reported value and every violation is constructible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionReview {
    pub interactions: u64,
    pub admitted: u64,
    pub answered: u64,
    pub admitted_ratio: Option<Ratio>,
    pub coverage_ratio: Option<Ratio>,
    pub approval_ratio: Option<Ratio>,
    pub modification_ratio: Option<Ratio>,
    pub rejection_ratio: Option<Ratio>,
    pub reject_reasons: BTreeMap<String, u64>,
    pub guard_hits: BTreeMap<(GuardStage, GuardVerdict), u64>,
    pub faults: u64,
    pub degradations: u64,
    pub latency: BTreeMap<Percentile, u64>,
    /// The forbidden-but-tempting stage sum, carried only as a labelled
    /// comparison against the end-to-end percentile (§12.5).
    pub stage_sum_p95: Option<u64>,
    pub cost: CostView,
    /// Names of fields that could not be measured, so a reader never mistakes
    /// an absent number for a zero.
    pub unmeasured: Vec<&'static str>,
}

pub const FIELD_ADMITTED_RATIO: &str = "admitted_ratio";
pub const FIELD_COVERAGE_RATIO: &str = "coverage_ratio";
pub const FIELD_APPROVAL_RATIO: &str = "approval_ratio";
pub const FIELD_REJECTION_RATIO: &str = "rejection_ratio";
pub const FIELD_LATENCY: &str = "latency";
pub const FIELD_COST: &str = "cost";

impl SessionReview {
    /// Project a report from counters. Refuses a set that cannot all be true:
    /// answers exceeding admissions, processed candidates exceeding admissions,
    /// or rejections not fully explained by their reasons.
    pub fn build(entries: &SessionEntries, latency: &LatencySamples) -> Result<Self> {
        if entries.admitted > entries.interactions {
            return Err(error(
                ErrorCode::InvalidArgument,
                "admissions exceed interactions",
            ));
        }
        if entries.answered > entries.admitted {
            return Err(error(
                ErrorCode::InvalidArgument,
                "answers exceed admissions",
            ));
        }
        let processed = entries.approved + entries.modified + entries.rejected;
        if processed > entries.admitted {
            return Err(error(
                ErrorCode::InvalidArgument,
                "decisions exceed admissions",
            ));
        }
        let explained: u64 = entries.reject_reasons.values().sum();
        if explained != entries.rejected {
            return Err(error(
                ErrorCode::InvalidArgument,
                "rejections are not fully explained by reasons",
            ));
        }

        let mut unmeasured = Vec::new();
        let mut ratio = |numerator: u64, denominator: u64, field: &'static str| {
            if denominator == 0 {
                unmeasured.push(field);
                None
            } else {
                Some(Ratio {
                    numerator,
                    denominator,
                })
            }
        };
        let admitted_ratio = ratio(entries.admitted, entries.interactions, FIELD_ADMITTED_RATIO);
        let coverage_ratio = ratio(entries.answered, entries.admitted, FIELD_COVERAGE_RATIO);
        let approval_ratio = ratio(entries.approved, entries.admitted, FIELD_APPROVAL_RATIO);
        let modification_ratio = ratio(entries.modified, entries.admitted, "modification_ratio");
        let rejection_ratio = ratio(entries.rejected, entries.admitted, FIELD_REJECTION_RATIO);

        let mut percentiles = BTreeMap::new();
        for percentile in Percentile::ALL {
            match latency.percentile(percentile) {
                Ok(value) => {
                    percentiles.insert(percentile, value);
                }
                Err(_) => unmeasured.push(FIELD_LATENCY),
            }
        }
        let stage_sum_p95 = latency.sum_of_stage_percentiles(Percentile::P95).ok();

        let cost = match entries.cost_cent {
            Some(cost_cent) => {
                let (budget_cent, remaining_cent, over_budget) = match entries.budget_cent {
                    Some(budget_cent) => (
                        Some(budget_cent),
                        Some(budget_cent.saturating_sub(cost_cent)),
                        cost_cent > budget_cent,
                    ),
                    None => (None, None, false),
                };
                CostView::Known {
                    cost_cent,
                    budget_cent,
                    remaining_cent,
                    over_budget,
                }
            }
            None => {
                unmeasured.push(FIELD_COST);
                CostView::Unknown
            }
        };

        unmeasured.sort_unstable();
        unmeasured.dedup();

        Ok(Self {
            interactions: entries.interactions,
            admitted: entries.admitted,
            answered: entries.answered,
            admitted_ratio,
            coverage_ratio,
            approval_ratio,
            modification_ratio,
            rejection_ratio,
            reject_reasons: entries.reject_reasons.clone(),
            guard_hits: entries.guard_hits.clone(),
            faults: entries.faults,
            degradations: entries.degradations,
            latency: percentiles,
            stage_sum_p95,
            cost,
            unmeasured,
        })
    }

    pub fn guard_hit_total(&self) -> u64 {
        self.guard_hits
            .iter()
            .filter(|((_, verdict), _)| verdict.is_hit())
            .map(|(_, count)| *count)
            .sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewViolation {
    pub token: &'static str,
    pub detail: String,
}

/// Check a reported review for self-contradiction. Returns every violation, in
/// a fixed order, so a report can be classified without a debug build.
pub fn audit_review(review: &SessionReview) -> Vec<ReviewViolation> {
    let mut violations = Vec::new();
    let mut push = |token: &'static str, detail: &str| {
        violations.push(ReviewViolation {
            token,
            detail: detail.to_owned(),
        });
    };

    let named = |field: &'static str| review.unmeasured.contains(&field);

    for (field, value) in [
        (FIELD_ADMITTED_RATIO, review.admitted_ratio),
        (FIELD_COVERAGE_RATIO, review.coverage_ratio),
        (FIELD_APPROVAL_RATIO, review.approval_ratio),
        ("modification_ratio", review.modification_ratio),
        (FIELD_REJECTION_RATIO, review.rejection_ratio),
    ] {
        if let Some(ratio) = value {
            if ratio.denominator == 0 {
                push("ratio_without_denominator", field);
            } else if ratio.exceeds_unity() {
                push("ratio_exceeds_unity", field);
            }
            if named(field) {
                push("field_measured_but_marked_unmeasured", field);
            }
        }
    }

    if review
        .coverage_ratio
        .is_some_and(|r| r.numerator > review.admitted)
    {
        push("coverage_exceeds_admission", "answers exceed admissions");
    }

    let explained: u64 = review.reject_reasons.values().sum();
    let rejected = review.rejection_ratio.map_or(0, |r| r.numerator);
    if rejected > 0 && review.reject_reasons.is_empty() {
        push("rejections_without_reasons", "no reason recorded");
    } else if explained != rejected {
        push("reasons_do_not_match_rejections", "reason counts differ");
    }

    if let CostView::Known { cost_cent, .. } = review.cost {
        if cost_cent == 0 && named(FIELD_COST) {
            push("unknown_cost_reported_as_zero", "cost is unknown, not ¥0");
        }
    } else if !named(FIELD_COST) {
        push(
            "unknown_cost_not_declared",
            "unknown cost not marked unmeasured",
        );
    }

    if review.latency.len() < Percentile::ALL.len() && !named(FIELD_LATENCY) {
        push("partial_latency_not_declared", "missing percentiles");
    }
    if let (Some(p50), Some(p99)) = (
        review.latency.get(&Percentile::P50),
        review.latency.get(&Percentile::P99),
    ) && p50 > p99
    {
        push("percentiles_out_of_order", "p50 exceeds p99");
    }

    violations
}
