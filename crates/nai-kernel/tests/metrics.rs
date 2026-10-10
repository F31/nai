use nai_contracts::v1::ErrorCode;
use nai_kernel::metrics::*;

fn samples(values: &[u64]) -> LatencySamples {
    let mut samples = LatencySamples::new();
    for value in values {
        samples.record(*value).unwrap();
    }
    samples
}

fn ramp(count: u64) -> LatencySamples {
    let mut samples = LatencySamples::new();
    for value in 1..=count {
        samples.record(value).unwrap();
    }
    samples
}

#[test]
fn zero_latency_is_rejected() {
    let mut samples = LatencySamples::new();
    assert_eq!(
        samples.record(0).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(
        samples.record_stage(Stage::Ingest, 0).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
}

#[test]
fn a_percentile_needs_enough_samples() {
    let mut samples = LatencySamples::new();
    for value in 1..MIN_PERCENTILE_SAMPLES as u64 {
        samples.record(value).unwrap();
    }
    for percentile in Percentile::ALL {
        assert_eq!(
            samples.percentile(percentile).unwrap_err().code,
            ErrorCode::CapabilityNotReady,
            "{}",
            percentile.label()
        );
    }
    samples.record(1).unwrap();
    assert!(samples.percentile(Percentile::P95).is_ok());
}

#[test]
fn nearest_rank_percentiles_match_the_definition() {
    let samples = ramp(100);
    assert_eq!(samples.percentile(Percentile::P50).unwrap(), 50);
    assert_eq!(samples.percentile(Percentile::P95).unwrap(), 95);
    assert_eq!(samples.percentile(Percentile::P99).unwrap(), 99);
    // Nearest rank never interpolates: the P95 of 100 samples is an existing
    // sample, not an average of two.
    assert_eq!(samples.end_to_end_count(), 100);
}

#[test]
fn the_end_to_end_percentile_ignores_stage_samples() {
    // End-to-end samples are all 100 ms, while each stage is filled with 40 ms
    // samples. §12.5 forbids quoting the stage sum as the end-to-end figure.
    let mut samples = samples(&[100; MIN_PERCENTILE_SAMPLES]);
    for stage in Stage::ALL {
        for _ in 0..MIN_PERCENTILE_SAMPLES {
            samples.record_stage(stage, 40).unwrap();
        }
    }
    assert_eq!(samples.percentile(Percentile::P95).unwrap(), 100);
    let summed = samples.sum_of_stage_percentiles(Percentile::P95).unwrap();
    assert_eq!(summed, 40 * Stage::ALL.len() as u64);
    assert_ne!(summed, samples.percentile(Percentile::P95).unwrap());
    assert_eq!(
        samples.stage_sum_gap(Percentile::P95).unwrap(),
        100 - summed as i64
    );
}

#[test]
fn a_stage_percentile_needs_its_own_samples() {
    let mut samples = LatencySamples::new();
    for _ in 0..MIN_PERCENTILE_SAMPLES {
        samples.record(100).unwrap();
    }
    // End-to-end is measurable, but no stage was ever sampled.
    assert!(samples.percentile(Percentile::P95).is_ok());
    assert_eq!(samples.stage_count(Stage::Ingest), 0);
    assert_eq!(
        samples
            .stage_percentile(Stage::Ingest, Percentile::P95)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityNotReady
    );
    // And the sum cannot be produced from missing stages.
    assert_eq!(
        samples
            .sum_of_stage_percentiles(Percentile::P95)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityNotReady
    );
}

#[test]
fn targets_are_meet_or_exceeded_and_never_assumed() {
    let template = DESIGN_TARGETS[0];
    assert_eq!(template.name, "template_utterance");
    assert_eq!(template.limit_ms, 1_500);

    // P95 of 1400 ms is exactly at the limit: meets.
    let under = samples(&[1_400; MIN_PERCENTILE_SAMPLES]);
    assert_eq!(
        evaluate_target(&under, &template).unwrap(),
        TargetVerdict::Meets { p_ms: 1_400 }
    );

    // P95 of 1500 ms is at the limit: still meets (inclusive).
    let edge = samples(&[1_500; MIN_PERCENTILE_SAMPLES]);
    assert_eq!(
        evaluate_target(&edge, &template).unwrap(),
        TargetVerdict::Meets { p_ms: 1_500 }
    );

    let over = samples(&[1_600; MIN_PERCENTILE_SAMPLES]);
    assert_eq!(
        evaluate_target(&over, &template).unwrap(),
        TargetVerdict::Exceeds { p_ms: 1_600 }
    );

    // Too few samples is an error, not a pass.
    let thin = samples(&[100; 3]);
    assert_eq!(
        evaluate_target(&thin, &template).unwrap_err().code,
        ErrorCode::CapabilityNotReady
    );
}

fn soak(windows: usize, head_value: u64, tail_value: u64) -> SoakSamples {
    let mut samples = SoakSamples::new();
    let span = TWO_HOURS.span_ms;
    let midpoint = span / 2;
    for index in 0..windows {
        let at = span / (windows as u64 - 1) * index as u64;
        let value = if at < midpoint {
            head_value
        } else {
            tail_value
        };
        samples.record(at, value).unwrap();
    }
    samples
}

#[test]
fn soak_samples_must_be_time_ordered() {
    let mut samples = SoakSamples::new();
    samples.record(1_000, 10).unwrap();
    assert_eq!(
        samples.record(999, 10).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    samples.record(1_000, 12).unwrap();
    assert_eq!(samples.len(), 2);
}

#[test]
fn a_short_run_is_insufficient_not_stable() {
    let policy = SoakPolicy::new(TWO_HOURS, 10);

    // Too few samples.
    let mut few = SoakSamples::new();
    few.record(0, 100).unwrap();
    few.record(TWO_HOURS.span_ms, 100).unwrap();
    assert!(matches!(
        few.assess(&policy),
        SoakVerdict::Insufficient { samples: 2, .. }
    ));

    // Enough samples but a span shorter than the window.
    let mut short = SoakSamples::new();
    let step = TWO_HOURS.span_ms / 20;
    for index in 0..MIN_SOAK_SAMPLES + 2 {
        short.record(step * index as u64, 100).unwrap();
    }
    let verdict = short.assess(&policy);
    assert!(matches!(verdict, SoakVerdict::Insufficient { .. }));
    assert!(!verdict.is_conclusive());
}

#[test]
fn a_flat_run_is_stable() {
    let policy = SoakPolicy::new(TWO_HOURS, 10);
    let verdict = soak(9, 100, 100).assess(&policy);
    assert_eq!(
        verdict,
        SoakVerdict::Stable {
            drift_pct: 0,
            head_median: 100,
            tail_median: 100
        }
    );
    assert!(verdict.is_conclusive());
}

#[test]
fn a_rising_run_is_growing() {
    let policy = SoakPolicy::new(TWO_HOURS, 10);
    let verdict = soak(9, 100, 150).assess(&policy);
    assert!(matches!(
        verdict,
        SoakVerdict::Growing { drift_pct: 50, .. }
    ));
}

#[test]
fn a_single_end_spike_does_not_read_as_growth() {
    // The tail half carries one 300 spike among 100s; the median ignores it, so
    // one noisy sample cannot fail a healthy soak.
    let policy = SoakPolicy::new(TWO_HOURS, 10);
    let mut samples = SoakSamples::new();
    let span = TWO_HOURS.span_ms;
    for index in 0..9u64 {
        let at = span / 8 * index;
        samples.record(at, 100).unwrap();
    }
    samples.record(span, 300).unwrap();
    assert_eq!(
        samples.assess(&policy),
        SoakVerdict::Stable {
            drift_pct: 0,
            head_median: 100,
            tail_median: 100
        }
    );
}

#[test]
fn a_single_start_spike_does_not_hide_growth() {
    // Mirror of the previous case: a low start spike must not mask a rising tail.
    let policy = SoakPolicy::new(TWO_HOURS, 10);
    let mut samples = SoakSamples::new();
    let span = TWO_HOURS.span_ms;
    samples.record(0, 1).unwrap();
    for index in 1..9u64 {
        let at = span / 8 * index;
        let value = if at < span / 2 { 100 } else { 150 };
        samples.record(at, value).unwrap();
    }
    assert!(matches!(
        samples.assess(&policy),
        SoakVerdict::Growing { drift_pct: 50, .. }
    ));
}

#[test]
fn the_window_is_a_minimum_not_a_nominal_label() {
    // A 2h run judged against the 8h window is not evidence about 8h.
    let samples = soak(17, 100, 100);
    assert!(samples.span_ms() >= TWO_HOURS.span_ms);
    assert!(
        samples
            .assess(&SoakPolicy::new(TWO_HOURS, 10))
            .is_conclusive()
    );
    assert!(matches!(
        samples.assess(&SoakPolicy::new(EIGHT_HOURS, 10)),
        SoakVerdict::Insufficient { .. }
    ));
    assert_eq!(SOAK_WINDOWS.len(), 3);
}
