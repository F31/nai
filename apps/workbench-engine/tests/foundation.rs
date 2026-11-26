use foundation_command::Operation as C;
use nai_contracts::{workbench::*, workbench_wire as wire};
use nai_foundation_sqlite::Store;
use nai_runtime_core::foundation::Operation;
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};
use tempfile::tempdir;
static KEY: AtomicU64 = AtomicU64::new(500);
#[test]
fn real_ipc_avatar_selection_is_atomic_and_revocation_persists_after_restart() {
    let root = tempdir().unwrap();
    let mut c = Client::open(root.path(), true, 1);
    c.send(workspace()).unwrap();
    let document = |name: &str, selected: bool, revoked: bool| {
        serde_json::json!({"schema_version":1,"kind":"avatar_profile_draft","name":name,"avatar_id":name,"package_version":"1","package_sha256":"a".repeat(64),"license_sha256":"b".repeat(64),"width":2,"height":1,"selected":selected,"revoked":revoked}).to_string()
    };
    let put = |n, revision, body| {
        C::PutPreparation(FoundationObject {
            workspace_id: id(10),
            object_id: id(n),
            expected_revision: revision,
            body,
        })
    };
    let read = |n| {
        C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(n),
            revision: 0,
        })
    };
    c.send(put(70, 0, document("one", true, false))).unwrap();
    c.send(put(71, 0, document("two", false, false))).unwrap();
    c.send(put(71, 1, document("two", true, false))).unwrap();
    let old = c.send(read(70)).unwrap();
    assert_eq!(old.revision, 2);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&old.body).unwrap()["selected"],
        false
    );
    c.send(put(71, 2, document("two", false, true))).unwrap();
    drop(c);
    let mut c = Client::open(root.path(), false, 1);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&c.send(read(71)).unwrap().body).unwrap()["revoked"],
        true
    );
    assert_eq!(
        c.send(put(71, 3, document("two", true, false))),
        Err(WorkbenchError::PermissionDenied)
    );
}
#[test]
fn real_ipc_avatar_backup_survives_engine_restart_and_migration() {
    use sha2::{Digest, Sha256};
    let root = tempdir().unwrap();
    let mut c = Client::open(root.path(), true, 1);
    c.send(workspace()).unwrap();
    let ids = ["base", "closed", "open"];
    let assets: [&[u8]; 3] = [b"base", b"closed", b"open"];
    let manifest = b"test-manifest";
    let config = b"test-config";
    let license = b"test-license";
    let mut h = Sha256::new();
    h.update(manifest);
    h.update(config);
    h.update(license);
    for (name, bytes) in ids.iter().zip(assets) {
        h.update(name.as_bytes());
        h.update(bytes);
    }
    let package = h
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let rights = Sha256::digest(license)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let dir = root.path().join("avatar-packages").join(&package);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, bytes) in [
        ("manifest.json", manifest.as_slice()),
        ("config.json", config.as_slice()),
        ("LICENSE.txt", license.as_slice()),
        ("preview.png", b"preview".as_slice()),
    ] {
        std::fs::write(dir.join(name), bytes).unwrap();
    }
    std::fs::write(
        dir.join("asset-ids.json"),
        serde_json::to_vec(&ids).unwrap(),
    )
    .unwrap();
    for (index, bytes) in assets.iter().enumerate() {
        std::fs::write(dir.join(format!("asset-{index}.rgba")), bytes).unwrap();
    }
    let avatar=serde_json::json!({"schema_version":1,"kind":"avatar_profile_draft","name":"avatar","avatar_id":"avatar","package_version":"1","package_sha256":package,"license_sha256":rights,"width":2,"height":1,"selected":false,"revoked":false}).to_string();
    c.send(C::PutPreparation(FoundationObject {
        workspace_id: id(10),
        object_id: id(70),
        expected_revision: 0,
        body: avatar,
    }))
    .unwrap();
    let preview = c
        .send(C::PreviewSharedModels(FoundationLlmRead::default()))
        .unwrap()
        .shared_model_preview
        .unwrap();
    assert!(preview.rows.is_empty());
    let migration = SharedModelMigration {
        migration_id: id(900),
        plan_sha256: preview.plan_sha256,
        entries: vec![],
    };
    assert_eq!(
        c.send(C::MigrateSharedModels(migration.clone()))
            .unwrap()
            .shared_model_migration
            .unwrap()
            .stage,
        "preparing"
    );
    let saved = c
        .send(C::MigrateSharedModels(migration.clone()))
        .unwrap()
        .shared_model_migration
        .unwrap();
    assert_eq!(saved.stage, "rewrapping");
    let backup = root
        .path()
        .join("model-migration-backups")
        .join(saved.backup_id)
        .join("avatar-packages")
        .join(&package);
    assert_eq!(
        std::fs::read(backup.join("asset-1.rgba")).unwrap(),
        b"closed"
    );
    assert!(backup.join("preview.png").exists());
    drop(c);
    let mut c = Client::open(root.path(), false, 1);
    assert!(
        c.send(C::MigrateSharedModels(migration))
            .unwrap()
            .shared_catalog_enabled
    );
    assert_eq!(std::fs::read(dir.join("asset-1.rgba")).unwrap(), b"closed");
    assert!(
        c.send(C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(70),
            revision: 0
        }))
        .unwrap()
        .body
        .contains(&package)
    );
}
#[test]
fn real_ipc_private_photo_persists_and_revokes_without_source() {
    let root = tempdir().unwrap();
    let mut c = Client::open(root.path(), true, 1);
    c.send(workspace()).unwrap();
    let jpeg = vec![
        0xff, 0xd8, 0xff, 0xc0, 0, 11, 8, 0, 2, 0, 3, 1, 1, 0x11, 0, 0xff, 0xda, 0, 8, 1, 1, 0, 0,
        63, 0, 0, 0xff, 0xd9,
    ];
    let photo = nai_avatar_files::photo::store_photo(root.path(), &jpeg).unwrap();
    let doc = |revoked| {
        serde_json::json!({"schema_version":1,"kind":"avatar_photo_draft","name":"photo","photo_sha256":photo.sha256,"width":photo.width,"height":photo.height,"revoked":revoked}).to_string()
    };
    let put = |revision, body| {
        C::PutPreparation(FoundationObject {
            workspace_id: id(10),
            object_id: id(70),
            expected_revision: revision,
            body,
        })
    };
    c.send(put(0, doc(false))).unwrap();
    let items = c
        .send(C::ListPreparation(FoundationBrowse {
            workspace_id: id(10),
            kind: "avatar_photo_draft".into(),
            limit: 32,
            ..Default::default()
        }))
        .unwrap()
        .resources;
    assert!(items.iter().any(|r| r.id == id(70)));
    drop(c);
    let mut c = Client::open(root.path(), false, 1);
    let loaded = c
        .send(C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(70),
            revision: 0,
        }))
        .unwrap();
    assert!(loaded.body.contains(&photo.sha256));
    std::fs::remove_file(
        root.path()
            .join("avatar-photos")
            .join(format!("{}.jpg", photo.sha256)),
    )
    .unwrap();
    c.send(put(1, doc(true))).unwrap();
    assert_eq!(
        c.send(put(2, doc(false))),
        Err(WorkbenchError::PermissionDenied)
    );
}
fn fixture_mp4(timescale: u32, duration: u32) -> Vec<u8> {
    fn box_of(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut v = ((8 + payload.len()) as u32).to_be_bytes().to_vec();
        v.extend(kind);
        v.extend(payload);
        v
    }
    let mut ftyp = Vec::from(*b"isom");
    ftyp.extend(0u32.to_be_bytes());
    ftyp.extend(b"isomiso2mp41");
    let mut mvhd = vec![0; 4];
    mvhd.extend([0u8; 8]);
    mvhd.extend(timescale.to_be_bytes());
    mvhd.extend(duration.to_be_bytes());
    let mut v = box_of(b"ftyp", &ftyp);
    v.extend(box_of(b"moov", &box_of(b"mvhd", &mvhd)));
    v.extend(box_of(b"mdat", b"x"));
    v
}
#[test]
fn real_ipc_private_video_persists_and_revokes_without_source() {
    let root = tempdir().unwrap();
    let mut c = Client::open(root.path(), true, 1);
    c.send(workspace()).unwrap();
    let mp4 = fixture_mp4(1000, 5000);
    let video = nai_avatar_files::video::store_video(root.path(), &mp4).unwrap();
    let doc = |revoked| {
        serde_json::json!({"schema_version":1,"kind":"avatar_video_draft","name":"video","video_sha256":video.sha256,"duration_ms":video.duration_ms,"inspect_version":"mp4-ftyp-mvhd-v1","realistic_drive":"model_missing","revoked":revoked}).to_string()
    };
    let put = |revision, body| {
        C::PutPreparation(FoundationObject {
            workspace_id: id(10),
            object_id: id(72),
            expected_revision: revision,
            body,
        })
    };
    c.send(put(0, doc(false))).unwrap();
    let items = c
        .send(C::ListPreparation(FoundationBrowse {
            workspace_id: id(10),
            kind: "avatar_video_draft".into(),
            limit: 32,
            ..Default::default()
        }))
        .unwrap()
        .resources;
    assert!(items.iter().any(|r| r.id == id(72)));
    drop(c);
    let mut c = Client::open(root.path(), false, 1);
    let loaded = c
        .send(C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(72),
            revision: 0,
        }))
        .unwrap();
    assert!(loaded.body.contains(&video.sha256));
    std::fs::remove_file(
        root.path()
            .join("avatar-videos")
            .join(format!("{}.mp4", video.sha256)),
    )
    .unwrap();
    c.send(put(1, doc(true))).unwrap();
    assert_eq!(
        c.send(put(2, doc(false))),
        Err(WorkbenchError::PermissionDenied)
    );
}
#[test]
#[ignore = "explicit user-provided local photo; isolated data root only"]
fn real_ipc_local_photo_backup_and_restart() {
    let path = std::path::PathBuf::from(
        std::env::var_os("NAI_LOCAL_PHOTO_PROBE").expect("explicit photo path"),
    );
    let parent = path.parent().unwrap();
    let filename = path.file_name().unwrap().to_str().unwrap();
    let bytes = nai_avatar_files::SelectedDirectory::open(parent)
        .unwrap()
        .read(filename, nai_avatar_files::photo::MAX_PHOTO_BYTES)
        .unwrap();
    let root = tempdir().unwrap();
    let mut c = Client::open(root.path(), true, 1);
    c.send(workspace()).unwrap();
    let photo = nai_avatar_files::photo::store_photo(root.path(), &bytes).unwrap();
    let doc=serde_json::json!({"schema_version":1,"kind":"avatar_photo_draft","name":"local photo","photo_sha256":photo.sha256,"width":photo.width,"height":photo.height,"revoked":false}).to_string();
    c.send(C::PutPreparation(FoundationObject {
        workspace_id: id(10),
        object_id: id(70),
        expected_revision: 0,
        body: doc,
    }))
    .unwrap();
    let preview = c
        .send(C::PreviewSharedModels(FoundationLlmRead::default()))
        .unwrap()
        .shared_model_preview
        .unwrap();
    let migration = SharedModelMigration {
        migration_id: id(900),
        plan_sha256: preview.plan_sha256,
        entries: vec![],
    };
    c.send(C::MigrateSharedModels(migration.clone())).unwrap();
    let status = c
        .send(C::MigrateSharedModels(migration.clone()))
        .unwrap()
        .shared_model_migration
        .unwrap();
    let backup = root
        .path()
        .join("model-migration-backups")
        .join(status.backup_id);
    assert_eq!(
        nai_avatar_files::photo::read_stored_photo(&backup, &photo.sha256)
            .unwrap()
            .0,
        bytes
    );
    drop(c);
    let mut c = Client::open(root.path(), false, 1);
    assert!(
        c.send(C::MigrateSharedModels(migration))
            .unwrap()
            .shared_catalog_enabled
    );
    let items = c
        .send(C::ListPreparation(FoundationBrowse {
            workspace_id: id(10),
            kind: "avatar_photo_draft".into(),
            limit: 32,
            ..Default::default()
        }))
        .unwrap()
        .resources;
    assert!(items.iter().any(|r| r.id == id(70)));
    assert_eq!(
        nai_avatar_files::photo::read_stored_photo(root.path(), &photo.sha256)
            .unwrap()
            .0,
        bytes
    );
}
#[cfg(windows)]
#[path = "foundation_llm.rs.inc"]
mod llm_tests;
#[test]
fn real_publication_review_revoke_and_restart_preserve_current_status() {
    use serde_json::json;
    fn save(c: &mut Client, n: u64, doc: String) -> FoundationResult {
        c.send(C::PutPreparation(FoundationObject {
            workspace_id: id(10),
            object_id: id(n),
            expected_revision: 0,
            body: doc,
        }))
        .unwrap()
    }
    let root = tempdir().unwrap();
    let mut c = Client::open(root.path(), true, 1);
    c.send(workspace()).unwrap();
    let p = save(
        &mut c,
        80,
        include_str!("../../../tests/fixtures/contracts/preparation.json").into(),
    );
    let h=save(&mut c,81,json!({"schema_version":1,"kind":"host_draft","name":"助播","style":"professional","preferences":""}).to_string());
    let series = save(
        &mut c,
        82,
        json!({"schema_version":1,"kind":"series_draft","name":"系列","goal":"产品说明"})
            .to_string(),
    );
    let pointer = |n, r: &FoundationResult| json!({"object_id":id(n),"revision":r.revision,"sha256":r.sha256});
    save(&mut c,83,json!({"schema_version":1,"kind":"show_draft","name":"首场","product":pointer(80,&p),"host":pointer(81,&h),"series":pointer(82,&series),"cues":[{"id":"intro","title":"介绍","talk_track_id":"opening","mandatory_fact_ids":["capacity"],"allow_questions":true}]}).to_string());
    let report = c
        .send(C::PublicationImpact(FoundationImpact {
            workspace_id: id(10),
            report_id: id(85),
            show_id: id(83),
            show_revision: 1,
            previous_snapshot_id: String::new(),
        }))
        .unwrap();
    let mut publish = FoundationPublish {
        workspace_id: id(10),
        snapshot_id: id(90),
        report_id: id(85),
        report_sha256: report.sha256,
        acknowledge_unknown: false,
    };
    assert_eq!(
        c.send(C::Publish(publish.clone())),
        Err(WorkbenchError::InvalidArgument)
    );
    publish.acknowledge_unknown = true;
    let snapshot = c.send(C::Publish(publish)).unwrap();
    assert_eq!(
        snapshot.publication_status,
        PublicationStatus::PrivateContent as i32
    );
    let revoked = c
        .send(C::RevokePublication(FoundationPublicationRevoke {
            workspace_id: id(10),
            revocation_id: id(95),
            snapshot_id: id(90),
            snapshot_sha256: snapshot.sha256.clone(),
            reason: "已确认错误".into(),
        }))
        .unwrap();
    assert_eq!(
        revoked.publication_status,
        PublicationStatus::Blocked as i32
    );
    drop(c);
    let mut c = Client::open(root.path(), false, 1);
    let read = c
        .send(C::ReadPublication(FoundationPublicationRead {
            workspace_id: id(10),
            snapshot_id: id(90),
        }))
        .unwrap();
    assert_eq!(read.publication_status, PublicationStatus::Blocked as i32);
    assert_eq!(read.sha256, snapshot.sha256);
    assert_eq!(read.body, snapshot.body);
}
fn id(n: u64) -> String {
    format!("00000000-0000-0000-0000-{n:012x}")
}
fn cmd(c: C) -> FoundationCommand {
    FoundationCommand {
        idempotency_key: id(KEY.fetch_add(1, Ordering::Relaxed)),
        operation: Some(c),
    }
}
fn workspace() -> C {
    C::CreateWorkspace(FoundationWorkspace {
        workspace_id: id(10),
        name: "真实进程工作区".into(),
        budget_limit_minor: 100,
        currency: "CNY".into(),
    })
}
fn object() -> C {
    C::PutObject(FoundationObject {
        workspace_id: id(10),
        object_id: id(20),
        expected_revision: 0,
        body: "人工备播文本，无语音与付费调用".into(),
    })
}
fn session() -> C {
    C::CreateSession(FoundationSession {
        workspace_id: id(10),
        session_id: id(30),
    })
}
fn delivery(sha: &str) -> C {
    C::PrepareDelivery(FoundationDelivery {
        workspace_id: id(10),
        delivery_id: id(40),
        session_id: id(30),
        object_id: id(20),
        object_revision: 1,
        object_sha256: sha.into(),
        target: FoundationTarget::PrivateCard as i32,
        scope: DeliveryScope::Private as i32,
        ttl_ms: 30000,
    })
}
fn child(root: &Path, initialize: bool, actor: u64) -> Child {
    child_mode(root, initialize, actor, false)
}
fn child_mode(root: &Path, initialize: bool, actor: u64, desktop: bool) -> Child {
    let mut c = Command::new(env!("CARGO_BIN_EXE_nai-foundation-engine"));
    c.arg("--data-root").arg(root);
    if desktop {
        c.arg("--desktop-owner");
    }
    if initialize {
        c.arg("--initialize");
    }
    c.env("NAI_WORKBENCH_TOKEN", "a".repeat(64))
        .env("NAI_LLM_TEST_LOOPBACK", "1")
        .env("NAI_FOUNDATION_PRINCIPAL", id(actor))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}
