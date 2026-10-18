//! W142（N1）：`utterances.status` 的取值域现在由 **schema** 强制。
//!
//! W141 把取值域收成一处 Rust 声明（[`crate::utterance_status`]），并让每条 SQL 的守卫都由
//! 那里派生 —— 但**表本身**仍写着 `status INTEGER NOT NULL DEFAULT 1`，没有任何约束，
//! 直连数据库照样能写进 `7`、`0`、`-1`、`16`。本轮的改动把同一份域放进 schema：
//! 拒绝发生在**写入时**，不是在下次读到它的时候。
//!
//! 这里的判据只有一条主张：**schema 拒绝的集合 == `classify` 不肯命名的集合**
//! （[`StatusClass::Unassigned`]）。注意它的期望值是从 `classify` **派生**的，不是把迁移文件
//! 里的数字再抄一遍 —— SQLite 调不到 Rust，所以字面量必然存在两处，但「两处是否一字不差」
//! 由这里钉死：改了域而没写新迁移 ⇒ 红；改了迁移的列表而没改域 ⇒ 红。
//!
//! 扫描区间与 `usable_values` 同一口径（`[0, 不可用带上界]`），且**补集也要查**：
//! 若哪天有人在带上界之外新增一段已声明的取值，下面的补集断言会先红，逼扫描区间跟着改。

use super::*;
use crate::utterance_status::{StatusClass, UNUSABLE_BAND_HI, classify};
use rusqlite::params;

/// 一个取值是否**在已声明的取值域内** —— 即 `classify` 不为它返回 `Unassigned`。
fn declared(value: i64) -> bool {
    classify(value) != StatusClass::Unassigned
}

/// 已声明的取值。与 `usable_values` 同区间扫描，逐个问 [`declared`]。
fn declared_values() -> Vec<i64> {
    (0..=UNUSABLE_BAND_HI)
        .filter(|value| declared(*value))
        .collect()
}

/// 派生出来的集合先自己站得住：**每一项都在册**，且**补集真的在域外**。
/// 少了补集那一半，扫描区间写窄了就永远发现不了（新增的带落在区间外 ⇒ 集合少一项而无人知）。
#[test]
fn the_declared_set_is_pinned_and_its_complement_stays_outside() {
    let values = declared_values();
    assert_eq!(
        values,
        vec![1, 2, 3, 4, 5, 6, 11, 12, 13, 14, 15],
        "已声明域 = 待批准带 1..=5 ∪ 已批准 6 ∪ 仅受保护 11 ∪ 不可用带 12..=15"
    );
    // 补集：扫描区间内的 0，以及区间上下的取值，一律不在域内。
    for value in [0, UNUSABLE_BAND_HI + 1, UNUSABLE_BAND_HI + 2, 100, -1] {
        assert!(!declared(value), "status={value} 不得被算进已声明域");
    }
    assert!(!declared(i64::MIN));
    assert!(!declared(i64::MAX));
}

/// 表上那条 CHECK 必须逐字列举**同一个**集合。
///
/// 判定输入从存储读出（`sqlite_master`），不是读迁移文件的文本：真正约束写入的是建出来的表，
/// 而迁移文件只是它的来源。
#[test]
fn the_stored_check_clause_lists_exactly_the_declared_values() {
    let dir = tempfile::tempdir().unwrap();
    let storage = store(dir.path());
    let ddl: String = storage
        .conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name='utterances'",
            [],
            |row| row.get(0),
        )
        .expect("utterances 表必须存在（迁移链跑到 0012）");
    assert!(
        ddl.starts_with("CREATE TABLE") && ddl.contains("utterance_id"),
        "读到的 MUST 是 utterances 的建表语句，实际：{ddl}"
    );
    let expected = format!(
        "CHECK(status IN({}))",
        declared_values()
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    assert!(
        ddl.contains(&expected),
        "表上的取值域与 utterance_status 声明的域不一致：期望包含 {expected}，实际：{ddl}"
    );
    // 重建用的临时表 MUST NOT 留在 schema 里（半途而废的迁移会留下它）。
    let leftovers: i64 = storage
        .conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name LIKE 'utterances\\_%' ESCAPE '\\'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(leftovers, 0, "重建的中间表 MUST NOT 残留");
}

/// **行为判据**：逐值写入，schema 接受 ⟺ `classify` 不为它返回 `Unassigned`。
///
/// 两侧都要有样本（否则「全拒」或「全放」都能让这条看起来是绿的）。
/// 拒绝消息里 MUST 出现 `CHECK constraint failed` —— 否则「写不进去」可能只是撞上了别的
/// 约束（`NOT NULL`、外键、`STRICT` 类型），而那与取值域无关。
#[test]
fn the_schema_refuses_exactly_the_values_the_classifier_cannot_name() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .create_session(&op(), "test-workspace", "operator", "session-schema")
        .unwrap();
    let mut accepted = Vec::new();
    let mut refused = Vec::new();
    for value in -3..=24 {
        let id = format!("schema-{value}");
        let outcome = storage.conn.execute(
            "INSERT INTO utterances(utterance_id,workspace_id,session_id,generation_epoch,status,text_hash,expires_at_ms) \
             VALUES(?1,'test-workspace','session-schema',0,?2,?1,4102444800000)",
            params![&id, value],
        );
        match outcome {
            Ok(_) => {
                assert!(
                    declared(value),
                    "status={value} 从未被声明，schema MUST 拒绝它"
                );
                accepted.push(value);
            }
            Err(error) => {
                assert!(
                    !declared(value),
                    "status={value} 在已声明域内，schema 不得拒绝：{error}"
                );
                let message = error.to_string();
                assert!(
                    message.contains("CHECK constraint failed"),
                    "拒绝 MUST 由取值域的 CHECK 给出、可归因，实际：{message}"
                );
                refused.push(value);
            }
        }
    }
    assert!(!accepted.is_empty(), "接受侧没有样本 ⇒ 这条判据恒真");
    assert!(!refused.is_empty(), "拒绝侧没有样本 ⇒ 这条判据恒真");
    assert_eq!(
        accepted,
        declared_values(),
        "schema 接受的取值集合 MUST 与声明的域逐值相同"
    );
}
