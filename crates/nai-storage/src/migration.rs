use crate::{Operation, Result, error, hash, utc_ms};
use nai_contracts::v1::ErrorCode;
use rusqlite::{Connection, TransactionBehavior};

pub(crate) const VERSION: i64 = 7;
const SQL: [&str; 7] = [
    include_str!("../../../migrations/sqlite/0001-foundation.sql"),
    include_str!("../../../migrations/sqlite/0002-authority.sql"),
    include_str!("../../../migrations/sqlite/0003-knowledge-index.sql"),
    include_str!("../../../migrations/sqlite/0004-knowledge-publication.sql"),
    include_str!("../../../migrations/sqlite/0005-decision-log.sql"),
    include_str!("../../../migrations/sqlite/0006-utterance-display-text.sql"),
    include_str!("../../../migrations/sqlite/0007-decision-log-assembly-kind.sql"),
];

pub(crate) fn verify(connection: &Connection) -> Result<i64> {
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if !(0..=VERSION).contains(&version) {
        return Err(error(
            ErrorCode::VersionIncompatible,
            "unsupported database schema",
        ));
    }
    if version > 0 {
        let records: Vec<(i64, String)> = connection
            .prepare("SELECT version,checksum FROM schema_migrations ORDER BY version")?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?;
        if records.len() != version as usize
            || records.iter().enumerate().any(|(index, (v, checksum))| {
                *v != index as i64 + 1 || *checksum != hash(SQL[index].as_bytes())
            })
        {
            return Err(error(
                ErrorCode::VersionIncompatible,
                "migration checksum or sequence drift",
            ));
        }
        let expected = Connection::open_in_memory()?;
        for sql in SQL.iter().take(version as usize) {
            expected.execute_batch(sql)?;
        }
        fn layout(db: &Connection) -> Result<Vec<(String, String, String)>> {
            Ok(db.prepare("SELECT type,name,sql FROM sqlite_master WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' ORDER BY type,name")?.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?.collect::<std::result::Result<_,_>>()?)
        }
        if layout(connection)? != layout(&expected)? {
            return Err(error(ErrorCode::VersionIncompatible, "database DDL drift"));
        }
    }
    Ok(version)
}
pub(crate) fn migrate(connection: &mut Connection, op: &Operation) -> Result<()> {
    op.check()?;
    let version = verify(connection)?;
    if version == 0 {
        let count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )?;
        if count != 0 {
            return Err(error(
                ErrorCode::VersionIncompatible,
                "unversioned database has tables",
            ));
        }
    }
    for (index, sql) in SQL.iter().enumerate().skip(version as usize) {
        apply(connection, op, index as i64 + 1, sql)?;
    }
    Ok(())
}
fn apply(connection: &mut Connection, op: &Operation, version: i64, sql: &str) -> Result<()> {
    op.check()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute_batch(sql)?;
    transaction.execute(
        "INSERT INTO schema_migrations VALUES(?1,?2,?3,?1,?1)",
        rusqlite::params![version, hash(sql.as_bytes()), utc_ms()?],
    )?;
    transaction.pragma_update(None, "user_version", version)?;
    op.check()?;
    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn op() -> Operation {
        Operation::new(std::time::Duration::from_secs(5)).unwrap()
    }
    #[test]
    fn migration_rollback_and_drift_are_detected() {
        let mut db = Connection::open_in_memory().unwrap();
        migrate(&mut db, &op()).unwrap();
        assert_eq!(verify(&db).unwrap(), VERSION);
        assert!(
            apply(
                &mut db,
                &op(),
                3,
                "CREATE TABLE incomplete(id INTEGER); BAD SQL;"
            )
            .is_err()
        );
        assert_eq!(
            db.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name='incomplete'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        assert_eq!(verify(&db).unwrap(), VERSION);
        db.execute(
            "UPDATE schema_migrations SET checksum='changed' WHERE version=1",
            [],
        )
        .unwrap();
        assert_eq!(
            verify(&db).unwrap_err().code,
            ErrorCode::VersionIncompatible
        );
    }
    #[test]
    fn future_schema_and_unversioned_tables_are_rejected() {
        let mut db = Connection::open_in_memory().unwrap();
        db.pragma_update(None, "user_version", 99).unwrap();
        assert!(migrate(&mut db, &op()).is_err());
        db.pragma_update(None, "user_version", 0).unwrap();
        db.execute_batch("CREATE TABLE unexplained(id INTEGER)")
            .unwrap();
        assert!(migrate(&mut db, &op()).is_err());
    }

    #[test]
    fn version_three_upgrade_preserves_preparation_without_activating_legacy_metadata() {
        let mut db = Connection::open_in_memory().unwrap();
        for (index, sql) in SQL.iter().take(3).enumerate() {
            apply(&mut db, &op(), index as i64 + 1, sql).unwrap();
        }
        db.execute(
            "INSERT INTO workspaces(workspace_id,name) VALUES('w','legacy')",
            [],
        )
        .unwrap();
        db.execute("INSERT INTO documents VALUES('w','doc','txt','old-revision','sku','Published','acl',NULL)",[]).unwrap();
        db.execute("INSERT INTO knowledge_snapshots VALUES('w','old-snapshot','kb','claimed','old-facts',NULL,'published')",[]).unwrap();
        migrate(&mut db, &op()).unwrap();
        assert_eq!(verify(&db).unwrap(), VERSION);
        assert_eq!(
            db.query_row("SELECT current_revision FROM documents", [], |row| row
                .get::<_, String>(
                0
            ))
            .unwrap(),
            "old-revision"
        );
        assert_eq!(
            db.query_row(
                "SELECT COUNT(*) FROM knowledge_active_snapshots",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM knowledge_snapshots", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    /// W104: the assembly kind widens the decision-log CHECK, and the only way to do that in
    /// SQLite is a table rebuild. This exercises the **upgrade** path (not the fresh install): a
    /// version-6 database with a row already in the chain must come out with its rows verbatim
    /// and with the new kind accepted. Without this the rebuild's `INSERT ... SELECT` would only
    /// ever run on an empty table.
    #[test]
    fn version_six_upgrade_preserves_the_decision_chain_and_accepts_the_assembly_kind() {
        let mut db = Connection::open_in_memory().unwrap();
        for (index, sql) in SQL.iter().take(VERSION as usize - 1).enumerate() {
            apply(&mut db, &op(), index as i64 + 1, sql).unwrap();
        }
        assert_eq!(verify(&db).unwrap(), VERSION - 1);
        db.execute(
            "INSERT INTO workspaces(workspace_id,name) VALUES('w','legacy')",
            [],
        )
        .unwrap();
        let row = |kind: &str| {
            format!(
                "INSERT INTO decision_log(workspace_id,session_id,kind,criticality,payload_json,payload_hash,prev_hash,entry_hash,created_at_ms,trace_id) VALUES('w',NULL,'{kind}','telemetry','{{}}','h','p','e',1,'t')"
            )
        };
        db.execute(&row("observation"), []).unwrap();
        // The pre-W104 CHECK is a closed set of twelve: the new kind is refused by the schema,
        // which is exactly the silent-drop trap this migration closes.
        assert!(db.execute(&row("assembly"), []).is_err());

        migrate(&mut db, &op()).unwrap();
        assert_eq!(verify(&db).unwrap(), VERSION);

        // The row already in the chain survives the rebuild verbatim, and the widened CHECK now
        // takes the new kind.
        let (kind, payload): (String, String) = db
            .query_row("SELECT kind,payload_json FROM decision_log", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(kind, "observation");
        assert_eq!(payload, "{}");
        db.execute(&row("assembly"), []).unwrap();
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM decision_log", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
        // The append-only triggers come back with the rebuilt table.
        assert!(db.execute("DELETE FROM decision_log", []).is_err());
    }
}
