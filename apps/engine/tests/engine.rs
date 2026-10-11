//! Real-subprocess engine tests: loopback control on the shared session, keeper
//! supervision with a hard kill, the explicit media drill and the graceful halt.
//! These are Linux engineering drills, not device or Windows certification.
//!
//! The supervision drills are Linux-only on purpose. They find the supervised
//! keeper by reading `/proc` and end it with `kill`, so they need the Unix
//! process model. The media path needs a second, honest limit on the Windows
//! host used for this round: `nai-runtime`'s supervisor hands the keeper a pipe
//! as its stdin, and this host refuses that handoff, so no keeper can be
//! supervised there at all. Both facts are recorded in
//! docs/reports/2026-10-10-engineering-loop-platform-truths.md rather than being
//! asserted through a drill that cannot reach the behaviour it names.
mod common;

use common::*;
use nai_storage::DecisionKind;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tempfile::tempdir;

/// Minimal HTTP/1.1 client: no extra dependency, no redirects, no retries.
fn request(
    port: u16,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<&str>,
) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("control port reachable");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut payload =
        format!("{method} {path} HTTP/1.1\r\nhost: 127.0.0.1\r\nconnection: close\r\n");
    if let Some(token) = token {
        payload.push_str(&format!("authorization: Bearer {token}\r\n"));
    }
    if body.is_some() {
        payload.push_str("content-type: application/json\r\n");
    }
    payload.push_str(&format!(
        "content-length: {}\r\n\r\n",
        body.map(str::len).unwrap_or(0)
    ));
    if let Some(body) = body {
        payload.push_str(body);
    }
    stream.write_all(payload.as_bytes()).unwrap();
    let mut raw = String::new();
    stream.read_to_string(&mut raw).unwrap();
    let status = raw
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .expect("status line");
    let body = raw
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or_default();
    (status, body)
}

/// The route decides the action; the body carries only the validated fields.
fn session_command(epoch: u64, revision: u64, seq: u64, id: &str) -> String {
    format!(
        "{{\"session_id\":\"{WORKSPACE}\",\"device_id\":\"local-operator\",\"command_id\":\"{id}\",\
         \"generation_epoch\":\"{epoch}\",\"expected_state_revision\":\"{revision}\",\"seq\":\"{seq}\",\
         \"ttl_ms\":5000}}"
    )
}

struct Engine {
    child: Child,
    port: u16,
    halt: std::path::PathBuf,
}

impl Engine {
    fn start(root: &Path, extra: &[String]) -> Self {
        let token_path = root.join("token");
        write_token(&token_path);
        let halt = root.join("halt");
        let mut args = vec![
            "--data-root".into(),
            root.display().to_string(),
            "--workspace".into(),
            WORKSPACE.into(),
            "--actor".into(),
            ACTOR.into(),
            "--profile".into(),
            PROFILE.into(),
            "--token-file".into(),
            token_path.display().to_string(),
            "--halt-file".into(),
            halt.display().to_string(),
            "--control-port".into(),
            "0".into(),
        ];
        args.extend(extra.iter().cloned());
        let mut child = Command::new(env!("CARGO_BIN_EXE_nai-engine"))
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("engine binary spawns");
        let stdout = child.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout);
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut port = None;
        while Instant::now() < deadline {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            if let Some(rest) = line.split("http://127.0.0.1:").nth(1)
                && let Some(text) = rest.split('/').next()
                && let Ok(parsed) = text.parse()
            {
                port = Some(parsed);
                break;
            }
        }
        let port = port.expect("engine printed its control address");
        // Keep draining stdout so the child never blocks on a full pipe.
        thread::spawn(move || {
            let mut sink = String::new();
            let _ = reader.read_to_string(&mut sink);
        });
        Self { child, port, halt }
    }

    fn halt_and_wait(&mut self) -> std::process::ExitStatus {
        std::fs::write(&self.halt, b"halt").unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            if Instant::now() >= deadline {
                let _ = self.child.kill();
                panic!("engine did not halt in time");
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn control_surface_rejects_missing_token_and_revokes_the_shared_session() {
    let root = tempdir().unwrap();
    // Real provisioning through the engine's own trusted-owner path.
    provision_with_engine(root.path());
    let mut engine = Engine::start(root.path(), &[]);

    let (status, body) = request(engine.port, "GET", "/api/v1/runtime/status", None, None);
    assert_eq!(status, 401, "unauthenticated status must be rejected");
    assert!(body.contains("UNAUTHENTICATED"));

    let (status, body) = request(
        engine.port,
        "GET",
        "/api/v1/runtime/status",
        Some(TOKEN),
        None,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"generation_epoch\":\"0\""), "{body}");
    assert!(body.contains("\"lifecycle\":\"created\""), "{body}");

    let (status, body) = request(
        engine.port,
        "POST",
        &format!("/api/v1/sessions/{WORKSPACE}/mute"),
        Some(TOKEN),
        Some(&session_command(0, 0, 1, "mute-1")),
    );
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("\"executed\":true"), "{body}");

    let (_, body) = request(
        engine.port,
        "GET",
        "/api/v1/runtime/status",
        Some(TOKEN),
        None,
    );
    assert!(
        body.contains("\"generation_epoch\":\"1\""),
        "the shared session must show the revoke: {body}"
    );

    assert!(engine.halt_and_wait().success(), "engine must halt cleanly");
    let store = open_store(root.path());
    store
        .verify_decision_chain(&store_operation(), WORKSPACE)
        .expect("decision chain stays intact");
    let entries = store
        .decision_entries(&store_operation(), WORKSPACE, 50)
        .unwrap();
    assert!(
        entries
            .iter()
            .any(|entry| entry.kind == DecisionKind::Cancellation.as_str()
                && entry.payload_json.contains("\"action\":\"mute\"")),
        "the HTTP revoke must be logged: {entries:?}"
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.payload_json.contains("\"action\":\"mute\"")
                && entry.kind == DecisionKind::Cancellation.as_str()),
        "the halt must be logged as a critical cancellation"
    );
}

