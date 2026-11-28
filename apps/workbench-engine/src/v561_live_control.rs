//! Private stdio control. Input cancellation never takes the model lock.
use nai_contracts::{
    v561_live::{Command, Event, Kind, Request},
    v561_live_validation,
};
use nai_runtime_core::foundation::Operation;
use prost::Message;
use serde::Deserialize;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub llm: Option<crate::v561_reply_llm::Config>,
    pub avatar_config: PathBuf,
    pub tts_config: PathBuf,
    pub worker: PathBuf,
    pub prefill_samples: u64,
    pub tts_warmup_text: String,
    pub prepared_cache: crate::v561_prepared_asset::Policy,
    pub obs: Option<ObsProfile>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObsProfile {
    ffmpeg: PathBuf,
    port: u16,
    ready: PathBuf,
    #[serde(default = "single_obs_playback")]
    repetitions: u32,
    #[serde(default)]
    extended_session: bool,
}

fn single_obs_playback() -> u32 {
    1
}

#[cfg(windows)]
impl ObsProfile {
    fn limits(&self) -> (u32, u64) {
        if self.extended_session {
            (
                nai_media_core::continuous_audio::MAX_SCRIPT_SEGMENTS,
                nai_media_core::continuous_audio::MAX_SCRIPT_SAMPLES,
            )
        } else {
            (
                crate::v561_continuous_session::MAX_SEGMENTS,
                crate::v561_continuous_session::MAX_SAMPLES,
            )
        }
    }
    fn recording_ended(&self) -> bool {
        ["obs.stopped", "stop.request", "stop.receipt"]
            .iter()
            .any(|name| self.ready.with_file_name(name).exists())
    }

