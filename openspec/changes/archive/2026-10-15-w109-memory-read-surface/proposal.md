## Why

W107 留下两处**具名开放问题**，而它们是**同一个缺失**的两半：

1. **H04 的读取面零调用方。** `read_records`（§42.3）有完整判定面——六类条目级拒绝、顺序断言、
   按优先级裁剪且裁剪可见——但生产代码里**零调用方**。W107 只落了取数入口 `Store::memory_records`，
   并明写「MUST NOT 被读作 H04 已接线」。
2. **§42.4 遗忘链的执行体未建。** `forget()` 判定的是**陈述是否如实**（先阻断读取、再异步清除），
   但生产里**没有任何路径真的阻断读取** ⇒ `ReadSideNotBlocked` 只能被**断言**，不能被**挣得**。

W107 之所以**刻意不接**，理由是判定输入无处可取：

> `read_records(records: &[ScopedRecord], …)` 的条目级门控输入是 `ScopedRecord` 的
> `revoked` / `permitted` / `approximate_of` …… 但 **`revoked` 在 `memory_records` 里没有任何承载列**
> ⇒ 在读取面写死 `revoked: false` 就是**凭空断言一条判定输入**。

这是 W94 的形状：**注入「判定」而非注入「事实」**。所以 H04 的缝**不是「没人调函数」**，
而是**「判定要读的那条事实，仓库里根本不存在」**——缺的是**承载物**，不是调用方。

### 与 W103–W107 同母题，第 6 轮

判定面存在、执行体不存在（H15 → H02 → H18 → H01 → H03 → **H04**）。本轮与前五轮的**不同点**：
前五轮的执行体是**已有的**（哈希链/领域表在那儿，只是没人调）；本轮**连执行体的输入都不存在**
⇒ 必须**先造承载物**，接线才有东西可读。

### 承载物 MUST 是一次新迁移，不能编辑 0001

`migration::verify()` 校验**每个已应用迁移文件**的 SHA-256（`0006-utterance-display-text.sql`
的注释逐字写着：编辑 0001 会让所有既有数据库失效）⇒ 加列只能走新迁移。

## What Changes

- **承载物**：`migrations/sqlite/0008-memory-revocation.sql`
  —— `ALTER TABLE memory_records ADD COLUMN revoked_at_ms INTEGER;`（**可空**，NULL = 未撤销）。
  **同族同形、不新造**：`permission_grants` / `approvals` / `devices` 三处既有撤销列**都**是
  `revoked_at_ms INTEGER` 可空 ⇒ 本列沿用同一形状。

- **版本号成套四处**（W51 回归教训：`db_schema_version` 与 `migration::VERSION` 成对，漏一处 CI 就红）：
  `migration::VERSION` 7→8、`SQL` 数组 7→8、`schemas/config/backup.schema.json` 的
  `db_schema_version` const 7→8、契约样本断言（`tests/knowledge_publication.rs`）7→8。

- **三条派生，各来自一条不同的仓库事实**（这是本轮的核心：判定输入 MUST 是事实，不是默认值）：

  | 条目级输入 | 来自哪条仓库事实 | 对应拒因 |
  |---|---|---|
  | `revoked` | `memory_records.revoked_at_ms IS NOT NULL` | `Revoked` |
  | `permitted` | `permission_grants` 里该 actor 对该记录作用域的授权**未被撤销** | `PermissionDenied` |
  | `approximate_of` | 恒 `None`——表里**不存**顶替条目（结构蕴含） | `ApproximateSubstitution` |

  三条**互不顶替**：撤销记录 ≠ 撤销授权 ≠ 顶替条目，正好对应 §42.4 四类触发中的三类
  （`Ttl` 走已有的 `expires_at_ms`）。⇒ `PermissionDenied` 在本轮是**可达的**
  （`recover(restore=true)` 会撤销所有 `permission_grants`），不是一个空转的分支。

- **存储层新增两个入口，既有 `Store::memory_records` 一个字不改**：
  - `memory_scoped`：按 **workspace** 取、**不按 session 过滤**，并带出 `revoked_at_ms`。
    ⚠️ 为什么不复用 `memory_records`：它 `WHERE workspace_id=?1 AND session_id=?2`
    **已经在做作用域判定** ⇒ 拿它喂 `read_records`，`out_of_scope` 永远**没有对象**
    ⇒ 守卫在、但它守的东西被上游先筛掉了（**假阴性**，最危险的方向）。
  - `block_memory_read`：遗忘链的**阻断落点**，写 `revoked_at_ms`（只置一次，不覆盖）。

- **runtime 新增两个入口**（`crates/nai-runtime/src/memory.rs`）：
  - `Composition::read_memory(query, policy)`：取数 → **诚实派生** `ScopedRecord` → 过 `read_records`。
    请求级违规与条目级违规分开；**判定一次**（存储层不重跑 §42.3）。
  - `Composition::forget_memory`：先写 `revoked_at_ms`（阻断读取）→ 再过 `forget()` 判定陈述。
    **顺序不可交换**（§42.4 原话：先阻断、再清除）。

## Modified Capabilities

- `runtime-composition`：新增一条 ADDED Requirement——记忆读取面的三条判定输入 MUST 各由一条
  仓库事实派生。既有两条同族需求（H03 写入面接线、判定一次）的行为**不变**。

## Impact

- 迁移：`migrations/sqlite/0008-memory-revocation.sql`（**新增**）。
- 版本号：`crates/nai-storage/src/migration.rs`、`schemas/config/backup.schema.json`、
  `crates/nai-storage/src/tests/knowledge_publication.rs`、TS 生成物（本轮用
  `cargo xtask schema --check` **真验**，本机实测可跑 ⇒ 不再是 W51 那轮的「不可再生面」）。
- 存储：`crates/nai-storage/src/store.rs`（新增两个方法）。
- runtime：`crates/nai-runtime/src/memory.rs`（新增两个方法）。
- 测试：`crates/nai-runtime/tests/memory.rs` 扩（新增测试目标 ⇒ Rust 目标数 +1，
  由 `cargo test --workspace` 自动覆盖 ⇒ **无需** `engineering_checks` 登记）。
- 文档：`docs/adr/0087-memory-read-surface.md`、`docs/reports/2026-10-15-w109-memory-read-surface.md`；
  活文档 `runtime.md` / `storage.md` / `kernel.md` 里 W107 写的「H04 未接线」叙述**就地订正**；
  `tests/registry.yaml` 的 H03 / H04 notes 追加 W109 段。

### 明确不做

- **不改** `admit_write` / `read_records` / `forget()` 的任何判定规则（一行未改）。
- **不改**既有 `Store::memory_records`（有测试专门钉它「已过期记录仍被读回」）。
- **不新增权限名**：复用 `session.manage`（与 W107 同一个选择，不扩张授权面）。
- **不改任何验收 `status`**：H03 / H04 仍 `pending`（交付判定面 ≠ 验收达标）。
- **不做** §42.5 Pro 跨会话、**不做**配额来源（`budget` 由调用方给，本轮不发明配额）。
- **不做**遗忘链的**清除**侧（`purge_state` 只如实登记 `NotStarted`）——清除是另一条工作。
