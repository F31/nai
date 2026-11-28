//! Explicit same-PCM resident-worker diagnostic; does not grant public playback.
use nai_contracts::v561_avatar::Render;
use nai_runtime_core::foundation::Operation;
use nai_workbench_engine::v561_avatar_worker::{Config, Resident};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};

fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 4 {
        return Err("usage: nai-v561-avatar-probe <config.json> <worker.py> <pcm-s16le-24khz> <report-directory>".into());
    }
    let mut raw = Vec::new();
    std::fs::File::open(&args[0])
        .map_err(|_| "CONFIG_IO")?
        .take(65537)
        .read_to_end(&mut raw)
        .map_err(|_| "CONFIG_IO")?;
    if raw.len() > 65536 {
        return Err("CONFIG_LIMIT".into());
    }
    let config: Config = serde_json::from_slice(&raw).map_err(|_| "CONFIG_INVALID")?;
    let pcm = std::fs::canonicalize(&args[2]).map_err(|_| "PCM_IO")?;
    let mut bytes = Vec::new();
    std::fs::File::open(&pcm)
        .map_err(|_| "PCM_IO")?
        .take(288001)
        .read_to_end(&mut bytes)
        .map_err(|_| "PCM_IO")?;
    if bytes.is_empty() || bytes.len() > 288000 || !bytes.len().is_multiple_of(2) {
        return Err("PCM_LIMIT".into());
    }
    let digest = Sha256::digest(&bytes).to_vec();
    let directory = Path::new(&args[3]);
    std::fs::create_dir_all(directory).map_err(|_| "REPORT_IO")?;
    let mut worker = Resident::start(
        &config,
        Path::new(&args[1]),
        &Operation::maintenance(config.maintenance_timeout_ms).map_err(|e| e.as_str_name())?,
        &directory.join("worker.log"),
    )
    .map_err(|e| e.as_str_name())?;
    let mut jobs = Vec::new();
    for task in 1..=2 {
        let request = Render {
            pcm_path: pcm.to_str().ok_or("PCM_PATH")?.to_owned(),
            pcm_sha256: digest.clone(),
            samples: (bytes.len() / 2) as u64,
            sample_rate: 24000,
        };
        let event = worker
            .render(
                task,
                task,
                request,
                &Operation::model_task(45000).map_err(|e| e.as_str_name())?,
            )
            .map_err(|e| e.as_str_name())?;
        let stats = nai_workbench_engine::v561_avatar_metrics::json(
            event.stats.as_ref().ok_or("METRICS_MISSING")?,
        );
        jobs.push(serde_json::json!({"task":task,"epoch":event.epoch,"elapsed_ms":event.elapsed_ms,"frames":event.frames,"frames_acked":event.frames_acked,"model_loads":event.model_loads,"stats":stats}));
    }
    let request = Render {
        pcm_path: pcm.to_str().ok_or("PCM_PATH")?.to_owned(),
        pcm_sha256: digest.clone(),
        samples: (bytes.len() / 2) as u64,
        sample_rate: 24000,
    };
    let cancelled = Operation::model_task(45000).map_err(|e| e.as_str_name())?;
    let trigger = cancelled.clone();
    let cancellation = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(250));
        trigger.cancel();
    });
    let result = worker.render(3, 3, request.clone(), &cancelled);
    cancellation.join().map_err(|_| "CANCEL_THREAD")?;
    if result != Err(nai_contracts::workbench::WorkbenchError::Cancelled) {
        return Err("CANCEL_NOT_OBSERVED".into());
    }
    worker
        .drain_cancelled(&Operation::model_task(45000).map_err(|e| e.as_str_name())?)
        .map_err(|e| e.as_str_name())?;
    let resumed = worker
        .render(
            4,
            4,
            request,
            &Operation::model_task(45000).map_err(|e| e.as_str_name())?,
        )
        .map_err(|e| e.as_str_name())?;
    let resumed_stats = nai_workbench_engine::v561_avatar_metrics::json(
        resumed.stats.as_ref().ok_or("METRICS_MISSING")?,
    );
    let report = serde_json::json!({"scope":"resident_frame_receiver_only_not_public_playback","worker_protocol":nai_contracts::v561_avatar::WireVersion::Current as u32,"compiled_warmup":worker.warmup_json(),"ready_ms":worker.ready_ms,"samples":bytes.len()/2,"jobs":jobs,"cancelled_task":3,"resumed_task":4,"resumed_frames":resumed.frames,"resumed_frames_acked":resumed.frames_acked,"resumed_model_loads":resumed.model_loads,"resumed_stats":resumed_stats});
    serde_json::to_writer_pretty(
        std::fs::File::create_new(directory.join("report.json")).map_err(|_| "REPORT_IO")?,
        &report,
    )
    .map_err(|_| "REPORT_IO")?;
    println!("{report}");
    Ok(())
}
fn main() {
    if let Err(error) = run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