struct Client {
    child: Child,
    input: Option<ChildStdin>,
    out: mpsc::Receiver<Result<FoundationResponse, WorkbenchError>>,
    seq: u64,
}
impl Drop for Client {
    fn drop(&mut self) {
        self.input.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Client {
    fn open(root: &Path, initialize: bool, actor: u64) -> Self {
        Self::open_mode(root, initialize, actor, false)
    }
    fn open_mode(root: &Path, initialize: bool, actor: u64, desktop: bool) -> Self {
        let mut child = child_mode(root, initialize, actor, desktop);
        let input = child.stdin.take().unwrap();
        let mut output = child.stdout.take().unwrap();
        let (tx, out) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            loop {
                let r = wire::read(&mut output);
                let failed = r.is_err();
                if tx.send(r).is_err() || failed {
                    break;
                }
            }
        });
        Self {
            child,
            input: Some(input),
            out,
            seq: 0,
        }
    }
    fn request(&mut self, command: FoundationCommand) -> FoundationRequest {
        self.seq += 1;
        FoundationRequest {
            protocol_major: 2,
            authorization: vec![b'a'; 64],
            request_id: id(7000 + self.seq),
            seq: self.seq,
            ttl_ms: 4000,
            command: Some(command),
        }
    }
    fn raw(&mut self, r: &FoundationRequest) -> FoundationResponse {
        if let Err(error) = wire::write(self.input.as_mut().unwrap(), r) {
            let _ = self.child.kill();
            let status = self.child.wait();
            let mut diagnostic = String::new();
            if let Some(stderr) = self.child.stderr.take() {
                let _ = stderr.take(4096).read_to_string(&mut diagnostic);
            }
            panic!("test IPC write {error:?}; child {status:?}; {diagnostic}");
        }
        match self.out.recv_timeout(Duration::from_secs(6)) {
            Ok(Ok(response)) => response,
            failure => {
                let _ = self.child.kill();
                let status = self.child.wait();
                let mut diagnostic = String::new();
                if let Some(stderr) = self.child.stderr.take() {
                    let _ = stderr.take(4096).read_to_string(&mut diagnostic);
                }
                panic!(
                    "isolated fixture engine response failed: seq={} failure={failure:?} status={status:?} stderr={diagnostic}",
                    r.seq
                );
            }
        }
    }
    fn send(&mut self, c: C) -> Result<FoundationResult, WorkbenchError> {
        let r = self.request(cmd(c));
        let out = self.raw(&r);
        assert_eq!(out.request_id, r.request_id);
        assert_eq!(out.seq, r.seq);
        match out.outcome.unwrap() {
            foundation_response::Outcome::Result(r) => Ok(r),
            foundation_response::Outcome::Problem(p) => {
                Err(WorkbenchError::try_from(p.code).unwrap())
            }
        }
    }
}
#[test]
fn real_preparation_command_survives_restart_and_refuses_draft_dispatch() {
    let root = tempdir().unwrap();
    let text = include_str!("../../../tests/fixtures/contracts/preparation.json");
    let mut c = Client::open(root.path(), true, 1);
    c.send(workspace()).unwrap();
    let payload = FoundationObject {
        workspace_id: id(10),
        object_id: id(20),
        expected_revision: 0,
        body: text.into(),
    };
    assert_eq!(
        c.send(C::PutObject(payload.clone())),
        Err(WorkbenchError::InvalidArgument)
    );
    let saved = c.send(C::PutPreparation(payload.clone())).unwrap();
    assert_eq!(
        c.send(C::PutPreparation(payload)),
        Err(WorkbenchError::RevisionConflict)
    );
    c.send(session()).unwrap();
    assert_eq!(
        c.send(delivery(&saved.sha256)),
        Err(WorkbenchError::CapabilityUnavailable)
    );
    drop(c);
    let mut c = Client::open(root.path(), false, 1);
    let loaded = c
        .send(C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(20),
            revision: 0,
        }))
        .unwrap();
    assert_eq!(loaded.body, text);
    assert_eq!(loaded.sha256, saved.sha256);
    assert_eq!(loaded.revision, 1);
}
#[test]
fn real_owner_persists_and_gates_private_text_without_fabricated_completion() {
    let root = tempdir().unwrap();
    let mut c = Client::open(root.path(), true, 1);
    c.send(workspace()).unwrap();
    let o = c.send(object()).unwrap();
    c.send(session()).unwrap();
    let d = c.send(delivery(&o.sha256)).unwrap();
    c.send(C::Approve(FoundationApprove {
        workspace_id: id(10),
        delivery_id: id(40),
        bundle_sha256: d.sha256,
    }))
    .unwrap();
    let card = c
        .send(C::Dispatch(FoundationDispatch {
            workspace_id: id(10),
            delivery_id: id(40),
        }))
        .unwrap();
    assert_eq!(card.state, DeliveryState::Delivering as i32);
    assert_eq!(card.evidence, EvidenceKind::Submitted as i32);
    assert_eq!(card.body, "人工备播文本，无语音与付费调用");
    drop(c);
    let mut c = Client::open(root.path(), false, 1);
    let read = c
        .send(C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(20),
            revision: 1,
        }))
        .unwrap();
    assert_eq!(read.sha256, o.sha256);
    assert_eq!(read.body, card.body);
    assert_eq!(c.send(delivery(&o.sha256)), Err(WorkbenchError::Cancelled));
    let historical = c
        .send(C::ReadDelivery(FoundationRead {
            workspace_id: id(10),
            delivery_id: id(40),
        }))
        .unwrap();
    assert_eq!(historical.evidence, EvidenceKind::Submitted as i32);
    assert_ne!(historical.state, DeliveryState::Completed as i32);
}
#[test]
fn authentication_protocol_and_sequence_are_rejected() {
    for fault in 0..3 {
        let root = tempdir().unwrap();
        let mut c = Client::open(root.path(), true, 1);
        let mut r = c.request(cmd(workspace()));
        let expected = match fault {
            0 => {
                r.authorization = vec![b'b'; 64];
                WorkbenchError::AuthenticationFailed
            }
            1 => {
                r.protocol_major = 1;
                WorkbenchError::ProtocolIncompatible
            }
            _ => {
                r.seq = 2;
                WorkbenchError::ProtocolError
            }
        };
        let out = c.raw(&r);
        assert!(out.request_id.is_empty());
        let Some(foundation_response::Outcome::Problem(p)) = out.outcome else {
            panic!("must refuse")
        };
        assert_eq!(p.code, expected as i32);
    }
}
#[test]
fn authenticated_nonmember_cannot_read_stored_workspace() {
    let root = tempdir().unwrap();
    let mut owner = Client::open(root.path(), true, 1);
    owner.send(workspace()).unwrap();
    owner.send(object()).unwrap();
    drop(owner);
    let mut outsider = Client::open(root.path(), false, 2);
    assert_eq!(
        outsider.send(C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(20),
            revision: 1
        })),
        Err(WorkbenchError::PermissionDenied)
    );
}
#[test]
fn two_real_processes_cannot_hold_the_writer() {
    let root = tempdir().unwrap();
    let mut owner = Client::open(root.path(), true, 1);
    owner.send(workspace()).unwrap();
    let mut second = child(root.path(), false, 1);
    second.stdin.take();
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let o = second.wait_with_output().unwrap();
        tx.send(o).unwrap();
    });
    let output = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap().trim(),
        "WORKBENCH_ERROR_OWNER_BUSY"
    );
}
#[test]
fn priority_stop_file_revokes_without_database_round_trip() {
    let root = tempdir().unwrap();
    let mut c = Client::open(root.path(), true, 1);
    c.send(workspace()).unwrap();
    let o = c.send(object()).unwrap();
    c.send(session()).unwrap();
    let d = c.send(delivery(&o.sha256)).unwrap();
    std::fs::write(
        root.path().join("stop.request"),
        b"explicit root-parent stop",
    )
    .unwrap();
    let end = std::time::Instant::now() + Duration::from_secs(1);
    loop {
        let result = c.send(C::Approve(FoundationApprove {
            workspace_id: id(10),
            delivery_id: id(40),
            bundle_sha256: d.sha256.clone(),
        }));
        if result == Err(WorkbenchError::Cancelled) {
            break;
        }
        assert!(
            std::time::Instant::now() < end,
            "priority stop was not observed"
        );
        std::thread::yield_now();
    }
    assert_eq!(
        c.send(C::Dispatch(FoundationDispatch {
            workspace_id: id(10),
            delivery_id: id(40)
        })),
        Err(WorkbenchError::Cancelled)
    );
}
#[test]
#[ignore = "explicit test-only crash worker; parent invokes with --ignored"]
fn crash_boundary_worker() {
    let root = std::env::var_os("NAI_RF02_CRASH_ROOT").expect("explicit test parent required");
    let mut s = Store::open(Path::new(&root), &id(1), true, Arc::default()).unwrap();
    let op = Operation::new(4000).unwrap();
    s.execute(&op, &id(1), &cmd(workspace())).unwrap();
    let o = s.execute(&op, &id(1), &cmd(object())).unwrap();
    s.execute(&op, &id(1), &cmd(session())).unwrap();
    let d = s.execute(&op, &id(1), &cmd(delivery(&o.sha256))).unwrap();
    s.execute(
        &op,
        &id(1),
        &cmd(C::Approve(FoundationApprove {
            workspace_id: id(10),
            delivery_id: id(40),
            bundle_sha256: d.sha256,
        })),
    )
    .unwrap();
    s.execute(
        &op,
        &id(1),
        &cmd(C::Dispatch(FoundationDispatch {
            workspace_id: id(10),
            delivery_id: id(40),
        })),
    )
    .unwrap();
    println!("RF02_DISPATCH_INTENT_DURABLE");
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::park();
    }
}
#[test]
fn killed_dispatch_owner_recovers_unknown_and_never_resends() {
    let root = tempdir().unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "crash_boundary_worker",
            "--ignored",
            "--nocapture",
        ])
        .env("NAI_RF02_CRASH_ROOT", root.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let output = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        for line in BufReader::new(output).lines() {
            if line.unwrap().contains("RF02_DISPATCH_INTENT_DURABLE") {
                let _ = tx.send(());
                break;
            }
        }
    });
    let witness = rx.recv_timeout(Duration::from_secs(8));
    if witness.is_err() {
        let _ = child.kill();
        let _ = child.wait();
        panic!("crash worker never reached durable intent");
    }
    child.kill().unwrap();
    child.wait().unwrap();
    let mut recovered = Client::open(root.path(), false, 1);
    let d = recovered
        .send(C::ReadDelivery(FoundationRead {
            workspace_id: id(10),
            delivery_id: id(40),
        }))
        .unwrap();
    assert_eq!(d.state, DeliveryState::OutcomeUnknown as i32);
    assert!(d.body.is_empty());
    assert_eq!(d.evidence, 0);
    assert_eq!(
        recovered.send(C::Dispatch(FoundationDispatch {
            workspace_id: id(10),
            delivery_id: id(40)
        })),
        Err(WorkbenchError::OutcomeUnknown)
    );
}

