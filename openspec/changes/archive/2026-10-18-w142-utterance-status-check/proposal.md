# Proposal: W142 — utterance 取值域下沉到 schema（N1）

## Why

W141 把 `utterances.status` 的取值域收成一处 Rust 声明，并让每条 SQL 的守卫都由那里派生。
但它**只约束了代码路径**：`migrations/sqlite/0001-foundation.sql` 里这一列仍是
`status INTEGER NOT NULL DEFAULT 1`，没有 CHECK、没有枚举。直连数据库（`sqlite3` shell、
外部同步工具、误写的一次性脚本）往 `7`、`0`、`-1`、`16` 写，一律成功，而读侧只会在
`classify()` 的 `else` 分支里把它们归成 `Unassigned` —— 也就是说，**数据损坏被当成一个状态**。

N1 记的就是这件事，并标注「需迁移（`migration::VERSION` 升版 + 六处成套），属独立一轮」。
本轮把这条不实声明堵上：拒绝发生在**写入时**，不是在下次读到它的时候。

已声明域 = `classify()` 不肯给 `Unassigned` 的那些取值 = 待批准带 `1..=5` ∪ 已批准 `6` ∪
仅受保护 `11` ∪ 不可用带 `12..=15`。于是 `Unassigned`（`0`、`7..=10`、`16..`、负数）
在存储里**不可表示** —— 这正是要的效果：没人声明过的取值是损坏，不是程序必须建模的态。

## What changes

- `migrations/sqlite/0012-utterance-status-domain.sql`（新）：整表重建 `utterances`，
  给 `status` 加 `CHECK(status IN(1,2,3,4,5,6,11,12,13,14,15))`。重建在**外键生效**下进行
  （`Store::open` 在 `migrate` 之前就打开了 `foreign_keys`），所以迁移里带一对
  `defer_foreign_keys` pragma —— 实测：缺 `ON` ⇒ `DROP TABLE` 报 `FOREIGN KEY constraint failed`；
  缺 `OFF` ⇒ 行插回来了但 `COMMIT` 仍失败（此时 `PRAGMA foreign_key_check` 是空的）。
- `migration::VERSION` 11 → 12 + `SQL` 数组加一项（六处成套的其余五处同步）。
- 判据：`crates/nai-storage/src/tests/status_schema.rs`（3 条）+ `migration.rs` 的升级用例（1 条）。
- W141 的 `a_candidate_whose_status_was_never_declared_is_not_approvable` **修前提**：
  域外取值已写不进存储，那一半样本无法构造；判定位置前移到写入时（见下）。
- 文档：ADR 0116、报告、`docs/development-plan.md`。

## Impact

- **行为变化只落在写入面**：域外整数的 INSERT/UPDATE 现在被 schema 拒绝。生产路径写入的
  取值（`1`/`6`/`12`/`14`）全在域内，故既有用例零回归。
- **W141 那条判据不是被削弱**：它从「调用时拒绝」前移到「写入时拒绝」，是更早的闸门；
  「域内但不可用」（`11`/`12`/`13`/`14`/`15`）仍在原处走真实 `Store::approve`。
- 迁移是**有数据风险**的操作（整表重建 + 外键）。故升级路径由一条**带子行**的用例钉住：
  行逐字保留、`evidence_links` 子行仍在、`PRAGMA foreign_key_check` 为空。
- `tests/registry.yaml` 的 88 条里仍没有对应「status 取值域」的验收条目 ⇒ 本轮**不**标任何条目。
