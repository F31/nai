use super::*;
fn settings(create: bool, revision: u64, mode: WorkspaceBudgetMode, limit: u64) -> C {
    let x = WorkspaceSettingsCommand {
        workspace_id: id(85),
        settings: Some(WorkspaceSettingsDraft {
            name: "预算直播间".into(),
            budget_mode: mode as i32,
            budget_limit_minor: limit,
            currency: "CNY".into(),
            expected_revision: revision,
        }),
    };
    if create {
        C::CreateWorkspaceSettings(x)
    } else {
        C::EditWorkspaceSettings(x)
    }
}
#[test]
fn unlimited_is_durable_but_still_checks_call_cap() {
    let (t, mut s, _) = setup();
    exec(
        &mut s,
        1,
        settings(true, 0, WorkspaceBudgetMode::Unlimited, 0),
    )
    .unwrap();
    let b = llm::budget(&s.conn, &id(85)).unwrap();
    assert_eq!(b.mode, WorkspaceBudgetMode::Unlimited as i32);
    llm::admit_budget(&b, 5, 5).unwrap();
    assert!(llm::admit_budget(&b, 6, 5).is_err());
    s.reserve(
        &Operation::new(4000).unwrap(),
        &id(1),
        &id(85),
        &id(86),
        "CNY",
        20,
    )
    .unwrap();
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    let b = llm::budget(&s.conn, &id(85)).unwrap();
    assert_eq!(b.mode, 3);
    assert_eq!(b.unknown_minor, 20);
    exec(
        &mut s,
        1,
        settings(false, b.revision, WorkspaceBudgetMode::Limited, 19),
    )
    .unwrap_err();
    exec(
        &mut s,
        1,
        settings(false, b.revision, WorkspaceBudgetMode::Limited, 30),
    )
    .unwrap();
    let b = llm::budget(&s.conn, &id(85)).unwrap();
    llm::admit_budget(&b, 10, 10).unwrap();
    assert!(llm::admit_budget(&b, 11, 20).is_err());
}
#[test]
fn old_zero_is_disabled_and_budget_edit_is_admin_cas() {
    let (_root, mut s, _) = setup();
    exec(
        &mut s,
        1,
        C::CreateWorkspace(FoundationWorkspace {
            workspace_id: id(85),
            name: "旧区".into(),
            budget_limit_minor: 0,
            currency: "CNY".into(),
        }),
    )
    .unwrap();
    let b = llm::budget(&s.conn, &id(85)).unwrap();
    assert_eq!(b.mode, 1);
    assert!(llm::admit_budget(&b, 1, 1).is_err());
    exec(
        &mut s,
        1,
        C::Grant(FoundationGrant {
            workspace_id: id(85),
            principal_id: id(2),
            role: FoundationRole::Operator as i32,
            expected_revision: 0,
        }),
    )
    .unwrap();
    assert_eq!(
        exec(
            &mut s,
            2,
            settings(false, 1, WorkspaceBudgetMode::Unlimited, 0)
        )
        .unwrap_err(),
        Error::PermissionDenied
    );
    assert_eq!(
        exec(
            &mut s,
            1,
            settings(false, 0, WorkspaceBudgetMode::Unlimited, 0)
        )
        .unwrap_err(),
        Error::RevisionConflict
    );
    exec(
        &mut s,
        1,
        settings(false, 1, WorkspaceBudgetMode::Unlimited, 0),
    )
    .unwrap();
    exec(
        &mut s,
        1,
        settings(false, 2, WorkspaceBudgetMode::Disabled, 0),
    )
    .unwrap();
    assert!(llm::admit_budget(&llm::budget(&s.conn, &id(85)).unwrap(), 1, 1).is_err());
}
