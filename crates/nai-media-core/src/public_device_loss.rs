//! Device-stream failure stops presentation. This is not a hardware mute ACK.
use nai_contracts::workbench::PublicWindowPrepared;

pub const DEVICE_LOST: &str = "DEVICE_LOST";

/// Clears the clock observation and marks execution stopped.
/// The caller still revokes the frame board and the audio fence.
pub fn apply_device_loss(view: &mut PublicWindowPrepared) {
    view.device_clock = None;
    view.frame = None;
    view.playing = false;
    view.execution_stopped = true;
    view.device_ready = false;
    view.execution_challenge = 0;
    view.reason = DEVICE_LOST.into();
}

#[cfg(test)]
mod tests {
    use super::*;
    use nai_contracts::workbench::{PublicDeviceClockObservation, PublicMediaFrameInfo};

    #[test]
    fn device_loss_clears_the_clock_and_the_frame() {
        let mut view = PublicWindowPrepared {
            playing: true,
            device_ready: true,
            execution_stopped: false,
            execution_challenge: 7,
            reason: "PUBLIC_EXECUTING".into(),
            device_clock: Some(PublicDeviceClockObservation {
                stream_generation: 1,
                device_frequency: 384000,
                device_position: 100,
                sample_rate: 24000,
                played_samples: 800,
                ..Default::default()
            }),
            frame: Some(PublicMediaFrameInfo {
                frame_index: 2,
                sample_start: 800,
                sample_count: 800,
                sample_rate: 24000,
                ..Default::default()
            }),
            ..Default::default()
        };
        apply_device_loss(&mut view);
        assert!(view.device_clock.is_none());
        assert!(view.frame.is_none());
        assert!(!view.playing);
        assert!(view.execution_stopped);
        assert!(!view.device_ready);
        assert_eq!(view.execution_challenge, 0);
        assert_eq!(view.reason, DEVICE_LOST);
    }
}
