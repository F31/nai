use super::*;
use foundation_command::Operation as C;
use nai_runtime_core::foundation::{BudgetPort, Settlement};
use std::sync::atomic::{AtomicU64, Ordering};
use tempfile::{TempDir, tempdir};
mod migration_backup_tests;
mod model_catalog_tests;
mod personal_policy_migration_tests;
mod publication_tests;
mod shared_catalog_tests;
mod shared_model_preview_tests;
mod workspace_budget_tests;
mod workspace_purge_tests;
mod workspace_removal_tests;
#[test]
fn workspace_management_preserves_creation_and_enforces_admin_cas() {
    let (t, mut s, _) = setup();
    let browse = || {
        C::ListWorkspaces(FoundationBrowse {
            limit: 32,
            ..Default::default()
        })
    };
    let before = exec(&mut s, 1, browse()).unwrap().resources.remove(0);
    assert!(before.created_at_ms > 0);
    let rename = |revision| {
        C::RenameWorkspace(FoundationWorkspaceRename {
            workspace_id: id(10),
            name: "新直播间".into(),
            expected_revision: revision,
        })
    };
    exec(
        &mut s,
        1,
        C::Grant(FoundationGrant {
            workspace_id: id(10),
            principal_id: id(2),
            role: FoundationRole::Operator as i32,
            expected_revision: 0,
        }),
    )
    .unwrap();
    assert_eq!(
        exec(&mut s, 2, rename(1)).unwrap_err(),
        Error::PermissionDenied
    );
    exec(&mut s, 1, rename(1)).unwrap();
    assert_eq!(
        exec(&mut s, 1, rename(1)).unwrap_err(),
        Error::RevisionConflict
    );
    let invalid = C::RenameWorkspace(FoundationWorkspaceRename {
        workspace_id: id(10),
        name: " \n ".into(),
        expected_revision: 2,
    });
    assert_eq!(
        exec(&mut s, 1, invalid).unwrap_err(),
        Error::InvalidArgument
    );
    let after = exec(&mut s, 1, browse()).unwrap().resources.remove(0);
    assert_eq!(after.created_at_ms, before.created_at_ms);
    assert_eq!(after.name, "新直播间");
    assert_eq!(after.revision, 2);
    assert!(after.updated_at_ms >= before.updated_at_ms);
    let count: i64 = s
        .conn
        .query_row(
            "SELECT count(*) FROM audit_events WHERE action='rename_workspace'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
    drop(s);
    let mut reopened = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    assert_eq!(
        exec(&mut reopened, 1, browse()).unwrap().resources[0].name,
        "新直播间"
    );
    assert!(
        exec(
            &mut reopened,
            1,
            C::ReadObject(FoundationReadObject {
                workspace_id: id(10),
                object_id: id(20),
                revision: 0
            })
        )
        .is_ok()
    );
}
#[test]
fn format_two_is_explicit_and_old_roots_are_rejected_without_writing_any_file() {
    let legacy = tempdir().unwrap();
    let marker=br#"{"kind":"nai-foundation-v1","schema_version":1,"owner":"00000000-0000-0000-0000-000000000001"}"#;
    std::fs::write(legacy.path().join("foundation-root.json"), marker).unwrap();
    std::fs::write(
        legacy.path().join("foundation.sqlite"),
        b"old database bytes",
    )
    .unwrap();
    assert!(matches!(
        Store::open(legacy.path(), &id(1), false, Arc::default()),
        Err(Error::DataRootRejected)
    ));
    assert_eq!(
        std::fs::read(legacy.path().join("foundation-root.json")).unwrap(),
        marker
    );
    assert_eq!(
        std::fs::read(legacy.path().join("foundation.sqlite")).unwrap(),
        b"old database bytes"
    );
    assert_eq!(std::fs::read_dir(legacy.path()).unwrap().count(), 2);
    let (_t, s, _) = setup();
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(s.root.join("foundation-root.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["kind"], "nai-foundation-v2");
    assert_eq!(manifest["schema_version"], 2);
    assert_eq!(
        s.conn
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        std::fs::read(s.root.join("initialized.ok")).unwrap(),
        b"nai-foundation-schema-2"
    );
}
static NEXT: AtomicU64 = AtomicU64::new(1000);
const PRODUCT_DRAFT: &str = include_str!("../../../tests/fixtures/contracts/preparation.json");

fn draft(body: &str, revision: u64) -> C {
    C::PutPreparation(FoundationObject {
        workspace_id: id(10),
        object_id: id(70),
        expected_revision: revision,
        body: body.into(),
    })
}
#[test]
fn avatar_draft_selection_is_atomic_and_revoked_identity_cannot_reactivate() {
    let (_temp, mut store, _) = setup();
    let make = |name: &str, selected: bool, revoked: bool| {
        serde_json::json!({
            "schema_version":1,"kind":"avatar_profile_draft","name":name,"avatar_id":name,
            "package_version":"1","package_sha256":"a".repeat(64),"license_sha256":"b".repeat(64),
            "width":2,"height":1,"selected":selected,"revoked":revoked
        })
        .to_string()
    };
    let put = |number, revision, body: String| {
        C::PutPreparation(FoundationObject {
            workspace_id: id(10),
            object_id: id(number),
            expected_revision: revision,
            body,
        })
    };
    exec(&mut store, 1, put(70, 0, make("one", true, false))).unwrap();
    exec(&mut store, 1, put(71, 0, make("two", false, false))).unwrap();
    store.conn.execute_batch("CREATE TRIGGER fail_avatar_audit BEFORE INSERT ON audit_events WHEN NEW.action='avatar_draft_deselected' BEGIN SELECT RAISE(ABORT, 'audit'); END;").unwrap();
    assert!(exec(&mut store, 1, put(71, 1, make("two", true, false))).is_err());
    let read = |number| {
        C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(number),
            revision: 0,
        })
    };
    assert!(
        exec(&mut store, 1, read(70))
            .unwrap()
            .body
            .contains("\"selected\":true")
    );
    assert_eq!(exec(&mut store, 1, read(71)).unwrap().revision, 1);
    store
        .conn
        .execute_batch("DROP TRIGGER fail_avatar_audit")
        .unwrap();
    exec(&mut store, 1, put(71, 1, make("two", true, false))).unwrap();
    let old = exec(&mut store, 1, read(70)).unwrap();
    assert_eq!(old.revision, 2);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&old.body).unwrap()["selected"],
        false
    );
    exec(&mut store, 1, put(71, 2, make("two", false, true))).unwrap();
    assert_eq!(
        exec(&mut store, 1, put(71, 3, make("two", true, false))),
        Err(Error::PermissionDenied)
    );
    assert_eq!(
        exec(&mut store, 1, put(70, 1, make("one", true, false))),
        Err(Error::RevisionConflict)
    );
}
#[test]
fn preparation_persists_exact_versions_and_refuses_stale_or_untyped_overwrite() {
    let (t, mut s, _) = setup();
    let command = command(draft(PRODUCT_DRAFT, 0));
    let first = s
        .execute(&Operation::new(4000).unwrap(), &id(1), &command)
        .unwrap();
    assert_eq!(
        s.execute(&Operation::new(4000).unwrap(), &id(1), &command)
            .unwrap(),
        first
    );
    assert_eq!(
        exec(&mut s, 1, draft(PRODUCT_DRAFT, 0)),
        Err(Error::RevisionConflict)
    );
    let mut modified: serde_json::Value = serde_json::from_str(PRODUCT_DRAFT).unwrap();
    modified["name"] = "新标题".into();
    assert_eq!(
        exec(&mut s, 1, draft(&modified.to_string(), 1))
            .unwrap()
            .revision,
        2
    );
    assert_eq!(
        exec(
            &mut s,
            1,
            C::PutObject(FoundationObject {
                workspace_id: id(10),
                object_id: id(70),
                expected_revision: 2,
                body: "抹除类型".into()
            })
        ),
        Err(Error::RevisionConflict)
    );
    let host = r#"{"schema_version":1,"kind":"host_draft","name":"主持","style":"friendly","preferences":""}"#;
    assert_eq!(
        exec(&mut s, 1, draft(host, 2)),
        Err(Error::RevisionConflict)
    );
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    let original = exec(
        &mut s,
        1,
        C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(70),
            revision: 1,
        }),
    )
    .unwrap();
    assert_eq!(original.body, PRODUCT_DRAFT);
    assert_eq!(original.sha256, first.sha256);
    let latest = exec(
        &mut s,
        1,
        C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(70),
            revision: 0,
        }),
    )
    .unwrap();
    assert_eq!(latest.revision, 2);
    assert_eq!(latest.body, modified.to_string());
}
#[test]
fn preparation_failure_has_no_write_and_draft_cannot_be_delivered() {
    let (_temp, mut s, _) = setup();
    let before: i64 = s
        .conn
        .query_row("SELECT COUNT(*) FROM audit_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        exec(&mut s, 2, draft(PRODUCT_DRAFT, 0)),
        Err(Error::PermissionDenied)
    );
    assert_eq!(exec(&mut s, 1, draft("{}", 0)), Err(Error::InvalidArgument));
    assert_eq!(
        exec(
            &mut s,
            1,
            C::PutObject(FoundationObject {
                workspace_id: id(10),
                object_id: id(70),
                expected_revision: 0,
                body: PRODUCT_DRAFT.into()
            })
        ),
        Err(Error::InvalidArgument)
    );
    let after: i64 = s
        .conn
        .query_row("SELECT COUNT(*) FROM audit_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(before, after);
    let saved = exec(&mut s, 1, draft(PRODUCT_DRAFT, 0)).unwrap();
    assert_eq!(
        exec(
            &mut s,
            1,
            C::PrepareDelivery(FoundationDelivery {
                workspace_id: id(10),
                delivery_id: id(71),
                session_id: id(30),
                object_id: id(70),
                object_revision: 1,
                object_sha256: saved.sha256,
                target: FoundationTarget::PrivateCard as i32,
                scope: DeliveryScope::Private as i32,
                ttl_ms: 30000
            })
        ),
        Err(Error::CapabilityUnavailable)
    );
    let count: i64 = s
        .conn
        .query_row("SELECT COUNT(*) FROM deliveries", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}
fn id(n: u64) -> String {
    format!("00000000-0000-0000-0000-{n:012x}")
}
fn command(c: C) -> FoundationCommand {
    FoundationCommand {
        idempotency_key: id(NEXT.fetch_add(1, Ordering::Relaxed)),
        operation: Some(c),
    }
}
fn exec(s: &mut Store, actor: u64, c: C) -> Result<FoundationResult> {
    s.execute(&Operation::new(4000)?, &id(actor), &command(c))
}
fn setup() -> (TempDir, Store, String) {
    let temp = tempdir().unwrap();
    let mut s = Store::open(temp.path(), &id(1), true, Arc::default()).unwrap();
    exec(
        &mut s,
        1,
        C::CreateWorkspace(FoundationWorkspace {
            workspace_id: id(10),
            name: "独立工作区".into(),
            budget_limit_minor: 100,
            currency: "CNY".into(),
        }),
    )
    .unwrap();
    let o = exec(
        &mut s,
        1,
        C::PutObject(FoundationObject {
            workspace_id: id(10),
            object_id: id(20),
            expected_revision: 0,
            body: "人工编写，仅本地私有展示".into(),
        }),
    )
    .unwrap();
    exec(
        &mut s,
        1,
        C::CreateSession(FoundationSession {
            workspace_id: id(10),
            session_id: id(30),
        }),
    )
    .unwrap();
    (temp, s, o.sha256)
}
fn prepare(s: &mut Store, d: u64, sha: &str) -> FoundationResult {
    exec(
        s,
        1,
        C::PrepareDelivery(FoundationDelivery {
            workspace_id: id(10),
            delivery_id: id(d),
            session_id: id(30),
            object_id: id(20),
            object_revision: 1,
            object_sha256: sha.into(),
            target: FoundationTarget::PrivateCard as i32,
            scope: DeliveryScope::Private as i32,
            ttl_ms: 30000,
        }),
    )
    .unwrap()
}
fn approve(s: &mut Store, d: u64, bundle: &str) -> FoundationResult {
    exec(
        s,
        1,
        C::Approve(FoundationApprove {
            workspace_id: id(10),
            delivery_id: id(d),
            bundle_sha256: bundle.into(),
        }),
    )
    .unwrap()
}
fn dispatch(s: &mut Store, d: u64) -> Result<FoundationResult> {
    exec(
        s,
        1,
        C::Dispatch(FoundationDispatch {
            workspace_id: id(10),
            delivery_id: id(d),
        }),
    )
}
#[test]
fn sole_writer_and_legacy_root_are_rejected_without_changes() {
    let (t, s, _) = setup();
    assert!(matches!(
        Store::open(t.path(), &id(1), false, Arc::default()),
        Err(Error::OwnerBusy)
    ));
    drop(s);
    Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    let old = tempdir().unwrap();
    std::fs::write(old.path().join("old.sqlite"), b"old bytes").unwrap();
    assert!(matches!(
        Store::open(old.path(), &id(1), true, Arc::default()),
        Err(Error::DataRootRejected)
    ));
    assert_eq!(
        std::fs::read(old.path().join("old.sqlite")).unwrap(),
        b"old bytes"
    );
    assert_eq!(std::fs::read_dir(old.path()).unwrap().count(), 1);
}
#[test]
fn workspace_acl_revocation_and_replay_do_not_leak() {
    let (_t, mut s, _) = setup();
    assert_eq!(
        exec(
            &mut s,
            2,
            C::ReadObject(FoundationReadObject {
                workspace_id: id(10),
                object_id: id(20),
                revision: 0
            })
        ),
        Err(Error::PermissionDenied)
    );
    exec(
        &mut s,
        1,
        C::Grant(FoundationGrant {
            workspace_id: id(10),
            principal_id: id(2),
            role: FoundationRole::Operator as i32,
            expected_revision: 0,
        }),
    )
    .unwrap();
    let cmd = command(C::PutObject(FoundationObject {
        workspace_id: id(10),
        object_id: id(21),
        expected_revision: 0,
        body: "非公开资料".into(),
    }));
    s.execute(&Operation::new(4000).unwrap(), &id(2), &cmd)
        .unwrap();
    exec(
        &mut s,
        1,
        C::Grant(FoundationGrant {
            workspace_id: id(10),
            principal_id: id(2),
            role: FoundationRole::None as i32,
            expected_revision: 1,
        }),
    )
    .unwrap();
    assert_eq!(
        s.execute(&Operation::new(4000).unwrap(), &id(2), &cmd),
        Err(Error::PermissionDenied)
    );
    assert_eq!(
        exec(
            &mut s,
            1,
            C::ReadObject(FoundationReadObject {
                workspace_id: id(99),
                object_id: id(20),
                revision: 0
            })
        ),
        Err(Error::PermissionDenied)
    );
}
#[test]
fn concurrent_cas_has_one_winner_and_keeps_original_version() {
    let (_t, s, old) = setup();
    let shared = Arc::new(std::sync::Mutex::new(s));
    let barrier = Arc::new(std::sync::Barrier::new(3));
    let mut threads = vec![];
    for text in ["新版本A", "新版本B"] {
        let shared = shared.clone();
        let barrier = barrier.clone();
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            exec(
                &mut shared.lock().unwrap(),
                1,
                C::PutObject(FoundationObject {
                    workspace_id: id(10),
                    object_id: id(20),
                    expected_revision: 1,
                    body: text.into(),
                }),
            )
        }));
    }
    barrier.wait();
    let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| **r == Err(Error::RevisionConflict))
            .count(),
        1
    );
    let mut s = shared.lock().unwrap();
    let original = exec(
        &mut s,
        1,
        C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(20),
            revision: 1,
        }),
    )
    .unwrap();
    assert_eq!(original.sha256, old);
    assert_eq!(original.body, "人工编写，仅本地私有展示");
}
#[test]
fn failed_critical_audit_rolls_back_version_and_command_result() {
    let (_t, mut s, _) = setup();
    s.conn.execute_batch("CREATE TRIGGER fail_audit BEFORE INSERT ON audit_events BEGIN SELECT RAISE(ABORT,'injected audit storage failure'); END;").unwrap();
    let count: i64 = s
        .conn
        .query_row("SELECT COUNT(*) FROM command_results", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        exec(
            &mut s,
            1,
            C::PutObject(FoundationObject {
                workspace_id: id(10),
                object_id: id(22),
                expected_revision: 0,
                body: "不会提交".into()
            })
        ),
        Err(Error::IoFailed)
    );
    assert_eq!(
        exec(
            &mut s,
            1,
            C::ReadObject(FoundationReadObject {
                workspace_id: id(10),
                object_id: id(22),
                revision: 0
            })
        ),
        Err(Error::NotFound)
    );
    assert_eq!(
        s.conn
            .query_row("SELECT COUNT(*) FROM command_results", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        count
    );
}
#[test]
fn idempotency_conflict_and_object_tampering_are_detected() {
    let (t, mut s, sha) = setup();
    let mut cmd = command(C::PutObject(FoundationObject {
        workspace_id: id(10),
        object_id: id(23),
        expected_revision: 0,
        body: "一次写入".into(),
    }));
    let first = s
        .execute(&Operation::new(4000).unwrap(), &id(1), &cmd)
        .unwrap();
    assert_eq!(
        s.execute(&Operation::new(4000).unwrap(), &id(1), &cmd)
            .unwrap(),
        first
    );
    if let Some(C::PutObject(ref mut x)) = cmd.operation {
        x.body = "不同正文".into();
    }
    assert_eq!(
        s.execute(&Operation::new(4000).unwrap(), &id(1), &cmd),
        Err(Error::RevisionConflict)
    );
    std::fs::write(t.path().join("objects").join(sha), b"changed").unwrap();
    assert_eq!(
        exec(
            &mut s,
            1,
            C::ReadObject(FoundationReadObject {
                workspace_id: id(10),
                object_id: id(20),
                revision: 0
            })
        ),
        Err(Error::IntegrityFailed)
    );
}
#[test]
fn exact_approval_and_target_gate_reject_changes() {
    let (_t, mut s, sha) = setup();
    let prepared = prepare(&mut s, 40, &sha);
    assert!(dispatch(&mut s, 40).is_err());
    assert_eq!(
        exec(
            &mut s,
            1,
            C::Approve(FoundationApprove {
                workspace_id: id(10),
                delivery_id: id(40),
                bundle_sha256: "0".repeat(64)
            })
        ),
        Err(Error::IntegrityFailed)
    );
    approve(&mut s, 40, &prepared.sha256);
    approve(&mut s, 40, &prepared.sha256);
    let votes: i64 = s
        .conn
        .query_row("SELECT COUNT(*) FROM approvals", [], |r| r.get(0))
        .unwrap();
    assert_eq!(votes, 1);
    exec(
        &mut s,
        1,
        C::PutObject(FoundationObject {
            workspace_id: id(10),
            object_id: id(20),
            expected_revision: 1,
            body: "事实版本已变化".into(),
        }),
    )
    .unwrap();
    assert_eq!(dispatch(&mut s, 40), Err(Error::RevisionConflict));
    assert_eq!(
        s.conn
            .query_row("SELECT COUNT(*) FROM private_cards", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn queue_and_private_scope_are_bounded() {
    let (_t, mut s, sha) = setup();
    for d in 40..56 {
        prepare(&mut s, d, &sha);
    }
    let x = FoundationDelivery {
        workspace_id: id(10),
        delivery_id: id(56),
        session_id: id(30),
        object_id: id(20),
        object_revision: 1,
        object_sha256: sha,
        target: FoundationTarget::PrivateCard as i32,
        scope: DeliveryScope::Private as i32,
        ttl_ms: 30000,
    };
    assert_eq!(
        exec(&mut s, 1, C::PrepareDelivery(x.clone())),
        Err(Error::ResourceExhausted)
    );
    assert_eq!(
        exec(
            &mut s,
            1,
            C::PrepareDelivery(FoundationDelivery {
                scope: DeliveryScope::Public as i32,
                ..x
            })
        ),
        Err(Error::CapabilityUnavailable)
    );
}
#[test]
fn expired_and_stopped_permits_cannot_send_even_when_database_fails() {
    let (_t, mut s, sha) = setup();
    let d = prepare(&mut s, 40, &sha);
    approve(&mut s, 40, &d.sha256);
    s.origin = Instant::now() - std::time::Duration::from_secs(31);
    assert_eq!(dispatch(&mut s, 40), Err(Error::DeadlineExceeded));
    s.conn.execute_batch("BEGIN EXCLUSIVE;").unwrap();
    assert!(
        exec(
            &mut s,
            1,
            C::Stop(FoundationStop {
                workspace_id: id(10),
                session_id: id(30)
            })
        )
        .is_err()
    );
    assert_eq!(s.fence.check(s.fence.generation()), Err(Error::Cancelled));
    s.conn.execute_batch("ROLLBACK;").unwrap();
    assert_eq!(dispatch(&mut s, 40), Err(Error::Cancelled));
}
#[test]
fn projection_is_submitted_not_completed_and_stop_does_not_claim_recall() {
    let (_t, mut s, sha) = setup();
    let d = prepare(&mut s, 40, &sha);
    approve(&mut s, 40, &d.sha256);
    assert_eq!(
        dispatch(&mut s, 40).unwrap().state,
        DeliveryState::Committed as i32
    );
    let card = s
        .project_private_card(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(40))
        .unwrap();
    assert_eq!(card.state, DeliveryState::Delivering as i32);
    assert_eq!(card.evidence, EvidenceKind::Submitted as i32);
    assert_eq!(card.body, "人工编写，仅本地私有展示");
    let stop = exec(
        &mut s,
        1,
        C::Stop(FoundationStop {
            workspace_id: id(10),
            session_id: id(30),
        }),
    )
    .unwrap();
    assert_eq!(stop.already_submitted, 1);
    assert_eq!(stop.cancelled_pending, 0);
    assert_ne!(card.state, DeliveryState::Completed as i32);
}
#[test]
fn restart_preserves_uncertainty_and_never_restores_authorization() {
    let (t, mut s, sha) = setup();
    let a = prepare(&mut s, 40, &sha);
    approve(&mut s, 40, &a.sha256);
    dispatch(&mut s, 40).unwrap();
    let b = prepare(&mut s, 41, &sha);
    approve(&mut s, 41, &b.sha256);
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    assert_eq!(dispatch(&mut s, 40), Err(Error::OutcomeUnknown));
    let a = exec(
        &mut s,
        1,
        C::ReadDelivery(FoundationRead {
            workspace_id: id(10),
            delivery_id: id(40),
        }),
    )
    .unwrap();
    assert_eq!(a.state, DeliveryState::OutcomeUnknown as i32);
    assert!(a.body.is_empty());
    let b = exec(
        &mut s,
        1,
        C::ReadDelivery(FoundationRead {
            workspace_id: id(10),
            delivery_id: id(41),
        }),
    )
    .unwrap();
    assert_eq!(b.state, DeliveryState::Cancelled as i32);
    assert_eq!(
        s.conn
            .query_row("SELECT COUNT(*) FROM approvals", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        s.conn
            .query_row("SELECT SUM(attempts) FROM outbox", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}
#[test]
fn unknown_budget_is_not_zero_and_cannot_be_cancelled_away() {
    let (_t, mut s, _) = setup();
    let op = Operation::new(4000).unwrap();
    s.reserve(&op, &id(1), &id(10), &id(60), "CNY", 80).unwrap();
    s.settle(&op, &id(1), &id(10), &id(60), Settlement::Unknown)
        .unwrap();
    assert_eq!(
        s.reserve(&op, &id(1), &id(10), &id(61), "CNY", 30),
        Err(Error::ResourceExhausted)
    );
    assert_eq!(
        s.settle(&op, &id(1), &id(10), &id(60), Settlement::Released),
        Err(Error::RevisionConflict)
    );
    s.settle(&op, &id(1), &id(10), &id(60), Settlement::Known(20))
        .unwrap();
    s.reserve(&op, &id(1), &id(10), &id(61), "CNY", 30).unwrap();
    assert_eq!(
        s.reserve(&op, &id(1), &id(10), &id(61), "USD", 30),
        Err(Error::InvalidArgument)
    );
}
#[cfg(unix)]
#[test]
fn symlink_root_and_object_are_rejected() {
    let (t, mut s, sha) = setup();
    let outside = tempdir().unwrap();
    let link = outside.path().join("root-link");
    std::os::unix::fs::symlink(t.path(), &link).unwrap();
    assert!(matches!(
        Store::open(&link, &id(1), false, Arc::default()),
        Err(Error::DataRootRejected)
    ));
    let object = t.path().join("objects").join(sha);
    std::fs::remove_file(&object).unwrap();
    std::os::unix::fs::symlink(outside.path().join("missing"), &object).unwrap();
    assert_eq!(
        exec(
            &mut s,
            1,
            C::ReadObject(FoundationReadObject {
                workspace_id: id(10),
                object_id: id(20),
                revision: 0
            })
        ),
        Err(Error::DataRootRejected)
    );
}

#[test]
fn deletion_revokes_access_and_garbage_collection_preserves_shared_content() {
    let (t, mut s, sha) = setup();
    let d = prepare(&mut s, 40, &sha);
    approve(&mut s, 40, &d.sha256);
    exec(
        &mut s,
        1,
        C::PutObject(FoundationObject {
            workspace_id: id(10),
            object_id: id(21),
            expected_revision: 0,
            body: "人工编写，仅本地私有展示".into(),
        }),
    )
    .unwrap();
    exec(
        &mut s,
        1,
        C::DeleteObject(FoundationDelete {
            workspace_id: id(10),
            object_id: id(20),
            expected_revision: 1,
        }),
    )
    .unwrap();
    assert!(t.path().join("objects").join(&sha).exists());
    assert_eq!(
        exec(
            &mut s,
            1,
            C::ReadObject(FoundationReadObject {
                workspace_id: id(10),
                object_id: id(20),
                revision: 1
            })
        ),
        Err(Error::NotFound)
    );
    assert!(dispatch(&mut s, 40).is_err());
    exec(
        &mut s,
        1,
        C::DeleteObject(FoundationDelete {
            workspace_id: id(10),
            object_id: id(21),
            expected_revision: 1,
        }),
    )
    .unwrap();
    assert!(!t.path().join("objects").join(sha).exists());
}

#[test]
fn interrupted_initial_creation_is_recoverable_but_missing_committed_database_is_not_recreated() {
    let temp = tempdir().unwrap();
    let marker = serde_json::json!({"kind":"nai-foundation-v2","schema_version":2,"owner":id(1)});
    std::fs::write(
        temp.path().join("foundation-root.json"),
        serde_json::to_vec(&marker).unwrap(),
    )
    .unwrap();
    let store = Store::open(temp.path(), &id(1), false, Arc::default()).unwrap();
    assert!(temp.path().join("initialized.ok").exists());
    drop(store);
    std::fs::remove_file(temp.path().join("foundation.sqlite")).unwrap();
    assert!(matches!(
        Store::open(temp.path(), &id(1), false, Arc::default()),
        Err(Error::IntegrityFailed)
    ));
    assert!(!temp.path().join("foundation.sqlite").exists());
}

#[test]
fn sqlite_progress_handler_observes_cancellation() {
    let (_t, s, _) = setup();
    let op = Operation::new(4000).unwrap();
    s.begin(&op).unwrap();
    op.cancel();
    let result=s.conn.query_row("WITH RECURSIVE n(x) AS (VALUES(0) UNION ALL SELECT x+1 FROM n WHERE x<100000000) SELECT SUM(x) FROM n",[],|r|r.get::<_,i64>(0)).wb();
    assert_eq!(result, Err(Error::Cancelled));
}
#[test]
fn role_revision_cancels_old_approvals_and_unknown_roles_are_refused() {
    let (_t, mut s, sha) = setup();
    let d = prepare(&mut s, 40, &sha);
    approve(&mut s, 40, &d.sha256);
    assert_eq!(
        exec(
            &mut s,
            1,
            C::Grant(FoundationGrant {
                workspace_id: id(10),
                principal_id: id(2),
                role: 999,
                expected_revision: 0
            })
        ),
        Err(Error::InvalidArgument)
    );
    exec(
        &mut s,
        1,
        C::Grant(FoundationGrant {
            workspace_id: id(10),
            principal_id: id(1),
            role: FoundationRole::Admin as i32,
            expected_revision: 1,
        }),
    )
    .unwrap();
    assert_eq!(
        s.conn
            .query_row("SELECT COUNT(*) FROM approvals", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert!(dispatch(&mut s, 40).is_err());
}

#[test]
fn stop_does_not_call_uncertain_or_in_flight_work_submitted() {
    let (_t, mut s, sha) = setup();
    let d = prepare(&mut s, 40, &sha);
    approve(&mut s, 40, &d.sha256);
    dispatch(&mut s, 40).unwrap();
    let stop = exec(
        &mut s,
        1,
        C::Stop(FoundationStop {
            workspace_id: id(10),
            session_id: id(30),
        }),
    )
    .unwrap();
    assert_eq!(stop.already_submitted, 0);
    assert_eq!(stop.dispatch_in_flight, 1);
    let history = exec(
        &mut s,
        1,
        C::ReadDelivery(FoundationRead {
            workspace_id: id(10),
            delivery_id: id(40),
        }),
    )
    .unwrap();
    assert_eq!(history.state, DeliveryState::OutcomeUnknown as i32);
    assert_eq!(history.evidence, 0);
}

#[test]
fn restart_never_releases_an_unsettled_budget_reservation() {
    let (t, mut s, _) = setup();
    s.reserve(
        &Operation::new(4000).unwrap(),
        &id(1),
        &id(10),
        &id(60),
        "CNY",
        80,
    )
    .unwrap();
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    assert_eq!(
        s.settle(
            &Operation::new(4000).unwrap(),
            &id(1),
            &id(10),
            &id(60),
            Settlement::Released
        ),
        Err(Error::RevisionConflict)
    );
    assert_eq!(
        s.reserve(
            &Operation::new(4000).unwrap(),
            &id(1),
            &id(10),
            &id(61),
            "CNY",
            21
        ),
        Err(Error::ResourceExhausted)
    );
}

#[test]
fn failed_revoke_audit_keeps_the_old_role_denied_across_restart() {
    let (t, mut s, _) = setup();
    exec(
        &mut s,
        1,
        C::Grant(FoundationGrant {
            workspace_id: id(10),
            principal_id: id(2),
            role: FoundationRole::Operator as i32,
            expected_revision: 0,
        }),
    )
    .unwrap();
    s.conn.execute_batch("CREATE TRIGGER fail_revoke_audit BEFORE INSERT ON audit_events BEGIN SELECT RAISE(ABORT,'audit failure'); END;").unwrap();
    assert!(
        exec(
            &mut s,
            1,
            C::Grant(FoundationGrant {
                workspace_id: id(10),
                principal_id: id(2),
                role: FoundationRole::None as i32,
                expected_revision: 1
            })
        )
        .is_err()
    );
    let read = C::ReadObject(FoundationReadObject {
        workspace_id: id(10),
        object_id: id(20),
        revision: 0,
    });
    assert_eq!(exec(&mut s, 2, read.clone()), Err(Error::PermissionDenied));
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    assert_eq!(exec(&mut s, 2, read.clone()), Err(Error::PermissionDenied));
    s.conn
        .execute_batch("DROP TRIGGER fail_revoke_audit")
        .unwrap();
    exec(
        &mut s,
        1,
        C::Grant(FoundationGrant {
            workspace_id: id(10),
            principal_id: id(2),
            role: FoundationRole::Viewer as i32,
            expected_revision: 1,
        }),
    )
    .unwrap();
    assert!(exec(&mut s, 2, read).is_ok());
}
#[test]
fn failed_delete_audit_keeps_content_revoked_across_restart() {
    let (t, mut s, _) = setup();
    s.conn.execute_batch("CREATE TRIGGER fail_delete_audit BEFORE INSERT ON audit_events BEGIN SELECT RAISE(ABORT,'audit failure'); END;").unwrap();
    assert!(
        exec(
            &mut s,
            1,
            C::DeleteObject(FoundationDelete {
                workspace_id: id(10),
                object_id: id(20),
                expected_revision: 1
            })
        )
        .is_err()
    );
    let read = C::ReadObject(FoundationReadObject {
        workspace_id: id(10),
        object_id: id(20),
        revision: 1,
    });
    assert_eq!(exec(&mut s, 1, read.clone()), Err(Error::NotFound));
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    assert_eq!(exec(&mut s, 1, read), Err(Error::NotFound));
}

#[test]
fn cleanup_never_deletes_unknown_assets_or_manually_replaced_owned_files() {
    let (t, mut s, _) = setup();
    let unknown = t.path().join("objects").join("a".repeat(64));
    std::fs::write(&unknown, b"foreign asset, not created by this writer").unwrap();
    s.collect_unreferenced().unwrap();
    assert!(unknown.exists());
    let sha = store_body(t.path(), b"orphan created by this writer").unwrap();
    let path = t.path().join("objects").join(sha);
    std::fs::write(&path, b"manually replaced material").unwrap();
    assert_eq!(s.collect_unreferenced(), Err(Error::IntegrityFailed));
    assert_eq!(std::fs::read(path).unwrap(), b"manually replaced material");
    assert!(unknown.exists());
}

#[test]
fn unknown_ownership_marker_is_never_overwritten() {
    let (t, mut s, _) = setup();
    let text = b"new body under an occupied marker";
    let name = hash(text);
    let marker = t.path().join("object-ownership").join(name);
    std::fs::write(&marker, b"foreign license material").unwrap();
    assert_eq!(
        exec(
            &mut s,
            1,
            C::PutObject(FoundationObject {
                workspace_id: id(10),
                object_id: id(29),
                expected_revision: 0,
                body: String::from_utf8(text.to_vec()).unwrap()
            })
        ),
        Err(Error::IntegrityFailed)
    );
    assert_eq!(std::fs::read(marker).unwrap(), b"foreign license material");
}

mod tts_task_tests;