/// The keeper is found through `/proc` and ended through the Unix signal API, so
/// this drill runs where that process model exists.
#[cfg(target_os = "linux")]
#[test]
fn killed_keeper_is_halted_by_the_engine_and_the_chain_survives() {
    let root = tempdir().unwrap();
    provision(
        root.path(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        &runtime_profile(WORKSPACE),
    );
    let keeper = binary("nai-output-keeper");
    let render_worker = binary("nai-render-worker");
    let mut engine = Engine::start(
        root.path(),
        &[
            "--keeper".into(),
            keeper.display().to_string(),
            "--render-worker".into(),
            render_worker.display().to_string(),
        ],
    );
    let keeper_pid = wait_for_keeper_pid(engine.child.id(), Duration::from_secs(15))
        .expect("keeper is supervised");
    let (status, body) = request(
        engine.port,
        "GET",
        "/api/v1/runtime/status",
        Some(TOKEN),
        None,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"generation_epoch\":\"0\""), "{body}");

    // Hard kill: the engine must mute, revoke and stand by, not resume output.
    let killed = Command::new("kill")
        .arg("-9")
        .arg(keeper_pid.to_string())
        .status()
        .expect("kill runs");
    assert!(killed.success());
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut revoked = false;
    while Instant::now() < deadline {
        let (_, body) = request(
            engine.port,
            "GET",
            "/api/v1/runtime/status",
            Some(TOKEN),
            None,
        );
        if body.contains("\"generation_epoch\":\"1\"") {
            revoked = true;
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert!(revoked, "a killed keeper must revoke output authority");

    // The control surface stays reachable after the media halt.
    let (status, _) = request(
        engine.port,
        "GET",
        "/api/v1/runtime/status",
        Some(TOKEN),
        None,
    );
    assert_eq!(status, 200);

    assert!(engine.halt_and_wait().success());
    let store = open_store(root.path());
    store
        .verify_decision_chain(&store_operation(), WORKSPACE)
        .expect("chain survives the kill");
    let entries = store
        .decision_entries(&store_operation(), WORKSPACE, 200)
        .unwrap();
    assert!(
        entries
            .iter()
            .any(|entry| entry.kind == DecisionKind::Cancellation.as_str()
                && entry.payload_json.contains("keeper")),
        "the fault must be logged as a critical cancellation: {entries:?}"
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.payload_json.contains("\"event\":\"armed\"")),
        "the lease arm must be recorded: {entries:?}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn media_drill_produces_a_committed_frame_through_the_real_worker() {
    let root = tempdir().unwrap();
    provision(
        root.path(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        &runtime_profile(WORKSPACE),
    );
    let keeper = binary("nai-output-keeper");
    let render_worker = binary("nai-render-worker");
    let avatar = write_avatar(root.path());
    let pcm = write_pcm(root.path(), 320);
    let mut extra = vec![
        "--keeper".to_string(),
        keeper.display().to_string(),
        "--render-worker".to_string(),
        render_worker.display().to_string(),
        "--media-drill".to_string(),
        "--pcm".to_string(),
        pcm.display().to_string(),
        "--avatar".to_string(),
        avatar.manifest.display().to_string(),
        "--avatar-config".to_string(),
        avatar.config.display().to_string(),
        "--slot-bytes".to_string(),
        "8".to_string(),
    ];
    for (id, path) in &avatar.assets {
        extra.push("--asset".to_string());
        extra.push(format!("{id}={}", path.display()));
    }
    extra.push("--license".to_string());
    extra.push("test-license".to_string());
    let mut engine = Engine::start(root.path(), &extra);

    // The drill runs after arming; the halt waits for it to finish.
    thread::sleep(Duration::from_secs(3));
    assert!(engine.halt_and_wait().success());
    let store = open_store(root.path());
    store
        .verify_decision_chain(&store_operation(), WORKSPACE)
        .unwrap();
    let entries = store
        .decision_entries(&store_operation(), WORKSPACE, 200)
        .unwrap();
    let committed = entries
        .iter()
        .find(|entry| entry.payload_json.contains("media_drill_committed"))
        .unwrap_or_else(|| panic!("the media drill must commit a frame: {entries:?}"));
    assert!(
        committed.payload_json.contains("\"bytes\":8"),
        "{}",
        committed.payload_json
    );
    assert!(
        committed
            .payload_json
            .contains("\"buffer_ref\":\"nai-frame-"),
        "{}",
        committed.payload_json
    );
    assert!(
        committed.payload_json.contains(":0:1\""),
        "the commit must reference slot 0 generation 1: {}",
        committed.payload_json
    );
}

#[cfg(target_os = "linux")]
#[test]
fn halt_during_media_drill_prewarm_never_commits_a_frame() {
    let root = tempdir().unwrap();
    provision(
        root.path(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        &runtime_profile(WORKSPACE),
    );
    let keeper = binary("nai-output-keeper");
    let render_worker = binary("nai-render-worker");
    let avatar = write_avatar(root.path());
    // Park the render-worker in prewarm: its first declared asset is a named
    // pipe with no writer, so the prewarm read blocks. Without this the drill
    // could reach its commit before the halt is observed, and the assertion
    // below would be measuring scheduler timing rather than the halt. The same
    // fixture without the pipe commits a frame in the drill test above, so this
    // removes the race, not the ability to commit.
    park_asset_on_fifo(&avatar.assets[0].1);
    let pcm = write_pcm(root.path(), 320);
    let mut extra = vec![
        "--keeper".to_string(),
        keeper.display().to_string(),
        "--render-worker".to_string(),
        render_worker.display().to_string(),
        "--media-drill".to_string(),
        "--pcm".to_string(),
        pcm.display().to_string(),
        "--avatar".to_string(),
        avatar.manifest.display().to_string(),
        "--avatar-config".to_string(),
        avatar.config.display().to_string(),
        "--slot-bytes".to_string(),
        "8".to_string(),
    ];
    for (id, path) in &avatar.assets {
        extra.push("--asset".to_string());
        extra.push(format!("{id}={}", path.display()));
    }
    extra.push("--license".to_string());
    extra.push("test-license".to_string());
    let mut engine = Engine::start(root.path(), &extra);

    // Halt only once the drill has actually spawned its render-worker. From
    // that point the worker is inside prewarm and parked on the pipe, so the
    // halt is issued while the worker is prewarming and never before it exists
    // — otherwise the drill would never reach the stage this test names. The
    // bounded wait must stop without blocking and must never log a commit.
    wait_for_worker_pid(engine.child.id(), Duration::from_secs(15))
        .expect("the drill spawns its render-worker");
    assert!(
        engine.halt_and_wait().success(),
        "halt must be bounded and clean"
    );
    let store = open_store(root.path());
    store
        .verify_decision_chain(&store_operation(), WORKSPACE)
        .expect("chain survives a prewarm halt");
    let entries = store
        .decision_entries(&store_operation(), WORKSPACE, 200)
        .unwrap();
    assert!(
        !entries
            .iter()
            .any(|entry| entry.payload_json.contains("media_drill_committed")),
        "a halt before the worker commits must not log a committed frame: {entries:?}"
    );
}

/// Waits for the supervised keeper, which needs the Unix process model: the pid
/// is discovered by scanning `/proc`, as the drill above does.
#[cfg(target_os = "linux")]
#[test]
fn http_revoke_stops_lease_renewal_without_regranting_output() {
    let root = tempdir().unwrap();
    provision(
        root.path(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        &runtime_profile(WORKSPACE),
    );
    let keeper = binary("nai-output-keeper");
    let render_worker = binary("nai-render-worker");
    let mut engine = Engine::start(
        root.path(),
        &[
            "--keeper".into(),
            keeper.display().to_string(),
            "--render-worker".into(),
            render_worker.display().to_string(),
        ],
    );
    wait_for_keeper_pid(engine.child.id(), Duration::from_secs(15)).expect("keeper is supervised");

    let (status, body) = request(
        engine.port,
        "POST",
        &format!("/api/v1/sessions/{WORKSPACE}/mute"),
        Some(TOKEN),
        Some(&session_command(0, 0, 1, "mute-1")),
    );
    assert_eq!(status, 200, "{body}");

    // The engine must stop renewing the stale epoch instead of re-granting it.
    // Give the supervision loop several renew intervals to notice the revoke.
    thread::sleep(Duration::from_millis(800));
    assert!(
        engine.child.try_wait().unwrap().is_none(),
        "the control service stays up after a media revoke"
    );
    assert!(engine.halt_and_wait().success());
    let store = open_store(root.path());
    let entries = store
        .decision_entries(&store_operation(), WORKSPACE, 200)
        .unwrap();
    assert!(
        entries
            .iter()
            .any(|entry| entry.payload_json.contains("renewal_halted_after_revoke")),
        "a control revoke must stop renewal: {entries:?}"
    );
    assert!(
        !entries
            .iter()
            .any(|entry| entry.payload_json.contains("renewal_failed")),
        "the engine must not keep renewing a revoked lease: {entries:?}"
    );
}

#[test]
fn incomplete_or_unsafe_invocation_is_rejected_without_listening() {
    let root = tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_nai-engine"))
        .args(["--data-root", &root.path().display().to_string()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--workspace"),
        "usage must name the missing inputs"
    );

    // A world-readable token file is refused before any service starts. The rule
    // is a Unix mode check and only exists there: Windows ACLs are not inspected
    // yet. So the invocation is not run off Unix — it would measure some later,
    // unrelated rejection and record that as a permission decision — and the gap
    // is pinned by its own assertion below instead.
    let token_path = root.path().join("token");
    write_token(&token_path);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&token_path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_nai-engine"))
            .args([
                "--data-root",
                &root.path().display().to_string(),
                "--workspace",
                WORKSPACE,
                "--actor",
                ACTOR,
                "--profile",
                PROFILE,
                "--token-file",
                &token_path.display().to_string(),
            ])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("0600"),
            "the token file permission must be named"
        );
    }
    #[cfg(not(unix))]
    {
        // Characterisation of the documented gap, so it cannot change silently:
        // a credential readable beyond its owner is accepted because no ACL check
        // exists yet. Replace this with a refusal when Windows gains the check.
        assert_eq!(
            nai_engine::read_token(&token_path).expect("token is read"),
            TOKEN,
            "the owner-only token rule is still Unix-only"
        );
    }
}

/// Provisions through the engine binary itself: workspace, permissions and the
/// runtime profile. This is the trusted-owner path, not a model path.
fn provision_with_engine(root: &Path) {
    let profile_path = root.join("runtime-profile.json");
    std::fs::write(&profile_path, runtime_profile(WORKSPACE).to_string()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_nai-engine"))
        .args([
            "--data-root",
            &root.display().to_string(),
            "--workspace",
            WORKSPACE,
            "--actor",
            ACTOR,
            "--profile",
            PROFILE,
            "--provision",
            &profile_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "provisioning failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("provisioned"));
}

/// Finds this engine's supervised keeper by scanning /proc and matching the
/// parent pid. Linux-only evidence gathering.
#[cfg(target_os = "linux")]
fn wait_for_keeper_pid(parent: u32, timeout: Duration) -> Option<u32> {
    wait_for_supervised_pid(parent, timeout, "nai-output-keeper", "--watchdog-drill")
}

/// Finds this engine's supervised render-worker, which is spawned by the drill
/// and parked in prewarm. Linux-only, like the helper above.
#[cfg(target_os = "linux")]
fn wait_for_worker_pid(parent: u32, timeout: Duration) -> Option<u32> {
    wait_for_supervised_pid(parent, timeout, "nai-render-worker", "--render-drill")
}

/// Finds a supervised child of `parent` whose command line names `exe` and
/// carries `marker`. A /proc scan: an entry may vanish between read_dir and the
/// status read, so a missing file skips the entry rather than failing.
#[cfg(target_os = "linux")]
fn wait_for_supervised_pid(parent: u32, timeout: Duration, exe: &str, marker: &str) -> Option<u32> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(entries) = std::fs::read_dir("/proc") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let Some(pid) = name.to_str().and_then(|text| text.parse::<u32>().ok()) else {
                    continue;
                };
                let Ok(status) = std::fs::read_to_string(entry.path().join("status")) else {
                    continue;
                };
                let child = status
                    .lines()
                    .find_map(|line| line.strip_prefix("PPid:"))
                    .and_then(|value| value.trim().parse::<u32>().ok());
                if child != Some(parent) {
                    continue;
                }
                let Ok(cmdline) = std::fs::read(entry.path().join("cmdline")) else {
                    continue;
                };
                let text = String::from_utf8_lossy(&cmdline);
                if text.contains(exe) && text.contains(marker) {
                    return Some(pid);
                }
            }
        }
        thread::sleep(Duration::from_millis(25));
    }
    None
}
