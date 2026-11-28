//! Structural checks only. The owner must separately authenticate the pool,
//! clock source and epoch before using any message.
use crate::v561_media::{
    ClockState, FrameRef, FrameRelease, PixelFormat, PlaybackClockObservation,
};

#[derive(Clone, Copy)]
pub struct FrameLimits {
    pub slots: u32,
    pub max_width: u32,
    pub max_height: u32,
    pub max_payload_bytes: u64,
    pub max_sample_rate: u32,
}

pub fn validate_frame(frame: &FrameRef, limits: FrameLimits) -> Result<(), &'static str> {
    if frame.pool_id.is_empty()
        || frame.pool_id.len() > 128
        || !frame
            .pool_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("invalid pool identity");
    }
    if frame.epoch == 0
        || frame.sequence.is_none()
        || frame.source_sample.is_none()
        || frame.pts_ns.is_none()
        || frame.slot_generation == 0
    {
        return Err("missing frame identity or timeline");
    }
    if frame.slot_index.is_none_or(|slot| slot >= limits.slots)
        || frame.source_sample_rate == 0
        || frame.source_sample_rate > limits.max_sample_rate
    {
        return Err("invalid slot or sample rate");
    }
    if frame.width == 0
        || frame.height == 0
        || frame.width > limits.max_width
        || frame.height > limits.max_height
        || frame.pixel_format != PixelFormat::Bgra8 as i32
    {
        return Err("invalid frame format");
    }
    let min_stride = frame.width.checked_mul(4).ok_or("stride overflow")?;
    let size = u64::from(frame.stride_bytes)
        .checked_mul(u64::from(frame.height))
        .ok_or("size overflow")?;
    if frame.stride_bytes < min_stride
        || size != frame.payload_bytes
        || size > limits.max_payload_bytes
        || (!frame.content_sha256.is_empty() && frame.content_sha256.len() != 32)
    {
        return Err("invalid frame size or digest");
    }
    Ok(())
}

pub fn release_matches(frame: &FrameRef, release: &FrameRelease) -> bool {
    frame.sequence.is_some()
        && frame.slot_index.is_some()
        && frame.epoch != 0
        && frame.slot_generation != 0
        && frame.pool_id == release.pool_id
        && frame.epoch == release.epoch
        && frame.sequence == release.sequence
        && frame.slot_index == release.slot_index
        && frame.slot_generation == release.slot_generation
}

pub fn validate_clock(
    clock: &PlaybackClockObservation,
    max_sample_rate: u32,
) -> Result<(), &'static str> {
    if clock.epoch == 0 || clock.device_generation == 0 || clock.observation_sequence.is_none() {
        return Err("missing clock identity");
    }
    match ClockState::try_from(clock.state) {
        Ok(ClockState::Running | ClockState::Paused | ClockState::Stopped)
            if clock.played_sample.is_some()
                && clock.sample_rate > 0
                && clock.sample_rate <= max_sample_rate =>
        {
            Ok(())
        }
        Ok(ClockState::Unavailable) if clock.played_sample.is_none() && clock.sample_rate == 0 => {
            Ok(())
        }
        _ => Err("invalid clock observation"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    fn frame() -> FrameRef {
        FrameRef {
            pool_id: "pool-1".into(),
            epoch: 1,
            sequence: Some(0),
            source_sample: Some(0),
            source_sample_rate: 24000,
            pts_ns: Some(0),
            width: 2,
            height: 2,
            stride_bytes: 8,
            pixel_format: PixelFormat::Bgra8 as i32,
            slot_index: Some(0),
            slot_generation: 1,
            payload_bytes: 16,
            content_sha256: vec![],
        }
    }
    fn limits() -> FrameLimits {
        FrameLimits {
            slots: 3,
            max_width: 1920,
            max_height: 1080,
            max_payload_bytes: 1920 * 1080 * 4,
            max_sample_rate: 48000,
        }
    }

    #[test]
    fn zero_values_survive_wire_and_missing_values_fail() {
        let decoded = FrameRef::decode(frame().encode_to_vec().as_slice()).unwrap();
        assert!(validate_frame(&decoded, limits()).is_ok());
        let mut missing = decoded;
        missing.sequence = None;
        assert!(validate_frame(&missing, limits()).is_err());
    }

    #[test]
    fn python_generated_protobuf_interoperates() {
        // Produced independently by protoc's Python FrameRef serializer.
        let python_wire = [
            0x0a, 0x06, 0x70, 0x6f, 0x6f, 0x6c, 0x2d, 0x31, 0x10, 0x01, 0x18, 0x00, 0x20, 0x00,
            0x28, 0xc0, 0xbb, 0x01, 0x30, 0x00, 0x38, 0x02, 0x40, 0x02, 0x48, 0x08, 0x50, 0x01,
            0x58, 0x00, 0x60, 0x01, 0x68, 0x10,
        ];
        let decoded = FrameRef::decode(python_wire.as_slice()).unwrap();
        assert_eq!(decoded, frame());
        assert_eq!(decoded.encode_to_vec(), python_wire);
    }

    #[test]
    fn reject_wrong_generation_and_invalid_geometry() {
        let f = frame();
        let mut release = FrameRelease {
            pool_id: f.pool_id.clone(),
            epoch: f.epoch,
            sequence: f.sequence,
            slot_index: f.slot_index,
            slot_generation: f.slot_generation,
        };
        assert!(release_matches(&f, &release));
        release.slot_generation += 1;
        assert!(!release_matches(&f, &release));
        let mut bad = f;
        bad.stride_bytes = 7;
        assert!(validate_frame(&bad, limits()).is_err());
    }

    #[test]
    fn clock_rejects_planned_or_missing_samples() {
        let mut c = PlaybackClockObservation {
            epoch: 1,
            device_generation: 1,
            observation_sequence: Some(0),
            state: ClockState::Unavailable as i32,
            played_sample: None,
            sample_rate: 0,
        };
        assert!(validate_clock(&c, 48000).is_ok());
        c.state = ClockState::Running as i32;
        assert!(validate_clock(&c, 48000).is_err());
        c.played_sample = Some(0);
        c.sample_rate = 24000;
        assert!(validate_clock(&c, 48000).is_ok());
    }
}