    fn start(&self, width: u32, height: u32) -> Result<crate::v561_obs_encoder::Encoder, ()> {
        if self.recording_ended() {
            return Err(());
        }
        let ready: serde_json::Value =
            serde_json::from_slice(&read(&self.ready, 8192)?).map_err(|_| ())?;
        if ready["recording_confirmed"] != true
            || ready["media_port"].as_u64() != Some(u64::from(self.port))
        {
            return Err(());
        }
        let nonce = ready["media_nonce"].as_str().ok_or(())?;
        let mut encoder =
            crate::v561_obs_encoder::Encoder::start(&self.ffmpeg, width, height, self.port)
                .map_err(|_| ())?;
        if self.recording_ended() {
            return Err(());
        }
        encoder
            .bind_recording_lease(&self.ready, nonce)
            .map_err(|_| ())?;
        Ok(encoder)
    }
}
fn read(path: &Path, limit: u64) -> Result<Vec<u8>, ()> {
    if !path.is_absolute() {
        return Err(());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| ())?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    if bytes.len() as u64 > limit {
        return Err(());
    }
    Ok(bytes)
}
fn packet(input: &mut impl Read) -> Result<Request, ()> {
    let mut prefix = [0; 4];
    input.read_exact(&mut prefix).map_err(|_| ())?;
    let n = u32::from_le_bytes(prefix) as usize;
    if n == 0 || n > 8192 {
        return Err(());
    }
    let mut bytes = vec![0; n];
    input.read_exact(&mut bytes).map_err(|_| ())?;
    Request::decode(bytes.as_slice()).map_err(|_| ())
}
fn bind(operation: Operation, generation: Arc<AtomicU64>, expected: u64) -> Result<Operation, ()> {
    operation
        .bind_permission(Arc::new(move || {
            generation.load(Ordering::Acquire) == expected
        }))
        .map_err(|_| ())
}
fn bind_worker(
    operation: Operation,
    generation: Arc<AtomicU64>,
    expected: u64,
    worker: Arc<AtomicBool>,
) -> Result<Operation, ()> {
    operation
        .bind_permission(Arc::new(move || {
            generation.load(Ordering::Acquire) == expected && worker.load(Ordering::Acquire)
        }))
        .map_err(|_| ())
}
#[cfg(windows)]
fn paint_idle(
    window: &mut minifb::Window,
    view: Option<&crate::v561_standby::View>,
    screen: &mut Vec<u32>,
    source_size: (usize, usize),
    witness: &mut crate::v561_window_witness::Witness,
) -> Result<(), ()> {
    // Match the speaking renderer: the native window may have a different
    // physical size after DPI scaling. It scales the canonical source buffer.
    let (w, h) = source_size;
    screen.resize(w.checked_mul(h).ok_or(())?, 0);
    crate::v561_standby::fill(screen, view);
    window.update_with_buffer(screen, w, h).map_err(|_| ())?;
    witness.observe(
        screen,
        (w, h),
        window.get_size(),
        window.is_active(),
        if view.is_some() {
            crate::v561_window_witness::Phase::Idle
        } else {
            crate::v561_window_witness::Phase::SafeBlack
        },
    );
    Ok(())
}
#[cfg(windows)]
pub fn run(profile: &Path, audio: &Path, artifacts: &Path) -> Result<(), &'static str> {
    run_inner(profile, audio, artifacts).map_err(|_| "SESSION_CONTROL_FAILED")
}
#[cfg(windows)]
fn run_inner(profile: &Path, audio: &Path, artifacts: &Path) -> Result<(), ()> {
    use crate::{v561_avatar_worker::Config, v561_stream_output::PreparedSession};
    let p: Profile = serde_json::from_slice(&read(profile, 32768)?).map_err(|_| ())?;
    if p.obs.as_ref().is_some_and(|obs| {
        obs.port < 1024
            || !obs.ffmpeg.is_absolute()
            || !obs.ffmpeg.is_file()
            || !obs.ready.is_absolute()
            || !(1..=obs.limits().0).contains(&obs.repetitions)
    }) {
        return Err(());
    }
    let obs_port = p.obs.as_ref().map(|obs| u32::from(obs.port));
    if let Some(llm) = &p.llm {
        llm.validate().map_err(|_| ())?;
    }
    if !p.worker.is_absolute()
        || !p.worker.is_file()
        || !(1..=96000).contains(&p.prefill_samples)
        || !artifacts.is_absolute()
        || crate::v561_voice_preparation::validate_text(&p.tts_warmup_text).is_err()
        || p.prepared_cache.validate().is_err()
    {
        return Err(());
    }
    let avatar: Config = serde_json::from_slice(&read(&p.avatar_config, 65536)?).map_err(|_| ())?;
    let tts: nai_llm_openai::cosyvoice::Config =
        serde_json::from_slice(&read(&p.tts_config, 32768)?).map_err(|_| ())?;
    if tts.max_samples > 144000 || p.prefill_samples > tts.max_samples {
        return Err(());
    }
    std::fs::create_dir_all(artifacts).map_err(|_| ())?;
    let generation = Arc::new(AtomicU64::new(1));
    let alive = Arc::new(AtomicBool::new(true));
    let (input_tx, input) = mpsc::sync_channel(2);
    let (output, events) = mpsc::sync_channel::<Event>(8);
    let input_generation = generation.clone();
    let input_alive = alive.clone();
    let priority_output = output.clone();
    std::thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        let mut last = 0;
        loop {
            let request = match packet(&mut input) {
                Ok(r) if v561_live_validation::request(&r, last) => r,
                _ => break,
            };
            last = request.sequence;
            let command = Command::try_from(request.command).unwrap_or(Command::Unspecified);
            if matches!(
                command,
                Command::Stop | Command::Shutdown | Command::ReplyText | Command::Ask
            ) {
                if input_generation
                    .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
                    .is_err()
                {
                    break;
                }
                let event = Event {
                    obs_port,
                    version: 1,
                    request_sequence: last,
                    kind: Kind::Stopping as i32,
                    busy: true,
                    code: if matches!(command, Command::ReplyText | Command::Ask) {
                        "REPLY_REQUESTED"
                    } else {
                        "STOP_REQUESTED"
                    }
                    .into(),
                    ..Default::default()
                };
                if priority_output.try_send(event).is_err() {
                    break;
                }
            }
            let epoch = input_generation.load(Ordering::Acquire);
            if input_tx.try_send((request, epoch)).is_err() {
                break;
            }
            if command == Command::Shutdown {
                break;
            }
        }
        input_alive.store(false, Ordering::Release);
        let _ = input_generation
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1));
    });
    let writer = std::thread::spawn(move || -> Result<(), ()> {
        let mut out = std::io::stdout().lock();
        let mut seq = 0u64;
        for mut event in events {
            seq = seq.checked_add(1).ok_or(())?;
            event.sequence = seq;
            if !v561_live_validation::event(&event, seq - 1) {
                return Err(());
            }
            let bytes = event.encode_to_vec();
            out.write_all(&(bytes.len() as u32).to_le_bytes())
                .and_then(|_| out.write_all(&bytes))
                .and_then(|_| out.flush())
                .map_err(|_| ())?;
        }
        Ok(())
    });
    let mut session = None::<PreparedSession>;
    let voice_ready = AtomicBool::new(false);
    let content_ready = AtomicBool::new(false);
    let mut content = None::<Arc<crate::v561_prepared_plan::Plan>>;
    let mut content_text = String::new();
    let mut content_generation = 0;
    let mut narration_cursor = None::<crate::v561_question::Cursor>;
    let mut questions = crate::v561_question::Slot::default();
    let mut public_window = None::<minifb::Window>;
    let mut standby = None::<crate::v561_standby::View>;
    let mut idle_witness =
        crate::v561_window_witness::Witness::new(artifacts.join("idle-window-pixels.json"));
    let mut idle_screen = Vec::new();
    let source_size = (avatar.width as usize, avatar.height as usize);
    let send = |kind: Kind,
                code: &str,
                request: u64,
                busy: bool,
                s: &Option<PreparedSession>,
                report: Option<&serde_json::Value>| {
        output
            .try_send(Event {
                obs_port,
                version: 1,
                request_sequence: request,
                kind: kind as i32,
                code: code.into(),
                busy,
                visual_prepared: s.as_ref().is_some_and(|s| s.available()),
                tts_prepared: voice_ready.load(Ordering::Acquire),
                content_prepared: content_ready.load(Ordering::Acquire),
                worker_pid: s.as_ref().map_or(0, |s| u64::from(s.worker_pid)),
                samples: report.and_then(|r| r["samples"].as_u64()),
                played_samples: report.and_then(|r| r["played_samples"].as_u64()),
                epoch: report.and_then(|r| r["epoch"].as_u64()).unwrap_or(0),
                ..Default::default()
            })
            .map_err(|_| ())
    };
    send(Kind::Unprepared, "UNPREPARED", 0, false, &session, None)?;
    let result = (|| {
        let mut last_request = 0;
        while alive.load(Ordering::Acquire) {
            if let Some(window) = public_window.as_mut() {
                if window.is_open() {
                    paint_idle(
                        window,
                        standby.as_ref(),
                        &mut idle_screen,
                        source_size,
                        &mut idle_witness,
                    )?;
                } else {
                    if let Some(view) = standby.take() {
                        view.revoke();
                    }
                    public_window = None;
                }
            }
            if session.as_ref().is_some_and(|s| !s.available())
                && (content.is_some() || standby.is_some())
            {
                if let Some(view) = standby.take() {
                    view.revoke();
                }
                content = None;
                narration_cursor = None;
                questions.cancel();
                content_ready.store(false, Ordering::Release);
                if let Some(window) = public_window.as_mut() {
                    paint_idle(
                        window,
                        None,
                        &mut idle_screen,
                        source_size,
                        &mut idle_witness,
                    )?;
                }
                send(
                    Kind::Unprepared,
                    "VISUAL_UNAVAILABLE",
                    last_request,
                    false,
                    &session,
                    None,
                )?;
            }
            if content.as_ref().is_some_and(|asset| !asset.valid()) {
                narration_cursor = None;
                questions.cancel();
                if let Some(view) = standby.take() {
                    view.revoke();
                }
                content = None;
                content_ready.store(false, Ordering::Release);
                let ready = voice_ready.load(Ordering::Acquire)
                    && session.as_ref().is_some_and(|s| s.available());
                send(
                    if ready {
                        Kind::Prepared
                    } else {
                        Kind::Unprepared
                    },
                    "CONTENT_EXPIRED",
                    last_request,
                    false,
                    &session,
                    None,
                )?;
            }
            let (request, expected) = match input.recv_timeout(Duration::from_millis(25)) {
                Ok(v) => v,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            };
            last_request = request.sequence;
            let kind = Command::try_from(request.command).map_err(|_| ())?;
            if matches!(kind, Command::Stop | Command::Shutdown) {
                narration_cursor = None;
                questions.cancel();
                if let Some(view) = standby.take() {
                    view.revoke();
                }
                if let Some(window) = public_window.as_mut() {
                    paint_idle(
                        window,
                        None,
                        &mut idle_screen,
                        source_size,
                        &mut idle_witness,
                    )?;
                }
                send(
                    Kind::Stopped,
                    "STOPPED",
                    request.sequence,
                    false,
                    &session,
                    None,
                )?;
                if kind == Command::Shutdown {
                    break;
                }
                continue;
            }
            if generation.load(Ordering::Acquire) != expected {
                send(
                    Kind::Stopped,
                    "CANCELLED",
                    request.sequence,
                    false,
                    &session,
                    None,
                )?;
                continue;
            }
            if kind == Command::Prepare {
                if voice_ready.load(Ordering::Acquire)
                    && session.as_ref().is_some_and(|s| s.available())
                {
                    send(
                        Kind::Prepared,
                        "PREPARED",
                        request.sequence,
                        false,
                        &session,
                        None,
                    )?;
                    continue;
                }
                send(
                    Kind::Preparing,
                    "TTS_PREPARING",
                    request.sequence,
                    true,
                    &session,
                    None,
                )?;
                let op = bind(
                    Operation::maintenance(avatar.maintenance_timeout_ms).map_err(|_| ())?,
                    generation.clone(),
                    expected,
                )?;
                let prepare_dir = artifacts.join(format!("prepare-{}", request.sequence));
                std::fs::create_dir_all(&prepare_dir).map_err(|_| ())?;
                if !voice_ready.load(Ordering::Acquire) {
                    let began = std::time::Instant::now();
                    let mut attempts = 0;
                    let outcome = crate::v561_voice_preparation::prepare(
                        &tts,
                        &p.tts_warmup_text,
                        &op,
                        &mut attempts,
                    );
                    let cancelled = matches!(
                        op.check(),
                        Err(nai_contracts::workbench::WorkbenchError::Cancelled)
                    );
                    let report = match &outcome {
                        Ok(summary) => {
                            serde_json::json!({"status":if cancelled {"cancelled"} else {"completed"},
                            "scope":"silent_tts_preparation_not_playback","samples":summary.samples,
                            "first_pcm_ms":summary.first_pcm_ms,"complete_ms":summary.complete_ms,
                            "elapsed_ms":began.elapsed().as_millis(),"audio_discarded":true,"synthesis_attempts":attempts,"model_api_calls":1})
                        }
                        Err(error) => {
                            serde_json::json!({"status":"failed_or_cancelled","code":error.as_str_name(),
                            "scope":"silent_tts_preparation_not_playback","elapsed_ms":began.elapsed().as_millis(),"audio_discarded":true,"synthesis_attempts":attempts,"model_api_calls":if attempts==0 {Some(0)}else{None}})
                        }
                    };
                    serde_json::to_writer_pretty(
                        std::fs::File::create_new(prepare_dir.join("tts-warmup.json"))
                            .map_err(|_| ())?,
                        &report,
                    )
                    .map_err(|_| ())?;
                    if outcome.is_err() || cancelled {
                        send(
                            if cancelled {
                                Kind::Stopped
                            } else {
                                Kind::Failed
                            },
                            if cancelled {
                                "PREPARATION_CANCELLED"
                            } else {
                                "TTS_PREPARATION_FAILED"
                            },
                            request.sequence,
                            false,
                            &session,
                            None,
                        )?;
                        continue;
                    }
                    voice_ready.store(true, Ordering::Release);
                }
                send(
                    Kind::Preparing,
                    "VISUAL_PREPARING",
                    request.sequence,
                    true,
                    &session,
                    None,
                )?;
                if session.as_ref().is_some_and(|s| s.available()) {
                    send(
                        Kind::Prepared,
                        "PREPARED",
                        request.sequence,
                        false,
                        &session,
                        None,
                    )?;
                    continue;
                }
                session = None;
                content = None;
                content_ready.store(false, Ordering::Release);
                let outcome =
                    PreparedSession::prepare_in(avatar.clone(), &p.worker, &prepare_dir, &op);
                if let Ok(value) = outcome
                    && op.check().is_ok()
                {
                    session = Some(value);
                }
                let cancelled = matches!(
                    op.check(),
                    Err(nai_contracts::workbench::WorkbenchError::Cancelled)
                );
                send(
                    if session.is_some() {
                        Kind::Prepared
                    } else if cancelled {
                        Kind::Stopped
                    } else {
                        Kind::Failed
                    },
                    if session.is_some() {
                        "PREPARED"
                    } else if cancelled {
                        "PREPARATION_CANCELLED"
                    } else if op.check().is_err() {
                        "PREPARATION_TIMEOUT"
                    } else {
                        crate::v561_avatar_stream::preparation_failure(&prepare_dir)
                    },
                    request.sequence,
                    false,
                    &session,
                    None,
                )?;
            } else if matches!(
                kind,
                Command::PrepareContent | Command::PlayPrepared | Command::PlayPreparedObs
            ) {
                if !voice_ready.load(Ordering::Acquire)
                    || !session.as_ref().is_some_and(|s| s.available())
                {
                    send(
                        Kind::Failed,
                        "NOT_PREPARED",
                        request.sequence,
                        false,
                        &session,
                        None,
                    )?;
                    continue;
                }
                let extended_obs = kind == Command::PlayPreparedObs
                    && p.obs.as_ref().is_some_and(|obs| obs.extended_session);
                let op = bind_worker(
                    if extended_obs {
                        Operation::prepared_playback(120000)
                    } else {
                        Operation::model_task(45000)
                    }
                    .map_err(|_| ())?,
                    generation.clone(),
                    expected,
                    session
                        .as_ref()
                        .and_then(PreparedSession::liveness)
                        .ok_or(())?,
                )?;
                if kind == Command::PrepareContent {
                    narration_cursor = None;
                    questions.cancel();
                    if let Some(view) = standby.take() {
                        view.revoke();
                    }
                    public_window = None;
                    content = None;
                    content_ready.store(false, Ordering::Release);
                    send(
                        Kind::ContentPreparing,
                        "CONTENT_PREPARING",
                        request.sequence,
                        true,
                        &session,
                        None,
                    )?;
                    let content_started = std::time::Instant::now();
                    let outcome = session.as_mut().ok_or(())?.prepare_plan(
                        &tts,
                        &request.text,
                        p.prepared_cache,
                        &artifacts.join(format!("content-{}", request.sequence)),
                        op.clone(),
                    );
                    let failure_reason = outcome
                        .as_ref()
                        .err()
                        .map(|e| crate::v561_prepared_plan::safe_failure_reason(e.as_ref()));
                    let mut failure_stage = "plan_preparation";
                    if let Ok(asset) = outcome
                        && op.check().is_ok()
                    {
                        failure_stage = "standby_window";
                        let view = asset.standby_view(
                            generation.clone(),
                            expected,
                            session
                                .as_ref()
                                .and_then(PreparedSession::liveness)
                                .ok_or(())?,
                        );
                        if let Ok(window) = session.as_ref().ok_or(())?.standby_window(Some(&view))
                            && op.check().is_ok()
                            && view.pixels().is_some()
                        {
                            let mut metadata = asset.standby_metadata();
                            let (native_width, native_height) = window.get_size();
                            metadata["native_window_width"] = native_width.into();
                            metadata["native_window_height"] = native_height.into();
                            serde_json::to_writer_pretty(
                                std::fs::File::create_new(
                                    artifacts
                                        .join(format!("content-{}/standby.json", request.sequence)),
                                )
                                .map_err(|_| ())?,
                                &metadata,
                            )
                            .map_err(|_| ())?;
                            public_window = Some(window);
                            standby = Some(view);
                            content = Some(asset);
                            content_text = request.text.clone();
                            content_generation = request.sequence;
                            content_ready.store(true, Ordering::Release);
                        }
                    }
                    let cancelled = generation.load(Ordering::Acquire) != expected;
                    if content.is_none() {
                        let dir = artifacts.join(format!("content-{}", request.sequence));
                        std::fs::create_dir_all(&dir).map_err(|_| ())?;
                        serde_json::to_writer_pretty(std::fs::File::create_new(dir.join("failure.json")).map_err(|_|())?,
                            &serde_json::json!({"scope":"private_session_asset_preparation","status":if cancelled{"cancelled"}else{"failed"},"published":false,
                                "stage":failure_stage,"reason":failure_reason,"operation_error":op.check().err().map(|e|e.as_str_name()),
                                "elapsed_ms":content_started.elapsed().as_millis()})).map_err(|_|())?;
                    }
                    send(
                        if content.is_some() {
                            Kind::ContentPrepared
                        } else if cancelled {
                            Kind::Stopped
                        } else {
                            Kind::Failed
                        },
                        if content.is_some() {
                            "CONTENT_PREPARED"
                        } else if cancelled {
                            "PREPARATION_CANCELLED"
                        } else {
                            "CONTENT_PREPARATION_FAILED"
                        },
                        request.sequence,
                        false,
                        &session,
                        None,
                    )?;
                } else {
                    let key = session
                        .as_ref()
                        .ok_or(())?
                        .content_keys(&tts, &request.text, &op);
                    let matching = key
                        .as_ref()
                        .ok()
                        .zip(content.as_ref())
                        .is_some_and(|(key, asset)| asset.matches(&request.text, key));
                    if !matching {
                        if let Some(view) = standby.take() {
                            view.revoke();
                        }
                        if let Some(window) = public_window.as_mut() {
                            paint_idle(
                                window,
                                None,
                                &mut idle_screen,
                                source_size,
                                &mut idle_witness,
                            )?;
                        }
                        content_ready.store(false, Ordering::Release);
                        send(
                            Kind::Failed,
                            "CONTENT_NOT_PREPARED",
                            request.sequence,
                            false,
                            &session,
                            None,
                        )?;
                        continue;
                    }
                    if let Some(view) = standby.take() {
                        view.revoke();
                    }
                    let view = content.as_ref().ok_or(())?.standby_view(
                        generation.clone(),
                        expected,
                        session
                            .as_ref()
                            .and_then(PreparedSession::liveness)
                            .ok_or(())?,
                    );
                    if public_window.is_none() {
                        public_window = Some(
                            session
                                .as_ref()
                                .ok_or(())?
                                .standby_window(Some(&view))
                                .map_err(|_| ())?,
                        );
                    }
                    standby = Some(view);
                    // Rebinding after Stop must restore the independent still
                    // before the potentially slow audio-device startup.
                    if let Some(window) = public_window.as_mut() {
                        paint_idle(
                            window,
                            standby.as_ref(),
                            &mut idle_screen,
                            source_size,
                            &mut idle_witness,
                        )?;
                    }
                    let obs = if kind == Command::PlayPreparedObs {
                        let assets = content.as_ref().ok_or(())?.assets();
                        let segments = assets.len() as u32;
                        let samples = assets.iter().map(|asset| asset.samples()).sum::<u64>();
                        if p.obs.as_ref().is_some_and(|profile| {
                            let (max_segments, max_samples) = profile.limits();
                            segments * profile.repetitions > max_segments
                                || samples * u64::from(profile.repetitions) > max_samples
                        }) {
                            send(
                                Kind::Failed,
                                "OBS_REPEAT_LIMIT",
                                request.sequence,
                                false,
                                &session,
                                None,
                            )?;
                            continue;
                        }
                        match p.obs.as_ref().ok_or(()).and_then(|obs| {
                            obs.start(avatar.width, avatar.height)
                                .map(|encoder| (encoder, obs.repetitions, obs.extended_session))
                        }) {
                            Ok(output) => Some(output),
                            Err(()) => {
                                send(
                                    Kind::Failed,
                                    "OBS_NOT_READY",
                                    request.sequence,
                                    false,
                                    &session,
                                    None,
                                )?;
                                continue;
                            }
                        }
                    } else {
                        None
                    };
                    send(
                        Kind::Starting,
                        if obs.is_some() {
                            "PLAYING_PREPARED_OBS"
                        } else {
                            "PLAYING_PREPARED"
                        },
                        request.sequence,
                        true,
                        &session,
                        None,
                    )?;
                    narration_cursor = Some(
                        crate::v561_question::Cursor::new(
                            content.as_ref().ok_or(())?.identity().to_vec(),
                            content_generation,
                            content.as_ref().ok_or(())?.assets().len() as u32,
                        )
                        .map_err(|_| ())?,
                    );
                    let outcome = crate::v561_prepared_plan::play_observed(
                        content.as_ref().ok_or(())?.clone(),
                        &key.map_err(|_| ())?,
                        session.as_mut().ok_or(())?,
                        audio,
                        &artifacts.join(format!("play-{}", request.sequence)),
                        p.prefill_samples,
                        op.clone(),
                        public_window.as_mut(),
                        standby.as_ref(),
                        narration_cursor.as_mut().ok_or(())?,
                        0,
                        obs,
                    );
                    let output_unavailable = outcome
                        .as_ref()
                        .err()
                        .is_some_and(|e| e.to_string() == "OUTPUT_WINDOW_UNAVAILABLE");
                    let obs_failed = kind == Command::PlayPreparedObs
                        && outcome
                            .as_ref()
                            .err()
                            .is_some_and(|error| error.to_string().starts_with("OBS "));
                    match outcome {
                        Ok(report)
                            if report["status"] == "completed"
                                && generation.load(Ordering::Acquire) == expected =>
                        {
                            send(
                                Kind::Completed,
                                if kind == Command::PlayPreparedObs {
                                    if report["obs_failed"] == true {
                                        "OBS_OUTPUT_FAILED"
                                    } else {
                                        "OBS_OUTPUT_COMPLETED"
                                    }
                                } else {
                                    "COMPLETED"
                                },
                                request.sequence,
                                false,
                                &session,
                                Some(&report),
                            )?
                        }
                        _ if generation.load(Ordering::Acquire) != expected => {
                            if let Some(view) = standby.take() {
                                view.revoke();
                            }
                            if let Some(window) = public_window.as_mut() {
                                paint_idle(
                                    window,
                                    None,
                                    &mut idle_screen,
                                    source_size,
                                    &mut idle_witness,
                                )?;
                            }
                            send(
                                Kind::Stopped,
                                "STOPPED",
                                request.sequence,
                                false,
                                &session,
                                None,
                            )?;
                        }
                        _ => {
                            if let Some(view) = standby.take() {
                                view.revoke();
                            }
                            if let Some(window) = public_window.as_mut() {
                                paint_idle(
                                    window,
                                    None,
                                    &mut idle_screen,
                                    source_size,
                                    &mut idle_witness,
                                )?;
                            }
                            content = None;
                            content_ready.store(false, Ordering::Release);
                            send(
                                Kind::Failed,
                                if output_unavailable {
                                    "OUTPUT_WINDOW_UNAVAILABLE"
                                } else if obs_failed {
                                    "OBS_PLAY_FAILED"
                                } else {
                                    "PREPARED_PLAY_FAILED"
                                },
                                request.sequence,
                                false,
                                &session,
                                None,
                            )?;
                        }
                    }
                }
            } else if matches!(kind, Command::ReplyText | Command::Ask) {
                if let Some(view) = standby.take() {
                    view.revoke();
                }
                let available = voice_ready.load(Ordering::Acquire)
                    && session.as_ref().is_some_and(|s| s.available())
                    && content.as_ref().is_some_and(|p| p.valid())
                    && narration_cursor
                        .as_ref()
                        .is_some_and(|c| c.resume().is_some());
                if !available {
                    questions.cancel();
                    send(
                        Kind::Failed,
                        "REPLY_NOT_AVAILABLE",
                        request.sequence,
                        false,
                        &session,
                        None,
                    )?;
                    continue;
                }
                send(
                    Kind::Starting,
                    if kind == Command::Ask {
                        "GENERATING_ANSWER"
                    } else {
                        "REPLY_TEXT"
                    },
                    request.sequence,
                    true,
                    &session,
                    None,
                )?;
                let key_op = bind(
                    Operation::model_task(45000).map_err(|_| ())?,
                    generation.clone(),
                    expected,
                );
                let keys = key_op.and_then(|op| {
                    session
                        .as_ref()
                        .ok_or(())?
                        .content_keys(&tts, &content_text, &op)
                        .map_err(|_| ())
                });
                let outcome = match keys {
                    Ok(keys) => crate::v561_reply_playback::execute(
                        crate::v561_reply_playback::Context {
                            slot: &mut questions,
                            cursor: narration_cursor.as_mut().ok_or(())?,
                            plan: content.as_ref().ok_or(())?.clone(),
                            keys: &keys,
                            content_generation,
                            generation: generation.clone(),
                            expected,
                            task_id: request.sequence,
                            question: if kind == Command::Ask {
                                &request.text
                            } else {
                                &request.reply.as_ref().ok_or(())?.question
                            },
                            answer: request.reply.as_ref().map(|r| r.answer.as_str()),
                            llm: p.llm.as_ref(),
                            source: &content_text,
                        },
                        session.as_mut().ok_or(())?,
                        tts.clone(),
                        audio,
                        &artifacts.join(format!("reply-{}", request.sequence)),
                        p.prefill_samples,
                        &mut public_window,
                        &mut standby,
                    ),
                    Err(_) => Err("RESUME_IDENTITY_CHANGED".into()),
                };
                let failure = outcome
                    .as_ref()
                    .err()
                    .map(|e| crate::v561_reply_llm::failure_code(&e.to_string()))
                    .unwrap_or("REPLY_FAILED");
                match outcome {
                    Ok(report) if generation.load(Ordering::Acquire) == expected => send(
                        Kind::Completed,
                        "REPLY_COMPLETED",
                        request.sequence,
                        false,
                        &session,
                        Some(&report),
                    )?,
                    _ => {
                        questions.cancel();
                        narration_cursor = None;
                        if let Some(view) = standby.take() {
                            view.revoke();
                        }
                        if let Some(window) = public_window.as_mut() {
                            paint_idle(
                                window,
                                None,
                                &mut idle_screen,
                                source_size,
                                &mut idle_witness,
                            )?;
                        }
                        let stopped = generation.load(Ordering::Acquire) != expected;
                        send(
                            if stopped { Kind::Stopped } else { Kind::Failed },
                            if stopped { "STOPPED" } else { failure },
                            request.sequence,
                            false,
                            &session,
                            None,
                        )?;
                    }
                }
            } else if kind == Command::Start {
                narration_cursor = None;
                questions.cancel();
                if let Some(view) = standby.take() {
                    view.revoke();
                }
                public_window = None;
                if !voice_ready.load(Ordering::Acquire)
                    || !session.as_ref().is_some_and(|s| s.available())
                {
                    send(
                        Kind::Failed,
                        "NOT_PREPARED",
                        request.sequence,
                        false,
                        &session,
                        None,
                    )?;
                    continue;
                }
                send(
                    Kind::Starting,
                    "PLAYING",
                    request.sequence,
                    true,
                    &session,
                    None,
                )?;
                let op = bind(
                    Operation::model_task(45000).map_err(|_| ())?,
                    generation.clone(),
                    expected,
                )?;
                let outcome = crate::v561_stream_script::play(
                    session.as_mut().ok_or(())?,
                    tts.clone(),
                    &request.text,
                    audio,
                    &artifacts.join(format!("play-{}", request.sequence)),
                    p.prefill_samples,
                    op.clone(),
                );
                let output_unavailable = outcome
                    .as_ref()
                    .err()
                    .is_some_and(|e| e.to_string() == "OUTPUT_WINDOW_UNAVAILABLE");
                match outcome {
                    Ok(report)
                        if report["status"] == "completed"
                            && generation.load(Ordering::Acquire) == expected =>
                    {
                        send(
                            Kind::Completed,
                            "COMPLETED",
                            request.sequence,
                            false,
                            &session,
                            Some(&report),
                        )?
                    }
                    Ok(report) if report["status"] == "stopped" => send(
                        Kind::Stopped,
                        "STOPPED",
                        request.sequence,
                        false,
                        &session,
                        None,
                    )?,
                    _ if generation.load(Ordering::Acquire) != expected => send(
                        Kind::Stopped,
                        "STOPPED",
                        request.sequence,
                        false,
                        &session,
                        None,
                    )?,
                    _ => send(
                        Kind::Failed,
                        if output_unavailable {
                            "OUTPUT_WINDOW_UNAVAILABLE"
                        } else {
                            "PLAY_FAILED"
                        },
                        request.sequence,
                        false,
                        &session,
                        None,
                    )?,
                }
            }
        }
        Ok(())
    })();
    let _ = generation.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1));
    if let Some(view) = standby.take() {
        view.revoke();
    }
    if let Some(window) = public_window.as_mut() {
        let _ = paint_idle(
            window,
            None,
            &mut idle_screen,
            source_size,
            &mut idle_witness,
        );
    }
    drop(session);
    drop(output);
    // Reader owns the other output clone until EOF; parent closes stdin on shutdown.
    if !alive.load(Ordering::Acquire) {
        writer.join().map_err(|_| ())??;
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_loss_revokes_media_operation_without_waiting_for_model() {
        let g = Arc::new(AtomicU64::new(1));
        let alive = Arc::new(AtomicBool::new(true));
        let op = bind_worker(Operation::model_task(1000).unwrap(), g, 1, alive.clone()).unwrap();
        assert!(op.check().is_ok());
        alive.store(false, Ordering::Release);
        assert!(op.check().is_err());
    }
    #[test]
    fn generation_revocation_cancels_without_model_lock() {
        let generation = Arc::new(AtomicU64::new(1));
        let op = bind(Operation::model_task(1000).unwrap(), generation.clone(), 1).unwrap();
        assert!(op.check().is_ok());
        generation.fetch_add(1, Ordering::AcqRel);
        assert!(op.check().is_err());
    }
    #[cfg(windows)]
    #[test]
    fn recorder_stop_receipt_blocks_reusing_start_acknowledgement() {
        let dir = tempfile::tempdir().unwrap();
        let profile = ObsProfile {
            ffmpeg: dir.path().join("ffmpeg.exe"),
            port: 19399,
            ready: dir.path().join("obs.ready"),
            repetitions: 1,
            extended_session: false,
        };
        std::fs::write(&profile.ready, b"{}").unwrap();
        assert!(!profile.recording_ended());
        std::fs::write(dir.path().join("stop.receipt"), b"recording stopped").unwrap();
        assert!(profile.recording_ended());
    }

    #[test]
    fn malformed_or_oversized_packets_rejected() {
        assert!(packet(&mut &0u32.to_le_bytes()[..]).is_err());
        assert!(packet(&mut &9000u32.to_le_bytes()[..]).is_err());
        assert!(packet(&mut &[1, 0, 0, 0, 255][..]).is_err());
    }
}
