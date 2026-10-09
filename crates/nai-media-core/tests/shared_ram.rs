#![cfg(target_os = "linux")]
use nai_contracts::v1::{
    FrameMetadata, FrameSlotCommit, FrameSlotEvent, FrameSlotTicket, frame_slot_event::Event,
};
use nai_media_core::frame_slots::{
    IoBudget, MAX_BUDGET, PoolSpec, SlotPhase,
    shared_ram::{SharedFramePool, write_frame},
};
use prost::Message;
use std::{
    io::Read,
    os::unix::fs::{FileExt, PermissionsExt},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

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
const PIXELS: [u8; 8] = [10, 20, 30, 255, 40, 50, 60, 255];
struct Peer {
    child: Child,
    events: mpsc::Receiver<FrameSlotEvent>,
}
impl Drop for Peer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn producer(pool: &SharedFramePool, ticket: &FrameSlotTicket, stall: bool) -> Peer {
    let hex: String = ticket
        .encode_to_vec()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let mut command = Command::new(env!("CARGO_BIN_EXE_nai-shared-frame-drill"));
    command
        .arg("--shared-frame-drill")
        .arg(hex)
        .stdin(Stdio::from(pool.producer_file("worker-1").unwrap()))
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if stall {
        command.arg("--stall-after-partial");
    }
    let mut child = command.spawn().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let (tx, events) = mpsc::sync_channel(4);
    std::thread::spawn(move || {
        loop {
            let mut size = [0; 4];
            if stdout.read_exact(&mut size).is_err() {
                break;
            }
            let n = u32::from_le_bytes(size) as usize;
            if n == 0 || n > 4096 {
                break;
            }
            let mut bytes = vec![0; n];
            if stdout.read_exact(&mut bytes).is_err() {
                break;
            }
            let Ok(event) = FrameSlotEvent::decode(bytes.as_slice()) else {
                break;
            };
            if tx.send(event).is_err() {
                break;
            }
        }
    });
    Peer { child, events }
}
fn event(peer: &Peer) -> Event {
    peer.events
        .recv_timeout(Duration::from_secs(2))
        .expect("bounded child notification")
        .event
        .unwrap()
}

#[test]
fn real_inherited_fd_producer_commits_and_reader_holds_owned_pixels() {
    let dir = ram_dir();
    let mut pool = SharedFramePool::create(dir.path(), spec()).unwrap();
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        0,
        "data backing is unlinked"
    );
    let ticket = pool.reserve("worker-1", frame(), Instant::now()).unwrap();
    let mut peer = producer(&pool, &ticket, false);
    assert_eq!(event(&peer), Event::WriteStarted(true));
    let Event::Committed(commit) = event(&peer) else {
        panic!("expected actual producer commit")
    };
    let commit = *commit;
    assert!(peer.child.wait().unwrap().success());
    pool.commit("worker-1", commit, Instant::now()).unwrap();
    let owned = pool
        .read_frame("keeper-1", &ticket, &IoBudget::new(MAX_BUDGET).unwrap())
        .unwrap();
    assert_eq!(owned.rgba, PIXELS);
    assert_eq!(owned.metadata, ticket.frame.clone().unwrap());
    assert!(pool.reserve("worker-1", frame(), Instant::now()).is_err());
    pool.release("keeper-1", &ticket, Instant::now()).unwrap();
    let next = pool.reserve("worker-1", frame(), Instant::now()).unwrap();
    assert!(next.slot_generation > ticket.slot_generation);
    assert!(pool.release("keeper-1", &ticket, Instant::now()).is_err());
    assert_eq!(
        owned.rgba, PIXELS,
        "owned copy survives slot reuse without being an output grant"
    );
}

