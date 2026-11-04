//! Prepared media and execution live in one fixed worker; audio never waits for its control lock.
use nai_audio_cpal::clocked_experiment::{ExperimentalFence, open_pcm_controlled};
use nai_contracts::workbench::*;
use nai_media_core::{
    played_frame::{PlayedFrameBoard, Shown},
    public_avatar::BuiltinAvatar,
    public_clip::PublicClipRenderer,
    public_cue::{CueClockGate, CueReading, map_public_cue},
    public_device_loss::{DEVICE_LOST, apply_device_loss},
};
use nai_runtime_core::{foundation::Operation, public_execution::PublicExecutionGate};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use zeroize::Zeroize;
pub struct CompositedFrame {
    pub base: PublicMediaFrameInfo,
    pub pixels: Vec<u32>,
    pub caption_visible: bool,
    pub caption_sha: String,
    pub sha: String,
    pub preparation_id: String,
}
enum Command {
    Prepare(Box<PublicWindowPreparation>, Arc<AtomicBool>, u64),
    Execute(Box<PublicExecutionLease>),
    Refresh,
}
#[derive(Clone)]
struct Snapshot {
    view: PublicWindowPrepared,
    frame: Option<Arc<CompositedFrame>>,
    generation: u64,
    retire: Instant,
    lease_until: Option<Instant>,
}
pub struct Preloader {
    tx: mpsc::SyncSender<Command>,
    state: Arc<Mutex<Option<Snapshot>>>,
    cancel: Option<Arc<AtomicBool>>,
    closed: Arc<AtomicBool>,
    generation: u64,
}
fn publish(state: &Mutex<Option<Snapshot>>, s: Snapshot) {
    if let Ok(mut old) = state.try_lock() {
        *old = Some(s);
    }
}
struct Environment<'a> {
    root: &'a PathBuf,
    id: &'a str,
    state: &'a Mutex<Option<Snapshot>>,
    closed: &'a AtomicBool,
    rx: &'a mpsc::Receiver<Command>,
}
fn clip(
    mut x: PublicWindowPreparation,
    cancel: Arc<AtomicBool>,
    generation: u64,
    e: &Environment<'_>,
) -> Result<Option<Command>, crate::caption::Failure> {
    let key = zeroize::Zeroizing::new(std::mem::take(&mut x.execution_key));
    let deadline = Instant::now() + Duration::from_secs(4);
    let check = || {
        if cancel.load(Ordering::Acquire) || e.closed.load(Ordering::Acquire) {
            Err(WorkbenchError::Cancelled)
        } else if Instant::now() >= deadline {
            Err(WorkbenchError::DeadlineExceeded)
        } else {
            Ok(())
        }
    };
    let consumed = nai_media_core::window_preload::load(e.root, e.id, &x, &check)?;
    let bundle = x.bundle.take().ok_or(WorkbenchError::IntegrityFailed)?;
    if nai_contracts::public_media_hash(&bundle) != x.bundle_sha256 {
        return Err(WorkbenchError::IntegrityFailed.into());
    }
    let output_id = bundle.output_id.clone();
    let session_id = bundle.session_id.clone();
    let epoch = bundle.epoch;
    if epoch == 0 {
        return Err(WorkbenchError::IntegrityFailed.into());
    }
    let caption = crate::caption::Caption::new(&x.caption_text, &bundle.caption_sha256, &check)?;
    let descriptor = x
        .descriptor
        .as_ref()
        .ok_or(WorkbenchError::IntegrityFailed)?;
    let mut gate = PublicExecutionGate::new(
        key.to_string(),
        bundle,
        descriptor,
        &consumed.receipt,
        x.preparation_id.clone(),
    )?;
    let fence = gate.callback_fence();
    let mut devices =
        nai_audio_cpal::snapshot().map_err(|_| WorkbenchError::CapabilityUnavailable)?;
    let index = devices
        .iter()
        .position(|(_, v)| v.is_default)
        .ok_or(WorkbenchError::CapabilityUnavailable)?;
    let device_view = devices.swap_remove(index).1;
    let avatar = BuiltinAvatar::new(&check)?;
    let mut renderer = PublicClipRenderer::new(
        &consumed.samples,
        descriptor
            .audio
            .as_ref()
            .ok_or(WorkbenchError::IntegrityFailed)?,
        &avatar,
        &check,
    )?;
    let base = Instant::now();
    let experimental = Arc::new(ExperimentalFence::externally_controlled());
    let origin = base;
    let allowed = fence.clone();
    let probe = open_pcm_controlled(
        experimental.clone(),
        consumed.samples.clone(),
        Arc::new(move || allowed.allows(origin.elapsed().as_millis() as u64)),
    )
    .map_err(|_| WorkbenchError::CapabilityUnavailable)?;
    let mut board =
        PlayedFrameBoard::open(epoch, 8).map_err(|_| WorkbenchError::InvalidArgument)?;
    check()?;
    gate.initialization_barrier()?;
    let mut view = PublicWindowPrepared {
        receipt: Some(consumed.receipt.clone()),
        device_ready: true,
        device_name: device_view.name,
        reason: "DEVICE_INITIALIZED_MUTED".into(),
        submitted_samples: 0,
        preparation_id: x.preparation_id.clone(),
        execution_challenge: gate.challenge(base.elapsed().as_millis() as u64)?,
        caption_ready: true,
        caption_sha256: caption.sha.clone(),
        caption_font: caption.font.clone(),
        caption_font_sha256: caption.font_sha.clone(),
        caption_state: PublicCaptionState::Ready as i32,
        ..Default::default()
    };
    let mut retire = Instant::now() + Duration::from_secs(5);
    let mut latest = None;
    let mut frame_index = None;
    let mut last_cue_played = None;
    loop {
        if cancel.load(Ordering::Acquire) || e.closed.load(Ordering::Acquire) {
            experimental.revoke();
            gate.stop();
            return Ok(None);
        }
        let now = base.elapsed().as_millis() as u64;
        let active = gate.active(now);
        if probe.failed.load(Ordering::Acquire) && view.reason != DEVICE_LOST {
            experimental.revoke();
            board.revoke();
            gate.stop();
            apply_device_loss(&mut view);
            last_cue_played = None;
            retire = Instant::now() + Duration::from_secs(5);
            latest = None;
        }
        let reading = probe.clock_reading();
        view.submitted_samples = reading.submitted_samples;
        let cue_gate = if view.reason == DEVICE_LOST {
            CueClockGate::DeviceLost
        } else if view.execution_stopped {
            CueClockGate::Cancelled
        } else {
            CueClockGate::Live
        };
        let mapped = map_public_cue(
            epoch,
            CueReading {
                stream_generation: reading.stream_generation,
                device_frequency: reading.device_frequency,
                device_position: reading.device_position,
                cue_device_start: reading.cue_device_start,
                cue_device_end: reading.cue_device_end,
                cue_span_count: reading.cue_span_count,
            },
            cue_gate,
        );
        if let Ok(Some(played)) = mapped {
            last_cue_played = Some(played);
        }
        if view.reason == DEVICE_LOST || view.reason == "DEVICE_INITIALIZED_MUTED" {
            // Sentence-bound clock requires an accepted execution lease.
            // Muted init still opens WASAPI; the host lease_epoch is 0 until execute().
            view.device_clock = None;
        } else if reading.stream_generation >= 1 && reading.device_frequency > 0 {
            view.device_clock = Some(PublicDeviceClockObservation {
                output_id: output_id.clone(),
                preparation_id: x.preparation_id.clone(),
                session_id: session_id.clone(),
                epoch,
                stream_generation: reading.stream_generation,
                device_frequency: reading.device_frequency,
                device_position: reading.device_position,
                sample_rate: 24000,
                played_samples: last_cue_played.unwrap_or(0),
            });
        } else {
            view.device_clock = None;
        }
        if active && view.reason != DEVICE_LOST {
            let caption_visible = nai_media_core::public_caption::active(
                true,
                false,
                view.submitted_samples,
                consumed.samples.len() as u64,
            );
            if let Ok(Some(played)) = mapped {
                board.offer_windows(played, consumed.samples.len() as u64, now);
                if let Shown::Frame {
                    frame_id,
                    source_sample,
                } = board.present(epoch, played)
                    && frame_index != Some((frame_id, caption_visible))
                {
                    let cursor = source_sample.min(consumed.samples.len() as u64 - 1);
                    let f = renderer
                        .frame_at(cursor, &|| {
                            if cancel.load(Ordering::Acquire) {
                                Err(WorkbenchError::Cancelled)
                            } else {
                                Ok(())
                            }
                        })?
                        .ok_or(WorkbenchError::IntegrityFailed)?;
                    view.frame = Some(f.info.clone());
                    let mut pixels = f.pixels;
                    if caption_visible {
                        caption.compose(&mut pixels, &|| {
                            if cancel.load(Ordering::Acquire) {
                                Err(WorkbenchError::Cancelled)
                            } else {
                                Ok(())
                            }
                        })?;
                    }
                    let sha = nai_media_core::public_clip::composited_hash(&pixels, &|| {
                        if cancel.load(Ordering::Acquire) {
                            Err(WorkbenchError::Cancelled)
                        } else {
                            Ok(())
                        }
                    })?;
                    latest = Some(Arc::new(CompositedFrame {
                        base: f.info,
                        pixels,
                        caption_visible,
                        caption_sha: caption.sha.clone(),
                        sha,
                        preparation_id: x.preparation_id.clone(),
                    }));
                    frame_index = Some((frame_id, caption_visible));
                }
            }
            view.playing = true;
            view.reason = "PUBLIC_EXECUTING".into();
            if view.submitted_samples == consumed.samples.len() as u64 {
                view.submission_complete = true;
                experimental.revoke();
                board.revoke();
                gate.stop();
                view.playing = false;
                view.execution_stopped = true;
                view.device_ready = false;
                view.execution_challenge = 0;
                view.reason = "SUBMISSION_COMPLETE".into();
                retire = Instant::now() + Duration::from_secs(5);
                latest = None;
            }
        } else if view.execution_seq > 0 && !view.execution_stopped {
            experimental.revoke();
            board.revoke();
            gate.stop();
            view.playing = false;
            view.execution_stopped = true;
            view.device_ready = false;
            view.execution_challenge = 0;
            view.reason = "EXECUTION_STOPPED".into();
            retire = Instant::now() + Duration::from_secs(5);
            latest = None;
        }
        let lease_until = if view.playing {
            Some(base + Duration::from_millis(gate.deadline()))
        } else {
            None
        };
        publish(
            e.state,
            Snapshot {
                view: view.clone(),
                frame: latest.clone(),
                generation,
                retire,
                lease_until,
            },
        );
        if Instant::now() >= retire && !view.playing {
            experimental.revoke();
            gate.stop();
            return Ok(None);
        }
        match e.rx.recv_timeout(Duration::from_millis(10)) {
            Ok(c @ Command::Prepare(..)) => {
                experimental.revoke();
                gate.stop();
                return Ok(Some(c));
            }
            Ok(Command::Refresh) => {
                if !view.execution_stopped {
                    match gate.challenge(base.elapsed().as_millis() as u64) {
                        Ok(ch) => view.execution_challenge = ch,
                        Err(_) => {
                            view.execution_stopped = true;
                            view.device_ready = false;
                            view.playing = false;
                            view.execution_challenge = 0;
                            view.reason = "EXECUTION_STOPPED".into();
                        }
                    }
                }
            }
            Ok(Command::Execute(mut lease)) => {
                if view.execution_stopped {
                    lease.authorization.zeroize();
                    continue;
                }
                let result = gate.accept(
                    &lease,
                    base.elapsed().as_millis() as u64,
                    &Operation::new(1000)?,
                );
                lease.authorization.zeroize();
                if result.is_ok() {
                    let _ = experimental.activate();
                    view.execution_seq = lease.seq;
                    view.execution_challenge = 0;
                    view.playing = true;
                } else {
                    experimental.revoke();
                    board.revoke();
                    gate.stop();
                    view.execution_stopped = true;
                    view.device_ready = false;
                    view.playing = false;
                    view.execution_challenge = 0;
                    view.reason = "EXECUTION_STOPPED".into();
                    latest = None;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => (),
            Err(_) => {
                experimental.revoke();
                gate.stop();
                return Ok(None);
            }
        }
    }
}
impl Preloader {
    pub fn new(root: PathBuf, id: String) -> Self {
        let (tx, rx) = mpsc::sync_channel(2);
        let state = Arc::new(Mutex::new(None));
        let closed = Arc::new(AtomicBool::new(false));
        let output = state.clone();
        let done = closed.clone();
        std::thread::spawn(move || {
            let e = Environment {
                root: &root,
                id: &id,
                state: &output,
                closed: &done,
                rx: &rx,
            };
            let mut pending = None;
            while !done.load(Ordering::Acquire) {
                let c = if let Some(c) = pending.take() {
                    c
                } else {
                    match rx.recv_timeout(Duration::from_millis(20)) {
                        Ok(c) => c,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(_) => break,
                    }
                };
                if let Command::Prepare(x, cancel, generation) = c {
                    match clip(*x, cancel.clone(), generation, &e) {
                        Ok(next) => pending = next,
                        Err(err) => {
                            cancel.store(true, Ordering::Release);
                            publish(
                                &output,
                                Snapshot {
                                    view: PublicWindowPrepared {
                                        reason: err.code.as_str_name().into(),
                                        caption_state: err.caption as i32,
                                        ..Default::default()
                                    },
                                    frame: None,
                                    generation,
                                    retire: Instant::now() + Duration::from_secs(5),
                                    lease_until: None,
                                },
                            );
                        }
                    }
                }
            }
        });
        Self {
            tx,
            state,
            cancel: None,
            closed,
            generation: 0,
        }
    }
    pub fn current_valid(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(|c| !c.load(Ordering::Acquire))
    }
    pub fn clear(&mut self) {
        if let Some(c) = self.cancel.take() {
            c.store(true, Ordering::Release);
        }
        self.generation = self.generation.saturating_add(1);
        if let Ok(mut s) = self.state.try_lock() {
            *s = None;
        }
    }
    pub fn prepare(&mut self, x: PublicWindowPreparation) -> Result<(), ()> {
        self.clear();
        let cancel = Arc::new(AtomicBool::new(false));
        self.tx
            .try_send(Command::Prepare(
                Box::new(x),
                cancel.clone(),
                self.generation,
            ))
            .map_err(|_| ())?;
        self.cancel = Some(cancel);
        Ok(())
    }
    pub fn execute(&mut self, x: PublicExecutionLease) -> Result<(), ()> {
        self.tx
            .try_send(Command::Execute(Box::new(x)))
            .map_err(|_| {
                self.clear();
            })
    }
    pub fn refresh(&mut self) -> Result<(), ()> {
        self.tx.try_send(Command::Refresh).map_err(|_| {
            self.clear();
        })
    }
    fn snapshot(&self) -> Option<Snapshot> {
        let mut s = self.state.try_lock().ok()?.as_ref()?.clone();
        if s.generation != self.generation || Instant::now() >= s.retire && !s.view.playing {
            return None;
        }
        if s.view.playing && s.lease_until.is_none_or(|t| Instant::now() >= t) {
            s.view.playing = false;
            s.view.device_ready = false;
            s.view.execution_stopped = true;
            s.view.execution_challenge = 0;
            s.view.reason = "EXECUTION_STOPPED".into();
            s.frame = None;
        }
        if !self.current_valid() && s.view.device_ready {
            return None;
        }
        Some(s)
    }
    pub fn poll(&mut self) -> Option<(PublicWindowPrepared, Instant)> {
        self.snapshot().map(|s| {
            (
                s.view,
                if s.lease_until.is_some() {
                    Instant::now() + Duration::from_secs(1)
                } else {
                    s.retire
                },
            )
        })
    }
    pub fn frame(&self) -> Option<Arc<CompositedFrame>> {
        let s = self.snapshot()?;
        if s.view.playing { s.frame } else { None }
    }
}
impl Drop for Preloader {
    fn drop(&mut self) {
        self.clear();
        self.closed.store(true, Ordering::Release);
    }
}
