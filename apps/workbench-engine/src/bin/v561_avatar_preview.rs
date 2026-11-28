//! Same-context audio batch; --tts verifies one real provider source before AV. Not online GPU input.
#[cfg(windows)]
fn run(args: &[String], tts: bool) -> Result<(), String> {
    use nai_contracts::workbench::WorkbenchError as Error;
    use nai_runtime_core::foundation::Operation;
    use nai_workbench_engine::{
        v561_avatar_batch::{Batch, Context},
        v561_avatar_preview::{Clip, play},
        v561_avatar_worker::{Config, Resident},
    };
    use ring::rand::{SecureRandom, SystemRandom};
    use std::{io::Read, path::Path, time::Instant};
    fn read(path: &str, limit: u64) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|_| "INPUT_IO")?
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "INPUT_IO")?;
        if bytes.len() as u64 > limit {
            return Err("INPUT_LIMIT".into());
        }
        Ok(bytes)
    }
    if !(6..=7).contains(&args.len()) {
        return Err("usage: nai-v561-avatar-preview <config.json> <worker.py> <pcm-one|tts-config-with---tts> <pcm-two|text-with---tts> <stream-audio-output.exe> <report-directory> [stop_at_device_ms]".into());
    }
    let config: Config =
        serde_json::from_slice(&read(&args[0], 65536)?).map_err(|_| "CONFIG_INVALID")?;
    let stop = args
        .get(6)
        .map(|s| s.parse::<u64>())
        .transpose()
        .map_err(|_| "STOP_INVALID")?
        .unwrap_or(0);
    if stop > 20000 {
        return Err("STOP_INVALID".into());
    }
    let mut inputs = Vec::new();
    let mut samples = 0u64;
    for path in if tts { &args[2..2] } else { &args[2..4] } {
        let bytes = read(path, 288000)?;
        if bytes.is_empty() || !bytes.len().is_multiple_of(2) {
            return Err("PCM_INVALID".into());
        }
        samples += (bytes.len() / 2) as u64;
        inputs.push(bytes);
    }
    let count = (samples * 25).div_ceil(24000);
    if samples > 144000
        || u64::from(config.width) * u64::from(config.height) * 4 * count > 268435456
    {
        return Err("SEQUENCE_LIMIT".into());
    }
    let dir = Path::new(&args[5]);
    std::fs::create_dir_all(dir).map_err(|_| "REPORT_IO")?;
    let mut worker = Resident::start(
        &config,
        Path::new(&args[1]),
        &Operation::maintenance(config.maintenance_timeout_ms).map_err(|e| e.as_str_name())?,
        &dir.join("worker.log"),
    )
    .map_err(|e| e.as_str_name())?;
    let mut nonce = [0u8; 8];
    SystemRandom::new().fill(&mut nonce).map_err(|_| "RANDOM")?;
    let epoch = (u64::from_le_bytes(nonce) & 0x3fff_ffff_ffff_ffff) | 1;
    let operation = Operation::model_task(45000).map_err(|e| e.as_str_name())?;
    let context = Context::new(epoch, &config.portrait_sha256, operation.clone())
        .map_err(|e| e.as_str_name())?;
    let mut provider = None;
    let sources = if tts {
        let config: nai_llm_openai::cosyvoice::Config =
            serde_json::from_slice(&read(&args[2], 32768)?).map_err(|_| "TTS_CONFIG_INVALID")?;
        let text = String::from_utf8(read(&args[3], 8192)?).map_err(|_| "TEXT_INVALID")?;
        let (source, summary) = context
            .cosyvoice_source(1, &config, text.trim())
            .map_err(|e| e.as_str_name())?;
        samples = summary.samples;
        provider = Some(
            serde_json::json!({"calls":1,"eos_verified":true,"first_pcm_ms":summary.first_pcm_ms,"transport_eof_ms":summary.transport_eof_ms,"complete_ms":summary.complete_ms}),
        );
        vec![source]
    } else {
        inputs
            .into_iter()
            .enumerate()
            .map(|(index, bytes)| context.source(index as u64 + 1, bytes))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.as_str_name())?
    };
    let count = (samples * 25).div_ceil(24000);
    if samples > 144000
        || u64::from(config.width) * u64::from(config.height) * 4 * count > 268435456
    {
        return Err("SEQUENCE_LIMIT".into());
    }
    let began = Instant::now();
    let batch = Batch::seal(
        &context,
        sources,
        &config.temp_dir,
        config.width,
        config.height,
    )
    .map_err(|e| e.as_str_name())?;
    if batch.portrait() != config.portrait_sha256 {
        return Err("PORTRAIT_CONTEXT".into());
    }
    let render = batch.render().map_err(|e| e.as_str_name())?;
    let digest = render
        .pcm_sha256
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let pcm = batch.audio().map_err(|e| e.as_str_name())?;
    if batch.epoch() != epoch {
        return Err("SOURCE_EPOCH".into());
    }
    let (width, height) = batch.dimensions();
    let origins=batch.origins().iter().map(|o|serde_json::json!({"source_id":o.id,"sha256":o.sha256.iter().map(|b|format!("{b:02x}")).collect::<String>(),"start_sample":o.start,"samples":o.samples})).collect::<Vec<_>>();
    let lengths = batch.origins().iter().map(|o| o.samples).collect();
    let (prefill, capacity) = nai_workbench_engine::v561_frame_feed::capacity(
        width,
        height,
        count as usize,
        worker.warmup.block_frames as usize,
    )
    .map_err(|e| e.as_str_name())?;
    let (tx, rx) = nai_workbench_engine::v561_frame_feed::channel();
    let (render_tx, render_rx) = std::sync::mpsc::sync_channel(1);
    let producer_op = operation.clone();
    let task = std::thread::spawn(move || {
        let _source = batch;
        let result = worker.render_observed(1, epoch, render, &producer_op, |frame| {
            nai_workbench_engine::v561_frame_feed::send(&tx, frame, &producer_op)
        });
        let _ = render_tx.send(result.as_ref().map(|_| ()).map_err(|e| *e));
        (worker, result)
    });
    let played = play(
        Clip {
            epoch,
            samples,
            width,
            height,
            prefill_frames: prefill,
            capacity_frames: capacity,
            segment_samples: lengths,
        },
        pcm,
        args[4].clone().into(),
        operation.clone(),
        stop,
        rx,
        render_rx,
    );
    operation.cancel();
    let (worker, rendered) = task.join().map_err(|_| "RENDER_THREAD")?;
    let result = played.map_err(|e| e.as_str_name())?;
    let event = match rendered {
        Ok(event) => Some(event),
        Err(Error::Cancelled) if result.stopped => None,
        Err(e) => return Err(e.as_str_name().into()),
    };
    let stats = event
        .as_ref()
        .and_then(|e| e.stats.as_ref())
        .map(nai_workbench_engine::v561_avatar_metrics::json);
    let early = result
        .first_audio_played_ms
        .zip(result.render_complete_ms)
        .map(|(a, b)| a < b);
    let report = serde_json::json!({"scope":"bounded_source_incremental_frames_not_online_tts_or_continuous_live","worker_protocol":nai_contracts::v561_avatar::WireVersion::Current as u32,"compiled_warmup":worker.warmup_json(),"worker_ready_ms":worker.ready_ms,"epoch":epoch,"source_samples":samples,"source_origins":origins,"batch_sha256":digest,"model_api_calls":u32::from(tts),"provider":provider,"mapping_acl_certified":false,"elapsed_ms":began.elapsed().as_millis(),"first_frame_received_ms":result.first_frame_received_ms,"first_block_ready_ms":result.first_block_ready_ms,"first_audio_played_ms":result.first_audio_played_ms,"device_prepared_ms":result.device_prepared_ms,"admitted_ms":result.admitted_ms,"admission_target_ms":result.admission_target_ms,"render_complete_ms":result.render_complete_ms,"audio_before_render_complete":early,"prefill_frames":prefill,"queue_capacity_frames":capacity,"peak_queued_frames":result.peak_queued_frames,"resident_frame_bound":capacity+nai_workbench_engine::v561_frame_feed::RESERVE_FRAMES,"frames_received":result.frames_received,"window_frames_submitted":result.shown,"frame_conversions":result.frame_conversions,"buffer_updates":result.buffer_updates,"segments_shown":result.segments_shown,"clock_observations":result.clock_observations,"device_generation":result.device_generation,"device_ready_observations":result.device_ready_observations,"frame_admission_ms":result.frame_admission_ms,"played_samples":result.played_samples,"completed":result.completed,"stopped":result.stopped,"safe_frame_submitted":result.safe_frame_submitted,"stop_to_safe_ms":result.stop_to_safe_ms,"model_loads":1,"render_ms":event.as_ref().map(|e|e.elapsed_ms),"stats":stats});
    serde_json::to_writer_pretty(
        std::fs::File::create_new(dir.join("report.json")).map_err(|_| "REPORT_IO")?,
        &report,
    )
    .map_err(|_| "REPORT_IO")?;
    println!("{report}");
    Ok(())
}
fn main() {
    #[cfg(windows)]
    {
        let mut args = std::env::args().skip(1).collect::<Vec<_>>();
        let tts = args.first().is_some_and(|a| a == "--tts");
        if tts {
            args.remove(0);
        }
        if let Err(error) = run(&args, tts) {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
    #[cfg(not(windows))]
    {
        eprintln!("WINDOWS_REQUIRED");
        std::process::exit(1);
    }
}
