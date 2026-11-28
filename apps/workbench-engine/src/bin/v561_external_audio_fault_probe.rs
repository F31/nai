//! Explicit native device fault drill using existing PCM, no model invocation.
#[cfg(not(windows))]
fn main() {
    std::process::exit(1);
}
#[cfg(windows)]
fn main() {
    if let Err(error) = run() {
        eprintln!("EXTERNAL_AUDIO_DRILL_FAILED {error}");
        std::process::exit(1);
    }
}
#[cfg(windows)]
fn run() -> Result<(), Box<dyn std::error::Error>> {
    use nai_runtime_core::foundation::Operation;
    use nai_workbench_engine::{v561_external_pcm, v561_stream_audio, v561_stream_clock};
    use std::{
        io::Read,
        time::{Duration, Instant},
    };
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    if a.len() != 4 || !["stop", "underrun", "disconnect"].contains(&a[3].as_str()) {
        return Err("expected pcm audio-exe report mode(stop|underrun|disconnect)".into());
    }
    let mut bytes = zeroize::Zeroizing::new(Vec::new());
    std::fs::File::open(&a[0])?
        .take(288001)
        .read_to_end(&mut bytes)?;
    if bytes.len() < 19200 || bytes.len() > 144000 || !bytes.len().is_multiple_of(2) {
        return Err("PCM bound".into());
    }
    let op = Operation::model_task(10000).map_err(|e| e.as_str_name())?;
    let (mut tx, source) = v561_external_pcm::channel(bytes.len() as u64 / 2, 16, op.clone())
        .map_err(|e| e.as_str_name())?;
    let (clock_tx, clock_rx) = v561_stream_clock::channel();
    let binary = std::path::PathBuf::from(&a[1]);
    let child_op = op.clone();
    let audio = std::thread::spawn(move || {
        let result = v561_stream_audio::play_external_with_clock(
            source,
            &binary,
            child_op.clone(),
            clock_tx,
            1,
            v561_stream_audio::Options::default(),
        );
        if result.is_err() {
            child_op.cancel();
        }
        result
    });
    let prepared = clock_rx.recv_timeout(Duration::from_secs(3))?;
    if !prepared.fresh() {
        op.cancel();
        return Err("prepared expired".into());
    }
    let data = if a[3] == "underrun" {
        &bytes[..19200]
    } else {
        &bytes[..]
    };
    for part in data.chunks(9600) {
        tx.push(part).map_err(|e| e.as_str_name())?;
    }
    if a[3] != "underrun" {
        tx.finish().map_err(|e| e.as_str_name())?;
    }
    let began = Instant::now();
    let mut played = 0;
    let mut first = None;
    let mut cancel_at = None;
    let mut terminal = false;
    while !audio.is_finished() && began.elapsed() < Duration::from_secs(4) {
        if let Ok(event) = clock_rx.recv_timeout(Duration::from_millis(10)) {
            terminal |=
                event.observation().state == nai_contracts::v561_media::ClockState::Stopped as i32;
            played = played.max(event.observation().played_sample.unwrap_or(0));
            if played > 0 && first.is_none() {
                first = Some(Instant::now());
                if a[3] == "disconnect" {
                    println!("DEVICE_PLAYING");
                }
            }
        }
        if a[3] == "stop"
            && cancel_at.is_none()
            && first.is_some_and(|t: Instant| t.elapsed() > Duration::from_millis(80))
        {
            cancel_at = Some(Instant::now());
            op.cancel();
        }
    }
    let finished = audio.is_finished();
    op.cancel();
    drop(tx);
    let result = audio.join().map_err(|_| "audio panic")?;
    let disconnected = loop {
        match clock_rx.try_recv() {
            Ok(event) => {
                terminal |= event.observation().state
                    == nai_contracts::v561_media::ClockState::Stopped as i32
            }
            Err(error) => break error == std::sync::mpsc::TryRecvError::Disconnected,
        }
    };
    if !finished || result.is_ok() || played == 0 {
        return Err("fault did not terminate active device playback".into());
    }
    if a[3] == "disconnect" && (!disconnected || terminal) {
        return Err("device clock did not disconnect without a stop receipt".into());
    }
    let report = serde_json::json!({"scope":"native_external_audio_fault_no_model","mode":a[3],"status":"passed",
        "observed_played_samples":played,"elapsed_ms":began.elapsed().as_millis(),"cancel_to_join_ms":cancel_at.map(|t|t.elapsed().as_millis()),
        "audio_result":result.err().map(|e|e.as_str_name()),"clock_disconnected":disconnected,"stop_receipt":terminal,"model_api_calls":0});
    serde_json::to_writer_pretty(std::fs::File::create_new(&a[2])?, &report)?;
    println!("{report}");
    Ok(())
}
