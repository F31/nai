//! Semantic script coordination. GPU/voice/device implementations stay separate.
use crate::{v561_script_source::Pipeline, v561_stream_output::PreparedSession};
use nai_llm_openai::cosyvoice::Config;
use nai_runtime_core::foundation::Operation;
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Instant,
};
#[allow(clippy::too_many_arguments)]
pub fn play(
    session: &mut PreparedSession,
    config: Config,
    text: &str,
    audio: &Path,
    dir: &Path,
    prefill: u64,
    operation: Operation,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    play_with_output(
        session, config, text, audio, dir, prefill, operation, None, None,
    )
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn play_with_output(
    session: &mut PreparedSession,
    config: Config,
    text: &str,
    audio: &Path,
    dir: &Path,
    prefill: u64,
    operation: Operation,
    shared_window: Option<&mut minifb::Window>,
    standby: Option<&crate::v561_standby::View>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    play_script(
        session,
        config,
        text,
        audio,
        dir,
        prefill,
        operation,
        shared_window,
        standby,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn play_with_obs(
    session: &mut PreparedSession,
    config: Config,
    text: &str,
    audio: &Path,
    dir: &Path,
    prefill: u64,
    operation: Operation,
    mut encoder: crate::v561_obs_encoder::Encoder,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    encoder.send_black_silence(crate::v561_obs_encoder::OBS_PREROLL_FRAMES)?;
    let report = play_script(
        session,
        config,
        text,
        audio,
        dir,
        prefill,
        operation,
        None,
        None,
        Some(&mut encoder),
    )?;
    if report["status"] == "completed" {
        encoder.finish_with_tail()?;
    }
    Ok(report)
}

#[allow(clippy::too_many_arguments)]
fn play_script(
    session: &mut PreparedSession,
    config: Config,
    text: &str,
    audio: &Path,
    dir: &Path,
    prefill: u64,
    operation: Operation,
    shared_window: Option<&mut minifb::Window>,
    standby: Option<&crate::v561_standby::View>,
    mut encoder: Option<&mut crate::v561_obs_encoder::Encoder>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let pipeline = Pipeline::new(config, text, operation.clone()).map_err(|e| e.as_str_name())?;
    let count = pipeline.count;
    let pipeline = Arc::new(Mutex::new(pipeline));
    std::fs::create_dir_all(dir)?;
    let mut reports = Vec::new();
    let mut samples = 0u64;
    let mut frames = 0u64;
    let mut shown = 0u64;
    let mut epoch = 0u64;
    let mut previous_completed = None;
    let mut overlaps = 0;
    let mut previous_started = None;
    let mut during_playback = 0;
    let window_creations = u32::from(shared_window.is_none());
    let mut owned_window;
    let window = match shared_window {
        Some(window) => window,
        None => {
            owned_window = session.script_window()?;
            &mut owned_window
        }
    };
    let mut waiting_screen = standby.map(|view| {
        let (w, h) = view.dimensions();
        vec![0; w * h]
    });
    let mut shared_audio =
        crate::v561_continuous_session::Session::start(audio, operation.clone(), count)
            .map_err(|e| e.as_str_name())?;
    let audio_handle = shared_audio.handle();
    let mut gpu = session
        .gpu_script(
            pipeline.clone(),
            operation.clone(),
            count,
            &dir.join("gpu-activity"),
        )
        .map_err(|e| e.as_str_name())?;
    let window_identity = format!("{:p}", window.get_window_handle());
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        loop {
            let waiting = Instant::now();
            let source = gpu
                .next_with(|| {
                    if let Some(encoder) = encoder.as_deref_mut() {
                        encoder
                            .keep_idle()
                            .map_err(|_| nai_contracts::workbench::WorkbenchError::IoFailed)?;
                    }
                    if let (Some(view), Some(screen)) = (standby, waiting_screen.as_mut()) {
                        crate::v561_standby::fill(screen, Some(view));
                        let (w, h) = view.dimensions();
                        window.update_with_buffer(screen, w, h).map_err(|_| {
                            nai_contracts::workbench::WorkbenchError::CapabilityUnavailable
                        })?;
                    } else {
                        window.update();
                    }
                    crate::v561_presentation::require_render_area(window.get_size(), &operation)
                        .map_err(|_| {
                            nai_contracts::workbench::WorkbenchError::CapabilityUnavailable
                        })?;
                    if !window.is_open() || window.is_key_down(minifb::Key::Escape) {
                        return Err(nai_contracts::workbench::WorkbenchError::Cancelled);
                    }
                    Ok(())
                })
                .map_err(|e| {
                    if window.get_size().0 == 0 || window.get_size().1 == 0 {
                        "OUTPUT_WINDOW_UNAVAILABLE"
                    } else if !window.is_open() || window.is_key_down(minifb::Key::Escape) {
                        "SCRIPT_STOPPED"
                    } else {
                        e.as_str_name()
                    }
                })?;
            let Some(source) = source else { break };
            let source_wait_ms = waiting.elapsed().as_millis();
            let facts = source.facts;
            let index = facts.index;
            let op = operation
                .model_child(operation.remaining_ms().map_err(|e| e.as_str_name())?)
                .map_err(|e| e.as_str_name())?;
            let observer = encoder.as_ref().map(|_| session.observe_output());
            let (playback, obs_result) = std::thread::scope(|scope| {
                let forwarder = observer
                    .zip(encoder.as_deref_mut())
                    .map(|(observer, encoder)| {
                        scope.spawn(move || {
                            crate::v561_obs_encoder::forward_segment(observer, encoder)
                        })
                    });
                let playback = session.play_source(
                    source,
                    audio,
                    &dir.join(format!("sentence-{index}")),
                    prefill,
                    op,
                    crate::v561_stream_output::ScriptOutput {
                        window: &mut *window,
                        audio: &audio_handle,
                        standby,
                    },
                );
                let obs_result = forwarder.map(|thread| thread.join());
                (playback, obs_result)
            });
            let mut report = playback?;
            if let Some(result) = obs_result {
                let (frames, completed) = result.map_err(|_| "OBS forwarder panicked")??;
                if !completed || frames == 0 {
                    return Err("OBS segment incomplete".into());
                }
                report["obs_forwarded_frames"] = frames.into();
            }
            let ready = report["source_ready_script_ms"].as_u64().map(u128::from);
            let overlap = ready
                .zip(previous_completed)
                .is_some_and(|(at, end)| at < end);
            report["source_wait_ms"] = serde_json::json!(source_wait_ms);
            report["output_window_identity"] = window_identity.clone().into();
            report["gap_from_previous_device_end_ms"] = serde_json::json!(
                report["device_first_played_script_ms"]
                    .as_u64()
                    .map(u128::from)
                    .zip(previous_completed)
                    .map(|(start, end)| start.saturating_sub(end))
            );
            report["sentence_index"] = index.into();
            let playing = ready
                .zip(previous_started.zip(previous_completed))
                .is_some_and(|(ready, (start, end))| ready >= start && ready < end);
            report["source_mode"] = if index > 1 {
                "streaming_future"
            } else {
                "streaming_first"
            }
            .into();
            previous_started = report["device_first_played_script_ms"]
                .as_u64()
                .map(u128::from);
            during_playback += u32::from(playing);
            report["source_ready_script_ms"] = serde_json::json!(ready);
            report["prefetched_before_previous_device_completion"] = overlap.into();
            if report["status"] != "completed" {
                reports.push(report);
                return Err("SCRIPT_STOPPED".into());
            }
            samples = samples
                .checked_add(report["played_samples"].as_u64().ok_or("missing samples")?)
                .ok_or("sample overflow")?;
            frames += report["frames"].as_u64().ok_or("missing frames")?;
            shown += report["shown_frames"]
                .as_u64()
                .ok_or("missing submissions")?;
            epoch = report["epoch"].as_u64().ok_or("missing epoch")?;
            previous_completed = report["device_completed_script_ms"]
                .as_u64()
                .map(u128::from);
            overlaps += u32::from(overlap);
            reports.push(report);
        }
        Ok(())
    })();
    let gpu_outcome = gpu.finish(result.is_err()).map_err(|e| e.as_str_name());
    let gpu_result = match gpu_outcome {
        Ok((owner, epoch, status)) => {
            session.restore_script(owner, epoch);
            status.map_err(|e| e.as_str_name())
        }
        Err(e) => Err(e),
    };
    let result = result
        .and_then(|_| gpu_result.map_err(Into::into))
        .and_then(|_| shared_audio.finish().map_err(|e| e.as_str_name().into()));
    let failure = result.as_ref().err().map(|e| {
        let code = e.to_string();
        if code.starts_with("WORKBENCH_ERROR_")
            && code.len() <= 64
            && code.bytes().all(|b| b.is_ascii_uppercase() || b == b'_')
        {
            code
        } else if matches!(code.as_str(), "stale device clock" | "device clock expired") {
            "OUTPUT_CLOCK_EXPIRED".into()
        } else if code == "SCRIPT_STOPPED" {
            "SCRIPT_STOPPED".into()
        } else {
            "SCRIPT_FAILED".into()
        }
    });
    let report = serde_json::json!({"status":if result.is_ok(){"completed"}else{"stopped_or_failed"},"failure_code":failure,"scope":"single_tts_semantic_script_not_gapless_or_interjection","output_window_creations":window_creations,"device_open_count":1,"device_process_id":shared_audio.pid,
        "samples":samples,"played_samples":samples,"frames":frames,"shown_frames":shown,"epoch":epoch,"sentences":count,
        "completed_sentences":reports.iter().filter(|r|r["status"]=="completed").count(),
        "model_api_calls":pipeline.lock().map_err(|_|"pipeline poisoned")?.model_calls(),"ready_before_previous_device_end_count":overlaps,"prefetch_ready_during_playback_count":during_playback,"worker_pid":session.worker_pid,"results":reports});
    serde_json::to_writer_pretty(std::fs::File::create_new(dir.join("script.json"))?, &report)?;
    if let Err(error) = result {
        if error.to_string() == "SCRIPT_STOPPED" {
            return Ok(serde_json::json!({"status":"stopped"}));
        }
        return Err(error);
    }
    Ok(report)
}
