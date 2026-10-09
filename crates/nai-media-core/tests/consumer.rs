#![cfg(target_os = "linux")]
use nai_contracts::v1::{
    FrameMetadata, FrameSlotCommit, FrameSlotEvent, FrameSlotTicket, frame_slot_event::Event,
};
use nai_media_core::frame_slots::{
    IoBudget, MAX_BUDGET, PoolSpec,
    consumer::{FrameConsumer, FrameReader, StandbyReason},
    shared_ram::{SharedFramePool, write_frame},
};
use std::{
    os::unix::fs::{FileExt, PermissionsExt},
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

/// A committed pool, its real spec (for the reader) and the committed ticket.
fn ready() -> (
    tempfile::TempDir,
    SharedFramePool,
    FrameSlotTicket,
    FrameSlotCommit,
) {
    let dir = ram_dir();
    let mut pool = SharedFramePool::create(dir.path(), spec()).unwrap();
    let ticket = pool.reserve("worker-1", frame(), Instant::now()).unwrap();
    let file = pool.producer_file("worker-1").unwrap();
    let commit = write_frame(&file, &ticket, &PIXELS, &IoBudget::new(MAX_BUDGET).unwrap()).unwrap();
    pool.commit("worker-1", commit.clone(), Instant::now())
        .unwrap();
    (dir, pool, ticket, commit)
}
fn reader(pool: &SharedFramePool) -> FrameReader {
    let file = pool.producer_file("worker-1").unwrap();
    FrameReader::open(file, pool.owner().spec().clone()).unwrap()
}
fn committed(commit: &FrameSlotCommit) -> FrameSlotEvent {
    FrameSlotEvent {
        event: Some(Event::Committed(Box::new(commit.clone()))),
    }
}

#[test]
fn invalid_lease_never_reads_and_reports_lease_lost() {
    let (_dir, pool, _ticket, commit) = ready();
    let reader = reader(&pool);
    let mut consumer = FrameConsumer::new("s", 0);
    let outcome = consumer.submit(committed(&commit), false, &reader, Instant::now());
    assert_eq!(outcome.standby(), Some(StandbyReason::LeaseLost));
    assert_eq!(consumer.last_delivered_epoch(), 0);
    assert!(outcome.delivered().is_none());
}

#[test]
fn write_started_and_missing_event_are_no_frame() {
    let (_dir, pool, _ticket, _commit) = ready();
    let reader = reader(&pool);
    let mut consumer = FrameConsumer::new("s", 0);
    let started = FrameSlotEvent {
        event: Some(Event::WriteStarted(true)),
    };
    assert_eq!(
        consumer
            .submit(started, true, &reader, Instant::now())
            .standby(),
        Some(StandbyReason::NoFrame)
    );
    let empty = FrameSlotEvent { event: None };
    assert_eq!(
        consumer
            .submit(empty, true, &reader, Instant::now())
            .standby(),
        Some(StandbyReason::NoFrame)
    );
}

#[test]
fn peer_error_event_is_retired() {
    let (_dir, pool, _ticket, _commit) = ready();
    let reader = reader(&pool);
    let mut consumer = FrameConsumer::new("s", 0);
    let error = FrameSlotEvent {
        event: Some(Event::Error(nai_contracts::v1::ContractError {
            code: nai_contracts::v1::ErrorCode::Internal as i32,
            message: "producer reported a retired pool".into(),
            ..Default::default()
        })),
    };
    assert_eq!(
        consumer
            .submit(error, true, &reader, Instant::now())
            .standby(),
        Some(StandbyReason::Retired)
    );
}

#[test]
fn epoch_at_or_below_floor_or_last_delivery_is_stale() {
    let (_dir, pool, _ticket, commit) = ready();
    let reader = reader(&pool);
    let mut floor = FrameConsumer::new("s", 7);
    assert_eq!(
        floor
            .submit(committed(&commit), true, &reader, Instant::now())
            .standby(),
        Some(StandbyReason::StaleEpoch)
    );
    let mut consumer = FrameConsumer::new("s", 0);
    assert!(
        consumer
            .submit(committed(&commit), true, &reader, Instant::now())
            .delivered()
            .is_some()
    );
    assert_eq!(consumer.last_delivered_epoch(), 7);
    assert_eq!(
        consumer
            .submit(committed(&commit), true, &reader, Instant::now())
            .standby(),
        Some(StandbyReason::StaleEpoch)
    );
}

#[test]
fn tampered_sha256_is_an_integrity_failure() {
    let (_dir, pool, _ticket, commit) = ready();
    let reader = reader(&pool);
    pool.producer_file("worker-1")
        .unwrap()
        .write_all_at(&[99], 0)
        .unwrap();
    let mut consumer = FrameConsumer::new("s", 0);
    let outcome = consumer.submit(committed(&commit), true, &reader, Instant::now());
    assert_eq!(outcome.standby(), Some(StandbyReason::IntegrityFailure));
    assert_eq!(consumer.last_delivered_epoch(), 0);
}

#[test]
fn changed_pool_length_is_an_integrity_failure() {
    let (_dir, pool, _ticket, commit) = ready();
    let reader = reader(&pool);
    pool.producer_file("worker-1").unwrap().set_len(4).unwrap();
    let mut consumer = FrameConsumer::new("s", 0);
    assert_eq!(
        consumer
            .submit(committed(&commit), true, &reader, Instant::now())
            .standby(),
        Some(StandbyReason::IntegrityFailure)
    );
}

#[test]
fn expired_read_budget_is_a_timeout() {
    let (_dir, pool, _ticket, commit) = ready();
    let reader = reader(&pool);
    let mut consumer = FrameConsumer::new("s", 0);
    let stale_now = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();
    assert_eq!(
        consumer
            .submit(committed(&commit), true, &reader, stale_now)
            .standby(),
        Some(StandbyReason::Timeout)
    );
}

#[test]
fn delivery_returns_owned_pixels_and_records_epoch_without_ack() {
    let (_dir, pool, ticket, commit) = ready();
    let reader = reader(&pool);
    let mut consumer = FrameConsumer::new("s", 0);
    let outcome = consumer.submit(committed(&commit), true, &reader, Instant::now());
    let owned = outcome.delivered().expect("owned frame copy");
    assert_eq!(owned.rgba, PIXELS);
    assert_eq!(owned.metadata, ticket.frame.clone().unwrap());
    assert_eq!(owned.ticket, ticket);
    assert_eq!(consumer.last_delivered_epoch(), 7);
    // The outcome enum has no variant that could carry a device or PlaybackAck.
}

#[test]
fn foreign_pool_identity_is_an_integrity_failure() {
    let (_dir, pool, _ticket, commit) = ready();
    let mut foreign = pool.owner().spec().clone();
    foreign.pool_id = "some-other-pool".into();
    let file = pool.producer_file("worker-1").unwrap();
    let reader = FrameReader::open(file, foreign).unwrap();
    let mut consumer = FrameConsumer::new("s", 0);
    assert_eq!(
        consumer
            .submit(committed(&commit), true, &reader, Instant::now())
            .standby(),
        Some(StandbyReason::IntegrityFailure)
    );
    assert!(reader.spec().pool_id != pool.owner().spec().pool_id);
}
