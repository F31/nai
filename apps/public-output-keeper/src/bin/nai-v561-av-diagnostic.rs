//! Explicit local AV diagnostic. This is not an authenticated live-output path.
// 实现整段是 Windows 专有的（minifb 窗口 + WASAPI 探测）。旧的 `#![cfg(windows)]`
// 会让非 Windows 的 bin crate 变成空 crate（E0601: `main` not found），所以入口按同目录
// `nai-v561-audio-output.rs` 的既有形状拆成「Windows 调 windows::run / 其它平台非零退出」。
#[cfg(windows)]
fn main() {
    if let Err(error) = windows::run() {
        eprintln!("AV_DIAGNOSTIC_ERROR: {error}");
        std::process::exit(1);
    }
}
#[cfg(not(windows))]
fn main() {
    eprintln!("AV_DIAGNOSTIC_ERROR: nai-v561-av-diagnostic is Windows-only");
    std::process::exit(1);
}
#[cfg(windows)]
mod windows {
    use minifb::{Window, WindowOptions};
    use nai_audio_cpal::{
        clocked_experiment::{ExperimentalFence, open_pcm_diagnostic},
        v561_clock::DeviceClock,
    };
    use nai_contracts::v561_media_validation::FrameLimits;
    use nai_kernel::output_owner::{OutputOwner, Speaker};
    use nai_media_core::{
        v561_frame_lease::Identity,
        v561_frame_scheduler::{Offer, OwnedVerifiedFrame, Scheduler},
        v561_leased_output::LeasedOutput,
        v561_shared_receiver,
    };
    use std::{
        num::NonZeroU64,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicU64, Ordering},
            mpsc,
        },
        time::{Duration, Instant},
    };
    use zeroize::Zeroizing;

    enum Event {
        Frame(OwnedVerifiedFrame),
        Closed,
        Failed(String),
    }

    const FRAME_TIMEOUT_MS: u64 = 500;

    fn renew_audio_deadline(deadline: &AtomicU64, now_ms: u64) -> bool {
        let Some(next) = now_ms.checked_add(FRAME_TIMEOUT_MS) else {
            return false;
        };
        deadline
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (now_ms < current).then_some(next)
            })
            .is_ok()
    }

    fn require_owner(
        permitted: &dyn Fn() -> bool,
        window: &mut Window,
        screen: &mut [u32],
        width: u32,
        height: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if permitted() {
            return Ok(());
        }
        screen.fill(0);
        window.update_with_buffer(screen, width as usize, height as usize)?;
        Err("output owner revoked".into())
    }

    fn playback_complete(
        closed: bool,
        finished: bool,
        played: u64,
        total: u64,
    ) -> Result<bool, &'static str> {
        if finished && played < total {
            return Err("audio stopped before submitted sentence completed");
        }
        if (finished || played >= total) && !closed {
            return Err("audio completed before all frames were received");
        }
        Ok(closed && played >= total)
    }

    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
        let args: Vec<_> = std::env::args_os().skip(1).collect();
        if args.len() != 10 {
            return Err("expected mapping arguments, PCM path and prefill count".into());
        }
        let width: u32 = args[4].to_string_lossy().parse()?;
        let height: u32 = args[5].to_string_lossy().parse()?;
        let slots: u32 = args[3].to_string_lossy().parse()?;
        let slot_bytes: u64 = args[2].to_string_lossy().parse()?;
        let epoch: u64 = args[7].to_string_lossy().parse()?;
        let prefill: usize = args[9].to_string_lossy().parse()?;
        if !(1..=96).contains(&prefill) {
            return Err("invalid prefill".into());
        }
        let file_bytes = std::fs::metadata(&args[8])?.len();
        if file_bytes == 0 || file_bytes % 2 != 0 || file_bytes > 24_000 * 600 * 2 {
            return Err("invalid PCM".into());
        }
        let bytes = std::fs::read(&args[8])?;
        if bytes.len() as u64 != file_bytes {
            return Err("PCM length changed".into());
        }
        let pcm = Zeroizing::new(
            bytes
                .chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]))
                .collect::<Vec<_>>(),
        );
        let sample_count = pcm.len() as u64;
        let queue_size = prefill + slots as usize + 16;
        let (tx, rx) = mpsc::sync_channel::<Event>(queue_size);
        let input_args = args[..8].to_vec();
        let alive = Arc::new(AtomicBool::new(true));
        let requested_stop = Arc::new(AtomicBool::new(false));
        let input_alive = alive.clone();
        let input_stop = requested_stop.clone();
        std::thread::spawn(move || {
            let send = tx.clone();
            let result =
                v561_shared_receiver::receive_verified(input_args.into_iter(), move |frame| {
                    send.send(Event::Frame(frame))
                        .map_err(|_| "presentation stopped".into())
                });
            let _ = tx.send(match result {
                Ok(()) => Event::Closed,
                Err(error) => {
                    input_stop.store(
                        error.is::<v561_shared_receiver::StopRequested>(),
                        Ordering::Release,
                    );
                    input_alive.store(false, Ordering::Release);
                    Event::Failed(error.to_string())
                }
            });
        });
        let mut window = Window::new(
            "nai V5.6.1 - local AV diagnostic",
            width as usize,
            height as usize,
            WindowOptions::default(),
        )?;
        let mut screen = vec![
            0u32;
            (width as usize)
                .checked_mul(height as usize)
                .ok_or("size overflow")?
        ];
        let output_owner = Arc::new(OutputOwner::default());
        let owner_permitted = Arc::new(
            output_owner
                .acquire_callback(Speaker::Narration)
                .ok_or("output owner unavailable")?,
        );
        let mut scheduler: Option<Scheduler> = None;
        let mut audio = None;
        let mut clock = DeviceClock::new(epoch, 1).ok_or("invalid clock")?;
        let mut buffered = 0usize;
        let mut received = 0u64;
        let mut dropped_late = 0u64;
        let mut shown = 0u64;
        let mut previous_frame = None;
        let identity = Identity {
            epoch: NonZeroU64::new(epoch).ok_or("invalid epoch")?,
            generation: NonZeroU64::MIN,
        };
        let mut selected_output: Option<LeasedOutput> = None;
        let mut selected_sequence = None;
        let mut candidate_frame = None;
        let mut completed = false;
        let mut max_lag_samples = 0u64;
        let mut blank_started: Option<Instant> = None;
        let mut max_blank = Duration::ZERO;
        let mut blank_ticks = 0u64;
        let mut blank_events = 0u64;
        let mut first_present_ms = None;
        let mut max_queued_lead_samples = 0u64;
        let mut closed = false;
        let start = Instant::now();
        let audio_deadline = Arc::new(AtomicU64::new(0));
        let mut last_buffer_report = start;
        window.update_with_buffer(&screen, width as usize, height as usize)?;
        let mut execution = (|| -> Result<(), Box<dyn std::error::Error>> {
            while window.is_open() {
                if !alive.load(Ordering::Acquire) {
                    if requested_stop.load(Ordering::Acquire) {
                        break;
                    }
                    return Err("frame input failed".into());
                }
                require_owner(
                    owner_permitted.as_ref(),
                    &mut window,
                    &mut screen,
                    width,
                    height,
                )?;
                while scheduler.as_ref().is_none_or(Scheduler::has_capacity) {
                    match rx.try_recv() {
                        Ok(Event::Frame(frame)) => {
                            require_owner(
                                owner_permitted.as_ref(),
                                &mut window,
                                &mut screen,
                                width,
                                height,
                            )?;
                            if scheduler.is_none() {
                                let limits = FrameLimits {
                                    slots,
                                    max_width: width,
                                    max_height: height,
                                    max_payload_bytes: slot_bytes,
                                    max_sample_rate: 192000,
                                };
                                scheduler = Some(
                                    Scheduler::new(
                                        frame.metadata().pool_id.clone(),
                                        epoch,
                                        1,
                                        limits,
                                        queue_size,
                                        4800,
                                    )
                                    .map_err(|_| "scheduler init")?,
                                );
                            }
                            let outcome = scheduler
                                .as_mut()
                                .unwrap()
                                .offer_verified(frame)
                                .map_err(|e| format!("frame rejected: {e:?}"))?;
                            if outcome == Offer::DiscardedLate {
                                dropped_late += 1;
                            } else {
                                buffered += 1;
                            }
                            received += 1;
                        }
                        Ok(Event::Closed) => {
                            closed = true;
                            break;
                        }
                        Ok(Event::Failed(error)) => {
                            if requested_stop.load(Ordering::Acquire) {
                                break;
                            }
                            return Err(error.into());
                        }
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            closed = true;
                            break;
                        }
                    }
                }
                if requested_stop.load(Ordering::Acquire) {
                    break;
                }
                if audio.is_none() && (buffered >= prefill || (closed && buffered > 0)) {
                    let gate = Arc::new(ExperimentalFence::externally_controlled());
                    let permitted = alive.clone();
                    let owner_permitted = owner_permitted.clone();
                    let deadline = audio_deadline.clone();
                    let probe = open_pcm_diagnostic(
                        gate.clone(),
                        pcm.clone(),
                        Arc::new(move || {
                            permitted.load(Ordering::Acquire)
                                && (start.elapsed().as_millis()
                                    < u128::from(deadline.load(Ordering::Acquire)))
                                && owner_permitted()
                        }),
                        sample_count as usize,
                    )?;
                    audio_deadline.store(
                        start.elapsed().as_millis() as u64 + FRAME_TIMEOUT_MS,
                        Ordering::Release,
                    );
                    if !gate.activate() {
                        return Err("audio activation failed".into());
                    }
                    audio = Some((probe, gate));
                }
                if let Some((probe, _)) = &audio {
                    if let Some(observation) = clock.observe(probe) {
                        let scheduler = scheduler.as_mut().unwrap();
                        if let Some(frame) = scheduler
                            .present(&observation)
                            .map_err(|e| format!("clock rejected: {e:?}"))?
                        {
                            let played =
                                observation.played_sample.ok_or("missing played sample")?;
                            let frame_sample =
                                frame.metadata.source_sample.ok_or("missing frame sample")?;
                            let converted = u128::from(frame_sample)
                                * u128::from(observation.sample_rate)
                                / u128::from(frame.metadata.source_sample_rate);
                            let converted = u64::try_from(converted)
                                .map_err(|_| "sample conversion overflow")?;
                            max_lag_samples = max_lag_samples.max(played.saturating_sub(converted));
                            if let Some(since) = blank_started.take() {
                                max_blank = max_blank.max(since.elapsed());
                            }
                            if previous_frame != frame.metadata.sequence {
                                for (pixel, bgra) in
                                    screen.iter_mut().zip(frame.pixels.chunks_exact(4))
                                {
                                    *pixel = u32::from_le_bytes([bgra[0], bgra[1], bgra[2], 0]);
                                }
                            }
                            candidate_frame = frame.metadata.sequence;
                            if let Some(frame) = scheduler.copy_selected_verified(selected_sequence)
                            {
                                let now = Instant::now();
                                let output = selected_output.get_or_insert_with(|| {
                                    LeasedOutput::new(
                                        identity,
                                        now,
                                        Duration::from_millis(FRAME_TIMEOUT_MS),
                                    )
                                    .expect("valid diagnostic lease duration")
                                });
                                selected_sequence = frame.metadata().sequence;
                                if !output.accept(identity, frame, now)
                                    || !renew_audio_deadline(
                                        &audio_deadline,
                                        start.elapsed().as_millis() as u64,
                                    )
                                {
                                    return Err("selected frame lease revoked".into());
                                }
                            }
                        } else {
                            if previous_frame.is_some() {
                                screen.fill(0);
                            }
                            candidate_frame = None;
                            blank_ticks = blank_ticks.saturating_add(1);
                            if blank_started.is_none() {
                                blank_events += 1;
                            }
                            blank_started.get_or_insert_with(Instant::now);
                        }
                    } else {
                        if previous_frame.is_some() {
                            screen.fill(0);
                        }
                        candidate_frame = None;
                    }
                    if probe.failed.load(Ordering::Acquire) {
                        return Err("audio device failed".into());
                    }
                    completed = match playback_complete(
                        closed,
                        probe.is_finished(),
                        probe.clock_reading().played_samples,
                        sample_count,
                    ) {
                        Ok(completed) => completed,
                        Err(error) => {
                            let coalesced_due_frames = scheduler
                                .as_ref()
                                .map_or(0, Scheduler::coalesced_due_frames);
                            let queued_frames =
                                scheduler.as_ref().map_or(0, Scheduler::queued_frames);
                            eprintln!(
                                "AV_DIAGNOSTIC_INCOMPLETE received={received} shown={shown} dropped_late={dropped_late} coalesced_due_frames={coalesced_due_frames} queued_frames={queued_frames} input_closed={closed}"
                            );
                            eprintln!(
                                "AV_TIMING_INCOMPLETE first_window_submit_ms={first_present_ms:?} blank_events={blank_events} max_queued_frame_lead_samples={max_queued_lead_samples} clock_rate=24000 reason={error}"
                            );
                            return Err(error.into());
                        }
                    };
                }
                require_owner(
                    owner_permitted.as_ref(),
                    &mut window,
                    &mut screen,
                    width,
                    height,
                )?;
                if !alive.load(Ordering::Acquire) {
                    if requested_stop.load(Ordering::Acquire) {
                        break;
                    }
                    return Err("frame input failed".into());
                }
                if !completed
                    && selected_output
                        .as_mut()
                        .is_some_and(|output| output.frame(Instant::now()).is_none())
                {
                    return Err("selected frame lease expired".into());
                }
                if previous_frame != candidate_frame {
                    window.update_with_buffer(&screen, width as usize, height as usize)?;
                    if candidate_frame.is_some() {
                        first_present_ms.get_or_insert_with(|| start.elapsed().as_millis());
                        shown += 1;
                    }
                    previous_frame = candidate_frame;
                } else {
                    window.update();
                }
                if let Some(lead) = scheduler.as_ref().and_then(Scheduler::queued_lead_samples) {
                    max_queued_lead_samples = max_queued_lead_samples.max(lead);
                    if last_buffer_report.elapsed() >= Duration::from_secs(1) {
                        let queued = scheduler.as_ref().map_or(0, Scheduler::queued_frames);
                        eprintln!(
                            "AV_BUFFER receiver_elapsed_ms={} queued_frames={queued} last_queued_frame_lead_samples={lead} clock_rate=24000",
                            start.elapsed().as_millis()
                        );
                        last_buffer_report = Instant::now();
                    }
                }
                if completed {
                    break;
                }
                if closed && received == 0 {
                    return Err("no frames received".into());
                }
                if start.elapsed() > Duration::from_secs(sample_count.div_ceil(24_000) + 30) {
                    return Err("diagnostic deadline exceeded".into());
                }
                std::thread::sleep(Duration::from_millis(4));
            }
            Ok(())
        })();
        alive.store(false, Ordering::Release);
        output_owner.stop();
        if let Some(output) = &mut selected_output {
            output.stop();
        }
        if window.is_open() {
            screen.fill(0);
            if let Err(error) = window.update_with_buffer(&screen, width as usize, height as usize)
                && execution.is_ok()
            {
                execution = Err(error.into());
            }
        }
        if let Some((mut probe, gate)) = audio {
            gate.revoke();
            if let Err(error) = probe.stop_within(Duration::from_millis(500)) {
                if execution.is_ok() {
                    execution = Err(error.into());
                } else {
                    eprintln!("AV_CLEANUP_ERROR: {error}");
                }
            }
        }
        let coalesced_due_frames = scheduler
            .as_ref()
            .map_or(0, Scheduler::coalesced_due_frames);
        let queued_frames = scheduler.as_ref().map_or(0, Scheduler::queued_frames);
        if let Some(mut scheduler) = scheduler {
            scheduler.stop();
        }
        if let Some(since) = blank_started {
            max_blank = max_blank.max(since.elapsed());
        }
        eprintln!(
            "AV_DIAGNOSTIC received={received} shown={shown} dropped_late={dropped_late} coalesced_due_frames={coalesced_due_frames} queued_frames={queued_frames} input_closed={closed} max_lag_samples={max_lag_samples} clock_rate=24000 blank_ticks={blank_ticks} max_blank_ms={}",
            max_blank.as_millis()
        );
        eprintln!(
            "AV_TIMING first_window_submit_ms={first_present_ms:?} blank_events={blank_events} max_queued_frame_lead_samples={max_queued_lead_samples} clock_rate=24000 scope=receiver_start_to_window_submission_not_hardware_display"
        );
        if execution.is_err() {
            eprintln!("AV_EXECUTION outcome=failed completed={completed}");
        }
        execution?;
        if requested_stop.load(Ordering::Acquire) {
            eprintln!("AV_EXECUTION outcome=stopped");
            return Ok(());
        }
        if !closed || !completed || shown == 0 {
            return Err("diagnostic ended without completed presentation".into());
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::{playback_complete, renew_audio_deadline};
        use std::sync::atomic::{AtomicU64, Ordering};

        #[test]
        fn audio_deadline_only_renews_before_expiry() {
            let deadline = AtomicU64::new(500);
            assert!(renew_audio_deadline(&deadline, 499));
            assert_eq!(deadline.load(Ordering::Acquire), 999);
            assert!(!renew_audio_deadline(&deadline, 999));
            assert!(!renew_audio_deadline(&deadline, 1000));
            assert_eq!(deadline.load(Ordering::Acquire), 999);
        }

        #[test]
        fn finished_audio_does_not_wait_for_late_producer() {
            assert!(playback_complete(false, true, 100, 100).is_err());
            assert!(playback_complete(false, false, 100, 100).is_err());
            assert!(playback_complete(true, true, 99, 100).is_err());
            assert_eq!(playback_complete(true, false, 100, 100), Ok(true));
            assert_eq!(playback_complete(false, false, 99, 100), Ok(false));
        }
    }
}
