//! One real TTS call -> device-verified source -> resident frame receiver. Not synchronous AV.
#[cfg(windows)]
fn run(args: &[String]) -> Result<(), String> {
    use nai_runtime_core::{foundation::Operation, v561_narration::Plan};
    use nai_workbench_engine::{
        v561_avatar_worker::{Config, Resident},
        v561_pcm_source::Source,
        v561_stream_audio::{Options, audition_plan_with_source},
    };
    use std::{io::Read, path::Path};
    fn read(path: &str, limit: u64) -> Result<Vec<u8>, String> {
        let mut v = Vec::new();
        std::fs::File::open(path)
            .map_err(|_| "INPUT_IO")?
            .take(limit + 1)
            .read_to_end(&mut v)
            .map_err(|_| "INPUT_IO")?;
        if v.len() as u64 > limit {
            return Err("INPUT_LIMIT".into());
        }
        Ok(v)
    }
    if args.len() != 6 {
        return Err("usage: nai-v561-av-source-probe <tts-config> <avatar-config> <worker.py> <audio-output.exe> <text.txt> <report-directory>".into());
    }
    let config: nai_llm_openai::cosyvoice::Config =
        serde_json::from_slice(&read(&args[0], 32768)?).map_err(|_| "TTS_CONFIG_INVALID")?;
    if config.max_samples > 144000 {
        return Err("SOURCE_LIMIT".into());
    }
    let avatar: Config =
        serde_json::from_slice(&read(&args[1], 65536)?).map_err(|_| "AVATAR_CONFIG_INVALID")?;
    let text = String::from_utf8(read(&args[4], 8192)?).map_err(|_| "TEXT_INVALID")?;
    let dir = Path::new(&args[5]);
    std::fs::create_dir_all(dir).map_err(|_| "REPORT_IO")?;
    let mut worker = Resident::start(
        &avatar,
        Path::new(&args[2]),
        &Operation::maintenance(avatar.maintenance_timeout_ms).map_err(|e| e.as_str_name())?,
        &dir.join("worker.log"),
    )
    .map_err(|e| e.as_str_name())?;
    let operation = Operation::model_task(45000).map_err(|e| e.as_str_name())?;
    let plan = Plan::new(text.trim(), config.lookahead.clone(), operation.clone())
        .map_err(|e| e.as_str_name())?;
    let mut source = Source::new(&avatar.temp_dir, config.max_samples, operation.clone())
        .map_err(|e| e.as_str_name())?;
    let audio = audition_plan_with_source(
        config,
        plan,
        Path::new(&args[3]),
        Options::default(),
        operation.clone(),
        None,
        Some(&mut source),
    )
    .map_err(|e| e.as_str_name())?;
    let (epoch, render) = source.render().map_err(|e| e.as_str_name())?;
    if epoch != audio.epoch || render.samples != audio.played_samples {
        return Err("SOURCE_IDENTITY".into());
    }
    let digest = render
        .pcm_sha256
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let frames = worker
        .render(1, epoch, render, &operation)
        .map_err(|e| e.as_str_name())?;
    let stats = nai_workbench_engine::v561_avatar_metrics::json(
        frames.stats.as_ref().ok_or("METRICS_MISSING")?,
    );
    let report = serde_json::json!({"scope":"device_then_avatar_not_synchronous_public_av","worker_protocol":nai_contracts::v561_avatar::WireVersion::Current as u32,"model_calls":audio.model_calls,"provider_eos_verified":true,"first_pcm_ms":audio.first_pcm_ms,"first_device_sample_ms":audio.first_played_ms,"http_eof_ms":audio.http_eof_ms,"audio_completed_ms":audio.completed_ms,"samples":audio.samples,"played_samples":audio.played_samples,"epoch":epoch,"pcm_sha256":digest,"worker_ready_ms":worker.ready_ms,"render_ms":frames.elapsed_ms,"frames":frames.frames,"frames_acked":frames.frames_acked,"model_loads":frames.model_loads,"source_is_task_local_not_cache":true,"stats":stats});
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
    if let Err(error) = run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
    #[cfg(not(windows))]
    {
        eprintln!("WINDOWS_REQUIRED");
        std::process::exit(1);
    }
}
