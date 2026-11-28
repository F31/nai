//! Explicit prepared-session diagnostic with optional local OBS encoder.
#[cfg(not(windows))]
fn main() {
    std::process::exit(1);
}
#[cfg(windows)]
fn main() {
    if let Err(error) = run() {
        eprintln!("STREAM_AV_FAILED {error}");
        std::process::exit(1);
    }
}
#[cfg(windows)]
fn run() -> Result<(), Box<dyn std::error::Error>> {
    use nai_workbench_engine::{v561_avatar_worker::Config, v561_stream_output::PreparedSession};
    use std::{io::Read, path::Path};
    fn read(path: &str, limit: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(limit + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > limit {
            return Err("input limit".into());
        }
        Ok(bytes)
    }
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    if ![8, 9, 11, 12, 13].contains(&a.len())
        || (a.len() == 9 && !["--manual-start", "--sequence"].contains(&a[8].as_str()))
        || ((11..=12).contains(&a.len())
            && !["--obs-loopback", "--obs-script-loopback"].contains(&a[8].as_str()))
        || (a.len() == 13 && a[8] != "--obs-prepared-loopback")
    {
        return Err("expected avatar-config worker tts-config text audio-exe report-dir prefill stop-ms [--manual-start|--sequence|--obs-loopback ffmpeg port [ready-file]|--obs-script-loopback ffmpeg port [ready-file]|--obs-prepared-loopback ffmpeg port ready-file cache-policy]".into());
    }
    let config: Config = serde_json::from_slice(&read(&a[0], 65536)?)?;
    config
        .validate()
        .map_err(|e| format!("avatar configuration: {}", e.as_str_name()))?;
    let geometry = (config.width, config.height);
    let tts: nai_llm_openai::cosyvoice::Config = serde_json::from_slice(&read(&a[2], 32768)?)?;
    let text = String::from_utf8(read(&a[3], 8192)?)?;
    let prefill = a[6].parse::<u64>()?;
    let stop = a[7].parse::<u64>()?;
    if stop != 0 && a.get(8).is_some_and(|mode| mode == "--obs-script-loopback") {
        return Err("script OBS diagnostic requires stop-ms=0".into());
    }
    let sequence = a.get(8).is_some_and(|s| s == "--sequence");
    let manual = a.get(8).is_some_and(|s| s == "--manual-start");
    let dir = Path::new(&a[5]);
    let ready_file = a.get(11).map(Path::new);
    if ready_file.is_some_and(|path| !path.is_absolute() || path.exists()) {
        return Err("OBS ready file must be absolute and absent before preparation".into());
    }
    let worker = Path::new(&a[1]);
    if !worker.is_absolute()
        || !worker.is_file()
        || worker.file_name().and_then(|name| name.to_str()) != Some("flashhead_stream_worker.py")
    {
        return Err("worker script must be an absolute path to flashhead_stream_worker.py".into());
    }
    let operation =
        nai_runtime_core::foundation::Operation::maintenance(config.maintenance_timeout_ms)
            .map_err(|error| format!("avatar maintenance budget: {}", error.as_str_name()))?;
    let mut session = PreparedSession::prepare_in(config, worker, dir, &operation)
        .map_err(|error| format!("avatar preparation: {error}"))?;
    let plan = if let Some(path) = a.get(12) {
        let policy = serde_json::from_slice(&read(path, 8192)?)?;
        let op = nai_runtime_core::foundation::Operation::model_task(45000)
            .map_err(|error| error.as_str_name())?;
        let keys = session
            .content_keys(&tts, &text, &op)
            .map_err(|error| error.as_str_name())?;
        let plan = session.prepare_plan(&tts, &text, policy, &dir.join("prepared-script"), op)?;
        Some((plan, keys))
    } else {
        None
    };
    let ready = serde_json::json!({"scope":"visual_worker_prepared_not_overall_live_ready","ready_ms":session.ready_ms,"preparation_ms":session.preparation_ms,"worker_pid":session.worker_pid,"model_loads":1,"tts_calls":plan.as_ref().map_or(0, |(plan, _)| plan.assets().len())});
    serde_json::to_writer_pretty(
        std::fs::File::create_new(dir.join("prepared.json"))?,
        &ready,
    )?;
    println!("{ready}");
    let mut media_nonce = None;
    if let Some(path) = ready_file {
        let began = std::time::Instant::now();
        loop {
            if path.is_file() {
                let ready: serde_json::Value =
                    serde_json::from_slice(&read(a[11].as_str(), 8192)?)?;
                if ready["recording_confirmed"] == true {
                    media_nonce = ready["media_nonce"].as_str().map(str::to_owned);
                    break;
                }
                return Err("invalid OBS recording acknowledgement".into());
            }
            if began.elapsed() >= std::time::Duration::from_secs(45) {
                return Err("OBS recording acknowledgement timed out".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    }
    if let Some((plan, keys)) = plan {
        let mut encoder = nai_workbench_engine::v561_obs_encoder::Encoder::start(
            Path::new(&a[9]),
            geometry.0,
            geometry.1,
            a[10].parse()?,
        )?;
        if let Some((ready, nonce)) = ready_file.zip(media_nonce.as_deref()) {
            encoder.bind_recording_lease(ready, nonce)?;
        }
        let operation = nai_runtime_core::foundation::Operation::model_task(45000)
            .map_err(|error| error.as_str_name())?;
        if stop > 0 {
            if stop > 20_000 || media_nonce.is_none() {
                return Err(
                    "OBS stop drill requires a leased recording and stop-ms <= 20000".into(),
                );
            }
            encoder.arm_stop_drill(operation.clone(), std::time::Duration::from_millis(stop));
        }
        let report = nai_workbench_engine::v561_prepared_plan::play_with_obs(
            plan,
            &keys,
            &mut session,
            Path::new(&a[4]),
            dir,
            prefill,
            operation,
            encoder,
        )?;
        println!("{report}");
        return Ok(());
    }
    let stops = if sequence {
        vec![0, 0, 80, 0]
    } else {
        vec![stop]
    };
    if a.get(8).is_some_and(|mode| mode == "--obs-script-loopback") {
        let encoder = nai_workbench_engine::v561_obs_encoder::Encoder::start(
            Path::new(&a[9]),
            geometry.0,
            geometry.1,
            a[10].parse()?,
        )?;
        let operation = nai_runtime_core::foundation::Operation::model_task(45000)
            .map_err(|error| error.as_str_name())?;
        let report = nai_workbench_engine::v561_stream_script::play_with_obs(
            &mut session,
            tts,
            &text,
            Path::new(&a[4]),
            dir,
            prefill,
            operation,
            encoder,
        )?;
        println!("{report}");
        return if report["status"] == "completed" {
            Ok(())
        } else {
            Err("OBS script stopped".into())
        };
    }
    let mut results = Vec::new();
    for (index, stop) in stops.into_iter().enumerate() {
        let step = if sequence {
            dir.join(format!("step-{}", index + 1))
        } else {
            dir.to_path_buf()
        };
        let forwarder = if a.len() >= 11 {
            use nai_workbench_engine::v561_obs_encoder::{Encoder, OBS_PREROLL_FRAMES, forward};
            let mut encoder =
                Encoder::start(Path::new(&a[9]), geometry.0, geometry.1, a[10].parse()?)?;
            encoder.send_black_silence(OBS_PREROLL_FRAMES)?;
            let observer = session.observe_output();
            Some(std::thread::spawn(move || forward(observer, encoder)))
        } else {
            None
        };
        let playback = session.play(
            tts.clone(),
            text.clone(),
            Path::new(&a[4]),
            &step,
            prefill,
            stop,
            manual,
        );
        let forwarding = forwarder.map(|worker| worker.join());
        if playback.is_err()
            && let Some(result) = &forwarding
        {
            match result {
                Ok(Ok(frames)) => {
                    eprintln!("OBS_FORWARDED_FRAMES_BEFORE_PLAYBACK_FAILURE={frames}")
                }
                Ok(Err(error)) => eprintln!("OBS_FORWARDING_FAILED {error}"),
                Err(_) => eprintln!("OBS_FORWARDER_PANICKED"),
            }
        }
        let report = playback?;
        if let Some(result) = forwarding {
            let frames = result.map_err(|_| "OBS forwarder panicked")??;
            if frames == 0 {
                return Err("OBS forwarding produced no frames".into());
            }
            println!("OBS_FORWARDED_FRAMES={frames}");
        }
        if sequence
            && (report["status"] != if index == 2 { "stopped" } else { "completed" }
                || !session.available())
        {
            return Err("prepared sequence incomplete".into());
        }
        println!("{report}");
        results.push(report);
    }
    if sequence {
        let report = serde_json::json!({"scope":"native_prepared_four_step_not_soak","model_loads":1,"ready_ms":session.ready_ms,"preparation_ms":session.preparation_ms,"worker_pid":session.worker_pid,"model_api_calls":results.len(),"results":results});
        serde_json::to_writer_pretty(
            std::fs::File::create_new(dir.join("sequence.json"))?,
            &report,
        )?;
    }
    Ok(())
}
