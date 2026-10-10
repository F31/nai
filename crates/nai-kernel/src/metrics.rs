//! Operations metrics: end-to-end latency percentiles and soak-growth verdicts.
//!
//! §12.5 states the rule that shapes this module: *measure the whole path and do
//! not add the per-stage P95s*. Those two numbers answer different questions,
//! and adding stage percentiles systematically **overstates** the end-to-end
//! figure — so the end-to-end percentile here is computed from end-to-end
//! samples only, and the sum of stage percentiles exists solely as a labelled
//! comparison, never as a substitute. §12.5 also fixes the reliability targets
//! (template utterance, API utterance, local mute, control response, frame
//! skew) and the soak windows (2h / 8h / 24h with no sustained resource
//! growth).
//!
//! A metric that was not measured has **no value**. A batch of ten samples is
//! not a P95, and a forty-minute run is not a soak. Both cases report
//! `Insufficient` with the reason rather than a number, because a number would
//! be read as a result.
use nai_contracts::v1::ErrorCode;
use std::collections::BTreeMap;

/// Stable code plus a portable token, matching the `UpgradeError` /
/// `MultiplexError` convention so a report can classify a refusal without a
/// debug build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricsError {
    pub code: ErrorCode,
    pub reason: &'static str,
}

impl std::fmt::Display for MetricsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "metrics: {} ({})", self.code.as_str_name(), self.reason)
    }
}

impl std::error::Error for MetricsError {}

fn error(code: ErrorCode, reason: &'static str) -> MetricsError {
    MetricsError { code, reason }
}

type Result<T> = std::result::Result<T, MetricsError>;

/// Smallest sample set that may be quoted as a percentile. Below this the only
/// honest answer is "not measured".
pub const MIN_PERCENTILE_SAMPLES: usize = 20;

/// The percentiles §12.5 quotes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Percentile {
    P50,
    P95,
    P99,
}

impl Percentile {
    pub const ALL: [Percentile; 3] = [Percentile::P50, Percentile::P95, Percentile::P99];

    pub fn rank(self) -> u8 {
        match self {
            Percentile::P50 => 50,
            Percentile::P95 => 95,
            Percentile::P99 => 99,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Percentile::P50 => "p50",
            Percentile::P95 => "p95",
            Percentile::P99 => "p99",
        }
    }
}

/// The stages of one interaction, kept apart so a report can show *where* time
/// went — never so the parts can be added into a total.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    Ingest,
    Retrieve,
    Decision,
    Guard,
    Synthesis,
    FirstAudio,
}

impl Stage {
    pub const ALL: [Stage; 6] = [
        Stage::Ingest,
        Stage::Retrieve,
        Stage::Decision,
        Stage::Guard,
        Stage::Synthesis,
        Stage::FirstAudio,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Stage::Ingest => "ingest",
            Stage::Retrieve => "retrieve",
            Stage::Decision => "decision",
            Stage::Guard => "guard",
            Stage::Synthesis => "synthesis",
            Stage::FirstAudio => "first_audio",
        }
    }
}

/// Nearest-rank percentile over a sorted slice: `ceil(p/100 * n)`, clamped to
/// the last element. Deterministic, so a report is reproducible.
fn nearest_rank(sorted: &[u64], percentile: Percentile) -> u64 {
    let n = sorted.len();
    let rank = (percentile.rank() as usize * n).div_ceil(100);
    sorted[rank.saturating_sub(1).min(n - 1)]
}

fn median(sorted: &[u64]) -> u64 {
    sorted[sorted.len() / 2]
}

/// End-to-end latency samples plus a per-stage breakdown. The two live in the
/// same value only so a report can print them side by side.
#[derive(Debug, Default)]
pub struct LatencySamples {
    end_to_end: Vec<u64>,
    stages: BTreeMap<Stage, Vec<u64>>,
}

impl LatencySamples {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one whole-path sample, in milliseconds.
    pub fn record(&mut self, total_ms: u64) -> Result<()> {
        if total_ms == 0 {
            return Err(error(ErrorCode::InvalidArgument, "zero latency sample"));
        }
        self.end_to_end.push(total_ms);
        Ok(())
    }

    /// Record one stage sample. Stage samples never contribute to the
    /// end-to-end percentile.
    pub fn record_stage(&mut self, stage: Stage, ms: u64) -> Result<()> {
        if ms == 0 {
            return Err(error(ErrorCode::InvalidArgument, "zero stage sample"));
        }
        self.stages.entry(stage).or_default().push(ms);
        Ok(())
    }

