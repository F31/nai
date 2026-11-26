use super::*;
fn lifecycle(action: WorkspaceAction, revision: u64) -> C {
    C::WorkspaceLifecycle(FoundationWorkspaceLifecycle {
        workspace_id: id(80),
        expected_revision: revision,
        action: action as i32,
    })
}
fn empty(s: &mut Store) {
    exec(
        s,
        1,
        C::CreateWorkspace(FoundationWorkspace {
            workspace_id: id(80),
            name: "可删除空区".into(),
            budget_limit_minor: 0,
            currency: "CNY".into(),
        }),
    )
    .unwrap();
}
fn list(s: &mut Store, trash: bool) -> Vec<FoundationResource> {
    exec(
        s,
        1,
        C::ListWorkspaces(FoundationBrowse {
            kind: if trash {
                "recycled".into()
            } else {
                String::new()
            },
            limit: 32,
            ..Default::default()
        }),
    )
    .unwrap()
    .resources
}
#[test]
fn empty_delete_keeps_audit_and_forbids_id_reuse() {
    let (t, mut s, _) = setup();
    empty(&mut s);
    assert!(
        !exec(&mut s, 1, lifecycle(WorkspaceAction::Inspect, 0))
            .unwrap()
            .resources[0]
            .has_content
    );
    exec(&mut s, 1, lifecycle(WorkspaceAction::DeleteEmpty, 1)).unwrap();
    assert!(!list(&mut s, false).iter().any(|w| w.id == id(80)));
    let roles: i64 = s
        .conn
        .query_row(
            "SELECT count(*) FROM roles WHERE workspace_id=?1",
            [id(80)],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(roles, 0);
    let audit: i64 = s
        .conn
        .query_row(
            "SELECT count(*) FROM audit_events WHERE workspace_id=?1 AND action='workspace_delete'",
            [id(80)],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(audit, 1);
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    assert_eq!(
        exec(
            &mut s,
            1,
            C::CreateWorkspace(FoundationWorkspace {
                workspace_id: id(80),
                name: "复用".into(),
                currency: "CNY".into(),
                budget_limit_minor: 0
            })
        )
        .unwrap_err(),
        Error::RevisionConflict
    );
}
#[test]
fn inspect_then_new_content_or_budget_blocks_permanent_delete() {
    let (_root, mut s, _) = setup();
    empty(&mut s);
    exec(&mut s, 1, lifecycle(WorkspaceAction::Inspect, 0)).unwrap();
    exec(
        &mut s,
        1,
        C::PutObject(FoundationObject {
            workspace_id: id(80),
            object_id: id(81),
            expected_revision: 0,
            body: "新增内容".into(),
        }),
    )
    .unwrap();
    assert_eq!(
        exec(&mut s, 1, lifecycle(WorkspaceAction::DeleteEmpty, 1)).unwrap_err(),
        Error::CapabilityUnavailable
    );
    assert!(list(&mut s, false).iter().any(|w| w.id == id(80)));
    let x = C::WorkspaceLifecycle(FoundationWorkspaceLifecycle {
        workspace_id: id(10),
        expected_revision: 1,
        action: WorkspaceAction::DeleteEmpty as i32,
    });
    assert_eq!(
        exec(&mut s, 1, x).unwrap_err(),
        Error::CapabilityUnavailable
    );
}
#[test]
fn removal_requires_admin_current_revision_and_valid_action() {
    let (_root, mut s, _) = setup();
    empty(&mut s);
    exec(
        &mut s,
        1,
        C::Grant(FoundationGrant {
            workspace_id: id(80),
            principal_id: id(2),
            role: FoundationRole::Operator as i32,
            expected_revision: 0,
        }),
    )
    .unwrap();
    for a in [
        WorkspaceAction::Inspect,
        WorkspaceAction::DeleteEmpty,
        WorkspaceAction::Recycle,
        WorkspaceAction::Restore,
    ] {
        assert_eq!(
            exec(&mut s, 2, lifecycle(a, 1)).unwrap_err(),
            Error::PermissionDenied
        );
    }
    assert_eq!(
        exec(&mut s, 1, lifecycle(WorkspaceAction::DeleteEmpty, 0)).unwrap_err(),
        Error::RevisionConflict
    );
    assert_eq!(
        exec(
            &mut s,
            1,
            C::WorkspaceLifecycle(FoundationWorkspaceLifecycle {
                workspace_id: id(80),
                expected_revision: 1,
                action: 99
            })
        )
        .unwrap_err(),
        Error::InvalidArgument
    );
}
#[test]
fn recycle_restart_restore_preserves_content_and_revokes_old_sessions() {
    let (t, mut s, _) = setup();
    empty(&mut s);
    exec(
        &mut s,
        1,
        C::PutObject(FoundationObject {
            workspace_id: id(80),
            object_id: id(81),
            expected_revision: 0,
            body: "原有内容".into(),
        }),
    )
    .unwrap();
    exec(
        &mut s,
        1,
        C::CreateSession(FoundationSession {
            workspace_id: id(80),
            session_id: id(82),
        }),
    )
    .unwrap();
    exec(&mut s, 1, lifecycle(WorkspaceAction::Recycle, 1)).unwrap();
    assert!(!list(&mut s, false).iter().any(|w| w.id == id(80)));
    assert_eq!(list(&mut s, true)[0].revision, 2);
    let read = || {
        C::ReadObject(FoundationReadObject {
            workspace_id: id(80),
            object_id: id(81),
            revision: 0,
        })
    };
    assert_eq!(
        exec(&mut s, 1, read()).unwrap_err(),
        Error::PermissionDenied
    );
    assert_eq!(
        exec(
            &mut s,
            1,
            C::RenameWorkspace(FoundationWorkspaceRename {
                workspace_id: id(80),
                name: "禁止".into(),
                expected_revision: 2
            })
        )
        .unwrap_err(),
        Error::PermissionDenied
    );
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    assert_eq!(list(&mut s, true).len(), 1);
    exec(&mut s, 1, lifecycle(WorkspaceAction::Restore, 2)).unwrap();
    assert_eq!(exec(&mut s, 1, read()).unwrap().body, "原有内容");
    assert!(list(&mut s, true).is_empty());
    assert_eq!(
        list(&mut s, false)
            .iter()
            .find(|w| w.id == id(80))
            .unwrap()
            .revision,
        3
    );
    // Committed role revocations are compacted on restart. Old authority is
    // rejected by the durable revision checked against task/approval bindings.
    assert!(raw_role(&s.conn, &id(80), &id(1)).unwrap().1 > 1);
    let stopped: bool = s
        .conn
        .query_row(
            "SELECT stopped FROM sessions WHERE workspace_id=?1 AND id=?2",
            params![id(80), id(82)],
            |r| r.get(0),
        )
        .unwrap();
    assert!(stopped);
}
#[test]
fn recycle_preserves_unknown_charge_and_is_scoped() {
    let (_root, mut s, _) = setup();
    empty(&mut s);
    s.conn
        .execute(
            "INSERT INTO budget_reservations VALUES(?1,?2,?3,1,NULL,'reserved','CNY',1,1,1)",
            params![id(80), id(83), id(1)],
        )
        .unwrap();
    s.conn
        .execute(
            "INSERT INTO outbox VALUES('llm:running',?1,?2,'dispatching',1,1,1,1)",
            params![id(80), id(83)],
        )
        .unwrap();
    exec(&mut s, 1, lifecycle(WorkspaceAction::Recycle, 1)).unwrap();
    let status: String = s
        .conn
        .query_row(
            "SELECT status FROM budget_reservations WHERE workspace_id=?1",
            [id(80)],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "unknown");
    exec(
        &mut s,
        1,
        C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(20),
            revision: 0,
        }),
    )
    .unwrap();
    exec(&mut s, 1, lifecycle(WorkspaceAction::Restore, 2)).unwrap();
    assert!(
        s.fence
            .check_scope(&object_scope(&id(80), &id(83)))
            .is_err()
    );
}

#[test]
fn restored_workspace_never_reuses_an_old_approved_delivery() {
    let (t, mut s, sha) = setup();
    let d = prepare(&mut s, 40, &sha);
    approve(&mut s, 40, &d.sha256);
    let change = |action, revision| {
        C::WorkspaceLifecycle(FoundationWorkspaceLifecycle {
            workspace_id: id(10),
            expected_revision: revision,
            action: action as i32,
        })
    };
    exec(&mut s, 1, change(WorkspaceAction::Recycle, 1)).unwrap();
    exec(&mut s, 1, change(WorkspaceAction::Restore, 2)).unwrap();
    assert!(dispatch(&mut s, 40).is_err());
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    assert!(dispatch(&mut s, 40).is_err());
    let approvals: i64 = s
        .conn
        .query_row(
            "SELECT count(*) FROM approvals WHERE workspace_id=?1",
            [id(10)],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(approvals, 0);
}
