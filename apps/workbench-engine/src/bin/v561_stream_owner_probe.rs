//! Real Rust owner diagnostic: no TTS, device, GUI or public authorization.
use nai_contracts::workbench::WorkbenchError as Error;
use nai_runtime_core::foundation::Operation;
use nai_workbench_engine::{v561_avatar_stream::Resident, v561_avatar_worker::Config};
use sha2::{Digest, Sha256};
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
fn run() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if !(5..=6).contains(&args.len()) {
        return Err("usage: v561_stream_owner_probe <config> <worker.py> <pcm> <reference.json> <report-dir>".into());
    }
    let pause = args
        .get(5)
        .map(|v| v.parse::<u64>())
        .transpose()
        .map_err(|_| "PAUSE_INVALID")?
        .unwrap_or(0);
    if pause > 2000 {
        return Err("PAUSE_INVALID".into());
    }
    let config: Config =
        serde_json::from_slice(&read(&args[0], 65536)?).map_err(|_| "CONFIG_INVALID")?;
    let source = read(&args[2], 144000)?;
    let pcm = source.repeat(2);
    if pcm.is_empty() || !pcm.len().is_multiple_of(2) || pcm.len() > 288000 {
        return Err("PCM_LIMIT".into());
    }
    let samples = (pcm.len() / 2) as u64;
    let digest = Sha256::digest(&pcm).to_vec();
    let reference: serde_json::Value =
        serde_json::from_slice(&read(&args[3], 65536)?).map_err(|_| "REFERENCE_INVALID")?;
    let hashes = reference["results"][0]["frame_digests"]
        .as_array()
        .ok_or("REFERENCE_INVALID")?
        .iter()
        .map(|v| v.as_str().map(str::to_owned).ok_or("REFERENCE_INVALID"))
        .collect::<Result<Vec<_>, _>>()?;
    if hashes.len() as u64 != (samples * 25).div_ceil(24000) {
        return Err("REFERENCE_COUNT".into());
    }
    let dir = Path::new(&args[4]);
    std::fs::create_dir_all(dir).map_err(|_| "REPORT_IO")?;
    let mut owner = Resident::start(
        &config,
        Path::new(&args[1]),
        &Operation::maintenance(config.maintenance_timeout_ms).map_err(|e| e.as_str_name())?,
        &dir.join("worker.log"),
    )
    .map_err(|e| e.as_str_name())?;
    let ready_ms = owner.ready.elapsed_ms;
    let mut results = Vec::new();
    for (index, mode) in ["complete", "cancelled", "recovered"]
        .into_iter()
        .enumerate()
    {
        let task = index as u64 + 1;
        let began = Instant::now();
        let parent = Operation::model_task(45000).map_err(|e| e.as_str_name())?;
        owner
            .begin(task, task, 144000, &parent)
            .map_err(|e| e.as_str_name())?;
        let mut received = Vec::new();
        let mut first = None;
        let mut cancelled = false;
        let mut before_eos = 0;
        let mut polled = 0;
        {
            let mut accept = |frame: nai_media_core::v561_frame_scheduler::Frame| {
                if first.is_none() {
                    first = Some(began.elapsed().as_millis());
                }
                received.push(
                    frame
                        .metadata
                        .content_sha256
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>(),
                );
                if mode == "cancelled" && received.len() == 10 {
                    return Err(Error::Cancelled);
                }
                Ok(())
            };
            for (index, chunk) in pcm.chunks(76800).enumerate() {
                match owner.chunk(chunk, &mut accept) {
                    Ok(()) => {}
                    Err(Error::Cancelled) if mode == "cancelled" => {
                        cancelled = true;
                        break;
                    }
                    Err(e) => return Err(e.as_str_name().into()),
                }
                if index == 1 && pause > 0 {
                    let end = Instant::now() + std::time::Duration::from_millis(pause);
                    while Instant::now() < end {
                        match owner.poll_frames(&mut accept) {
                            Ok(count) => polled += count,
                            Err(Error::Cancelled) if mode == "cancelled" => {
                                cancelled = true;
                                break;
                            }
                            Err(e) => return Err(e.as_str_name().into()),
                        }
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    if cancelled {
                        break;
                    }
                }
            }
        }
        if !cancelled {
            before_eos = received.len();
            owner
                .finish_verified(samples, &digest, |frame| {
                    if first.is_none() {
                        first = Some(began.elapsed().as_millis());
                    }
                    received.push(
                        frame
                            .metadata
                            .content_sha256
                            .iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<String>(),
                    );
                    Ok(())
                })
                .map_err(|e| e.as_str_name())?;
            if received != hashes {
                return Err("PIXEL_REFERENCE".into());
            }
        } else if received.len() != 10 {
            return Err("CANCEL_COUNT".into());
        }
        if !owner.available() {
            return Err("OWNER_NOT_RECOVERED".into());
        }
        if pause > 0 && !cancelled && polled == 0 {
            return Err("NO_FRAMES_DURING_SOURCE_PAUSE".into());
        }
        let result = serde_json::json!({"mode":mode,"frames_read":received.len(),"frames_before_eos":before_eos,"first_frame_ms":first,
            "elapsed_ms":began.elapsed().as_millis(),"cancelled":cancelled,"frames_polled_during_source_pause":polled,
            "source_pause_ms":pause,"pixels_equal_reference":if cancelled {None}else{Some(received==hashes)}});
        println!("{result}");
        results.push(result);
    }
    let report = serde_json::json!({"scope":"rust_owner_real_gpu_no_tts_or_device","protocol":2,"model_loads":1,"model_api_calls":0,"ready_ms":ready_ms,"samples":samples,"results":results});
    serde_json::to_writer_pretty(
        std::fs::File::create_new(dir.join("report.json")).map_err(|_| "REPORT_IO")?,
        &report,
    )
    .map_err(|_| "REPORT_IO")?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
