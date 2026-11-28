//! Bounded incremental-frame preview. PCM is sealed; not continuous live TTS.
#[cfg(windows)]
mod windows {
    use crate::v561_presentation::Presentation;
    use crate::{v561_stream_audio, v561_stream_clock};
    use nai_contracts::{
        v561_media::ClockState, v561_media_validation::FrameLimits,
        workbench::WorkbenchError as Error,
    };
    use nai_media_core::v561_frame_scheduler::{Frame, Scheduler};
    use nai_runtime_core::foundation::Operation;
    use std::{
        path::PathBuf,
        sync::mpsc,
        time::{Duration, Instant},
    };

    pub struct Clip {
        pub epoch: u64,
        pub samples: u64,
        pub width: u32,
        pub height: u32,
        pub prefill_frames: usize,
        pub capacity_frames: usize,
        pub segment_samples: Vec<u64>,
    }
    pub struct Report {
        pub shown: u64,
        pub clock_observations: u64,
        pub stopped: bool,
        pub completed: bool,
        pub played_samples: u64,
        pub safe_frame_submitted: bool,
        pub stop_to_safe_ms: Option<u64>,
        pub buffer_updates: u64,
        pub segments_shown: Vec<usize>,
        pub device_generation: u64,
        pub device_ready_observations: u64,
        pub frame_admission_ms: u64,
        pub frame_conversions: u64,
        pub first_frame_received_ms: u64,
        pub first_block_ready_ms: u64,
        pub first_audio_played_ms: Option<u64>,
        pub render_complete_ms: Option<u64>,
        pub peak_queued_frames: usize,
        pub frames_received: u64,
        pub device_prepared_ms: Option<u64>,
        pub admitted_ms: Option<u64>,
        pub admission_target_ms: Option<u64>,
    }
    pub fn play(
        clip: Clip,
        pcm: Vec<u8>,
        binary: PathBuf,
        operation: Operation,
        stop_after_ms: u64,
        input: mpsc::Receiver<Frame>,
        render_done: mpsc::Receiver<Result<(), Error>>,
    ) -> Result<Report, Error> {
        let segment_frame_ends = crate::v561_avatar_batch::frame_ends(&clip.segment_samples)?;
        if clip.epoch == 0
            || clip.samples == 0
            || clip.samples > 144000
            || pcm.len() as u64 != clip.samples * 2
            || clip.segment_samples.is_empty()
            || clip.segment_samples.len() > 2
            || clip.segment_samples.iter().any(|s| *s == 0 || *s > 144000)
            || clip.segment_samples.iter().sum::<u64>() != clip.samples
            || clip.prefill_frames == 0
            || clip.prefill_frames > clip.capacity_frames
            || clip.capacity_frames > 150
            || clip.capacity_frames > clip.prefill_frames.saturating_mul(2)
            || clip.capacity_frames as u64 > (clip.samples * 25).div_ceil(24000)
            || clip.prefill_frames as u64 > (clip.samples * 25).div_ceil(24000)
            || !(256..=1024).contains(&clip.width)
            || !(256..=1024).contains(&clip.height)
            || stop_after_ms > 20000
            || u64::from(clip.width)
                * u64::from(clip.height)
                * 4
                * (clip.capacity_frames + crate::v561_frame_feed::RESERVE_FRAMES) as u64
                > 268435456
        {
            return Err(Error::InvalidArgument);
        }
        operation.check()?;
        let started = Instant::now();
        let mut window = minifb::Window::new(
            "nai · 同源音画预演（非直播）",
            clip.width as usize,
            clip.height as usize,
            minifb::WindowOptions::default(),
        )
        .map_err(|_| Error::CapabilityUnavailable)?;
        window.set_target_fps(200);
        let mut screen = vec![0u32; (clip.width * clip.height) as usize];
        window
            .update_with_buffer(&screen, clip.width as usize, clip.height as usize)
            .map_err(|_| Error::IoFailed)?;
        let (clock_tx, clock_rx) = v561_stream_clock::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let mut ready_tx = Some(ready_tx);
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let check = operation.clone();
        let epoch = clip.epoch;
        let task = std::thread::spawn(move || {
            let value = v561_stream_audio::play_prepared_with_gate(
                pcm, &binary, check, clock_tx, epoch, ready_rx,
            );
            let _ = done_tx.send(value);
        });
        let mut device_prepared_ms = None;
        let mut admitted_ms = None;
        let mut admission_target_ms = None;
        let mut prefilled = false;
        let mut prepared = std::collections::VecDeque::<Frame>::new();
        let mut received = 0u64;
        let total_frames = (clip.samples * 25).div_ceil(24000);
        let mut rendered = false;
        let mut first_frame_received_ms = 0;
        let mut first_block_ready_ms = 0;
        let mut first_audio_played_ms = None;
        let mut render_complete_ms = None;
        let mut peak_queued_frames = 0;
        let mut scheduler = None::<Scheduler>;
        let mut latest = None::<v561_stream_clock::ClockEvent>;
        let mut shown = 0;
        let mut observations = 0;
        let mut selected = None;
        let mut presentation = Presentation::default();
        let mut displayed_sequences = std::collections::BTreeSet::new();
        let mut segments_shown = std::collections::BTreeSet::new();
        let mut buffer_updates = 0;
        let mut device_generation = 0;
        let mut device_ready_observations = 0;
        let mut frame_admission_ms = 0;
        let mut frame_conversions = 0;
        let mut stopped = false;
        let mut completed = false;
        let mut played_samples = 0;
        let mut done = None;
        let mut stopped_at = None;
        let result = (|| -> Result<(), Error> {
            loop {
                if !window.is_open()
                    || window.is_key_down(minifb::Key::Escape)
                    || (stop_after_ms > 0 && played_samples >= stop_after_ms * 24)
                {
                    stopped = true;
                    stopped_at = Some(Instant::now());
                    operation.cancel();
                    break;
                }
                operation.check()?;
                if !rendered {
                    match render_done.try_recv() {
                        Ok(value) => {
                            value?;
                            rendered = true;
                            render_complete_ms = Some(started.elapsed().as_millis() as u64);
                        }
                        Err(mpsc::TryRecvError::Disconnected) => return Err(Error::IoFailed),
                        Err(mpsc::TryRecvError::Empty) => {}
                    }
                }
                let mut admitted = 0;
                while admitted < crate::v561_frame_feed::TRANSFER_FRAMES
                    && scheduler
                        .as_ref()
                        .map_or(prepared.len() < clip.capacity_frames, |s| s.has_capacity())
                {
                    let frame = match input.try_recv() {
                        Ok(frame) => frame,
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) if received == total_frames => break,
                        Err(_) => return Err(Error::IoFailed),
                    };
                    if frame.metadata.epoch != clip.epoch
                        || frame.metadata.sequence != Some(received)
                        || frame.metadata.source_sample != Some(received * 960)
                        || received >= total_frames
                    {
                        return Err(Error::ProtocolError);
                    }
                    if received == 0 {
                        first_frame_received_ms = started.elapsed().as_millis() as u64;
                    }
                    received += 1;
                    admitted += 1;
                    if let Some(value) = scheduler.as_mut() {
                        value
                            .offer(frame.metadata, frame.pixels)
                            .map_err(|_| Error::ProtocolError)?;
                    } else {
                        prepared.push_back(frame);
                    }
                    peak_queued_frames = peak_queued_frames.max(
                        scheduler
                            .as_ref()
                            .map_or(prepared.len(), |s| s.queued_frames()),
                    );
                }
                if !prefilled && received >= clip.prefill_frames as u64 {
                    first_block_ready_ms = started.elapsed().as_millis() as u64;
                    prefilled = true;
                }
                while let Ok(event) = clock_rx.try_recv() {
                    observations += 1;
                    let clock = event.observation();
                    played_samples = clock.played_sample.unwrap_or(0);
                    if played_samples > 0 && first_audio_played_ms.is_none() {
                        first_audio_played_ms = Some(started.elapsed().as_millis() as u64);
                    }
                    if clock.epoch != clip.epoch
                        || (device_generation != 0 && clock.device_generation != device_generation)
                        || (ready_tx.is_some() && played_samples != 0)
                    {
                        return Err(Error::ProtocolError);
                    }
                    if clock.state == ClockState::Unavailable as i32 {
                        if device_generation != 0 {
                            return Err(Error::ProtocolError);
                        }
                        device_ready_observations += 1;
                        device_generation = clock.device_generation;
                        device_prepared_ms = Some(started.elapsed().as_millis() as u64);
                    } else if clock.state == ClockState::Stopped as i32 {
                        if let Some(value) = scheduler.as_mut() {
                            value.stop();
                        }
                        screen.fill(0);
                    } else if let Some(clock) = event.running() {
                        if ready_tx.is_some() {
                            latest = Some(event);
                            continue;
                        }
                        let frame = scheduler
                            .as_mut()
                            .ok_or(Error::ProtocolError)?
                            .present(clock)
                            .map_err(|_| Error::ProtocolError)?;
                        selected =
                            crate::v561_frame_feed::selection(frame, played_samples, clip.samples)
                                .inspect_err(|_| eprintln!("AVATAR_STREAM_UNDERRUN"))?;
                        if selected.is_none() {
                            screen.fill(0);
                        }
                    }
                    latest = Some(event);
                }
                if prefilled && ready_tx.is_some() {
                    let target = crate::v561_frame_feed::admit_after_ms(
                        total_frames as usize,
                        clip.prefill_frames,
                        first_frame_received_ms,
                        first_block_ready_ms,
                        rendered,
                    )?;
                    admission_target_ms = Some(target);
                    if started.elapsed().as_millis() as u64 >= target
                        && device_generation != 0
                        && latest.as_ref().is_some_and(|event| {
                            event.fresh() && event.observation().state != ClockState::Stopped as i32
                        })
                    {
                        let admission_at = Instant::now();
                        let pool = prepared
                            .front()
                            .ok_or(Error::ProtocolError)?
                            .metadata
                            .pool_id
                            .clone();
                        let mut value = Scheduler::new(
                            pool,
                            clip.epoch,
                            device_generation,
                            FrameLimits {
                                slots: 3,
                                max_width: clip.width,
                                max_height: clip.height,
                                max_payload_bytes: u64::from(clip.width)
                                    * u64::from(clip.height)
                                    * 4,
                                max_sample_rate: 24000,
                            },
                            clip.capacity_frames,
                            2400,
                        )
                        .map_err(|_| Error::ProtocolError)?;
                        for frame in prepared.drain(..) {
                            operation.check()?;
                            value
                                .offer(frame.metadata, frame.pixels)
                                .map_err(|_| Error::ProtocolError)?;
                        }
                        operation.check()?;
                        if !latest.as_ref().is_some_and(|event| event.fresh()) {
                            return Err(Error::DeadlineExceeded);
                        }
                        scheduler = Some(value);
                        ready_tx
                            .take()
                            .ok_or(Error::ProtocolError)?
                            .try_send(())
                            .map_err(|_| Error::IoFailed)?;
                        admitted_ms = Some(started.elapsed().as_millis() as u64);
                        frame_admission_ms = admission_at.elapsed().as_millis() as u64;
                    }
                }
                if latest.as_ref().and_then(|value| value.running()).is_none() {
                    if latest.as_ref().is_some_and(|event| {
                        event.observation().state != ClockState::Stopped as i32 && !event.fresh()
                    }) {
                        return Err(Error::DeadlineExceeded);
                    }
                    screen.fill(0);
                    selected = None;
                }
                // Coalesce clock selection before expensive pixel conversion.
                if selected.is_some() && presentation.changed(selected) {
                    let frame = scheduler
                        .as_ref()
                        .and_then(|value| value.selected_frame())
                        .filter(|frame| frame.metadata.sequence == selected)
                        .ok_or(Error::ProtocolError)?;
                    for (pixel, bgra) in screen.iter_mut().zip(frame.pixels.chunks_exact(4)) {
                        *pixel = u32::from_le_bytes([bgra[0], bgra[1], bgra[2], 0]);
                    }
                    frame_conversions += 1;
                    if latest.as_ref().and_then(|value| value.running()).is_none() {
                        screen.fill(0);
                        selected = None;
                    }
                }
                operation.check()?;
                if presentation.changed(selected) {
                    window
                        .update_with_buffer(&screen, clip.width as usize, clip.height as usize)
                        .map_err(|_| Error::IoFailed)?;
                    presentation.submitted(selected);
                    buffer_updates += 1;
                    if let Some(sequence) = selected {
                        if displayed_sequences.insert(sequence) {
                            shown += 1;
                        }
                        let segment = segment_frame_ends
                            .iter()
                            .position(|end| sequence < *end)
                            .ok_or(Error::ProtocolError)?;
                        segments_shown.insert(segment);
                    }
                } else {
                    window.update();
                }
                if done.is_none() {
                    match done_rx.try_recv() {
                        Ok(value) => {
                            done = Some(value);
                        }
                        Err(mpsc::TryRecvError::Disconnected) => return Err(Error::IoFailed),
                        Err(mpsc::TryRecvError::Empty) => {}
                    }
                }
                if let Some(Err(error)) = done.as_ref() {
                    return Err(*error);
                }
                if done.is_some() && rendered {
                    break;
                }
            }
            Ok(())
        })();
        // Logical revoke precedes window cleanup or waiting for device teardown.
        operation.cancel();
        if let Some(value) = scheduler.as_mut() {
            value.stop();
        }
        screen.fill(0);
        let safe = window.is_open()
            && window
                .update_with_buffer(&screen, clip.width as usize, clip.height as usize)
                .is_ok();
        let stop_to_safe_ms = stopped_at.map(|at| at.elapsed().as_millis() as u64);
        if done.is_none() {
            done = done_rx.recv_timeout(Duration::from_secs(2)).ok();
        }
        if done.is_some() {
            let _ = task.join();
        }
        result?;
        match done {
            Some(Ok(audio)) => {
                completed = true;
                played_samples = audio.played_samples;
                if audio.model_calls != 0
                    || audio.epoch != clip.epoch
                    || audio.played_samples != clip.samples
                {
                    return Err(Error::ProtocolError);
                }
            }
            Some(Err(Error::Cancelled)) if stopped => {}
            Some(Err(error)) => return Err(error),
            None => return Err(Error::DeadlineExceeded),
        }
        if !safe || (!stopped && shown == 0) {
            return Err(Error::CapabilityUnavailable);
        }
        Ok(Report {
            shown,
            clock_observations: observations,
            stopped,
            completed,
            played_samples,
            safe_frame_submitted: safe,
            stop_to_safe_ms,
            buffer_updates,
            segments_shown: segments_shown.into_iter().collect(),
            device_generation,
            device_ready_observations,
            frame_admission_ms,
            frame_conversions,
            first_frame_received_ms,
            first_block_ready_ms,
            first_audio_played_ms,
            render_complete_ms,
            peak_queued_frames,
            frames_received: received,
            device_prepared_ms,
            admitted_ms,
            admission_target_ms,
        })
    }
}
#[cfg(windows)]
pub use windows::*;

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use nai_contracts::{
        v561_media::{FrameRef, PixelFormat},
        workbench::WorkbenchError as Error,
    };
    use nai_media_core::v561_frame_scheduler::Frame;
    use nai_runtime_core::foundation::Operation;
    use sha2::{Digest, Sha256};
    use std::{
        sync::mpsc,
        time::{Duration, Instant},
    };
    #[test]
    fn oversized_stream_queue_is_rejected_before_device_or_window() {
        let (_tx, rx) = crate::v561_frame_feed::channel();
        let (_done, done) = mpsc::sync_channel(1);
        let result = play(
            Clip {
                epoch: 7,
                samples: 12 * 960,
                width: 256,
                height: 256,
                prefill_frames: 4,
                capacity_frames: 12,
                segment_samples: vec![12 * 960],
            },
            vec![0; 12 * 960 * 2],
            "not-an-output.exe".into(),
            Operation::model_task(100).unwrap(),
            0,
            rx,
            done,
        );
        assert!(matches!(result, Err(Error::InvalidArgument)));
    }
    #[test]
    #[ignore = "explicit Windows silent device preparation cancellation; no model"]
    fn native_parallel_preparation_cancels_without_video_or_source_audio() {
        let audio = std::env::var_os("NAI_STREAM_AUDIO_OUTPUT").expect("explicit native output");
        let parent = Operation::model_task(5000).unwrap();
        let cancel = parent.clone();
        let (_tx, rx) = crate::v561_frame_feed::channel();
        let (_done, done_rx) = mpsc::sync_channel(1);
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(1600));
            cancel.cancel();
        });
        let started = Instant::now();
        let result = play(
            Clip {
                epoch: 7,
                samples: 12 * 960,
                width: 256,
                height: 256,
                prefill_frames: 4,
                capacity_frames: 8,
                segment_samples: vec![12 * 960],
            },
            vec![0; 12 * 960 * 2],
            audio.into(),
            parent,
            0,
            rx,
            done_rx,
        );
        thread.join().unwrap();
        assert!(matches!(result, Err(Error::Cancelled)));
        assert!(started.elapsed() < Duration::from_millis(2500));
    }
    #[test]
    #[ignore = "explicit Windows silent synthetic frame underrun drill; not avatar acceptance"]
    fn native_incremental_underrun_cancels_audio_and_producer() {
        let audio = std::env::var_os("NAI_STREAM_AUDIO_OUTPUT").expect("explicit native output");
        let parent = Operation::model_task(5000).unwrap();
        let producer = parent.clone();
        let (tx, rx) = crate::v561_frame_feed::channel();
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let source = std::thread::spawn(move || {
            for n in 0u64..4 {
                let pixels = [32, 32, 32, 255].repeat(256 * 256);
                let metadata = FrameRef {
                    pool_id: "silent-underrun-drill".into(),
                    epoch: 7,
                    sequence: Some(n),
                    source_sample: Some(n * 960),
                    source_sample_rate: 24000,
                    pts_ns: Some(n * 40000000),
                    width: 256,
                    height: 256,
                    stride_bytes: 1024,
                    pixel_format: PixelFormat::Bgra8 as i32,
                    slot_index: Some((n % 3) as u32),
                    slot_generation: n / 3 + 1,
                    payload_bytes: pixels.len() as u64,
                    content_sha256: Sha256::digest(&pixels).to_vec(),
                };
                crate::v561_frame_feed::send(&tx, Frame { metadata, pixels }, &producer).unwrap();
            }
            while producer.check().is_ok() {
                std::thread::sleep(Duration::from_millis(1));
            }
            let _ = done_tx.send(Err(Error::Cancelled));
        });
        let started = Instant::now();
        let result = play(
            Clip {
                epoch: 7,
                samples: 12 * 960,
                width: 256,
                height: 256,
                prefill_frames: 4,
                capacity_frames: 8,
                segment_samples: vec![12 * 960],
            },
            vec![0; 12 * 960 * 2],
            audio.into(),
            parent.clone(),
            0,
            rx,
            done_rx,
        );
        assert!(matches!(result, Err(Error::CapabilityUnavailable)));
        assert_eq!(parent.check(), Err(Error::Cancelled));
        source.join().unwrap();
        assert!(started.elapsed() < Duration::from_secs(4));
    }
}