#[test]
fn oversized_truncated_and_malformed_frames_are_typed_refusals() {
    for (bytes, expected) in [
        (65537u32.to_le_bytes().to_vec(), WorkbenchError::FrameLimit),
        (vec![1, 0], WorkbenchError::Truncated),
        (vec![1, 0, 0, 0, 255], WorkbenchError::ProtocolError),
    ] {
        let root = tempdir().unwrap();
        let mut c = Client::open(root.path(), true, 1);
        let mut input = c.input.take().unwrap();
        input.write_all(&bytes).unwrap();
        drop(input);
        let response = c.out.recv_timeout(Duration::from_secs(6)).unwrap().unwrap();
        let Some(foundation_response::Outcome::Problem(p)) = response.outcome else {
            panic!("refusal required")
        };
        assert_eq!(p.code, expected as i32);
        assert_eq!(response.seq, 0);
        assert!(response.request_id.is_empty());
    }
}

#[test]
fn blocked_reply_pipe_has_a_bounded_exit() {
    let root = tempdir().unwrap();
    let mut store = Store::open(root.path(), &id(1), true, Arc::default()).unwrap();
    store
        .execute(&Operation::new(4000).unwrap(), &id(1), &cmd(workspace()))
        .unwrap();
    let mut payload = object();
    if let C::PutObject(ref mut x) = payload {
        x.body = "X".repeat(8192);
    }
    store
        .execute(&Operation::new(4000).unwrap(), &id(1), &cmd(payload))
        .unwrap();
    drop(store);
    let mut process = child(root.path(), false, 1);
    let mut input = process.stdin.take().unwrap();
    for seq in 1..=16 {
        wire::write(
            &mut input,
            &FoundationRequest {
                protocol_major: 2,
                authorization: vec![b'a'; 64],
                request_id: id(9000 + seq),
                seq,
                ttl_ms: 4000,
                command: Some(cmd(C::ReadObject(FoundationReadObject {
                    workspace_id: id(10),
                    object_id: id(20),
                    revision: 1,
                }))),
            },
        )
        .unwrap();
    }
    drop(input);
    let limit = std::time::Instant::now() + Duration::from_secs(7);
    loop {
        if let Some(status) = process.try_wait().unwrap() {
            assert!(!status.success());
            break;
        }
        if std::time::Instant::now() > limit {
            process.kill().unwrap();
            process.wait().unwrap();
            panic!("blocked stdout exceeded process deadline");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut error = String::new();
    process
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut error)
        .unwrap();
    assert_eq!(error.trim(), "WORKBENCH_ERROR_DEADLINE_EXCEEDED");
}
