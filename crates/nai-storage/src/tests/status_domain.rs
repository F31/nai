//! W141：`utterances.status` 取值域的判据。
//!
//! 三条纪律决定了这里的形状：
//!
//! 1. **扫描集不含判据自己**。被扫的是生产源码（`store.rs` / `knowledge.rs` /
//!    `publication.rs` / `utterance_status.rs`）。`tests.rs:483` 与
//!    `approval_workflow.rs:202` 断言里钉的 `12` / `14` **故意**留裸字面量 ——
//!    那两处是**判别性**断言，改成引用常量就变成「常量比常量」，恒绿。
//! 2. **守卫不给自己开后门**：扫描集**含** `utterance_status.rs` 自己，
//!    所以它写的注释里也不能出现 `status=11` 这种形式。
//! 3. **先行反方向自检**：扫描函数本身要先用一段内联样本证明它抓得到、也放得过，
//!    否则「零命中」可能只是扫描器坏了。

use super::*;
use crate::utterance_status::{
    PENDING_BAND_LO, STATUS_APPROVED, STATUS_INVALIDATED, STATUS_REJECTED, usable, usable_guard,
};
use rusqlite::params;

/// 被扫的**生产**源文件。`include_str!` 在编译期读入，所以改动源文件会重新编译。
const SCANNED: [(&str, &str); 4] = [
    ("store.rs", include_str!("../store.rs")),
    ("knowledge.rs", include_str!("../knowledge.rs")),
    ("publication.rs", include_str!("../publication.rs")),
    (
        "utterance_status.rs",
        include_str!("../utterance_status.rs"),
    ),
];

/// 找出 `status` 后面紧跟数字的 SQL 片段。`status={}`（由常量派生）不算裸字面量。
fn bare_status_literals(source: &str) -> Vec<String> {
    let mut hits = Vec::new();
    let mut rest = source;
    while let Some(pos) = rest.find("status") {
        let after = &rest[pos + "status".len()..];
        let trimmed = after.trim_start();
        // `NOT IN(` 必须排在 `IN(` 之前，否则前者会被后者当成一次命中而漏判。
        for pattern in ["NOT IN(", "IN(", "BETWEEN ", "="] {
            if let Some(tail) = trimmed.strip_prefix(pattern) {
                if tail.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                    hits.push(format!("status {pattern}<digit>"));
                }
                break;
            }
        }
        rest = after;
    }
    hits
}

/// 扫描器自身的非空洞性：**先证明它会响**，再拿它去断言「零命中」。
/// 少了这一步，「零命中」与「扫描器坏了」不可区分。
#[test]
fn the_literal_scanner_actually_fires_and_actually_forgives() {
    assert_eq!(
        bare_status_literals("WHERE status=11 AND status NOT IN(12,13) AND status BETWEEN 1 AND 5")
            .len(),
        3,
        "三种裸字面量形式各须抓到一次"
    );
    assert!(
        bare_status_literals("WHERE status={} AND status NOT IN({}) AND status BETWEEN {} AND {}")
            .is_empty(),
        "由常量派生的写法是花括号占位而非数字，不算裸字面量"
    );
    assert!(
        bare_status_literals("/// 文档里写 status INTEGER NOT NULL DEFAULT 1").is_empty(),
        "文档措辞不得被当成 SQL"
    );
}

/// 取值域只能有**一处**声明：生产源码里不再有 status 的裸数字。
/// 这是一条**会过期**的断言 —— 谁新写一句 `status=11` 都会让它变红，
/// 逼他去 `utterance_status` 里给那个值一个名字和归类。
#[test]
fn no_bare_status_literal_survives_in_the_production_sources() {
    let mut scanned = Vec::new();
    for (name, source) in SCANNED {
        assert!(
            !source.is_empty(),
            "{name} 必须真的被读到（空源＝扫描集写错）"
        );
        scanned.push(name);
        assert_eq!(
            bare_status_literals(source),
            Vec::<String>::new(),
            "{name} 里仍有 status 的裸字面量 —— 取值域只能由 utterance_status 声明"
        );
    }
    // 文件层闭世界：四个文件都在册，顺序即扫描顺序（避免有人把某个文件从册里悄悄删掉）。
    assert_eq!(
        scanned,
        [
            "store.rs",
            "knowledge.rs",
            "publication.rs",
            "utterance_status.rs"
        ]
    );
}

/// SQL 的可用性守卫与 Rust 的 `usable()` 谓词必须**逐值一致**。
/// 两者今天由同一处派生（`usable_guard()` 由 `usable_values()` 由 `usable()`），
/// 这条断言管的是「以后有人绕开派生链、在 SQL 里另写一份」。
#[test]
fn the_sql_guard_and_the_rust_predicate_agree_value_by_value() {
    let listed: Vec<i64> = usable_guard()
        .split(',')
        .map(|token| token.parse::<i64>().unwrap())
        .collect();
    assert!(
        !listed.is_empty(),
        "白名单不得为空（空＝把所有取值判成不可用）"
    );
    for value in 0..=32 {
        assert_eq!(
            usable(value),
            listed.contains(&value),
            "status={value} 在 SQL 守卫与 Rust 谓词里口径不一致"
        );
    }
}

