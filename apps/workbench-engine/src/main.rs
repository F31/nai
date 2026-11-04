//! Single authenticated inspection over inherited handles; no network or business output.
use nai_contracts::{workbench::*, workbench_wire as wire};
use std::{
    io,
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};
fn main() {
    if let Err(code) = run() {
        eprintln!("{}", code.as_str_name());
        std::process::exit(1);
    }
}
fn run() -> Result<(), WorkbenchError> {
    let started = Instant::now();
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 || args[0] != "--data-root" {
        return Err(WorkbenchError::InvalidArgument);
    }
    nai_runtime_core::validate_data_root(&PathBuf::from(&args[1]))?;
    let token =
        std::env::var("NAI_WORKBENCH_TOKEN").map_err(|_| WorkbenchError::AuthenticationFailed)?;
    if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(WorkbenchError::AuthenticationFailed);
    }
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = wire::read::<WorkbenchRequest>(&mut io::stdin().lock());
        let _ = tx.send(result);
    });
    let request = rx
        .recv_timeout(Duration::from_millis(wire::TTL_MS as u64))
        .map_err(|_| WorkbenchError::DeadlineExceeded)?;
    let response = match request {
        Ok(request) => nai_runtime_core::handle(request, token.as_bytes(), started.elapsed()),
        Err(e) => nai_runtime_core::problem("", 0, e),
    };
    wire::write(&mut io::stdout().lock(), &response)
}
