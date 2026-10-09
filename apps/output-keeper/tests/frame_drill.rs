//! Real native subprocess drills only. Not engine, Windows or hardware
//! certification; the keeper still reports no device output.
#![cfg(target_os = "linux")]
use nai_contracts::v1::{
    ContractError, ErrorCode, FrameMetadata, FrameSlotCommit, FrameSlotEvent, FrameSlotTicket,
    OutputWatchdogCommand, OutputWatchdogStatus, PlaybackLease, SessionMode,
    frame_slot_event::Event, output_watchdog_command::Action,
};
use nai_media_core::frame_slots::{PoolSpec, shared_ram::SharedFramePool};
use nai_plugin_sdk::frame;
use prost::Message;
use std::{
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{atomic::AtomicBool, mpsc},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const SESSION: &str = "s";

fn spec() -> PoolSpec {
    PoolSpec {
        pool_id: "root-placeholder".into(),
        session_id: SESSION.into(),
        generation_epoch: 7,
        producer_ref: "worker-1".into(),
        consumer_ref: "keeper-1".into(),
        slots: 1,
        slot_bytes: 8,
    }
}
fn frame() -> FrameMetadata {
    FrameMetadata {
        session_id: SESSION.into(),
        utterance_id: "u".into(),
        generation_epoch: 7,
        width: 2,
        height: 1,
        pixel_format: "rgba8".into(),
        duration: 320,
        time_base_num: 1,
        time_base_den: 8000,
        ..Default::default()
    }
}
fn ram_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir_in("/dev/shm").expect("Linux frame drill requires /dev/shm tmpfs");
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    dir
}
fn utc_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
fn drill_binary() -> PathBuf {
    let keeper = PathBuf::from(env!("CARGO_BIN_EXE_nai-output-keeper"));
    let drill = keeper.parent().unwrap().join(format!(
        "nai-shared-frame-drill{}",
        std::env::consts::EXE_SUFFIX
    ));
    assert!(
        drill.exists(),
        "build the nai-media-core binaries first; missing {}",
        drill.display()
    );
    drill
}
fn read_event(stream: &mut impl Read) -> Event {
    let mut size = [0u8; 4];
    stream.read_exact(&mut size).unwrap();
    let length = u32::from_le_bytes(size) as usize;
    assert!(length > 0 && length <= 4096);
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes).unwrap();
    FrameSlotEvent::decode(bytes.as_slice())
        .unwrap()
        .event
        .unwrap()
}
/// Real media-core drill subprocess writes the frame through the inherited FD.
fn produce(pool: &SharedFramePool, ticket: &FrameSlotTicket) -> FrameSlotCommit {
    let hex: String = ticket
        .encode_to_vec()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let mut child = Command::new(drill_binary())
        .arg("--shared-frame-drill")
        .arg(hex)
        .stdin(Stdio::from(pool.producer_file("worker-1").unwrap()))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    assert_eq!(read_event(&mut stdout), Event::WriteStarted(true));
    let committed = read_event(&mut stdout);
    assert!(child.wait().unwrap().success());
    let Event::Committed(commit) = committed else {
        panic!("expected actual producer commit")
    };
    *commit
}
fn arm(seq: u64, epoch: u64, expires: u64) -> OutputWatchdogCommand {
    OutputWatchdogCommand {
        schema_version: 1,
        session_id: SESSION.into(),
        seq,
        remaining_budget_ms: 250,
        action: Some(Action::Arm(PlaybackLease {
            lease_id: "drill-lease".into(),
            session_id: SESSION.into(),
            generation_epoch: epoch,
            allowed_utterance_ids: vec!["u".into()],
            mode: SessionMode::Rehearsal as i32,
            policy_revision: "test-only".into(),
            expires_at_utc_ms: expires,
            ..Default::default()
        })),
    }
}
fn submit(seq: u64, event: Event) -> OutputWatchdogCommand {
    OutputWatchdogCommand {
        schema_version: 1,
        session_id: SESSION.into(),
        seq,
        remaining_budget_ms: 250,
        action: Some(Action::SubmitFrame(FrameSlotEvent { event: Some(event) })),
    }
}
fn frame_drill_args(pool_id: &str) -> Vec<String> {
    [
        "--watchdog-drill",
        SESSION,
        "0",
        "--frame-drill",
        pool_id,
        "worker-1",
        "keeper-1",
        "1",
        "8",
        "--frame-epoch",
        "7",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

struct Keeper {
    child: Child,
    stdin: std::process::ChildStdin,
    statuses: Option<mpsc::Receiver<OutputWatchdogStatus>>,
    reader: Option<thread::JoinHandle<()>>,
}
impl Keeper {
    fn spawn(args: &[String], stderr: Stdio) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_nai-output-keeper"))
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(stderr)
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let mut output = child.stdout.take().unwrap();
        let (sender, statuses) = mpsc::sync_channel(32);
        let reader = thread::spawn(move || {
            while let Ok(Some(bytes)) = frame::read(
                &mut output,
                Instant::now() + Duration::from_secs(10),
                &AtomicBool::new(false),
            ) {
                let Ok(status) = OutputWatchdogStatus::decode(bytes.as_slice()) else {
                    break;
                };
                if sender.send(status).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            stdin,
            statuses: Some(statuses),
            reader: Some(reader),
        }
    }
    fn send(&mut self, command: &OutputWatchdogCommand) {
        let bytes = frame::encode(&command.encode_to_vec()).unwrap();
        self.stdin.write_all(&bytes).unwrap();
        self.stdin.flush().unwrap();
    }
    fn until(&self, predicate: impl Fn(&OutputWatchdogStatus) -> bool) -> OutputWatchdogStatus {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let status = self
                .statuses
                .as_ref()
                .unwrap()
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("bounded keeper observation missing");
            assert!(
                !status.output_available,
                "keeper must never report hardware output"
            );
            if predicate(&status) {
                return status;
            }
        }
    }
}
impl Drop for Keeper {
    fn drop(&mut self) {
        self.statuses.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

#[test]
fn real_frame_is_presented_then_lease_loss_is_standby() {
    let dir = ram_dir();
    let mut pool = SharedFramePool::create(dir.path(), spec()).unwrap();
    let ticket = pool.reserve("worker-1", frame(), Instant::now()).unwrap();
    let commit = produce(&pool, &ticket);
    let pool_id = pool.owner().spec().pool_id.clone();
    let fd = pool.producer_file("worker-1").unwrap();
    let mut keeper = Keeper::spawn(&frame_drill_args(&pool_id), Stdio::from(fd));
    keeper.send(&arm(1, 7, utc_ms() + 600));
    keeper.send(&submit(2, Event::Committed(Box::new(commit))));
    let presented = keeper.until(|status| status.presenting);
    assert!(presented.lease_valid);
    assert_eq!(presented.standby_reason, "");
    // Stop renewing: keeper must stop delivering and report standby without a
    // device or PlaybackAck.
    let standby =
        keeper.until(|status| !status.presenting && status.standby_reason == "lease_lost");
    assert!(!standby.lease_valid);
    assert!(!standby.output_available);
}

#[test]
fn peer_error_switches_keeper_to_retired_standby() {
    let dir = ram_dir();
    let mut pool = SharedFramePool::create(dir.path(), spec()).unwrap();
    let ticket = pool.reserve("worker-1", frame(), Instant::now()).unwrap();
    let commit = produce(&pool, &ticket);
    let pool_id = pool.owner().spec().pool_id.clone();
    let fd = pool.producer_file("worker-1").unwrap();
    let mut keeper = Keeper::spawn(&frame_drill_args(&pool_id), Stdio::from(fd));
    keeper.send(&arm(1, 7, utc_ms() + 600));
    keeper.send(&submit(2, Event::Committed(Box::new(commit))));
    keeper.until(|status| status.presenting);
    keeper.send(&submit(
        3,
        Event::Error(ContractError {
            code: ErrorCode::Internal as i32,
            message: "producer reported a retired pool".into(),
            ..Default::default()
        }),
    ));
    let retired = keeper.until(|status| !status.presenting && status.standby_reason == "retired");
    assert!(!retired.output_available);
}

#[test]
fn non_frame_drill_mode_rejects_frame_submission() {
    let args = ["--watchdog-drill", SESSION, "0"]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();
    let mut keeper = Keeper::spawn(&args, Stdio::null());
    keeper.send(&arm(1, 7, utc_ms() + 600));
    keeper.send(&submit(2, Event::WriteStarted(true)));
    let status = keeper.until(|status| {
        status
            .last_control_error
            .as_ref()
            .is_some_and(|error| error.code == ErrorCode::PermissionDenied as i32)
    });
    assert!(!status.presenting);
    assert_eq!(status.standby_reason, "not_ready");
}