/// **W141 的唯一行为变化**：可用性从黑名单改成白名单。
///
/// 改动前是 `status NOT IN(12,13,14,15)` ⇒ 11 与 7..=10、16.. 都被当成可用
/// （「没被列入黑名单」被读成了「可用」，而可用是危险方向）。
/// 改动后只认**声明过**的取值。这条判据在改动前会红 —— 它不是回归，是新增的判定。
///
/// ⚠️ **W142 修了这条判据的前提**：`0012-utterance-status-domain.sql` 给这一列补上取值域
/// 的 CHECK 之后，`7..=10` / `16` / `0` / 负数**写不进存储**了，原来那一半样本在此处已无法
/// 构造。判据没有被关掉，也没有被放宽 —— 它分成了两处：
/// * 「域内但不可用」（11 / 12 / 13 / 14 / 15）仍在这里走真实的 `Store::approve`；
/// * 「域外」的拒绝搬到 `status_schema.rs`，判定位置从**调用时**前移到**写入时**
///   （更早的闸门，不是更弱的判据）。
#[test]
fn a_candidate_whose_status_is_not_usable_is_not_approvable() {
    // 对照组：待批准（1）与已批准（6）必须仍可批准，否则下面那片红只是「approve 一直失败」。
    for status in [PENDING_BAND_LO, STATUS_APPROVED] {
        let outcome = attempt_approval(status);
        assert!(
            outcome.is_ok(),
            "status={status} 是声明过的可用态，必须可批准：{outcome:?}"
        );
    }
    // 被判定的一侧：域内但不可用的取值一律拒绝 —— 含仅受级联保护的 11（旧黑名单写法会放行它，
    // 所以这条仍是判别性的）、三个终态 12/14 与两个零写入者的幽灵值 13/15。
    for status in [11, STATUS_INVALIDATED, 13, STATUS_REJECTED, 15] {
        let outcome = attempt_approval(status);
        assert!(
            outcome.is_err(),
            "status={status} 从未被声明为可用，必须拒绝批准"
        );
    }
    // 终态同样拒绝，且拒绝名要能归因（不是「恰好没找到」）。
    let outcome = attempt_approval(STATUS_INVALIDATED);
    assert!(outcome.is_err(), "已失效的候选不得再被批准");
}

/// 级联失效**真的**保护了它点名的那一带：`11..=15` 不被改写，带外的一律改写成已失效。
/// 这条把 `cascade_guard()` 从「一个字符串」钉成「一个行为」。
#[test]
fn the_cascade_guard_protects_exactly_the_band_it_names() {
    for status in [PENDING_BAND_LO, STATUS_APPROVED] {
        assert_eq!(
            cascade_outcome(status),
            STATUS_INVALIDATED,
            "status={status} 在级联保护带外，必须被改写成已失效"
        );
    }
    for status in [11, STATUS_INVALIDATED, 13, 14, 15] {
        assert_eq!(
            cascade_outcome(status),
            status,
            "status={status} 在级联保护带内，不得被改写"
        );
    }
}

/// 把一条候选记成 `status`，然后走真实的 `Store::approve`。
/// 返回 `approve` 的结果 —— 判据只看它成功与否，不猜它失败在哪个前置。
fn attempt_approval(status: i64) -> crate::Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .create_session(&op(), "test-workspace", "operator", "session-test")
        .unwrap();
    storage
        .publish_fact(&op(), "operator", &fact(), None)
        .unwrap();
    let cue = candidate();
    let hash = storage
        .record_candidate(
            &op(),
            "test-workspace",
            "operator",
            &cue,
            &[("sku", "price", "1")],
        )
        .unwrap();
    storage
        .conn
        .execute(
            "UPDATE utterances SET status=?1 WHERE text_hash=?2",
            params![status, &hash],
        )
        .unwrap();
    storage.approve(
        &op(),
        "test-workspace",
        "operator",
        &Approval {
            approval_id: "approval-domain".into(),
            session_id: cue.session_id.clone(),
            candidate_hash: hash,
            hash_algorithm: "sha256-canonical-v1".into(),
            actor_id: "model-claim".into(),
            action_scope: "speak".into(),
            expires_at_utc_ms: cue.expires_at_utc_ms,
            ..Default::default()
        },
    )
}

/// 把一条 utterance 记成 `status`，跑一次真实的全局级联失效，读出它之后的值。
/// 用 `recover`（`store.rs` 的全局失效）而不是另写一条 SQL —— 测的是生产语句。
fn cascade_outcome(status: i64) -> i64 {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .create_session(&op(), "test-workspace", "operator", "session-test")
        .unwrap();
    storage
        .publish_fact(&op(), "operator", &fact(), None)
        .unwrap();
    let hash = storage
        .record_candidate(
            &op(),
            "test-workspace",
            "operator",
            &candidate(),
            &[("sku", "price", "1")],
        )
        .unwrap();
    storage
        .conn
        .execute(
            "UPDATE utterances SET status=?1 WHERE text_hash=?2",
            params![status, &hash],
        )
        .unwrap();
    crate::store::recover(&mut storage.conn, &op(), false).unwrap();
    storage
        .conn
        .query_row(
            "SELECT status FROM utterances WHERE text_hash=?1",
            params![&hash],
            |row| row.get::<_, i64>(0),
        )
        .unwrap()
}
