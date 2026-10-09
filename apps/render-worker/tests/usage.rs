//! Real native invocation of the render-worker binary. Not a device, Windows or
//! hardware certification; missing explicit drill mode is rejected.
#![cfg(target_os = "linux")]
use std::process::{Command, Stdio};

fn exit_code(args: &[&str]) -> Option<i32> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_nai-render-worker"))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child.wait().unwrap().code()
}

#[test]
fn missing_or_incomplete_explicit_drill_mode_is_rejected() {
    assert_eq!(exit_code(&[]), Some(2));
    assert_eq!(exit_code(&["--render-drill"]), Some(2));
    assert_eq!(
        exit_code(&["--render-drill", "--pcm", "missing.bin"]),
        Some(2)
    );
}
