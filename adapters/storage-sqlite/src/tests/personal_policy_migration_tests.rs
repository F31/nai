//! Isolated transaction checks; these do not activate a production migration.
use super::*;
use nai_runtime_core::personal_policy_migration::{BackupObservation, Journal, Recovery, recover};

fn database() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    conn.execute_batch(include_str!(
        "../../../../schemas/storage/foundation-v2.sql"
    ))
    .unwrap();
    conn.execute_batch("INSERT INTO workspaces VALUES('w','workspace',1,1,1,42,'CNY'); INSERT INTO budget_reservations VALUES('w','b','p',10,5,'known','CNY',1,1,1); INSERT INTO audit_events(workspace_id,principal_id,action,resource_id,revision,created_at,updated_at) VALUES('w','p','budget_limited','w',1,1,1);").unwrap();
    conn
}
fn journal() -> Journal {
    let raw = format!(
        r#"{{"schema_version":1,"migration_id":"00000000-0000-0000-0000-000000000001","source_version":2,"target_version":3,"phase":"backup_verified","backup_id":"00000000-0000-0000-0000-000000000002","backup_manifest_sha256":"{}"}}"#,
        "a".repeat(64)
    );
    Journal::parse(raw.as_bytes()).unwrap()
}
fn version(conn: &Connection) -> u32 {
    conn.pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}
fn has_table(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='personal_live_preferences')",
        [],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn transaction_commit_preserves_existing_budget_and_recovers_after_commit() {
    let mut conn = database();
    let j = journal();
    assert_eq!(
        recover(Some(&j), version(&conn), true, BackupObservation::Valid),
        Recovery::Activate
    );
    let tx = conn.transaction().unwrap();
    tx.execute_batch(include_str!(
        "../../../../schemas/storage/personal-policy-v2-to-v3.sql"
    ))
    .unwrap();
    tx.commit().unwrap();
    assert_eq!(version(&conn), 3);
    assert!(has_table(&conn));
    assert_eq!(
        recover(Some(&j), version(&conn), true, BackupObservation::Valid),
        Recovery::FinalizeJournal
    );
    assert_eq!(
        conn.query_row(
            "SELECT budget_limit FROM workspaces WHERE id='w'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        42
    );
    assert_eq!(
        conn.query_row(
            "SELECT actual_minor FROM budget_reservations WHERE id='b'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        5
    );
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM audit_events WHERE action='budget_limited'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert!(
        conn.execute(
            "INSERT INTO personal_live_preferences VALUES('missing','{}',1,1,1)",
            []
        )
        .is_err()
    );
}

#[test]
fn interruption_rolls_back_schema_and_version_together() {
    let mut conn = database();
    {
        let tx = conn.transaction().unwrap();
        tx.execute_batch(include_str!(
            "../../../../schemas/storage/personal-policy-v2-to-v3.sql"
        ))
        .unwrap();
        assert_eq!(version(&tx), 3);
        // Simulate a failure before commit by dropping the transaction.
    }
    assert_eq!(version(&conn), 2);
    assert!(!has_table(&conn));
    assert_eq!(
        recover(
            Some(&journal()),
            version(&conn),
            true,
            BackupObservation::Valid
        ),
        Recovery::Activate
    );
    assert_eq!(
        conn.query_row(
            "SELECT budget_limit FROM workspaces WHERE id='w'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        42
    );
}
