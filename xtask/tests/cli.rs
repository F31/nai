use std::process::Command;

#[test]
fn unimplemented_tasks_and_unknown_profiles_exit_nonzero() {
    for args in [
        vec!["schema", "--unknown"],
        vec!["certify", "--edition", "lite"],
        vec!["package"],
        vec!["test", "--suite", "integration"],
        vec!["check", "--profile", "unknown"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_nai-xtask"))
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("not implemented"));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("passed"));
    }
}

#[test]
#[cfg(not(target_os = "windows"))]
fn windows_profile_requires_native_environment() {
    let output = Command::new(env!("CARGO_BIN_EXE_nai-xtask"))
        .args(["check", "--profile", "windows-local"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("native Windows"));
}
