# ADR 0116：utterance 取值域下沉到 schema —— 拒绝写在写入时，不写在读到时

- 状态：已接受（W142，2026-10-18）
- 基线：`docs/直播数字人 + 智能体系统技术方案 V3.11.md`
- 相关：ADR 0115（W141 把取值域收成一处 Rust 声明 —— 本 ADR 是它的承载面补齐）、
  `crates/nai-storage/src/utterance_status.rs`、
  `migrations/sqlite/0012-utterance-status-domain.sql`、
  `crates/nai-storage/src/tests/status_schema.rs`

## 背景

W141 之后，取值域有了唯一声明处，每条 SQL 的守卫都由常量派生。但**表本身**仍写着
`status INTEGER NOT NULL DEFAULT 1`：约束只活在 Rust 的调用路径上，直连数据库写一个 `7`
照样成功，而读侧会把它归进 `StatusClass::Unassigned` 并按「不可用」处理 ——
**数据损坏被当成一个合法状态**。N1 记的就是这件事。

真正要决定的是三件事，而不是「加不加 CHECK」。

## 决定一：CHECK 的域 = `classify` 不肯命名的那些值

已声明域 = 待批准带 `1..=5` ∪ 已批准 `6` ∪ 仅受保护 `11` ∪ 不可用带 `12..=15`，
即 `classify()` 不返回 `Unassigned` 的取值。`0`、`7..=10`、`16..`、负数在存储里
**不可表示**。

否决的两个替代：

- **只在数据库里拒绝「明显越界」**（如负数与 `> 15`）：那等于把 `7..=10` 留在库里，
  而它们正是 W141 里被白名单化排除的那一批 —— 拒绝一半的域比不拒绝更难说清。
- **用 `BETWEEN 1 AND 6 OR BETWEEN 11 AND 15`**：语义等价但会放过未来在带内新增的取值，
  而「列表逐项」与 `classify` 的派生结果可以直接逐字比对。取列表。

`classify()` 的 `Unassigned` 分支**保留**：它是全域函数的完备性保障（任意 `i64` 都有归属），
只是升级之后**没有 SQL 写入路径**能造出它 —— 除非有人关掉约束。

## 决定二：整表重建，且必须在外键生效下跑

SQLite 不能 `ALTER TABLE ... ADD CONSTRAINT`，只能重建。这条表有子表：
`evidence_links` 有外键指向 `utterances(workspace_id,utterance_id)`；而 `Store::open`
在 `migration::migrate` **之前**就把 `foreign_keys` 打开了。于是重建落在「外键生效 + 在事务中」
这个夹缝里：

- `PRAGMA foreign_keys` 在事务内是 **no-op**（实测：设置后仍是 `1`）⇒ 迁移无法关掉强制；
- 不加 `PRAGMA defer_foreign_keys=ON` ⇒ `DROP TABLE` 直接报 `FOREIGN KEY constraint failed`；
- 加了 `ON` 而**不在结束后置回 `OFF`** ⇒ 行已经插回来了，但 `COMMIT` 仍报
  `FOREIGN KEY constraint failed`，而此刻 `PRAGMA foreign_key_check` 是**空的**
  （延迟违规是计数而不是重新求值）。

所以迁移里那两行 pragma 是承载物，不是美化。它们的失效**只在有数据的库上现形**，
故升级路径由一条带真实子行的用例钉住。

## 决定三：把 W141 那条判据的前提改掉，而不是关掉它

`a_candidate_whose_status_was_never_declared_is_not_approvable` 原来用 `7`、`16`、`0`、`-1`
这些**域外**取值构造样本。升级后它们写不进存储 ⇒ 样本无法构造。处置：

- 「域内但不可用」（`11` / `12` / `13` / `14` / `15`）留在原处，仍走真实 `Store::approve`；
- 「域外」的拒绝搬到 `status_schema.rs`，判定位置从**调用时**前移到**写入时**。

这不是「判据变少」也不是「放宽」：闸门提前了。如实写法是「位置前移」，
MUST NOT 记成「那条判据被迁移取代」。

## 后果

- 域外整数在写入面被拒；生产路径写入的取值（`1`/`6`/`12`/`14`）全在域内 ⇒ 既有用例零回归。
- 取值域存在**两处字面量**（Rust 的带 + 迁移的列表），这是 SQLite 调不到 Rust 的硬约束。
  绑死它们的是 `status_schema.rs` 的一条判据：期望值从 `classify` **派生**，
  与实际存储的建表语句逐字比较 —— 判据不读迁移文件，也不抄第二份列表。
- 迁移是有数据风险的操作。新增任何整表重建类迁移，都必须配一条**带子行**的升级用例。
- `tests/registry.yaml` 88 条仍无对应验收条目 ⇒ 不标任何条目状态。
