use nai_contracts::{workbench::*, workbench_wire as wire};
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use tempfile::tempdir;
fn request() -> WorkbenchRequest {
    WorkbenchRequest {
        protocol_major: wire::VERSION,
        authorization: vec![b'a'; 64],
        request_id: "integration-1".into(),
        seq: 1,
        ttl_ms: 3000,
        operation: Some(workbench_request::Operation::Inspect(WorkbenchInspect {
            config: Some(nai_runtime_core::config(Presentation::ContentOnly)),
        })),
    }
}
fn run(root: &Path, bytes: &[u8]) -> (bool, Vec<u8>, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_nai-workbench-engine"))
        .arg("--data-root")
        .arg(root)
        .env("NAI_WORKBENCH_TOKEN", "a".repeat(64))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    c.stdin.take().unwrap().write_all(bytes).unwrap();
    let deadline = Instant::now() + Duration::from_secs(6);
    while c.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            c.kill().unwrap();
            c.wait().unwrap();
            panic!("engine exceeded watchdog deadline");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let out = c.wait_with_output().unwrap();
    (
        out.status.success(),
        out.stdout,
        String::from_utf8(out.stderr).unwrap(),
    )
}
fn exchange(r: &WorkbenchRequest) -> WorkbenchResponse {
    let root = tempdir().unwrap();
    let mut bytes = vec![];
    wire::write(&mut bytes, r).unwrap();
    let (ok, bytes, err) = run(root.path(), &bytes);
    assert!(ok, "{err}");
    assert!(err.is_empty());
    wire::read(&mut bytes.as_slice()).unwrap()
}
#[test]
fn real_process_inspect_and_prepare_are_not_business_ready() {
    for prepare in [false, true] {
        let mut r = request();
        if prepare {
            r.operation = Some(workbench_request::Operation::Prepare(WorkbenchPrepare {
                config: Some(nai_runtime_core::config(Presentation::ContentOnly)),
            }));
        }
        let out = exchange(&r);
        assert_eq!(out.handshake.unwrap().protocol_major, 2);
        assert_eq!(out.request_id, r.request_id);
        let workbench_response::Outcome::Capabilities(c) = out.outcome.unwrap() else {
            panic!("capabilities")
        };
        assert!(!c.business_ready);
        assert_eq!(c.provider_calls, 0);
        assert_eq!(c.started_media_processes, 0);
        assert!(
            !c.required_capabilities.iter().any(|s| s.starts_with("tts.")
                || s.starts_with("avatar.")
                || s.starts_with("output."))
        );
        assert_eq!(
            c.status,
            if prepare {
                WorkbenchReadiness::NotReady
            } else {
                WorkbenchReadiness::ControlAvailable
            } as i32
        );
    }
}
#[test]
fn old_version_bad_token_unknown_config_and_expired_envelope_are_refused() {
    let good = request();
    let mut unknown = good.clone();
    unknown.operation = Some(workbench_request::Operation::Inspect(WorkbenchInspect {
        config: Some(WorkbenchConfig {
            presentation: 99,
            ..nai_runtime_core::config(Presentation::ContentOnly)
        }),
    }));
    for (r, code) in [
        (
            WorkbenchRequest {
                protocol_major: 1,
                ..good.clone()
            },
            WorkbenchError::ProtocolIncompatible,
        ),
        (
            WorkbenchRequest {
                authorization: vec![b'b'; 64],
                ..good.clone()
            },
            WorkbenchError::AuthenticationFailed,
        ),
        (
            WorkbenchRequest {
                ttl_ms: 4001,
                ..good
            },
            WorkbenchError::InvalidArgument,
        ),
        (unknown, WorkbenchError::InvalidArgument),
    ] {
        let out = exchange(&r);
        let workbench_response::Outcome::Problem(p) = out.outcome.unwrap() else {
            panic!("must reject")
        };
        assert_eq!(p.code, code as i32);
        assert!(!p.safe_message.contains(&"a".repeat(64)));
    }
}
#[test]
fn oversize_and_partial_frames_are_rejected() {
    let root = tempdir().unwrap();
    for input in [65537u32.to_le_bytes().to_vec(), vec![3, 0, 0, 0, 1]] {
        let (ok, bytes, err) = run(root.path(), &input);
        assert!(ok, "{err}");
        let out: WorkbenchResponse = wire::read(&mut bytes.as_slice()).unwrap();
        assert!(matches!(
            out.outcome,
            Some(workbench_response::Outcome::Problem(_))
        ));
    }
}
#[test]
fn occupied_legacy_root_is_rejected_without_modification() {
    let root = tempdir().unwrap();
    std::fs::write(root.path().join("state.sqlite3"), b"legacy-do-not-read").unwrap();
    let (ok, bytes, err) = run(root.path(), &[]);
    assert!(!ok);
    assert!(bytes.is_empty());
    assert!(err.contains("DATA_ROOT_REJECTED"));
    assert_eq!(
        std::fs::read(root.path().join("state.sqlite3")).unwrap(),
        b"legacy-do-not-read"
    );
}
#[test]
fn no_request_exits_at_bounded_deadline() {
    let root = tempdir().unwrap();
    let mut c = Command::new(env!("CARGO_BIN_EXE_nai-workbench-engine"))
        .arg("--data-root")
        .arg(root.path())
        .env("NAI_WORKBENCH_TOKEN", "a".repeat(64))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    while c.try_wait().unwrap().is_none() {
        if start.elapsed() > Duration::from_secs(6) {
            c.kill().unwrap();
            c.wait().unwrap();
            panic!("unbounded input read");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!c.wait().unwrap().success());
}
#[test]
fn relative_parent_and_symlink_roots_are_rejected() {
    assert!(nai_runtime_core::validate_data_root(Path::new("relative")).is_err());
    let root = tempdir().unwrap();
    assert!(nai_runtime_core::validate_data_root(&root.path().join("../bad")).is_err());
    #[cfg(unix)]
    {
        let other = tempdir().unwrap();
        std::os::unix::fs::symlink(other.path(), root.path().join("linked")).unwrap();
        assert!(nai_runtime_core::validate_data_root(&root.path().join("linked")).is_err());
    }
}
