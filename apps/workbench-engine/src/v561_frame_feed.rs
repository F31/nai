//! Three-frame transfer channel; producer backpressure never masks parent stop.
use nai_contracts::workbench::WorkbenchError as Error;
use nai_media_core::v561_frame_scheduler::Frame;
use nai_runtime_core::foundation::Operation;
use std::{sync::mpsc, time::Duration};
pub const TRANSFER_FRAMES: usize = 3;
pub const RESERVE_FRAMES: usize = TRANSFER_FRAMES + 3; // producer, selected and screen
pub const ADMISSION_GUARD_MS: u64 = 200;
/// A delay estimate, never an output grant or a sustainable-throughput claim.
pub fn admit_after_ms(
    total: usize,
    block: usize,
    first_frame: u64,
    first_block: u64,
    complete: bool,
) -> Result<u64, Error> {
    if total == 0 || total > 150 || block == 0 || block > 97 || first_frame > first_block {
        return Err(Error::InvalidArgument);
    }
    if complete {
        return Ok(first_block);
    }
    let period = block as u64 * 40;
    let debt = first_frame
        .saturating_sub(period)
        .checked_mul((total.div_ceil(block) - 1) as u64)
        .ok_or(Error::ResourceExhausted)?;
    let target = first_frame
        .checked_add(debt)
        .and_then(|v| v.checked_add(ADMISSION_GUARD_MS))
        .ok_or(Error::ResourceExhausted)?
        .max(first_block);
    if target > 45000 {
        return Err(Error::DeadlineExceeded);
    }
    Ok(target)
}
pub fn selection(frame: Option<&Frame>, played: u64, total: u64) -> Result<Option<u64>, Error> {
    if total == 0 || played > total {
        return Err(Error::ProtocolError);
    }
    if played == total {
        return Ok(None);
    }
    let frame = frame.ok_or(Error::CapabilityUnavailable)?;
    let start = frame.metadata.source_sample.ok_or(Error::ProtocolError)?;
    if frame.metadata.source_sample_rate != 24000
        || start > played
        || start.checked_add(960).is_none_or(|end| end <= played)
    {
        return Err(Error::CapabilityUnavailable);
    }
    Ok(Some(frame.metadata.sequence.ok_or(Error::ProtocolError)?))
}
pub fn channel() -> (mpsc::SyncSender<Frame>, mpsc::Receiver<Frame>) {
    mpsc::sync_channel(TRANSFER_FRAMES)
}
pub fn send(tx: &mpsc::SyncSender<Frame>, mut frame: Frame, op: &Operation) -> Result<(), Error> {
    loop {
        op.check()?;
        match tx.try_send(frame) {
            Ok(()) => return Ok(()),
            Err(mpsc::TrySendError::Full(value)) => frame = value,
            Err(mpsc::TrySendError::Disconnected(_)) => return Err(Error::Cancelled),
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
pub fn capacity(
    width: u32,
    height: u32,
    total: usize,
    block: usize,
) -> Result<(usize, usize), Error> {
    if total == 0
        || total > 150
        || block == 0
        || block > 97
        || !(256..=1024).contains(&width)
        || !(256..=1024).contains(&height)
    {
        return Err(Error::InvalidArgument);
    }
    let pixels = u64::from(width) * u64::from(height) * 4;
    // Root pixel memory, including channel, selected, producer-owned and screen.
    let available = (268435456 / pixels).saturating_sub(RESERVE_FRAMES as u64) as usize;
    let prefill = block.min(total);
    let capacity = (block * 2).min(total).min(available);
    if capacity < prefill {
        return Err(Error::ResourceExhausted);
    }
    Ok((prefill, capacity))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn frame() -> Frame {
        Frame {
            metadata: Default::default(),
            pixels: vec![],
        }
    }
    #[test]
    fn admission_adds_finite_supply_debt_without_inventing_progress() {
        assert_eq!(admit_after_ms(96, 40, 1800, 1950, false), Ok(2400));
        assert_eq!(admit_after_ms(96, 40, 1800, 1950, true), Ok(1950));
        assert_eq!(admit_after_ms(96, 40, 1000, 1200, false), Ok(1200));
        assert!(admit_after_ms(96, 0, 1000, 1200, false).is_err());
        assert!(admit_after_ms(150, 1, 45000, 45000, false).is_err());
    }
    #[test]
    fn memory_bound_and_short_source_prefill() {
        assert_eq!(capacity(768, 768, 96, 40), Ok((40, 80)));
        assert_eq!(capacity(768, 768, 1, 40), Ok((1, 1)));
        assert!(capacity(1024, 1024, 150, 88).is_err());
        let (prefill, count) = capacity(768, 768, 150, 88).unwrap();
        assert!(count >= prefill && (count + RESERVE_FRAMES) * 768 * 768 * 4 <= 268435456);
    }
    #[test]
    fn full_transfer_stops_on_cancel_and_receiver_loss() {
        let (tx, rx) = channel();
        let op = Operation::model_task(1000).unwrap();
        for _ in 0..TRANSFER_FRAMES {
            send(&tx, frame(), &op).unwrap();
        }
        let cancel = op.clone();
        let thread = std::thread::spawn(move || send(&tx, frame(), &op));
        cancel.cancel();
        assert_eq!(thread.join().unwrap(), Err(Error::Cancelled));
        drop(rx);
        let (tx, rx) = channel();
        drop(rx);
        assert_eq!(
            send(&tx, frame(), &Operation::model_task(1000).unwrap()),
            Err(Error::Cancelled)
        );
    }
    #[test]
    fn missing_or_expired_frame_never_lets_audio_run_alone() {
        let mut value = frame();
        value.metadata.source_sample = Some(2880);
        value.metadata.source_sample_rate = 24000;
        value.metadata.sequence = Some(3);
        assert_eq!(selection(Some(&value), 3839, 10000), Ok(Some(3)));
        assert!(selection(Some(&value), 3840, 10000).is_err());
        assert!(selection(None, 3000, 10000).is_err());
        assert_eq!(selection(None, 10000, 10000), Ok(None));
    }
}
