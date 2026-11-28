//! Local real HTTP -> bounded provisional compute blocks. No GPU or playback.
use nai_runtime_core::foundation::Operation;
use nai_workbench_engine::v561_avatar_input;
use std::{io::Read, time::Instant};
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
    if args.len() != 4 {
        return Err("usage: nai-v561-avatar-input-probe <tts-config> <text> <block-frames-from-ready> <report.json>".into());
    }
    let config = serde_json::from_slice(&read(&args[0], 32768)?).map_err(|_| "CONFIG_INVALID")?;
    let text = String::from_utf8(read(&args[1], 8192)?).map_err(|_| "TEXT_INVALID")?;
    let frames = args[2].parse().map_err(|_| "FRAMES_INVALID")?;
    let started = Instant::now();
    let mut blocks = Vec::new();
    let summary=v561_avatar_input::produce(&config,text.trim(),1,1,frames,Operation::model_task(45000).map_err(|e|e.as_str_name())?,|block| {
        blocks.push(serde_json::json!({"start_sample":block.start_sample,"samples":block.pcm.len()/2,"eos_verified":block.eos_verified,"received_ms":started.elapsed().as_millis()}));
        Ok(())
    }).map_err(|e|e.as_str_name())?;
    let report = serde_json::json!({"scope":"provisional_input_not_gpu_or_playback","model_calls":1,"samples":summary.samples,"first_pcm_ms":summary.first_pcm_ms,"http_eof_ms":summary.transport_eof_ms,"provider_complete_ms":summary.complete_ms,"completed_ms":started.elapsed().as_millis(),"eos_verified":true,"blocks":blocks});
    serde_json::to_writer_pretty(
        std::fs::File::create_new(&args[3]).map_err(|_| "REPORT_IO")?,
        &report,
    )
    .map_err(|_| "REPORT_IO")?;
    println!("{report}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