    pub fn end_to_end_count(&self) -> usize {
        self.end_to_end.len()
    }

    pub fn stage_count(&self, stage: Stage) -> usize {
        self.stages.get(&stage).map_or(0, Vec::len)
    }

    fn sorted_end_to_end(&self) -> Vec<u64> {
        let mut sorted = self.end_to_end.clone();
        sorted.sort_unstable();
        sorted
    }

    /// The end-to-end percentile. Refuses to quote a number from too few
    /// samples rather than returning a small-sample artefact.
    pub fn percentile(&self, percentile: Percentile) -> Result<u64> {
        if self.end_to_end_count() < MIN_PERCENTILE_SAMPLES {
            return Err(error(
                ErrorCode::CapabilityNotReady,
                "not enough end-to-end samples for a percentile",
            ));
        }
        Ok(nearest_rank(&self.sorted_end_to_end(), percentile))
    }

    /// One stage's percentile, for locating where time went.
    pub fn stage_percentile(&self, stage: Stage, percentile: Percentile) -> Result<u64> {
        let samples = self
            .stages
            .get(&stage)
            .map(Vec::as_slice)
            .unwrap_or_default();
        if samples.len() < MIN_PERCENTILE_SAMPLES {
            return Err(error(
                ErrorCode::CapabilityNotReady,
                "not enough stage samples for a percentile",
            ));
        }
        let mut sorted = samples.to_vec();
        sorted.sort_unstable();
        Ok(nearest_rank(&sorted, percentile))
    }

    /// The tempting wrong answer §12.5 forbids: adding the per-stage
    /// percentiles. It is exposed **labelled** so a report can show the gap,
    /// and it must never be presented as the end-to-end figure.
    pub fn sum_of_stage_percentiles(&self, percentile: Percentile) -> Result<u64> {
        let mut total = 0_u64;
        for stage in Stage::ALL {
            total = total.saturating_add(self.stage_percentile(stage, percentile)?);
        }
        Ok(total)
    }

    /// End-to-end minus the sum of stage percentiles, for the report. A
    /// negative "gap" means the stages were measured in separate runs; the
    /// number is reported, not interpreted.
    pub fn stage_sum_gap(&self, percentile: Percentile) -> Result<i64> {
        let end_to_end = self.percentile(percentile)? as i64;
        let summed = self.sum_of_stage_percentiles(percentile)? as i64;
        Ok(end_to_end - summed)
    }
}

/// A §12.5 reliability target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatencyTarget {
    pub name: &'static str,
    pub percentile: Percentile,
    pub limit_ms: u64,
}

/// The §12.5 targets that are per-interaction latencies. Occupancy and recovery
/// targets (mute, standby, frame skew) are measured by other means.
pub const DESIGN_TARGETS: [LatencyTarget; 3] = [
    LatencyTarget {
        name: "template_utterance",
        percentile: Percentile::P95,
        limit_ms: 1_500,
    },
    LatencyTarget {
        name: "api_utterance",
        percentile: Percentile::P95,
        limit_ms: 3_000,
    },
    LatencyTarget {
        name: "control_response",
        percentile: Percentile::P95,
        limit_ms: 50,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetVerdict {
    Meets { p_ms: u64 },
    Exceeds { p_ms: u64 },
}

/// Compare a sample set against a target. Too few samples is an error, not a
/// pass: an unmeasured target is exactly what a report must say.
pub fn evaluate_target(samples: &LatencySamples, target: &LatencyTarget) -> Result<TargetVerdict> {
    let p_ms = samples.percentile(target.percentile)?;
    Ok(if p_ms <= target.limit_ms {
        TargetVerdict::Meets { p_ms }
    } else {
        TargetVerdict::Exceeds { p_ms }
    })
}

/// A §12.5 soak window. The window is a **minimum**: a run shorter than it is
/// not evidence about that window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoakWindow {
    pub name: &'static str,
    pub span_ms: u64,
}

pub const TWO_HOURS: SoakWindow = SoakWindow {
    name: "2h",
    span_ms: 2 * 60 * 60 * 1_000,
};
pub const EIGHT_HOURS: SoakWindow = SoakWindow {
    name: "8h",
    span_ms: 8 * 60 * 60 * 1_000,
};
pub const RELEASE_CANDIDATE: SoakWindow = SoakWindow {
    name: "24h",
    span_ms: 24 * 60 * 60 * 1_000,
};

pub const SOAK_WINDOWS: [SoakWindow; 3] = [TWO_HOURS, EIGHT_HOURS, RELEASE_CANDIDATE];

/// How a soak is judged. `max_drift_pct` is the largest growth of the late-half
/// median over the early-half median that still counts as "no sustained
/// growth"; `min_samples` guards the median against a two-point "trend".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoakPolicy {
    pub window: SoakWindow,
    pub max_drift_pct: u64,
    pub min_samples: usize,
}

