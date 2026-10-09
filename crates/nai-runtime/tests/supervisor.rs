//! Real native subprocess drills for the composition root. Not engine, Windows or
//! hardware certification: the keeper still reports no device output and the
//! render-worker produces no PlaybackAck.
//!
//! The slot reserve window is non-renewable and only 250ms. Because the real 2D
//! avatar admit pays a one-time ~200ms schema cost, the worker is warmed and
//! signals readiness before the root reserves the ticket and delivers it
//! atomically; the reserve->commit window then only covers the write.
#![cfg(target_os = "linux")]
mod common;
use common::*;
use nai_contracts::v1::{
    ErrorCode, FrameMetadata, FrameSlotEvent, FrameSlotTicket, frame_slot_event::Event,
};
use nai_media_core::frame_slots::{
    IoBudget, PoolSpec, SlotPhase,
    consumer::{FrameConsumer, FrameReader},
    shared_ram::{SharedFramePool, digest},
};
use nai_runtime::{Binaries, Composition, RenderWorkerSpec, StandbyReason, Supervisor};
use nai_storage::DataPaths;
use prost::Message;
use serde_json::json;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tempfile::tempdir;

const PRODUCER: &str = "render-1";
const CONSUMER: &str = "keeper-1";

fn composition(root: &Path) -> Composition {
    provision(root, WORKSPACE, ACTOR, PROFILE, &runtime_profile(WORKSPACE));
    Composition::prepare(
        DataPaths::at(root).unwrap(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        false,
    )
    .unwrap()
}

fn binaries() -> Binaries {
    Binaries {
        keeper: binary("nai-output-keeper"),
        render_worker: binary("nai-render-worker"),
    }
}

fn ram_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir_in("/dev/shm").expect("Linux frame drill requires /dev/shm tmpfs");
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    dir
}

struct AvatarFiles {
    manifest: PathBuf,
    config: PathBuf,
    assets: Vec<(String, PathBuf)>,
}

fn write_avatar(dir: &Path) -> AvatarFiles {
    let base = vec![10u8, 20, 30, 255, 0, 0, 255, 255];
    let closed = vec![0u8, 0, 0, 255];
    let open = vec![255u8, 0, 0, 255];
    let (base_hash, closed_hash, open_hash) = (digest(&base), digest(&closed), digest(&open));
    let manifest = json!({
        "schema_version": 1,
        "avatar_id": "test-only-2d",
        "version": "1",
        "kind": "2d",
        "license_ref": "test-license",
        "assets": [
            {"id": "base", "path": "base.rgba", "sha256": base_hash, "license_ref": "test-license"},
            {"id": "closed", "path": "closed.rgba", "sha256": closed_hash, "license_ref": "test-license"},
            {"id": "open", "path": "open.rgba", "sha256": open_hash, "license_ref": "test-license"}
        ],
        "model_binding": "software-rgba8-v1",
        "resource_profile_ref": "test-software",
        "supported_actions": ["idle", "speak"],
        "supported_emotions": ["neutral"],
        "voice_binding": "test-voice",
        "native_width": 2,
        "native_height": 1,
        "output_width": 2,
        "output_height": 1,
        "fps_num": 25,
        "fps_den": 1,
        "base_duration_samples": "0",
        "switch_points": [],
        "personalization_refs": {}
    });
    let config = json!({
        "schema_version": 1,
        "pixel_format": "rgba8",
        "sample_rate": 8000,
        "max_buffer_samples": 8000,
        "noise_gate": 0.02,
        "full_open_rms": 0.5,
        "attack_ms": 10,
        "release_ms": 100,
        "layers": [
            {"asset_id": "base", "role": "base", "width": 2, "height": 1, "x": 0, "y": 0},
            {"asset_id": "closed", "role": "mouth_closed", "width": 1, "height": 1, "x": 1, "y": 0},
            {"asset_id": "open", "role": "mouth_open", "width": 1, "height": 1, "x": 1, "y": 0}
        ]
    });
    let manifest_path = dir.join("manifest.json");
    let config_path = dir.join("config.json");
    fs::write(&manifest_path, manifest.to_string()).unwrap();
    fs::write(&config_path, config.to_string()).unwrap();
    let mut assets = Vec::new();
    for (id, name, bytes) in [
        ("base", "base.rgba", base),
        ("closed", "closed.rgba", closed),
        ("open", "open.rgba", open),
    ] {
        let path = dir.join(name);
        fs::write(&path, &bytes).unwrap();
        assets.push((id.to_string(), path));
    }
    AvatarFiles {
        manifest: manifest_path,
        config: config_path,
        assets,
    }
}

fn create_pool(session: &str, epoch: u64) -> (tempfile::TempDir, SharedFramePool) {
    let ram = ram_dir();
    let spec = PoolSpec {
        pool_id: "root".into(),
        session_id: session.into(),
        generation_epoch: epoch,
        producer_ref: PRODUCER.into(),
        consumer_ref: CONSUMER.into(),
        slots: 1,
        slot_bytes: 8,
    };
    let pool = SharedFramePool::create(ram.path(), spec).unwrap();
    (ram, pool)
}

