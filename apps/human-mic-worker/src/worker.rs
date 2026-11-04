use nai_audio_cpal::microphone::{self, Counts};
use nai_contracts::{audition_wire as wire, workbench::*};
use nai_runtime_core::human_mic::Gate;
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use zeroize::Zeroizing;
struct Meter {
    quit: Arc<AtomicBool>,
    closed: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
    peak: Arc<AtomicU32>,
}
impl Drop for Meter {
    fn drop(&mut self) {
        self.quit.store(true, Ordering::Release);
    }
}
impl Meter {
    fn open() -> Result<Self, ()> {
        let meter = Self {
            quit: Arc::new(AtomicBool::new(false)),
            closed: Arc::new(AtomicBool::new(false)),
            active: Arc::new(AtomicBool::new(false)),
            peak: Arc::new(AtomicU32::new(0)),
        };
        let (quit, closed, active, peak) = (
            meter.quit.clone(),
            meter.closed.clone(),
            meter.active.clone(),
            meter.peak.clone(),
        );
        let (ready, wait) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(move || {
                let mut window = minifb::Window::new(
                    "nai · 真人麦克风（OBS音轨）",
                    256,
                    64,
                    minifb::WindowOptions {
                        resize: false,
                        ..Default::default()
                    },
                )
                .map_err(|_| ())?;
                window.set_target_fps(20);
                ready.send(()).map_err(|_| ())?;
                let mut pixels = vec![0u32; 256 * 64];
                while !quit.load(Ordering::Acquire) && window.is_open() {
                    let on = active.load(Ordering::Acquire);
                    let width = if on {
                        (peak.load(Ordering::Acquire).min(65535) as usize * 232 / 65535).max(8)
                    } else {
                        232
                    };
                    pixels.fill(0x0f172a);
                    for y in 16..48 {
                        for x in 12..12 + width {
                            pixels[y * 256 + x] = if on { 0x10b981 } else { 0x475569 };
                        }
                    }
                    window
                        .update_with_buffer(&pixels, 256, 64)
                        .map_err(|_| ())?;
                }
                Ok::<(), ()>(())
            });
            let _ = result;
            closed.store(true, Ordering::Release);
        });
        wait.recv_timeout(Duration::from_secs(2)).map_err(|_| ())?;
        Ok(meter)
    }
}
fn marker(p: &Path) -> Result<String, ()> {
    let mut at = std::path::PathBuf::new();
    for part in p.components() {
        if matches!(part, std::path::Component::ParentDir) {
            return Err(());
        }
        at.push(part);
        if let Ok(m) = std::fs::symlink_metadata(&at) {
            use std::os::windows::fs::MetadataExt;
            if m.file_type().is_symlink() || m.file_attributes() & 0x400 != 0 {
                return Err(());
            }
        }
    }
    match std::fs::symlink_metadata(p) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Ok(m) if m.is_file() && m.len() <= 64 => std::fs::read_to_string(p).map_err(|_| ()),
        _ => Err(()),
    }
}
pub fn run() -> Result<(), ()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 || args[0] != "--stop-file" {
        return Err(());
    }
    let path = std::path::PathBuf::from(&args[1]);
    if !path.is_absolute() || path.file_name().and_then(|s| s.to_str()) != Some("human-mic.stop") {
        return Err(());
    }
    let nonce = marker(&path)?;
    let token = Zeroizing::new(std::env::var("NAI_MIC_TOKEN").map_err(|_| ())?);
    let instance = std::env::var("NAI_MIC_INSTANCE").map_err(|_| ())?;
    if token.len() != 64
        || instance.len() != 36
        || std::env::var_os("NAI_WORKBENCH_TOKEN").is_some()
    {
        return Err(());
    }
    let inputs = microphone::devices(true).map_err(|_| ())?;
    let outputs = microphone::devices(false).map_err(|_| ())?;
    let (tx, rx) = mpsc::sync_channel(2);
    std::thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        loop {
            match wire::read::<MicControl>(&mut input) {
                Ok(x) => {
                    if tx.send(Ok(x)).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    let _ = tx.send(Err(()));
                    break;
                }
            }
        }
    });
    let base = Instant::now();
    let now = || base.elapsed().as_millis() as u64;
    let stop = Arc::new(AtomicBool::new(false));
    let deadline = Arc::new(AtomicU64::new(0));
    let mut counts = Arc::new(Counts::default());
    let mut gate = Gate::default();
    let mut route = None;
    let mut window: Option<Meter> = None;
    let mut input_id = 0;
    let mut output_id = 0;
    let mut reason = "OFF";
    let emit = |g: &mut Gate, reason: &str, c: &Counts, input_id, output_id| -> Result<(), ()> {
        g.issue(now());
        wire::write(
            &mut std::io::stdout().lock(),
            &MicStatus {
                protocol_major: 1,
                instance_id: instance.clone(),
                seq: g.seq,
                challenge: g.challenge,
                active: g.live(now()),
                reason: reason.into(),
                inputs: inputs.iter().map(|(_, v)| v.clone()).collect(),
                outputs: outputs.iter().map(|(_, v)| v.clone()).collect(),
                input_id,
                output_id,
                captured_frames: c.captured.load(Ordering::Acquire),
                submitted_frames: c.submitted.load(Ordering::Acquire),
                dropped_frames: c.dropped.load(Ordering::Acquire),
                underrun_frames: c.underrun.load(Ordering::Acquire),
                peak_q16: c.peak.load(Ordering::Acquire),
                session_id: g.session.clone(),
            },
        )
        .map_err(|_| ())
    };
    emit(&mut gate, reason, &counts, input_id, output_id)?;
    let mut idle = Instant::now();
    loop {
        if let Some(w) = window.as_ref() {
            if w.closed.load(Ordering::Acquire) {
                stop.store(true, Ordering::Release);
            }
            w.active.store(gate.live(now()), Ordering::Release);
            w.peak
                .store(counts.peak.load(Ordering::Acquire), Ordering::Release);
        }
        let bad_marker = marker(&path).as_ref() != Ok(&nonce);
        if bad_marker
            || stop.load(Ordering::Acquire)
            || gate.active && !gate.live(now())
            || idle.elapsed() > Duration::from_secs(600)
        {
            stop.store(true, Ordering::Release);
            deadline.store(0, Ordering::Release);
            route.take();
            gate.stop();
            reason = if bad_marker {
                "STOPPED"
            } else {
                "LEASE_OR_DEVICE_LOST"
            };
            emit(&mut gate, reason, &counts, input_id, output_id)?;
            return Ok(());
        }
        match rx.recv_timeout(Duration::from_millis(10)) {
            Ok(Ok(x)) => {
                idle = Instant::now();
                if x.protocol_major != 1 || x.authorization != *token || x.instance_id != instance {
                    return Err(());
                }
                if let Err(code) = gate.command(&x, now()) {
                    stop.store(true, Ordering::Release);
                    deadline.store(0, Ordering::Release);
                    route.take();
                    gate.stop();
                    emit(&mut gate, code, &counts, input_id, output_id)?;
                    return Ok(());
                }
                match x.action {
                    1 => {
                        let i = inputs
                            .iter()
                            .find(|(_, v)| v.device_id == x.input_id)
                            .ok_or(())?;
                        let o = outputs
                            .iter()
                            .find(|(_, v)| v.device_id == x.output_id)
                            .ok_or(())?;
                        stop.store(false, Ordering::Release);
                        counts = Arc::new(Counts::default());
                        match microphone::open(
                            i.0.clone(),
                            o.0.clone(),
                            base,
                            stop.clone(),
                            deadline.clone(),
                            counts.clone(),
                        ) {
                            Ok(r) => {
                                route = Some(r);
                                if window.is_none() {
                                    window = Some(Meter::open()?);
                                }
                                input_id = x.input_id;
                                output_id = x.output_id;
                                reason = "PREPARED"
                            }
                            Err(_) => {
                                gate.stop();
                                reason = "DEVICE_UNAVAILABLE"
                            }
                        }
                    }
                    4 | 2 => {
                        deadline.store(gate.deadline, Ordering::Release);
                        reason = "ACTIVE"
                    }
                    3 => {
                        deadline.store(0, Ordering::Release);
                        gate.stop();
                        // The callback is gated before acknowledging software mute.
                        // WASAPI stream destruction may block; it is not hardware completion.
                        reason = "STOPPED";
                        if let Some(w) = window.as_ref() {
                            w.active.store(false, Ordering::Release);
                        }
                        emit(&mut gate, reason, &counts, input_id, output_id)?;
                        route.take();
                        stop.store(false, Ordering::Release);
                        continue;
                    }
                    _ => (),
                }
                emit(&mut gate, reason, &counts, input_id, output_id)?;
            }
            Ok(Err(_)) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            Err(mpsc::RecvTimeoutError::Timeout) => (),
        }
    }
}