/// Default minimum sample count for a soak. Below this, halves would be judged
/// on a handful of points.
pub const MIN_SOAK_SAMPLES: usize = 8;

impl SoakPolicy {
    pub fn new(window: SoakWindow, max_drift_pct: u64) -> Self {
        Self {
            window,
            max_drift_pct,
            min_samples: MIN_SOAK_SAMPLES,
        }
    }
}

impl Default for SoakPolicy {
    fn default() -> Self {
        Self::new(TWO_HOURS, 10)
    }
}

/// Time-ordered resource samples, e.g. RSS or GPU memory.
#[derive(Debug, Default)]
pub struct SoakSamples {
    samples: Vec<(u64, u64)>,
}

impl SoakSamples {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one sample. Time must not go backwards, so a "trend" cannot be
    /// manufactured by reordering.
    pub fn record(&mut self, at_ms: u64, value: u64) -> Result<()> {
        if let Some((last, _)) = self.samples.last()
            && at_ms < *last
        {
            return Err(error(
                ErrorCode::InvalidArgument,
                "soak samples out of order",
            ));
        }
        self.samples.push((at_ms, value));
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Observed span from first to last sample.
    pub fn span_ms(&self) -> u64 {
        match (self.samples.first(), self.samples.last()) {
            (Some((first, _)), Some((last, _))) => last.saturating_sub(*first),
            _ => 0,
        }
    }

    fn half_values(&self) -> (Vec<u64>, Vec<u64>) {
        let (Some((start, _)), Some((_, _))) = (self.samples.first(), self.samples.last()) else {
            return (Vec::new(), Vec::new());
        };
        let midpoint = start + self.span_ms() / 2;
        let mut head = Vec::new();
        let mut tail = Vec::new();
        for (at_ms, value) in &self.samples {
            if *at_ms < midpoint {
                head.push(*value);
            } else {
                tail.push(*value);
            }
        }
        head.sort_unstable();
        tail.sort_unstable();
        (head, tail)
    }

    /// Judge the run against a window and drift budget.
    ///
    /// The verdict compares the **medians of the two halves**, not the first
    /// and last points: a single spike at the end must not read as growth, and
    /// a single spike at the start must not hide it. A run shorter than the
    /// window, or with too few samples, is `Insufficient` — never `Stable`.
    pub fn assess(&self, policy: &SoakPolicy) -> SoakVerdict {
        if self.len() < policy.min_samples || self.span_ms() < policy.window.span_ms {
            return SoakVerdict::Insufficient {
                span_ms: self.span_ms(),
                samples: self.len(),
            };
        }
        let (head, tail) = self.half_values();
        if head.is_empty() || tail.is_empty() {
            return SoakVerdict::Insufficient {
                span_ms: self.span_ms(),
                samples: self.len(),
            };
        }
        let head_median = median(&head);
        let tail_median = median(&tail);
        let drift_pct =
            (tail_median as i64 - head_median as i64) * 100 / (head_median.max(1) as i64);
        if drift_pct > policy.max_drift_pct as i64 {
            SoakVerdict::Growing {
                drift_pct,
                head_median,
                tail_median,
            }
        } else {
            SoakVerdict::Stable {
                drift_pct,
                head_median,
                tail_median,
            }
        }
    }
}

/// Outcome of a soak assessment. `Insufficient` carries what was observed, so a
/// short run reports its actual span instead of a pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoakVerdict {
    Stable {
        drift_pct: i64,
        head_median: u64,
        tail_median: u64,
    },
    Growing {
        drift_pct: i64,
        head_median: u64,
        tail_median: u64,
    },
    Insufficient {
        span_ms: u64,
        samples: usize,
    },
}

impl SoakVerdict {
    pub fn is_conclusive(&self) -> bool {
        !matches!(self, SoakVerdict::Insufficient { .. })
    }
}