fn frame_metadata(session: &str, epoch: u64) -> FrameMetadata {
    FrameMetadata {
        session_id: session.into(),
        utterance_id: "u".into(),
        generation_epoch: epoch,
        width: 2,
        height: 1,
        pixel_format: "rgba8".into(),
        duration: 320,
        time_base_num: 1,
        time_base_den: 8000,
        ..Default::default()
    }
}

fn write_pcm(work: &Path, pcm: &[f32]) -> PathBuf {
    let path = work.join("pcm.bin");
    let mut bytes = Vec::new();
    for sample in pcm {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    fs::write(&path, &bytes).unwrap();
    path
}

/// Atomically delivers the reserved ticket so the worker never reads a partial file.
fn deliver_ticket(ticket: &FrameSlotTicket, path: &Path) {
    let staging = path.with_extension("staging");
    fs::write(&staging, ticket.encode_to_vec()).unwrap();
    fs::rename(&staging, path).unwrap();
}

fn wait_for_file(path: &Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while !path.is_file() {
        assert!(
            Instant::now() < deadline,
            "worker did not signal readiness at {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Spawns a warmed render-worker and returns it with its reserved ticket and the
/// ticket file path. `epoch` is both the worker's view and the pool/ticket epoch.
fn warm_worker(
    session: &str,
    epoch: u64,
    work: &Path,
    pool: &mut SharedFramePool,
) -> (Supervisor, FrameSlotTicket, PathBuf) {
    let ticket_path = work.join("ticket.bin");
    let ready_path = work.join("ready");
    let pcm_path = write_pcm(work, &[0.5; 320]);
    let avatar = write_avatar(work);
    let spec = RenderWorkerSpec {
        broker: None,
        finish: None,
        session: session.to_string(),
        epoch,
        pcm: pcm_path,
        ticket: ticket_path.clone(),
        manifest: avatar.manifest,
        config: avatar.config,
        assets: avatar.assets,
        licenses: vec!["test-license".into()],
        ready: Some(ready_path.clone()),
    };
    let mut supervisor = Supervisor::new(binaries()).unwrap();
    let pool_file = pool.producer_file(PRODUCER).unwrap();
    supervisor.spawn_worker(&spec, pool_file).unwrap();
    wait_for_file(&ready_path, Duration::from_secs(10));
    let ticket = pool
        .reserve(PRODUCER, frame_metadata(session, epoch), Instant::now())
        .unwrap();
    deliver_ticket(&ticket, &ticket_path);
    (supervisor, ticket, ticket_path)
}

#[test]
fn killed_keeper_is_revoked_logged_and_stands_by() {
    let root = tempdir().unwrap();
    let mut composition = composition(root.path());
    let session = composition.session_id().to_string();
    let floor = composition
        .session_lock()
        .unwrap()
        .snapshot()
        .generation_epoch;
    let mut supervisor = Supervisor::new(binaries()).unwrap();
    supervisor.spawn_keeper(&session, floor, None).unwrap();
    let lease = composition
        .rehearsal_lease("drill-lease", &["u".to_string()], floor + 1, utc_ms())
        .unwrap();
    supervisor.arm(&session, lease).unwrap();
    let status = supervisor
        .wait_until(Instant::now() + Duration::from_secs(3), |status| {
            status.lease_valid
        })
        .unwrap();
    assert!(status.lease_valid);
    assert!(
        !status.output_available,
        "keeper must never report hardware output"
    );

    supervisor.kill_keeper().unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while supervisor.detect_fault().is_none() {
        assert!(Instant::now() < deadline, "killed keeper never observed");
        std::thread::sleep(Duration::from_millis(5));
    }
    let observed = composition
        .observe(&mut supervisor)
        .unwrap()
        .expect("a killed keeper is a supervised fault");
    assert_eq!(observed.fault.child, "keeper");
    assert!(observed.fault.exit_text().is_some());
    assert_eq!(supervisor.standby(), Some(StandbyReason::Retired));
    assert!(
        observed.ack.persisted && observed.ack.entry_hash.is_some(),
        "the critical cancellation entry must be persisted"
    );
    assert_eq!(
        composition
            .session_lock()
            .unwrap()
            .snapshot()
            .generation_epoch,
        floor + 1,
        "observe must revoke and increment the epoch"
    );
    verify_chain(&composition);
    supervisor.shutdown().unwrap();
}

#[test]
fn render_worker_writes_real_pixels_and_the_consumer_reads_them() {
    let root = tempdir().unwrap();
    let composition = composition(root.path());
    let session = composition.session_id().to_string();
    let epoch = 1u64;
    let (_ram, mut pool) = create_pool(&session, epoch);
    let work = tempdir().unwrap();
    let (mut supervisor, _ticket, _path) = warm_worker(&session, epoch, work.path(), &mut pool);

    let started = supervisor
        .read_worker_event(Instant::now() + Duration::from_secs(5))
        .unwrap();
    assert_eq!(started.event, Some(Event::WriteStarted(true)));
    let committed = supervisor
        .read_worker_event(Instant::now() + Duration::from_secs(5))
        .unwrap();
    let Some(Event::Committed(commit)) = committed.event.clone() else {
        panic!("expected a producer commit");
    };
    assert!(
        supervisor
            .wait_worker(Duration::from_secs(5))
            .unwrap()
            .success()
    );

    // The owner accepts the commit against its own ledger.
    pool.commit(PRODUCER, commit.as_ref().clone(), Instant::now())
        .unwrap();

    // The consumer reads owned pixels under a live lease; never an ACK.
    let reader_spec = pool.owner().spec().clone();
    let reader = FrameReader::open(pool.producer_file(PRODUCER).unwrap(), reader_spec).unwrap();
    let mut consumer = FrameConsumer::new(&session, 0);
    let outcome = consumer.submit(
        FrameSlotEvent {
            event: Some(Event::Committed(commit)),
        },
        true,
        &reader,
        Instant::now(),
    );
    let delivered = outcome
        .delivered()
        .expect("consumer must deliver owned pixels");
    assert_eq!(delivered.rgba.len(), 8);
    assert_eq!(&delivered.rgba[..4], &[10u8, 20, 30, 255]);
    assert!(
        delivered.rgba[4] > 240,
        "loud PCM must open the mouth layer"
    );
    assert!(delivered.rgba[6] < 5);
    assert_eq!(delivered.metadata.generation_epoch, epoch);
    supervisor.shutdown().unwrap();
}

#[test]
fn render_worker_rejects_a_ticket_from_another_epoch() {
    let root = tempdir().unwrap();
    let composition = composition(root.path());
    let session = composition.session_id().to_string();
    let epoch = 1u64;
    let (_ram, mut pool) = create_pool(&session, epoch);
    let ticket = pool
        .reserve(PRODUCER, frame_metadata(&session, epoch), Instant::now())
        .unwrap();

    let work = tempdir().unwrap();
    let ticket_path = work.path().join("ticket.bin");
    deliver_ticket(&ticket, &ticket_path);
    let avatar = write_avatar(work.path());
    let spec = RenderWorkerSpec {
        broker: None,
        finish: None,
        session: session.clone(),
        epoch: epoch + 1, // the worker is told a different current epoch
        pcm: write_pcm(work.path(), &[0.5; 320]),
        ticket: ticket_path,
        manifest: avatar.manifest,
        config: avatar.config,
        assets: avatar.assets,
        licenses: vec!["test-license".into()],
        ready: None,
    };
    let mut supervisor = Supervisor::new(binaries()).unwrap();
    let pool_file = pool.producer_file(PRODUCER).unwrap();
    supervisor.spawn_worker(&spec, pool_file).unwrap();

    let event = supervisor
        .read_worker_event(Instant::now() + Duration::from_secs(5))
        .unwrap();
    let Some(Event::Error(error)) = event.event.clone() else {
        panic!("expected an error event");
    };
    assert_eq!(error.code, ErrorCode::StaleEpoch as i32);
    assert!(
        !supervisor
            .wait_worker(Duration::from_secs(5))
            .unwrap()
            .success()
    );
    assert_eq!(pool.owner().phase(0).unwrap(), SlotPhase::Writing);
}

#[test]
fn tampered_commit_hash_is_rejected_and_retires_the_pool() {
    let root = tempdir().unwrap();
    let composition = composition(root.path());
    let session = composition.session_id().to_string();
    let epoch = 1u64;
    let (_ram, mut pool) = create_pool(&session, epoch);
    let work = tempdir().unwrap();
    let (mut supervisor, ticket, _path) = warm_worker(&session, epoch, work.path(), &mut pool);

    let committed = supervisor
        .read_worker_event(Instant::now() + Duration::from_secs(5))
        .unwrap();
    assert_eq!(committed.event, Some(Event::WriteStarted(true)));
    let committed = supervisor
        .read_worker_event(Instant::now() + Duration::from_secs(5))
        .unwrap();
    let Some(Event::Committed(commit)) = committed.event.clone() else {
        panic!("expected a producer commit");
    };
    assert!(
        supervisor
            .wait_worker(Duration::from_secs(5))
            .unwrap()
            .success()
    );

    // Tamper the SHA256 so the commit no longer matches the slot content.
    let mut tampered = commit.as_ref().clone();
    let first = tampered.sha256.remove(0);
    tampered
        .sha256
        .insert(0, if first == '0' { '1' } else { '0' });
    pool.commit(PRODUCER, tampered, Instant::now()).unwrap();

    let budget = IoBudget::new(Duration::from_millis(250)).unwrap();
    let result = pool.read_frame(CONSUMER, &ticket, &budget);
    assert!(result.is_err(), "an integrity mismatch must be rejected");
    assert!(
        pool.owner().is_retired(),
        "an integrity failure must retire the whole pool"
    );
    supervisor.shutdown().unwrap();
}
