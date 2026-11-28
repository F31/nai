//! Explicit local audition diagnostic. Does not start a public/live output.
use nai_llm_openai::cosyvoice::Config;
use nai_workbench_engine::{
    v561_stream_audio::{Options, audition_with_clock},
    v561_stream_clock,
};
use std::io::Read;

fn bounded_file(path: &std::ffi::OsStr, maximum: u64) -> Result<Vec<u8>, &'static str> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| "INPUT_IO")?
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "INPUT_IO")?;
    if bytes.len() as u64 > maximum {
        return Err("INPUT_LIMIT");
    }
    Ok(bytes)
}
fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !(4..=5).contains(&args.len()) {
        eprintln!(
            "usage: nai-v561-cosyvoice-stream <config.json> <text.txt> <audio-output.exe> <report.json> [prefill_samples]"
        );
        std::process::exit(2);
    }
    let (clock_tx, clock_rx) = v561_stream_clock::channel();
    let clock_observer = std::thread::spawn(move || {
        let mut delivered = 0u64;
        let mut running = 0u64;
        let mut stopped = false;
        let mut last = 0u64;
        for event in clock_rx {
            delivered += 1;
            if let Some(clock) = event.running() {
                running += 1;
                last = clock.played_sample.unwrap_or(0);
            }
            if event.observation().state == nai_contracts::v561_media::ClockState::Stopped as i32 {
                stopped = true;
                last = event.observation().played_sample.unwrap_or(last);
            }
        }
        (delivered, running, stopped, last)
    });
    let result = (|| {
        let raw = bounded_file(&args[0], 32768)?;
        let config: Config = serde_json::from_slice(&raw).map_err(|_| "CONFIG_INVALID")?;
        let text = String::from_utf8(bounded_file(&args[1], 8192)?).map_err(|_| "TEXT_ENCODING")?;
        let mut options = Options::default();
        if let Some(value) = args.get(4) {
            options.prefill_samples = value
                .to_str()
                .ok_or("PREFILL_INVALID")?
                .parse()
                .map_err(|_| "PREFILL_INVALID")?;
        }
        audition_with_clock(
            config,
            text.trim().to_string(),
            std::path::Path::new(&args[2]),
            options,
            Some(clock_tx),
        )
        .map_err(|e| e.as_str_name())
    })();
    let observed = clock_observer.join().unwrap_or_default();
    let (report, failed) = match result {
        Ok(r) if !observed.2 || observed.3 != r.played_samples => (
            serde_json::json!({"status":"failed","code":"CLOCK_OBSERVER_INCOMPLETE","samples":r.samples,"played_samples":r.played_samples}),
            true,
        ),
        Ok(r) => (
            serde_json::json!({"status":"completed","samples":r.samples,"played_samples":r.played_samples,"first_pcm_ms":r.first_pcm_ms,
            "first_device_sample_ms":r.first_played_ms,"http_eof_ms":r.http_eof_ms,"completed_ms":r.completed_ms,
            "device_progress_before_http_eof":r.first_played_ms<r.http_eof_ms,"application_eos_verified":true,"sentences":r.sentences,"overlap_prefetches":r.overlap_prefetches,"model_calls":r.model_calls,"scope":"local_voice_audition_not_public_av",
            "clock_observations":r.clock_observations,"max_clock_age_ms":r.max_clock_age_ms,"clock_events_delivered":observed.0,"running_clock_events":observed.1,"stopped_clock_seen":observed.2,"observer_last_played_samples":observed.3,"epoch":r.epoch,"device_generation":r.device_generation}),
            false,
        ),
        Err(code) => (serde_json::json!({"status":"failed","code":code}), true),
    };
    let bytes = serde_json::to_vec_pretty(&report).unwrap_or_default();
    if std::fs::write(&args[3], &bytes).is_err() {
        eprintln!("REPORT_IO_FAILED");
        std::process::exit(1);
    }
    println!("{}", String::from_utf8_lossy(&bytes));
    if failed {
        std::process::exit(1);
    }
}
