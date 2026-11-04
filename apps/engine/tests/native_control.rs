//! Private inherited control, exercised against the real engine process.
//! These are engineering drills: no device output and no Windows certification.
#![cfg(target_os = "linux")]
mod common;

use common::{ACTOR, PROFILE, WORKSPACE, open_store, provision, runtime_profile, store_operation};
use nai_contracts::v1::{LocalControlFrame as Frame, LocalControlKind as Kind};
use nai_control_ipc::{
    CALL_TIMEOUT, ControlProcess, FRAME_LIMIT, frame, read_frame, validate, write_frame,
};
use nai_storage::DecisionKind;
use prost::Message;
use serde_json::{Value, json};
use std::{
    io::Write as _,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tempfile::tempdir;

fn engine_command(root: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nai-engine"));
    command
        .arg("--control-stdio")
        .arg("--data-root")
        .arg(root)
        .arg("--workspace")
        .arg(WORKSPACE)
        .arg("--actor")
        .arg(ACTOR)
        .arg("--profile")
        .arg(PROFILE);
    command
}

fn request(id: &str, method: &str, path: &str, body: Option<Value>) -> Frame {
    let mut value = frame(Kind::LocalControlRequest);
    value.request_id = id.into();
    value.method = method.into();
    value.path = path.into();
    value.ttl_ms = CALL_TIMEOUT.as_millis() as u32;
    if let Some(body) = body {
        value.body = serde_json::to_vec(&body).unwrap();
    }
    value
}

fn body_of(response: &Frame) -> Value {
    serde_json::from_slice(&response.body).unwrap()
}

#[test]
fn native_status_and_mute_share_the_http_judgement_and_the_chain() {
    let root = tempdir().unwrap();
    provision(
        root.path(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        &runtime_profile(WORKSPACE),
    );
    let process = ControlProcess::spawn(&mut engine_command(root.path())).unwrap();

    let status = process
        .request(request("s1", "GET", "/api/v1/runtime/status", None))
        .unwrap();
    assert_eq!(status.status, 200);
    let status = body_of(&status);
    assert_eq!(status["session"]["generation_epoch"], "0");
    assert_eq!(status["session"]["lifecycle"], "created");

    // The transport cannot name the device or the route action: the same route
    // the HTTP adaptation uses decides it. The body still carries seq/identity.
    let mute = request(
        "m1",
        "POST",
        &format!("/api/v1/sessions/{WORKSPACE}/mute"),
        Some(json!({
            "session_id": WORKSPACE, "device_id": "local-operator", "command_id": "m1",
            "generation_epoch": "0", "expected_state_revision": "0", "seq": "1", "ttl_ms": 5000
        })),
    );
    let ack = process.request(mute).unwrap();
    assert_eq!(ack.status, 200, "{}", String::from_utf8_lossy(&ack.body));
    assert_eq!(body_of(&ack)["executed"], true);

    let status = process
        .request(request("s2", "GET", "/api/v1/runtime/status", None))
        .unwrap();
    assert_eq!(body_of(&status)["session"]["generation_epoch"], "1");

    process.shutdown();
    let store = open_store(root.path());
    store
        .verify_decision_chain(&store_operation(), WORKSPACE)
        .unwrap();
    let entries = store
        .decision_entries(&store_operation(), WORKSPACE, 50)
        .unwrap();
    assert!(
        entries
            .iter()
            .any(|entry| entry.kind == DecisionKind::Cancellation.as_str()
                && entry.payload_json.contains("\"action\":\"mute\"")),
        "the native mute must enter the same decision log: {entries:?}"
    );
}

#[test]
fn heartbeats_keep_the_lease_alive_without_any_request() {
    let root = tempdir().unwrap();
    provision(
        root.path(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        &runtime_profile(WORKSPACE),
    );
    let process = ControlProcess::spawn(&mut engine_command(root.path())).unwrap();
    // Longer than the engine's 5s peer lease: only the transport heartbeats keep
    // this connection open.
    thread::sleep(Duration::from_secs(6));
    let status = process
        .request(request("s1", "GET", "/api/v1/runtime/status", None))
        .unwrap();
    assert_eq!(status.status, 200);
    process.shutdown();
}

/// A partial length prefix is not EOF: the engine must stop safely, not wait.
#[test]
fn a_partial_frame_prefix_stops_the_engine() {
    let root = tempdir().unwrap();
    provision(
        root.path(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        &runtime_profile(WORKSPACE),
    );
    let mut child = engine_command(root.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = child.stdout.take().unwrap();
    let mut bootstrap = frame(Kind::LocalControlBootstrap);
    bootstrap.bootstrap_token = "a".repeat(64);
    write_frame(&mut input, &bootstrap).unwrap();
    assert_eq!(
        validate(&read_frame(&mut output).unwrap().unwrap()).unwrap(),
        Kind::LocalControlReady
    );
    input.write_all(&[0, 0]).unwrap();
    input.flush().unwrap();
    drop(input);

    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "engine ignored the truncated prefix"
        );
        thread::sleep(Duration::from_millis(20));
    };
    assert!(
        status.success(),
        "the safe halt path exits cleanly: {status}"
    );
    let mut stderr = String::new();
    use std::io::Read as _;
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(stderr.contains("IPC_TRUNCATED_PREFIX"), "{stderr}");

    let store = open_store(root.path());
    store
        .verify_decision_chain(&store_operation(), WORKSPACE)
        .unwrap();
    let entries = store
        .decision_entries(&store_operation(), WORKSPACE, 50)
        .unwrap();
    assert!(
        entries
            .iter()
            .any(|entry| entry.kind == DecisionKind::Cancellation.as_str()
                && entry.payload_json.contains("\"action\":\"mute\"")),
        "losing the control peer must P0-mute and log: {entries:?}"
    );
}

/// Keeping the handle open but silent is a lease violation, not a clean EOF.
#[test]
fn a_silent_peer_hits_the_lease_and_is_muted() {
    let root = tempdir().unwrap();
    provision(
        root.path(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        &runtime_profile(WORKSPACE),
    );
    let mut child = engine_command(root.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = child.stdout.take().unwrap();
    let mut bootstrap = frame(Kind::LocalControlBootstrap);
    bootstrap.bootstrap_token = "b".repeat(64);
    write_frame(&mut input, &bootstrap).unwrap();
    validate(&read_frame(&mut output).unwrap().unwrap()).unwrap();

    let deadline = Instant::now() + Duration::from_secs(12);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "engine waited past its peer lease"
        );
        thread::sleep(Duration::from_millis(20));
    };
    assert!(
        status.success(),
        "lease expiry follows the safe halt path: {status}"
    );
    let mut stderr = String::new();
    use std::io::Read as _;
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(stderr.contains("IPC_PEER_LEASE_EXPIRED"), "{stderr}");
    drop(input);

    let store = open_store(root.path());
    let entries = store
        .decision_entries(&store_operation(), WORKSPACE, 50)
        .unwrap();
    assert!(
        entries
            .iter()
            .any(|entry| entry.kind == DecisionKind::Cancellation.as_str()
                && entry.payload_json.contains("\"action\":\"mute\"")),
        "an expired peer is muted and logged: {entries:?}"
    );
}

/// The frame bound is checked before decoding; an oversized length never
/// allocates. This also pins that control frames cannot carry media payloads.
#[test]
fn a_frame_above_the_limit_is_refused_by_name() {
    let mut oversized = frame(Kind::LocalControlRequest);
    oversized.request_id = "x".into();
    oversized.method = "POST".into();
    oversized.path = "/api/v1/memory/read".into();
    oversized.ttl_ms = 1000;
    oversized.body = vec![7; FRAME_LIMIT];
    assert!(oversized.encoded_len() > FRAME_LIMIT);
    assert_eq!(
        write_frame(&mut Vec::new(), &oversized).unwrap_err(),
        "IPC_FRAME_LIMIT"
    );
    assert_eq!(
        read_frame(&mut u32::MAX.to_be_bytes().as_slice()).unwrap_err(),
        "IPC_FRAME_LIMIT"
    );
}
