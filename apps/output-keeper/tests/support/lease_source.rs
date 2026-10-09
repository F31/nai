//! Test-only native control source, compiled into a temporary directory by
//! faults.rs. Frames are opaque generated protobuf bytes, never hand DTOs.
use std::{
    io::{self, Write},
    time::Duration,
};

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 2);
    let bytes = std::fs::read(&args[0]).unwrap();
    assert!(bytes.len() <= 4 * 1024 * 1024);
    let mut output = io::stdout().lock();
    output.write_all(&bytes).unwrap();
    output.flush().unwrap();
    if args[1] == "partial" {
        std::thread::sleep(Duration::from_millis(100));
        output.write_all(&10u32.to_le_bytes()).unwrap();
        output.flush().unwrap();
        // Wait with an incomplete body. The keeper's own clock must still run.
    }
    std::thread::sleep(Duration::from_secs(60));
}
