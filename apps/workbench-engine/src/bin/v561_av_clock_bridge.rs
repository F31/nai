//! Explicit isolated AV diagnostic, not producer-authenticated production output.
#[cfg(windows)]
fn main() {
    if let Err(error) = windows::run() {
        eprintln!("AV_BRIDGE_FAILED {error}");
        std::process::exit(1);
    }
}
#[cfg(not(windows))]
fn main() {
    std::process::exit(1);
}

#[cfg(windows)]
mod windows {
    use minifb::{Window, WindowOptions};
    use nai_contracts::{
        v561_media::{ClockState, FrameRef, PlaybackClockObservation},
        v561_media_validation::FrameLimits,
    };
    use nai_kernel::output_owner::Speaker;
    use nai_media_core::{
        v561_clock_bridge::ClockBridge,
        v561_frame_scheduler::{Offer, Scheduler},
        v561_shared_receiver,
    };
    use nai_workbench_engine::{v561_audio, v561_tasks::Tasks};
    use std::{
        collections::VecDeque,
        io::Read,
        sync::mpsc,
        time::{Duration, Instant},
    };
    use zeroize::Zeroizing;
    enum Input {
        Frame(FrameRef, Vec<u8>),
        Closed,
        Failed,
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let args: Vec<_> = std::env::args_os().skip(1).collect();
        if args.len() != 13 {
            return Err(
                "expected frame mapping, PCM, prefill, audio executor, fps, stop milliseconds"
                    .into(),
            );
        }
        let parse = |i: usize| args[i].to_string_lossy().parse::<u64>();
        let slot_bytes = parse(2)?;
        let slots = u32::try_from(parse(3)?)?;
        let width = u32::try_from(parse(4)?)?;
        let height = u32::try_from(parse(5)?)?;
        let rate = u32::try_from(parse(6)?)?;
        let epoch = parse(7)?;
        let prefill = usize::try_from(parse(9)?)?;
        let fps = parse(11)?;
        let stop_ms = parse(12)?;
        let capacity = prefill
            .checked_add(slots as usize)
            .and_then(|n| n.checked_add(8))
            .ok_or("queue overflow")?;
        if !(1..=96).contains(&prefill)
            || !(1..=4).contains(&slots)
            || !(1..=1920).contains(&width)
            || !(1..=1080).contains(&height)
            || !(1..=60).contains(&fps)
            || !(1..=96000).contains(&rate)
            || epoch == 0
            || stop_ms > 20000
            || slot_bytes != u64::from(width) * u64::from(height) * 4
            || slot_bytes * capacity as u64 > 268435456
        {
            return Err("invalid bounds".into());
        }
        let source = std::fs::File::open(&args[8])?;
        let metadata = source.metadata()?;
        if !metadata.is_file() || metadata.len() > 960000 {
            return Err("PCM file bounds".into());
        }
        let mut pcm_bytes = Zeroizing::new(Vec::new());
        source.take(960001).read_to_end(&mut pcm_bytes)?;
        if pcm_bytes.len() as u64 != metadata.len() {
            return Err("PCM changed".into());
        }
        if pcm_bytes.is_empty() || pcm_bytes.len() % 2 != 0 || pcm_bytes.len() > 960000 {
            return Err("invalid PCM".into());
        }
        let pcm = Zeroizing::new(
            pcm_bytes
                .chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]))
                .collect::<Vec<_>>(),
        );
        let samples = pcm.len() as u64;
        if !(samples * fps).is_multiple_of(24000) {
            return Err("PCM/frame timeline mismatch".into());
        }
        let expected = samples * fps / 24000;
        let prefill = prefill.min(expected as usize);
        let audio_exe = std::path::PathBuf::from(&args[10]);
        let (frames_tx, frames_rx) = mpsc::sync_channel(1);
        let input_args = args[..8].to_vec();
        std::thread::spawn(move || {
            let result = v561_shared_receiver::receive(input_args.into_iter(), |frame, pixels| {
                frames_tx
                    .send(Input::Frame(frame, pixels))
                    .map_err(|_| "window stopped".into())
            });
            let _ = frames_tx.send(if result.is_ok() {
                Input::Closed
            } else {
                Input::Failed
            });
        });
        let mut tasks = Tasks::default();
        let task = tasks
            .begin(Speaker::Narration, 45000)
            .map_err(|_| "task denied")?;
        let operation = task.operation();
        let mut window = Window::new(
            "nai · isolated signed-clock AV",
            width as usize,
            height as usize,
            WindowOptions::default(),
        )?;
        window.set_target_fps(200);
        let mut screen = vec![0u32; (width * height) as usize];
        window.update_with_buffer(&screen, width as usize, height as usize)?;
        let (clock_tx, clock_rx) = mpsc::sync_channel::<PlaybackClockObservation>(4);
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let mut pending = VecDeque::new();
        let mut scheduler: Option<Scheduler> = None;
        let mut bridge = ClockBridge::new(epoch, samples, 24000, Duration::from_millis(200))
            .ok_or("invalid clock")?;
        let mut pool = None;
        let mut pcm = Some(pcm);
        let mut started = false;
        let mut closed = false;
        let mut received = 0u64;
        let mut shown = 0u64;
        let mut dropped = 0u64;
        let mut presented = None;
        let mut first_submit = None;
        let mut stopped_clock = false;
        let mut completed = None;
        let mut completed_at = None;
        let mut blank_events = 0u64;
        let mut blank = false;
        let mut running_seen = false;
        let began = Instant::now();
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            loop {
                if !window.is_open() {
                    return Err("window closed".into());
                }
                operation.check().map_err(|_| "task revoked")?;
                if stop_ms > 0 && began.elapsed() >= Duration::from_millis(stop_ms) {
                    tasks.stop();
                    return Err("stop requested".into());
                }
                while let Ok(clock) = clock_rx.try_recv() {
                    operation.check().map_err(|_| "task revoked")?;
                    bridge.accept(clock).map_err(|_| "clock rejected")?;
                    if scheduler.is_none() {
                        scheduler = Some(
                            Scheduler::new(
                                pool.clone().ok_or("missing pool")?,
                                epoch,
                                clock.device_generation,
                                FrameLimits {
                                    slots,
                                    max_width: width,
                                    max_height: height,
                                    max_payload_bytes: slot_bytes,
                                    max_sample_rate: 96000,
                                },
                                capacity,
                                2400,
                            )
                            .map_err(|_| "scheduler bounds")?,
                        );
                    }
                    let frame = scheduler
                        .as_mut()
                        .unwrap()
                        .present(&clock)
                        .map_err(|_| "frame clock rejected")?;
                    running_seen |= clock.state == ClockState::Running as i32;
                    stopped_clock = clock.state == ClockState::Stopped as i32;
                    if let Some(frame) = frame {
                        if presented != frame.metadata.sequence {
                            for (dst, pixel) in screen.iter_mut().zip(frame.pixels.chunks_exact(4))
                            {
                                *dst = u32::from_le_bytes([pixel[0], pixel[1], pixel[2], 0]);
                            }
                            operation.check().map_err(|_| "task revoked")?;
                            if bridge.running().is_none() {
                                return Err("clock expired before window submit".into());
                            }
                            window.update_with_buffer(&screen, width as usize, height as usize)?;
                            shown += 1;
                            first_submit.get_or_insert_with(|| began.elapsed().as_millis());
                            presented = frame.metadata.sequence;
                        }
                        blank = false;
                    } else if presented.take().is_some() {
                        screen.fill(0);
                        window.update_with_buffer(&screen, width as usize, height as usize)?;
                    }
                    if clock.state == ClockState::Unavailable as i32 {
                        let scheduler = scheduler.as_mut().unwrap();
                        while scheduler.has_capacity() {
                            let Some((frame, pixels)) = pending.pop_front() else {
                                break;
                            };
                            scheduler
                                .offer(frame, pixels)
                                .map_err(|_| "prefill rejected")?;
                        }
                    }
                }
                // Without fresh signed progress, holding an old face is forbidden.
                if running_seen && !stopped_clock && bridge.running().is_none() {
                    return Err("clock observation expired".into());
                }
                for _ in 0..4 {
                    let available = scheduler.as_ref().map_or(pending.len() < capacity, |s| {
                        s.queued_frames() + pending.len() < capacity
                    });
                    if !available {
                        break;
                    }
                    match frames_rx.try_recv() {
                        Ok(Input::Frame(frame, pixels)) => {
                            let sequence = frame.sequence.ok_or("missing sequence")?;
                            if sequence >= expected
                                || frame.source_sample != Some(sequence * u64::from(rate) / fps)
                                || frame.pts_ns != Some(sequence * 1_000_000_000 / fps)
                            {
                                return Err("frame timeline mismatch".into());
                            }
                            pool.get_or_insert_with(|| frame.pool_id.clone());
                            pending.push_back((frame, pixels));
                            received += 1;
                        }
                        Ok(Input::Closed) => {
                            closed = true;
                            break;
                        }
                        Ok(Input::Failed) => return Err("frame input failed".into()),
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            closed = true;
                            break;
                        }
                    }
                }
                if let Some(scheduler) = &mut scheduler {
                    while scheduler.has_capacity() {
                        let Some((frame, pixels)) = pending.pop_front() else {
                            break;
                        };
                        if scheduler
                            .offer(frame, pixels)
                            .map_err(|_| "frame rejected")?
                            == Offer::DiscardedLate
                        {
                            dropped += 1;
                        }
                    }
                }
                if !started && (pending.len() >= prefill || (closed && !pending.is_empty())) {
                    started = true;
                    let pcm = pcm.take().ok_or("PCM consumed")?;
                    let operation = operation.clone();
                    let clock_tx = clock_tx.clone();
                    let done_tx = done_tx.clone();
                    let audio_exe = audio_exe.clone();
                    std::thread::spawn(move || {
                        let result =
                            v561_audio::play_in_epoch(
                                &audio_exe,
                                &pcm,
                                operation,
                                epoch,
                                |clock| {
                                    let running = clock.state == ClockState::Running as i32;
                                    match clock_tx.try_send(clock) {
                                Ok(())=>Ok(()), Err(mpsc::TrySendError::Full(_)) if running=>Ok(()),
                                _=>Err(nai_contracts::workbench::WorkbenchError::ResourceExhausted),
                            }
                                },
                            );
                        let _ = done_tx.send(result);
                    });
                }
                if let Ok(done) = done_rx.try_recv() {
                    completed = Some(done);
                    completed_at = Some(Instant::now());
                }
                if let Some(done) = &completed {
                    if done.as_ref().ok() != Some(&samples) {
                        return Err("independent audio failed".into());
                    }
                    if received != expected {
                        return Err("audio ended before all source frames".into());
                    }
                    if closed && stopped_clock {
                        break;
                    }
                    if completed_at.unwrap().elapsed() > Duration::from_millis(200) {
                        return Err("frame completion missing".into());
                    }
                }
                if closed && received == 0 {
                    return Err("no frames".into());
                }
                if running_seen && !stopped_clock && presented.is_none() && !blank {
                    blank_events += 1;
                    blank = true;
                }
                window.update();
            }
            if shown == 0 {
                return Err("no source frame submitted to window".into());
            }
            tasks
                .finish(&task)
                .map_err(|_| "task completion rejected")?;
            Ok(())
        })();
        // P0 logical revoke precedes any final window/device cleanup.
        tasks.stop();
        bridge.stop();
        if let Some(scheduler) = &mut scheduler {
            scheduler.stop();
        }
        screen.fill(0);
        let _ = window.update_with_buffer(&screen, width as usize, height as usize);
        eprintln!(
            "AV_BRIDGE_RESULT received={received} shown={shown} dropped_late={dropped} blank_events={blank_events} first_window_submit_ms={first_submit:?} completed={}",
            result.is_ok()
        );
        result
    }
}
