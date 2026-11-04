#![cfg_attr(windows, windows_subsystem = "windows")]
use nai_contracts::{audition_wire as wire, workbench::*};
#[cfg(windows)]
use nai_runtime_core::audition::LeaseGate;
use sha2::{Digest, Sha256};
#[cfg(windows)]
use std::sync::mpsc;
use std::{
    io::Read,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
#[cfg(windows)]
mod device;
fn stop_nonce(path: &Path) -> Option<String> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(String::new()),
        Ok(m) if m.is_file() && !m.file_type().is_symlink() && m.len() <= 64 => {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if m.file_attributes() & 0x400 != 0 {
                    return None;
                }
            }
            std::fs::read_to_string(path).ok()
        }
        _ => None,
    }
}
fn emit(state: AuditionState, reason: &str, count: u64, challenge: u64) -> Result<(), ()> {
    wire::write(
        &mut std::io::stdout().lock(),
        &AuditionStatus {
            view: Some(AuditionView {
                state: state as i32,
                reason: reason.into(),
                submitted_samples: count,
                ..Default::default()
            }),
            challenge,
        },
    )
    .map_err(|_| ())
}
#[cfg(windows)]
fn emit_current(
    view: &AuditionView,
    state: AuditionState,
    reason: &str,
    count: u64,
    challenge: u64,
) -> Result<(), ()> {
    let mut v = view.clone();
    v.state = state as i32;
    v.reason = reason.into();
    v.submitted_samples = count;
    wire::write(
        &mut std::io::stdout().lock(),
        &AuditionStatus {
            view: Some(v),
            challenge,
        },
    )
    .map_err(|_| ())
}
fn run() -> Result<(), ()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 || args[0] != "--media" || args[2] != "--stop" {
        return Err(());
    }
    let base = Instant::now();
    let alive = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));
    let deadline = Arc::new(AtomicU64::new(0));
    let progress = Arc::new(AtomicU64::new(0));
    let a = alive.clone();
    let flag = stop.clone();
    std::thread::spawn(move || {
        loop {
            if base.elapsed() > Duration::from_secs(90)
                || !a.load(Ordering::Acquire) && base.elapsed() > Duration::from_secs(2)
            {
                flag.store(true, Ordering::Release);
                std::process::exit(2);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });
    let first: AuditionControl = wire::read(&mut std::io::stdin().lock()).map_err(|_| ())?;
    let Some(audition_control::Operation::Start(start)) = first.operation else {
        return Err(());
    };
    if start.sample_rate != 24000
        || start.sample_count == 0
        || start.sample_count > 480000
        || start.stop_nonce.len() > 64
        || start.output_session_id.len() != 64
        || !start
            .output_session_id
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
        || !Path::new(&args[1]).is_absolute()
        || !Path::new(&args[3]).is_absolute()
    {
        return Err(());
    }
    let meta = std::fs::symlink_metadata(&args[1]).map_err(|_| ())?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() != start.sample_count * 2 {
        return Err(());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(());
        }
    }
    let mut bytes = zeroize::Zeroizing::new(Vec::new());
    std::fs::File::open(&args[1])
        .map_err(|_| ())?
        .take(960001)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    let sha: String = Sha256::digest(&*bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if sha != start.pcm_sha256 || bytes.len() as u64 != start.sample_count * 2 {
        return Err(());
    }
    let pcm = zeroize::Zeroizing::new(
        bytes
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect::<Vec<_>>(),
    );
    let stop_path = args[3].clone();
    let expected = start.stop_nonce;
    let flag = stop.clone();
    std::thread::spawn(move || {
        while !flag.load(Ordering::Acquire) {
            if stop_nonce(Path::new(&stop_path)).as_deref() != Some(&expected) {
                flag.store(true, Ordering::Release);
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });
    #[cfg(not(windows))]
    {
        let _ = (pcm, deadline, progress);
        emit(AuditionState::Unavailable, "WINDOWS_DEVICE_REQUIRED", 0, 0)?;
        Err(())
    }
    #[cfg(windows)]
    {
        let devices = device::snapshot().map_err(|_| {
            let _ = emit(
                AuditionState::Unavailable,
                "DEVICE_DISCOVERY_UNAVAILABLE",
                0,
                0,
            );
        })?;
        let mut view = AuditionView {
            output_session_id: start.output_session_id.clone(),
            devices: devices.iter().map(|(_, v)| v.clone()).collect(),
            ..Default::default()
        };
        let mut devices: Vec<_> = devices.into_iter().map(|(d, _)| Some(d)).collect();
        let mut pcm = Some(pcm);
        let mut stream = None;
        alive.store(true, Ordering::Release);
        let (tx, rx) = mpsc::sync_channel(2);
        let flag = stop.clone();
        std::thread::spawn(move || {
            loop {
                let r = wire::read::<AuditionControl>(&mut std::io::stdin().lock());
                match r {
                    Ok(v) => {
                        if tx.try_send(v).is_err() {
                            flag.store(true, Ordering::Release);
                            break;
                        }
                    }
                    Err(_) => {
                        flag.store(true, Ordering::Release);
                        break;
                    }
                }
            }
        });
        let mut lease = LeaseGate::default();
        let mut next = 0u64;
        let mut submitted = None;
        loop {
            let mut now = base.elapsed().as_millis() as u64;
            if stop.load(Ordering::Acquire) {
                emit_current(
                    &view,
                    AuditionState::Stopped,
                    "STOPPED",
                    progress.load(Ordering::Acquire),
                    0,
                )?;
                return Ok(());
            }
            if lease.started() && !lease.valid(now) {
                stop.store(true, Ordering::Release);
                emit_current(
                    &view,
                    AuditionState::Stopped,
                    "LEASE_EXPIRED",
                    progress.load(Ordering::Acquire),
                    0,
                )?;
                return Ok(());
            }
            if let Ok(c) = rx.try_recv() {
                match c.operation {
                    Some(audition_control::Operation::Lease(l)) => {
                        if lease.accept(l.challenge, l.epoch, l.ttl_ms, now) {
                            deadline.store(lease.deadline(), Ordering::Release);
                        }
                    }
                    Some(audition_control::Operation::SelectDevice(s)) => {
                        if stream.is_some()
                            || s.output_session_id != start.output_session_id
                            || s.device_id == 0
                        {
                            return Err(());
                        }
                        let i = (s.device_id - 1) as usize;
                        let d = devices.get_mut(i).and_then(Option::take).ok_or(())?;
                        view.selected_device_name = view.devices.get(i).ok_or(())?.name.clone();
                        deadline.store(0, Ordering::Release);
                        if !lease.initialization_barrier() {
                            return Err(());
                        }
                        stream = Some(
                            device::open(
                                d,
                                pcm.take().ok_or(())?,
                                base,
                                stop.clone(),
                                deadline.clone(),
                                progress.clone(),
                            )
                            .map_err(|_| {
                                let _ = emit_current(
                                    &view,
                                    AuditionState::Unavailable,
                                    "SELECTED_DEVICE_UNAVAILABLE",
                                    0,
                                    0,
                                );
                            })?,
                        );
                        if !lease.initialization_barrier() {
                            return Err(());
                        }
                        now = base.elapsed().as_millis() as u64;
                        next = 0;
                    }
                    Some(audition_control::Operation::Stop(_)) => {
                        lease.stop();
                        stop.store(true, Ordering::Release);
                    }
                    _ => return Err(()),
                }
            }
            let count = progress.load(Ordering::Acquire);
            if stream.is_none() && now >= 60000 {
                emit_current(&view, AuditionState::Stopped, "SELECTION_EXPIRED", 0, 0)?;
                return Ok(());
            }
            if count == start.sample_count {
                let at = submitted.get_or_insert(now);
                if now - *at >= 200 {
                    emit_current(
                        &view,
                        AuditionState::Submitted,
                        "SUBMITTED_NOT_DEVICE_ACK",
                        count,
                        0,
                    )?;
                    return Ok(());
                }
            }
            if now >= next {
                let seq = lease.challenge(now).ok_or(())?;
                emit_current(
                    &view,
                    if stream.is_none() {
                        AuditionState::Choosing
                    } else if lease.valid(now) {
                        AuditionState::Playing
                    } else {
                        AuditionState::Ready
                    },
                    "",
                    count,
                    seq,
                )?;
                next = now + 150;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
fn main() {
    if run().is_err() {
        let _ = emit(AuditionState::Unavailable, "WORKER_UNAVAILABLE", 0, 0);
        std::process::exit(2);
    }
}
