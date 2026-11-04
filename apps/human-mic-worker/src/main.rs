#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(windows)]
mod worker;
fn main() {
    #[cfg(windows)]
    if worker::run().is_err() {
        std::process::exit(1)
    }
    #[cfg(not(windows))]
    std::process::exit(1);
}
