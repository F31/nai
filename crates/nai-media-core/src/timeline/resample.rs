//! Bounded, deterministic sample conversion/resampling. Ordering is exact so a
//! given input always maps to a deterministic output; output is capped.
use crate::MediaError;

/// Convert a mono i16 frame to f32 in [-1.0, 1.0].
pub fn i16_to_f32(frame: i16) -> f32 {
    frame as f32 / 32768.0
}
/// Convert a mono f32 frame back to i16 with saturation.
pub fn f32_to_i16(frame: f32) -> i16 {
    let clamped = frame.clamp(-1.0, 1.0) * 32767.0;
    clamped.round() as i16
}

/// Deterministic linear-interpolation resampler for mono i16. Output length is
/// `ceil(input * to / from)` bounded by `max_output`; oversized results error.
pub fn resample_linear_i16(
    input: &[i16],
    from_rate: u32,
    to_rate: u32,
    max_output: usize,
) -> Result<Vec<i16>, MediaError> {
    if input.is_empty() || from_rate == 0 || to_rate == 0 {
        return Err(MediaError::Stale);
    }
    if from_rate == to_rate {
        if input.len() > max_output {
            return Err(MediaError::Capacity);
        }
        return Ok(input.to_vec());
    }
    let output_len = (input.len() as u128 * to_rate as u128).div_ceil(from_rate as u128);
    let output_len = usize::try_from(output_len).map_err(|_| MediaError::Capacity)?;
    if output_len > max_output {
        return Err(MediaError::Capacity);
    }
    let mut output = Vec::with_capacity(output_len);
    let step = from_rate as f64 / to_rate as f64;
    let mut source = 0.0f64;
    for _ in 0..output_len {
        let lower = source.floor() as usize;
        if lower >= input.len() - 1 {
            output.push(*input.last().unwrap());
        } else {
            let frac = source - lower as f64;
            let interpolated = input[lower] as f64 * (1.0 - frac) + input[lower + 1] as f64 * frac;
            output.push(interpolated.round().clamp(i16::MIN as f64, i16::MAX as f64) as i16);
        }
        source += step;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_rate_is_a_copy() {
        let input = vec![1i16, 2, 3];
        assert_eq!(
            resample_linear_i16(&input, 16_000, 16_000, 8).unwrap(),
            input
        );
    }

    #[test]
    fn upsampling_16k_to_48k_triples_len_and_is_bounded() {
        let input: Vec<i16> = (0..160).map(|frame| (frame as i16) * 100).collect();
        let output = resample_linear_i16(&input, 16_000, 48_000, 10_000).unwrap();
        assert_eq!(output.len(), 480);
        assert!(output.iter().all(|frame| (*frame as i32).abs() < 34_000));
        assert!(resample_linear_i16(&input, 16_000, 48_000, 100).is_err());
    }

    #[test]
    fn empty_or_zero_rate_is_rejected() {
        assert_eq!(
            resample_linear_i16(&[], 16_000, 48_000, 8),
            Err(MediaError::Stale)
        );
        assert_eq!(
            resample_linear_i16(&[1], 0, 48_000, 8),
            Err(MediaError::Stale)
        );
    }
}
