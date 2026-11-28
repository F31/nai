//! One real local TTS -> Rust owner -> GPU -> verified pixels. No audio device.
use nai_contracts::workbench::WorkbenchError as Error;
use nai_media_core::v561_frame_scheduler::Frame;
use nai_runtime_core::foundation::Operation;
use nai_workbench_engine::{
    v561_avatar_stream::Resident, v561_avatar_worker::Config, v561_stream_synthesis,
};
use nai_workbench_engine::{v561_pcm_gate::PcmGate, v561_stream_synthesis::MediaSink};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};
struct Sink {
    gate: PcmGate,
    frames: u64,
    released_hash: Sha256,
    source_hash: Option<Vec<u8>>,
    released_before_eos: u64,
}
impl Sink {
    fn drain(&mut self) -> Result<(), Error> {
        while let Some(mut bytes) = self
            .gate
            .take(4800, false)
            .map_err(|_| Error::ProtocolError)?
        {
            self.released_hash.update(&bytes);
            zeroize::Zeroize::zeroize(&mut bytes);
        }
        Ok(())
    }
}
impl MediaSink for Sink {
    fn pcm(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.gate.append(bytes).map_err(|_| Error::ProtocolError)
    }
    fn frame(&mut self, frame: Frame) -> Result<(), Error> {
        self.gate
            .frame(frame.metadata.source_sample.ok_or(Error::ProtocolError)?)
            .map_err(|_| Error::ProtocolError)?;
        self.frames += 1;
        self.drain()
    }
    fn verified_eos(&mut self, samples: u64, digest: &[u8]) -> Result<(), Error> {
        self.released_before_eos = self.gate.submitted();
        self.gate
            .seal(samples, digest)
            .map_err(|_| Error::ProtocolError)?;
        self.source_hash = Some(digest.to_vec());
        self.drain()
    }
}
fn read(path: &str, limit: u64) -> Result<Vec<u8>, String> {
    let mut b = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| "INPUT_IO")?
        .take(limit + 1)
        .read_to_end(&mut b)
        .map_err(|_| "INPUT_IO")?;
    if b.len() as u64 > limit {
        return Err("INPUT_LIMIT".into());
    }
    Ok(b)
}
fn run() -> Result<(), String> {
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    if a.len() != 5 {
        return Err("usage: v561_stream_tts_probe <avatar-config> <worker.py> <tts-config> <text.txt> <report-dir>".into());
    }
    let avatar: Config =
        serde_json::from_slice(&read(&a[0], 65536)?).map_err(|_| "CONFIG_INVALID")?;
    let tts: nai_llm_openai::cosyvoice::Config =
        serde_json::from_slice(&read(&a[2], 32768)?).map_err(|_| "TTS_CONFIG_INVALID")?;
    let text = String::from_utf8(read(&a[3], 8192)?).map_err(|_| "TEXT_INVALID")?;
    let dir = Path::new(&a[4]);
    std::fs::create_dir_all(dir).map_err(|_| "REPORT_IO")?;
    let mut owner = Resident::start(
        &avatar,
        Path::new(&a[1]),
        &Operation::maintenance(avatar.maintenance_timeout_ms).map_err(|e| e.as_str_name())?,
        &dir.join("worker.log"),
    )
    .map_err(|e| e.as_str_name())?;
    let operation = Operation::model_task(45000).map_err(|e| e.as_str_name())?;
    let mut sink = Sink {
        gate: PcmGate::new(
            tts.max_samples,
            u64::from(nai_workbench_engine::v561_stream_audio::Options::default().prefill_samples),
            operation.clone(),
        )
        .map_err(|_| "GATE_CONFIG")?,
        frames: 0,
        released_hash: Sha256::new(),
        source_hash: None,
        released_before_eos: 0,
    };
    let production = v561_stream_synthesis::generate_media(
        &mut owner,
        &tts,
        text.trim(),
        1,
        1,
        operation,
        &mut sink,
    )
    .map_err(|e| e.as_str_name())?;
    let summary = &production.provider;
    let event = &production.render;
    let released_digest = sink.released_hash.finalize();
    if sink.gate.submitted() != summary.samples
        || sink.source_hash.as_deref() != Some(released_digest.as_slice())
    {
        return Err("SOURCE_RELEASE_MISMATCH".into());
    }
    let report = serde_json::json!({"scope":"real_local_tts_rust_owner_gpu_verified_pixels_no_audio_device","model_api_calls":1,"model_loads":1,"ready_ms":owner.ready.elapsed_ms,
        "provider_eos_verified":true,"samples":summary.samples,"adapter_first_pcm_ms":summary.first_pcm_ms,"adapter_complete_ms":summary.complete_ms,
        "diagnostic_version":2,"timeline_origin":"generate_entry_shared_monotonic","first_pcm_ms":production.first_pcm_ms,
        "provider_verified_ms":production.provider_verified_ms,"first_verified_frame_ms":production.first_frame_ms,
        "frame_before_provider_eos":production.first_frame_ms.map(|v|v<production.provider_verified_ms),
        "completed_ms":production.completed_ms,"frames_read":sink.frames,"frames":event.frames,
        "pcm_gate":{"released_samples":sink.gate.submitted(),"released_before_eos":sink.released_before_eos,"source_digest_equal":true,"device_played":false},
        "stats":nai_workbench_engine::v561_avatar_metrics::json(event.stats.as_ref().ok_or("STATS_MISSING")?)});
    serde_json::to_writer_pretty(
        std::fs::File::create_new(dir.join("report.json")).map_err(|_| "REPORT_IO")?,
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
