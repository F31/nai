//! One-shot diagnostic producer. Inherited stdin is a shared RAM FD; stdout is
//! bounded Protobuf metadata only. No production worker, GPU, Ready or ACK.
use nai_contracts::v1::FrameSlotTicket;
#[cfg(target_os = "linux")]
use nai_contracts::v1::{FrameSlotEvent, frame_slot_event::Event};
use nai_media_core::frame_slots::validate_ticket;
#[cfg(target_os = "linux")]
use nai_media_core::frame_slots::{
    IoBudget, MAX_BUDGET,
    shared_ram::{validate_producer_handle, write_frame},
};
use prost::Message;
#[cfg(target_os = "linux")]
use std::io::{self, Write};

#[cfg(target_os = "linux")]
fn publish(event: Event) -> io::Result<()> {
    let bytes = FrameSlotEvent { event: Some(event) }.encode_to_vec();
    if bytes.len() > 4096 {
        return Err(io::Error::other("drill notification too large"));
    }
    let mut out = io::stdout().lock();
    out.write_all(&(bytes.len() as u32).to_le_bytes())?;
    out.write_all(&bytes)?;
    out.flush()
}
fn ticket(hex: &str) -> Result<FrameSlotTicket, String> {
    if hex.is_empty()
        || hex.len() > 8192
        || !hex.len().is_multiple_of(2)
        || !hex.bytes().all(|v| v.is_ascii_hexdigit())
    {
        return Err("bounded protobuf ticket hex required".into());
    }
    let bytes: Result<Vec<_>, _> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
        .collect();
    FrameSlotTicket::decode(bytes.map_err(|_| "invalid ticket hex")?.as_slice())
        .map_err(|_| "invalid ticket protobuf".into())
}
/// Broker drill: the child connects to the broker UDS (which verifies the
/// kernel-declared pid) and then writes via the inherited pool FD. The ticket is
/// delivered out of band by root after authorization; the socket never carries
/// pixel data.
#[cfg(target_os = "linux")]
fn broker_drill(socket: &str, ticket_file: &str) -> Result<(), String> {
    use std::{
        fs::OpenOptions,
        os::unix::net::UnixStream,
        time::{Duration, Instant},
    };
    let path = std::path::Path::new(socket);
    let deadline = Instant::now() + Duration::from_secs(5);
    let stream = loop {
        match UnixStream::connect(path) {
            Ok(stream) => break stream,
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            Err(_) => return Err("broker socket unavailable".into()),
        }
    };
    // Root delivers the reserved ticket after granting this peer's identity.
    let ticket = loop {
        if let Ok(text) = std::fs::read_to_string(ticket_file)
            && let Ok(ticket) = ticket(text.trim())
        {
            break ticket;
        }
        if Instant::now() >= deadline {
            return Err("broker ticket delivery timeout".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let length = validate_ticket(&ticket).map_err(|e| e.message)? as usize;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/proc/self/fd/0")
        .map_err(|_| "inherited shared RAM FD unavailable")?;
    validate_producer_handle(&file, &ticket).map_err(|e| e.message)?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(length)
        .map_err(|_| "drill frame allocation failed")?;
    rgba.extend((0..length).map(|i| [10, 20, 30, 255, 40, 50, 60, 255][i % 8]));
    let budget = IoBudget::new(MAX_BUDGET).map_err(|e| e.message)?;
    publish(Event::WriteStarted(true)).map_err(|_| "drill status write failed")?;
    let result = write_frame(&file, &ticket, &rgba, &budget);
    drop(stream);
    match result {
        Ok(commit) => publish(Event::Committed(Box::new(commit)))
            .map_err(|_| "commit notification failed".into()),
        Err(error) => {
            let message = error.message.clone();
            let _ = publish(Event::Error(error));
            Err(message)
        }
    }
}
#[cfg(not(target_os = "linux"))]
fn broker_drill(socket: &str, hex: &str) -> Result<(), String> {
    let _ = (socket, hex);
    Err("CAPABILITY_NOT_READY: tmpfs-file-drill is Linux-only".into())
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--broker-drill") {
        if args.len() != 3 {
            return Err("explicit --broker-drill <socket-path> <ticket-file> required".into());
        }
        return broker_drill(&args[1], &args[2]);
    }
    if !(2..=3).contains(&args.len())
        || args[0] != "--shared-frame-drill"
        || args
            .get(2)
            .is_some_and(|arg| arg != "--stall-after-partial")
    {
        return Err(
            "explicit --shared-frame-drill <protobuf-ticket-hex> [--stall-after-partial] required"
                .into(),
        );
    }
    let ticket = ticket(&args[1])?;
    let length = validate_ticket(&ticket).map_err(|e| e.message)? as usize;
    #[cfg(target_os = "linux")]
    {
        use std::{fs::OpenOptions, os::unix::fs::FileExt};
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/proc/self/fd/0")
            .map_err(|_| "inherited shared RAM FD unavailable")?;
        let offset = validate_producer_handle(&file, &ticket).map_err(|e| e.message)?;
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(length)
            .map_err(|_| "drill frame allocation failed")?;
        rgba.extend((0..length).map(|i| [10, 20, 30, 255, 40, 50, 60, 255][i % 8]));
        let budget = IoBudget::new(MAX_BUDGET).map_err(|e| e.message)?;
        file.write_all_at(&rgba[..1], offset)
            .map_err(|_| "partial drill write failed")?;
        publish(Event::WriteStarted(true)).map_err(|_| "drill status write failed")?;
        if args.len() == 3 {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        match write_frame(&file, &ticket, &rgba, &budget) {
            Ok(commit) => publish(Event::Committed(Box::new(commit)))
                .map_err(|_| "commit notification failed".into()),
            Err(error) => {
                let message = error.message.clone();
                let _ = publish(Event::Error(error));
                Err(message)
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (ticket, length);
        Err("CAPABILITY_NOT_READY: tmpfs-file-drill is Linux-only".into())
    }
}
fn main() {
    if let Err(message) = run() {
        eprintln!("shared-frame-drill unavailable: {message}");
        std::process::exit(1);
    }
}
