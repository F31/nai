use crate::{Operation, Result, error, hash, utc_ms};
use nai_contracts::v1::ErrorCode;
use rusqlite::{Connection, TransactionBehavior};

pub(crate) const VERSION: i64 = 12;
const SQL: [&str; 12] = [
    include_str!("../../../migrations/sqlite/0001-foundation.sql"),
    include_str!("../../../migrations/sqlite/0002-authority.sql"),
    include_str!("../../../migrations/sqlite/0003-knowledge-index.sql"),
    include_str!("../../../migrations/sqlite/0004-knowledge-publication.sql"),
    include_str!("../../../migrations/sqlite/0005-decision-log.sql"),
    include_str!("../../../migrations/sqlite/0006-utterance-display-text.sql"),
    include_str!("../../../migrations/sqlite/0007-decision-log-assembly-kind.sql"),
    include_str!("../../../migrations/sqlite/0008-memory-revocation.sql"),
    include_str!("../../../migrations/sqlite/0009-memory-purge.sql"),
    include_str!("../../../migrations/sqlite/0010-document-local-only.sql"),
    include_str!("../../../migrations/sqlite/0011-decision-log-degradation-kind.sql"),
    include_str!("../../../migrations/sqlite/0012-utterance-status-domain.sql"),
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
    ///
    /// ⚠️ **钉住具体版本号，MUST NOT 写 `VERSION - 1`**（W109 实测回归）：本用例验的是
    /// `0007-decision-log-assembly-kind` **这一次**升级，而 `VERSION - 1` 会随后续每次升版本
    /// 自动前移——VERSION 7 → 8 时它悄悄改测了 v7→v8，用例名随之变成不实陈述，而 0007
    /// 那条升级路径**失去覆盖**却零信号。钉住之后，后续升版本不会再改变这里被测的对象。
    #[test]
    fn version_six_upgrade_preserves_the_decision_chain_and_accepts_the_assembly_kind() {
        /// `0007-decision-log-assembly-kind` 的版本号。改它 MUST 同时改用例名。
        const ASSEMBLY_KIND: usize = 7;
        let mut db = Connection::open_in_memory().unwrap();
        for (index, sql) in SQL.iter().take(ASSEMBLY_KIND - 1).enumerate() {
            apply(&mut db, &op(), index as i64 + 1, sql).unwrap();
        }
        assert_eq!(verify(&db).unwrap(), (ASSEMBLY_KIND - 1) as i64);
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

    /// W109: `0008-memory-revocation.sql` 给 `memory_records` 加一列。加列本身不会丢行，
    /// 但「老行的新列是什么」才是要钉的事实：它 MUST 是 NULL（未撤销），MUST NOT 是 0——
    /// 0 会被 `revoked_at_ms.is_some()` 读成「1970 年就已撤销」，把**每一行老记忆**一次性
    /// 判死。这条升级路径 MUST 在一个**已有行**的库上跑：空表上的 `ALTER TABLE` 什么也证明不了。
    ///
    /// ⚠️ **版本号钉死**（W111 修）：此处原写 `VERSION as usize - 1`——正是本文件上方
    /// 警告过的那种写法。W111 把 `VERSION` 升到 9 之后，它**悄悄改测了 v8→v9**：
    /// 用例名还写着 version_eight，而被测对象已经不是那一次升级。
    /// 钉住具体数字后，后续升版本不会再改变这里被测的对象。
    #[test]
    fn version_eight_upgrade_preserves_memory_rows_and_defaults_the_revocation_column_to_null() {
        /// `0008-memory-revocation.sql` 的版本号。改它 MUST 同时改用例名。
        const MEMORY_REVOCATION: usize = 8;
        let mut db = Connection::open_in_memory().unwrap();
        for (index, sql) in SQL.iter().take(MEMORY_REVOCATION - 1).enumerate() {
            apply(&mut db, &op(), index as i64 + 1, sql).unwrap();
        }
        assert_eq!(verify(&db).unwrap(), (MEMORY_REVOCATION - 1) as i64);
        db.execute(
            "INSERT INTO workspaces(workspace_id,name) VALUES('w','legacy')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO sessions(session_id,workspace_id) VALUES('session-1','w')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO memory_records VALUES('w','memory-legacy','session-1',NULL,'session','order_status','auth-1','body',4102444800000)",
            [],
        )
        .unwrap();

        migrate(&mut db, &op()).unwrap();
        assert_eq!(verify(&db).unwrap(), VERSION);

        // 既有行**逐字**保留：新增一列没有改写任何一列。
        let row = db
            .prepare(
                "SELECT workspace_id,record_id,session_id,kind,purpose,authorization_ref,content,expires_at_ms,viewer_scope FROM memory_records",
            )
            .unwrap()
            .query_row([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, Option<String>>(8)?,
                ))
            })
            .unwrap();
        assert_eq!(row.0, "w");
        assert_eq!(row.1, "memory-legacy");
        assert_eq!(row.2, "session-1");
        assert_eq!(row.3, "session");
        assert_eq!(row.4, "order_status");
        assert_eq!(row.5, "auth-1");
        assert_eq!(row.6, "body");
        assert_eq!(row.7, 4_102_444_800_000);
        assert_eq!(
            row.8, None,
            "观众作用域是**可空**列：老行的 NULL MUST 保持 NULL"
        );

        // 新列可读，且老行取值是 NULL——不是 0，也不是「未知」。
        let revoked: Option<i64> = db
            .query_row("SELECT revoked_at_ms FROM memory_records", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(revoked, None);
    }

    /// W111: `0009-memory-purge.sql` 给 `memory_records` 加 `purged_at_ms`。
    ///
    /// 要钉的是**两件事**，且它们**互不相同**：
    /// * 老行在新列上是 `NULL`（**未清除**），MUST NOT 是 0——0 会被任何按值比较读成
    ///   「1970 年就已清」，让**每一行**老记忆一夜之间变成「已擦除」；
    /// * 载荷 `content` **一字未动**——加列不是清除。
    ///
    /// 同样 MUST 在一个**已有行**的库上跑：空表上的 `ALTER TABLE` 什么也证明不了。
    #[test]
    fn version_nine_upgrade_preserves_the_payload_and_defaults_the_purge_column_to_null() {
        /// `0009-memory-purge.sql` 的版本号。改它 MUST 同时改用例名。
        const MEMORY_PURGE: usize = 9;
        let mut db = Connection::open_in_memory().unwrap();
        for (index, sql) in SQL.iter().take(MEMORY_PURGE - 1).enumerate() {
            apply(&mut db, &op(), index as i64 + 1, sql).unwrap();
        }
        assert_eq!(verify(&db).unwrap(), (MEMORY_PURGE - 1) as i64);
        db.execute(
            "INSERT INTO workspaces(workspace_id,name) VALUES('w','legacy')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO sessions(session_id,workspace_id) VALUES('session-1','w')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO memory_records(workspace_id,record_id,session_id,kind,purpose,authorization_ref,content,expires_at_ms) \
             VALUES('w','memory-legacy','session-1','session','order_status','auth-1','body',4102444800000)",
            [],
        )
        .unwrap();

        migrate(&mut db, &op()).unwrap();
        assert_eq!(verify(&db).unwrap(), VERSION);

        let (content, purged): (String, Option<i64>) = db
            .query_row(
                "SELECT content,purged_at_ms FROM memory_records WHERE record_id='memory-legacy'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(content, "body", "加列 MUST NOT 改写载荷");
        assert_eq!(
            purged, None,
            "老行的清除时刻 MUST 是 NULL（未清除），MUST NOT 是 0"
        );
    }

    /// W131: `0010-document-local-only.sql` 给 `documents` 加 `local_only`（§13.2 / §281）。
    ///
    /// 要钉的是**两件事**：
    /// * 老行在新列上读到 **0**。这里的 `0` 含义是「**从未被标记**」——它是升级时点的
    ///   既定值，**不是**「已核实这份文档可以外发」。两种说法差得很远：前者是历史事实，
    ///   后者是一个没人做过的核对。本用例只钉前者，轮次报告按前者写。
    /// * `status` 与 `current_revision` **一字未动**——加列不是重建。
    ///
    /// 同样 MUST 在一个**已有行**的库上跑：空表上的 `ALTER TABLE` 什么也证明不了。
    #[test]
    fn version_ten_upgrade_keeps_documents_and_reads_the_new_mark_as_unmarked() {
        /// `0010-document-local-only.sql` 的版本号。改它 MUST 同时改用例名。
        const DOCUMENT_LOCAL_ONLY: usize = 10;
        let mut db = Connection::open_in_memory().unwrap();
        for (index, sql) in SQL.iter().take(DOCUMENT_LOCAL_ONLY - 1).enumerate() {
            apply(&mut db, &op(), index as i64 + 1, sql).unwrap();
        }
        assert_eq!(verify(&db).unwrap(), (DOCUMENT_LOCAL_ONLY - 1) as i64);
        db.execute(
            "INSERT INTO workspaces(workspace_id,name) VALUES('w','legacy')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO documents(workspace_id,document_id,source,content_hash,product_scope,status,acl_ref,current_revision) \
             VALUES('w','doc-legacy','txt','old-revision','sku','Staged','acl','old-revision')",
            [],
        )
        .unwrap();

        migrate(&mut db, &op()).unwrap();
        assert_eq!(verify(&db).unwrap(), VERSION);

        let (mark, status, revision): (i64, String, Option<String>) = db
            .query_row(
                "SELECT local_only,status,current_revision FROM documents WHERE document_id='doc-legacy'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            mark, 0,
            "老行在新列上 MUST 读到 0（从未被标记），不是因为有人核实过它可以外发"
        );
        assert_eq!(status, "Staged", "加列 MUST NOT 改写文档状态");
        assert_eq!(
            revision.as_deref(),
            Some("old-revision"),
            "加列 MUST NOT 挪动当前版本"
        );
    }

    /// W132: `0011-decision-log-degradation-kind.sql` 给 `decision_log` 的 kind CHECK 加
    /// `degradation`（§47.4 / INV-32 / H17）。
    ///
    /// 要钉的是**三件事**：
    /// * 重建**逐字**保留了既有行——四个哈希列一个字节都没动（迁移注释声称的正是这一点，
    ///   而「行还在」比「哈希一致」弱得多）。
    /// * 升级**前** `degradation` 被 CHECK 拒绝：这是「CHECK 真的是闸门」的判别性证据，
    ///   否则「升级后能插入」什么都证明不了（一个恒真的插入断言）。
    /// * 升级**后** `degradation` 可插入——枚举与 CHECK 因此同态。
    ///
    /// MUST 在有既有行的库上跑：空表上的重建什么也证明不了。
    #[test]
    fn version_eleven_upgrade_keeps_the_chain_verbatim_and_admits_the_degradation_kind() {
        /// `0011-decision-log-degradation-kind.sql` 的版本号。改它 MUST 同时改用例名。
        const DEGRADATION_KIND: usize = 11;
        let mut db = Connection::open_in_memory().unwrap();
        for (index, sql) in SQL.iter().take(DEGRADATION_KIND - 1).enumerate() {
            apply(&mut db, &op(), index as i64 + 1, sql).unwrap();
        }
        assert_eq!(verify(&db).unwrap(), (DEGRADATION_KIND - 1) as i64);
        db.execute(
            "INSERT INTO workspaces(workspace_id,name) VALUES('w','legacy')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO decision_log(seq,workspace_id,session_id,kind,criticality,payload_json,payload_hash,prev_hash,entry_hash,created_at_ms,trace_id) \
             VALUES(1,'w',NULL,'event','telemetry','{}','payload-hash','0','entry-hash',1,'trace-legacy')",
            [],
        )
        .unwrap();
        // 判别性前提：升级前 CHECK 不放行 `degradation`。没有这一步，「升级后可插入」
        // 在一个本来就放行该 kind 的实现上也成立。
        assert!(
            db.execute(
                "INSERT INTO decision_log(seq,workspace_id,session_id,kind,criticality,payload_json,payload_hash,prev_hash,entry_hash,created_at_ms,trace_id) \
                 VALUES(2,'w',NULL,'degradation','critical','{}','ph','entry-hash','eh',2,'trace-new')",
                [],
            )
            .is_err(),
            "升级前 kind CHECK MUST 拒绝 degradation"
        );

        migrate(&mut db, &op()).unwrap();
        assert_eq!(verify(&db).unwrap(), VERSION);

        let (payload, payload_hash, prev_hash, entry_hash): (String, String, String, String) = db
            .query_row(
                "SELECT payload_json,payload_hash,prev_hash,entry_hash FROM decision_log WHERE seq=1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(payload, "{}", "重建 MUST NOT 改写载荷");
        assert_eq!(payload_hash, "payload-hash", "重建 MUST 逐字保留哈希列");
        assert_eq!(prev_hash, "0");
        assert_eq!(entry_hash, "entry-hash");

        db.execute(
            "INSERT INTO decision_log(seq,workspace_id,session_id,kind,criticality,payload_json,payload_hash,prev_hash,entry_hash,created_at_ms,trace_id) \
             VALUES(2,'w',NULL,'degradation','critical','{}','ph','entry-hash','eh',2,'trace-new')",
            [],
        )
        .expect("升级后 kind CHECK MUST 放行 degradation");
    }

    /// W142（N1）：`0012-utterance-status-domain.sql` 给 `utterances.status` 补上取值域的
    /// CHECK。它同样靠整表重建，但**比 0011 多一个维度**：`utterances` 有子表
    /// （`evidence_links` 外键指进来），而 `Store::open` 在 `migrate` **之前**就把
    /// `foreign_keys` 打开了 ⇒ 重建是在**外键生效**的状态下跑的。
    ///
    /// 这条用例因此钉的是四件事，其中后两件只有 `0012` 里那一对
    /// `defer_foreign_keys` pragma **都在**时才成立（实测：缺 ON ⇒ `DROP TABLE` 直接
    /// `FOREIGN KEY constraint failed`；缺 OFF ⇒ 行插回来了但 `COMMIT` 仍失败，
    /// 而此刻 `PRAGMA foreign_key_check` 是空的）：
    /// * 升级**前**这一列接受域外值（判别性前提，否则「升级后拒绝」在本来就拒绝的实现上
    ///   同样成立）；
    /// * 升级**后**域外值被 CHECK 拒绝、域内值照旧可写；
    /// * 既有行**逐字**保留（含 `display_text` 与 `actual_played`）；
    /// * **子行仍在且外键完好** —— 重建没有顺手把 `evidence_links` 清掉。
    ///
    /// MUST 在有既有行**且带子行**的库上跑：空表上重建，那两个 pragma 一个都测不出来。
    #[test]
    fn version_twelve_upgrade_enforces_the_utterance_status_domain_on_a_populated_database() {
        /// `0012-utterance-status-domain.sql` 的版本号。改它 MUST 同时改用例名。
        const UTTERANCE_STATUS_DOMAIN: usize = 12;
        let mut db = Connection::open_in_memory().unwrap();
        // 与生产同形：`store::open` 先打开外键再迁移。
        db.pragma_update(None, "foreign_keys", true).unwrap();
        for (index, sql) in SQL.iter().take(UTTERANCE_STATUS_DOMAIN - 1).enumerate() {
            apply(&mut db, &op(), index as i64 + 1, sql).unwrap();
        }
        assert_eq!(verify(&db).unwrap(), (UTTERANCE_STATUS_DOMAIN - 1) as i64);
        db.execute(
            "INSERT INTO workspaces(workspace_id,name) VALUES('w','legacy')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO sessions(session_id,workspace_id) VALUES('s','w')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO utterances(utterance_id,workspace_id,session_id,generation_epoch,status,text_hash,expires_at_ms,actual_played,display_text) \
             VALUES('u1','w','s',0,12,'text-hash',4102444800000,0,'legacy text')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO evidence_links(workspace_id,utterance_id,evidence_id) VALUES('w','u1','e1')",
            [],
        )
        .unwrap();
        // 判别性前提：升级前该列接受域外值 —— 这正是本迁移要补的洞。
        db.execute("UPDATE utterances SET status=7 WHERE utterance_id='u1'", [])
            .expect("升级前这一列没有任何 CHECK，域外值必然写得进去");
        db.execute(
            "UPDATE utterances SET status=12 WHERE utterance_id='u1'",
            [],
        )
        .unwrap();

        migrate(&mut db, &op()).unwrap();
        assert_eq!(verify(&db).unwrap(), VERSION);

        let (status, hash, played, text): (i64, String, i64, String) = db
            .query_row(
                "SELECT status,text_hash,actual_played,display_text FROM utterances WHERE utterance_id='u1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(status, 12, "重建 MUST 逐字保留 status");
        assert_eq!(hash, "text-hash", "重建 MUST 逐字保留 text_hash");
        assert_eq!(played, 0);
        assert_eq!(text, "legacy text", "重建 MUST 逐字保留 display_text");
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM evidence_links", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1,
            "重建 MUST NOT 顺手清掉子行（0012 的两个 defer pragma 就是为这一点存在的）"
        );
        assert!(
            db.prepare("PRAGMA foreign_key_check")
                .unwrap()
                .query([])
                .unwrap()
                .next()
                .unwrap()
                .is_none(),
            "重建后外键 MUST 完好"
        );

        let refused = db
            .execute("UPDATE utterances SET status=7 WHERE utterance_id='u1'", [])
            .unwrap_err();
        assert!(
            refused.to_string().contains("CHECK constraint failed"),
            "拒绝 MUST 由 CHECK 给出、可归因：{refused}"
        );
        for value in [1, 6, 11, 13, 14, 15] {
            db.execute(
                "UPDATE utterances SET status=?1 WHERE utterance_id='u1'",
                [value],
            )
            .unwrap_or_else(|error| panic!("status={value} 在声明域内 MUST 可写：{error}"));
        }
    }
}