#[test]
fn killing_partial_writer_never_marks_ready_and_new_pool_ignores_old_fd() {
    let dir = ram_dir();
    let mut pool = SharedFramePool::create(dir.path(), spec()).unwrap();
    let old_file = pool.producer_file("worker-1").unwrap();
    let ticket = pool.reserve("worker-1", frame(), Instant::now()).unwrap();
    let mut peer = producer(&pool, &ticket, true);
    assert_eq!(event(&peer), Event::WriteStarted(true));
    let mut actual = [0; 8];
    old_file.read_exact_at(&mut actual, 0).unwrap();
    assert_eq!(actual, [10, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(pool.owner().phase(0).unwrap(), SlotPhase::Writing);
    peer.child.kill().unwrap();
    peer.child.wait().unwrap();
    pool.disconnect("worker-1").unwrap();
    assert!(
        pool.read_frame("keeper-1", &ticket, &IoBudget::new(MAX_BUDGET).unwrap())
            .is_err()
    );
    let mut new = SharedFramePool::create(dir.path(), spec()).unwrap();
    assert_ne!(pool.owner().spec().pool_id, new.owner().spec().pool_id);
    old_file.write_all_at(&[255; 8], 0).unwrap();
    let fresh = new.reserve("worker-1", frame(), Instant::now()).unwrap();
    assert!(
        new.commit(
            "worker-1",
            FrameSlotCommit {
                ticket: Some(ticket),
                byte_length: 8,
                sha256: "a".repeat(64)
            },
            Instant::now()
        )
        .is_err()
    );
    let file = new.producer_file("worker-1").unwrap();
    let commit = write_frame(&file, &fresh, &PIXELS, &IoBudget::new(MAX_BUDGET).unwrap()).unwrap();
    new.commit("worker-1", commit, Instant::now()).unwrap();
    assert_eq!(
        new.read_frame("keeper-1", &fresh, &IoBudget::new(MAX_BUDGET).unwrap())
            .unwrap()
            .rgba,
        PIXELS
    );
}

#[test]
fn changed_hash_or_truncation_retires_the_entire_pool() {
    for truncate in [false, true] {
        let dir = ram_dir();
        let mut pool = SharedFramePool::create(dir.path(), spec()).unwrap();
        let file = pool.producer_file("worker-1").unwrap();
        let ticket = pool.reserve("worker-1", frame(), Instant::now()).unwrap();
        let commit =
            write_frame(&file, &ticket, &PIXELS, &IoBudget::new(MAX_BUDGET).unwrap()).unwrap();
        pool.commit("worker-1", commit, Instant::now()).unwrap();
        if truncate {
            file.set_len(4).unwrap();
        } else {
            file.write_all_at(&[99], 0).unwrap();
        }
        assert!(
            pool.read_frame("keeper-1", &ticket, &IoBudget::new(MAX_BUDGET).unwrap())
                .is_err()
        );
        assert!(pool.owner().is_retired());
    }
}

#[test]
fn foreign_path_fd_identity_and_io_cancellation_are_explicit() {
    let ordinary = tempfile::tempdir().unwrap();
    assert!(SharedFramePool::create(ordinary.path(), spec()).is_err());
    let dir = ram_dir();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(SharedFramePool::create(dir.path(), spec()).is_err());
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut pool = SharedFramePool::create(dir.path(), spec()).unwrap();
    assert!(pool.producer_file("alien").is_err());
    let ticket = pool.reserve("worker-1", frame(), Instant::now()).unwrap();
    let file = pool.producer_file("worker-1").unwrap();
    let budget = IoBudget::new(MAX_BUDGET).unwrap();
    budget
        .cancelled
        .store(true, std::sync::atomic::Ordering::Release);
    assert!(write_frame(&file, &ticket, &PIXELS, &budget).is_err());
    assert_eq!(pool.owner().phase(0).unwrap(), SlotPhase::Writing);
    let ordinary_file = tempfile::tempfile().unwrap();
    ordinary_file.set_len(8).unwrap();
    assert!(
        write_frame(
            &ordinary_file,
            &ticket,
            &PIXELS,
            &IoBudget::new(MAX_BUDGET).unwrap()
        )
        .is_err()
    );
    let status = Command::new(env!("CARGO_BIN_EXE_nai-shared-frame-drill"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(!status.success());
    assert!(pool.poll(Instant::now() + MAX_BUDGET).is_err());
    assert!(pool.owner().is_retired());
}

#[test]
fn cancelled_read_retires_committed_frames_instead_of_leaving_a_pin() {
    let dir = ram_dir();
    let mut pool = SharedFramePool::create(dir.path(), spec()).unwrap();
    let ticket = pool.reserve("worker-1", frame(), Instant::now()).unwrap();
    let file = pool.producer_file("worker-1").unwrap();
    let commit = write_frame(&file, &ticket, &PIXELS, &IoBudget::new(MAX_BUDGET).unwrap()).unwrap();
    pool.commit("worker-1", commit, Instant::now()).unwrap();
    let budget = IoBudget::new(MAX_BUDGET).unwrap();
    budget
        .cancelled
        .store(true, std::sync::atomic::Ordering::Release);
    assert!(pool.read_frame("keeper-1", &ticket, &budget).is_err());
    assert!(pool.owner().is_retired());
    assert!(pool.release("keeper-1", &ticket, Instant::now()).is_err());
}
