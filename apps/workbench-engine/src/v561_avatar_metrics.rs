//! Consistency of diagnostic work counts, never a playback grant.
use nai_contracts::{v561_avatar::RenderStats, workbench::WorkbenchError as Error};

pub fn validate(
    stats: &RenderStats,
    samples: u64,
    frames: u64,
    frame_num: u32,
    elapsed_ms: u64,
    expected_steps: u32,
) -> Result<(), Error> {
    if samples == 0
        || samples > 144000
        || !(17..=97).contains(&frame_num)
        || elapsed_ms > 45000
        || frames != (samples * 25).div_ceil(24000)
        || stats.block_frames == 0
        || stats.motion_frames == 0
        || stats.block_frames.checked_add(stats.motion_frames) != Some(frame_num)
        || stats.inference_steps != expected_steps
        || stats.feature_sample_rate != 16000
    {
        return Err(Error::ProtocolError);
    }
    let blocks = frames.div_ceil(u64::from(stats.block_frames));
    let generated = blocks * u64::from(stats.block_frames);
    let features = (samples * 2).div_ceil(3);
    if u64::from(stats.blocks) != blocks
        || stats.generated_frames != generated
        || stats.retained_frames != frames
        || stats.transferred_frames != frames
        || stats.tail_frames != generated - frames
        || stats.feature_samples != features
        || stats
            .feature_samples
            .checked_add(stats.padded_feature_samples)
            != Some(generated * 640)
    {
        return Err(Error::ProtocolError);
    }
    let sum = [
        stats.setup_ms,
        stats.compute_ms,
        stats.transfer_ms,
        stats.delivery_ms,
    ]
    .into_iter()
    .try_fold(0u64, |total, value| total.checked_add(value));
    if stats.overlap_ms > stats.compute_ms.min(stats.delivery_ms)
        || sum.is_none_or(|sum| sum.saturating_sub(stats.overlap_ms) > elapsed_ms.saturating_add(5))
    {
        return Err(Error::ProtocolError);
    }
    Ok(())
}
pub fn json(stats: &RenderStats) -> serde_json::Value {
    serde_json::json!({"blocks":stats.blocks,"block_frames":stats.block_frames,"motion_frames":stats.motion_frames,"generated_frames":stats.generated_frames,"retained_frames":stats.retained_frames,"transferred_frames":stats.transferred_frames,"tail_frames":stats.tail_frames,"feature_sample_rate":stats.feature_sample_rate,"feature_samples":stats.feature_samples,"padded_feature_samples":stats.padded_feature_samples,"setup_ms":stats.setup_ms,"compute_ms":stats.compute_ms,"transfer_ms":stats.transfer_ms,"delivery_ms":stats.delivery_ms,"inference_steps":stats.inference_steps,"overlap_ms":stats.overlap_ms})
}
#[cfg(test)]
mod tests {
    use super::*;
    fn stats() -> RenderStats {
        RenderStats {
            blocks: 2,
            block_frames: 40,
            motion_frames: 9,
            generated_frames: 80,
            retained_frames: 48,
            transferred_frames: 48,
            tail_frames: 32,
            feature_sample_rate: 16000,
            feature_samples: 30720,
            padded_feature_samples: 20480,
            setup_ms: 10,
            compute_ms: 3000,
            transfer_ms: 20,
            delivery_ms: 500,
            inference_steps: 4,
            overlap_ms: 0,
        }
    }
    #[test]
    fn actual_work_and_padding_equations_hold() {
        assert!(validate(&stats(), 46080, 48, 49, 3600, 4).is_ok());
        let mut edge = stats();
        edge.blocks = 1;
        edge.generated_frames = 40;
        edge.retained_frames = 1;
        edge.transferred_frames = 1;
        edge.tail_frames = 39;
        edge.feature_samples = 1;
        edge.padded_feature_samples = 25599;
        assert!(validate(&edge, 1, 1, 49, 3600, 4).is_ok());
        assert!(validate(&stats(), 46080, 48, 49, 45001, 4).is_err());
    }
    #[test]
    fn inference_steps_must_match_configured_schedule() {
        let mut two = stats();
        two.inference_steps = 2;
        assert!(validate(&two, 46080, 48, 49, 3600, 2).is_ok());
        assert_eq!(
            validate(&two, 46080, 48, 49, 3600, 4),
            Err(Error::ProtocolError)
        );
        let mut three = stats();
        three.inference_steps = 3;
        assert_eq!(
            validate(&three, 46080, 48, 49, 3600, 2),
            Err(Error::ProtocolError)
        );
        assert_eq!(
            validate(&three, 46080, 48, 49, 3600, 4),
            Err(Error::ProtocolError)
        );
    }
    #[test]
    fn counters_and_duration_tampering_fail() {
        for mode in 0..6 {
            let mut s = stats();
            match mode {
                0 => s.generated_frames = 48,
                1 => s.transferred_frames = 80,
                2 => s.tail_frames = 0,
                3 => s.padded_feature_samples = 0,
                4 => s.blocks = 1,
                _ => s.compute_ms = u64::MAX,
            };
            assert_eq!(
                validate(&s, 46080, 48, 49, 3600, 4),
                Err(Error::ProtocolError)
            );
        }
    }
    #[test]
    fn overlapping_stages_are_bounded_by_each_stage_and_wall_time() {
        let mut s = stats();
        s.overlap_ms = 400;
        assert!(validate(&s, 46080, 48, 49, 3140, 4).is_ok());
        assert!(validate(&s, 46080, 48, 49, 3000, 4).is_err());
        s.overlap_ms = 501;
        assert!(validate(&s, 46080, 48, 49, 3600, 4).is_err());
    }
}
