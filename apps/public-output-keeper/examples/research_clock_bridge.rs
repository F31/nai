#[cfg(windows)]
include!("support/research_clock_bridge_windows.rs");

#[cfg(not(windows))]
fn main() {
    eprintln!("research_clock_bridge is a Windows-only local audio device bridge");
}
