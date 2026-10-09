#![cfg(target_os = "linux")]
use nai_contracts::v1::{
    ErrorCode, FrameMetadata, FrameSlotCommit, FrameSlotEvent, FrameSlotTicket,
    frame_slot_event::Event,
};
use nai_media_core::frame_slots::{
    IoBudget, MAX_BUDGET, PoolSpec, SlotPhase,
    broker::{FrameBroker, MAX_ACCEPT_ATTEMPTS, PeerIdentity, PeerRole},
    shared_ram::write_frame,
};
use prost::Message;
use std::{
    fs::File,
    io::Read,
    os::unix::fs::{MetadataExt, PermissionsExt},
    os::unix::net::UnixStream,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

const PIXELS: [u8; 8] = [10, 20, 30, 255, 40, 50, 60, 255];

fn spec() -> PoolSpec {
    PoolSpec {
        pool_id: "root-placeholder".into(),
        session_id: "s".into(),
        generation_epoch: 7,
        producer_ref: "worker-1".into(),
        consumer_ref: "keeper-1".into(),
        slots: 1,
        slot_bytes: 8,
    }
}
fn frame() -> FrameMetadata {
    FrameMetadata {
        session_id: "s".into(),
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
fn owner_uid() -> u32 {
    std::fs::metadata("/proc/self").unwrap().uid()
}
fn declare(role: PeerRole, pid: i32) -> PeerIdentity {
    PeerIdentity {
        role,
        pid,
        uid: owner_uid(),
    }
}

struct Peer {
    child: Child,
    stdout: std::process::ChildStdout,
}
impl Peer {
    fn event(&mut self) -> Event {
        let mut size = [0u8; 4];
        self.stdout.read_exact(&mut size).unwrap();
        let length = u32::from_le_bytes(size) as usize;
        assert!(length > 0 && length <= 4096);
        let mut bytes = vec![0; length];
        self.stdout.read_exact(&mut bytes).unwrap();
        FrameSlotEvent::decode(bytes.as_slice())
            .unwrap()
            .event
            .unwrap()
    }
    fn wait(&mut self) -> std::process::ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(Instant::now() < deadline, "owned subprocess did not exit");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn spawn_broker_drill(socket: &Path, ticket_file: &Path, pool: &File) -> Peer {
    let mut child = Command::new(env!("CARGO_BIN_EXE_nai-shared-frame-drill"))
        .arg("--broker-drill")
        .arg(socket)
        .arg(ticket_file)
        .stdin(Stdio::from(pool.try_clone().unwrap()))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    Peer {
        stdout: child.stdout.take().unwrap(),
        child,
    }
}
fn ticket_hex(ticket: &FrameSlotTicket) -> String {
    ticket
        .encode_to_vec()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn unverified_peer_cannot_touch_slots() {
    let dir = ram_dir();
    let mut broker = FrameBroker::bind(dir.path(), spec(), vec![]).unwrap();
    let now = Instant::now();
    assert_eq!(
        broker.reserve(frame(), now).unwrap_err().code,
        ErrorCode::PermissionDenied as i32
    );
    assert_eq!(
        broker
            .commit(FrameSlotCommit::default(), now)
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied as i32
    );
    assert_eq!(
        broker.producer_file().unwrap_err().code,
        ErrorCode::PermissionDenied as i32
    );
    assert_eq!(
        broker
            .read_frame(
                &FrameSlotTicket::default(),
                &IoBudget::new(MAX_BUDGET).unwrap()
            )
            .err()
            .unwrap()
            .code,
        ErrorCode::PermissionDenied as i32
    );
    assert_eq!(broker.owner().phase(0).unwrap(), SlotPhase::Free);
}

#[test]
fn wrong_declared_pid_is_never_granted() {
    let dir = ram_dir();
    let mut broker = FrameBroker::bind(dir.path(), spec(), vec![]).unwrap();
    // i32::MAX is never a live pid, so it can never hold the pool file.
    broker
        .declare_peer(declare(PeerRole::Producer, i32::MAX))
        .unwrap();
    let _client = UnixStream::connect(broker.socket_path()).unwrap();
    assert_eq!(
        broker
            .poll_grants(Instant::now() + Duration::from_millis(300))
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied as i32
    );
    assert_eq!(
        broker.producer_file().unwrap_err().code,
        ErrorCode::PermissionDenied as i32
    );
    assert_eq!(broker.owner().phase(0).unwrap(), SlotPhase::Free);
}

#[test]
fn accept_budget_is_bounded() {
    let dir = ram_dir();
    let mut broker = FrameBroker::bind(dir.path(), spec(), vec![]).unwrap();
    broker
        .declare_peer(declare(PeerRole::Producer, i32::MAX))
        .unwrap();
    let _clients: Vec<_> = (0..MAX_ACCEPT_ATTEMPTS + 1)
        .map(|_| UnixStream::connect(broker.socket_path()).unwrap())
        .collect();
    assert_eq!(
        broker
            .poll_grants(Instant::now() + Duration::from_secs(1))
            .unwrap_err()
            .code,
        ErrorCode::ResourceExhausted as i32
    );
}

#[test]
fn deadline_without_connection_is_explicit() {
    let dir = ram_dir();
    let mut broker = FrameBroker::bind(dir.path(), spec(), vec![]).unwrap();
    assert_eq!(
        broker
            .poll_grants(Instant::now() + Duration::from_millis(50))
            .unwrap_err()
            .code,
        ErrorCode::DeadlineExceeded as i32
    );
}

#[test]
fn verified_producer_writes_and_consumer_reads() {
    let dir = ram_dir();
    let pid = std::process::id() as i32;
    let mut broker = FrameBroker::bind(
        dir.path(),
        spec(),
        vec![
            declare(PeerRole::Producer, pid),
            declare(PeerRole::Consumer, pid),
        ],
    )
    .unwrap();
    let _producer = UnixStream::connect(broker.socket_path()).unwrap();
    assert_eq!(
        broker
            .poll_grants(Instant::now() + Duration::from_secs(2))
            .unwrap()
            .role,
        PeerRole::Producer
    );
    let _consumer = UnixStream::connect(broker.socket_path()).unwrap();
    assert_eq!(
        broker
            .poll_grants(Instant::now() + Duration::from_secs(2))
            .unwrap()
            .role,
        PeerRole::Consumer
    );
    let ticket = broker.reserve(frame(), Instant::now()).unwrap();
    let file = broker.producer_file().unwrap();
    let commit = write_frame(&file, &ticket, &PIXELS, &IoBudget::new(MAX_BUDGET).unwrap()).unwrap();
    broker.commit(commit, Instant::now()).unwrap();
    let owned = broker
        .read_frame(&ticket, &IoBudget::new(MAX_BUDGET).unwrap())
        .unwrap();
    assert_eq!(owned.rgba, PIXELS);
    broker.disconnect(PeerRole::Consumer).unwrap();
    assert!(
        broker
            .read_frame(&ticket, &IoBudget::new(MAX_BUDGET).unwrap())
            .is_err()
    );
}

#[test]
fn real_subprocess_declared_pid_is_granted_and_writes() {
    let dir = ram_dir();
    let mut broker = FrameBroker::bind(dir.path(), spec(), vec![]).unwrap();
    let ticket_file = dir.path().join("ticket.hex");
    let pool = broker.producer_spawn_handle().unwrap();
    let mut peer = spawn_broker_drill(broker.socket_path(), &ticket_file, &pool);
    let pid = peer.child.id() as i32;
    broker
        .declare_peer(declare(PeerRole::Producer, pid))
        .unwrap();
    let granted = broker
        .poll_grants(Instant::now() + Duration::from_secs(3))
        .unwrap();
    assert_eq!(granted.role, PeerRole::Producer);
    assert_eq!(granted.pid, pid);
    let ticket = broker.reserve(frame(), Instant::now()).unwrap();
    std::fs::write(&ticket_file, ticket_hex(&ticket)).unwrap();
    assert_eq!(peer.event(), Event::WriteStarted(true));
    let Event::Committed(commit) = peer.event() else {
        panic!("expected actual producer commit")
    };
    assert!(peer.wait().success());
    broker.commit(*commit, Instant::now()).unwrap();
    assert_eq!(broker.owner().phase(0).unwrap(), SlotPhase::Ready);
}

#[test]
fn real_subprocess_wrong_pid_is_refused_without_commit() {
    let dir = ram_dir();
    let mut broker = FrameBroker::bind(dir.path(), spec(), vec![]).unwrap();
    let ticket_file = dir.path().join("ticket.hex");
    // A decodable but semantically invalid ticket lets the child exit promptly
    // after its connection is refused.
    let bogus = FrameSlotTicket {
        schema_version: 1,
        ..Default::default()
    };
    std::fs::write(&ticket_file, ticket_hex(&bogus)).unwrap();
    let pool = broker.producer_spawn_handle().unwrap();
    let mut peer = spawn_broker_drill(broker.socket_path(), &ticket_file, &pool);
    broker
        .declare_peer(declare(PeerRole::Producer, i32::MAX))
        .unwrap();
    assert_eq!(
        broker
            .poll_grants(Instant::now() + Duration::from_millis(400))
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied as i32
    );
    assert_eq!(
        broker.producer_file().unwrap_err().code,
        ErrorCode::PermissionDenied as i32
    );
    assert_eq!(broker.owner().phase(0).unwrap(), SlotPhase::Free);
    assert!(!peer.wait().success());
}
