# w109 任务

## 1 建立 OpenSpec 变更

- [x] 写 `proposal.md` / `design.md` / tasks / 2 条 ADDED + 1 条 MODIFIED
- [x] `openspec validate 2026-10-15-w109-memory-read-surface --strict` 报 `is valid`

## 2 承载物：迁移 0008 + 版本号成套四处

- [x] 新建 `migrations/sqlite/0008-memory-revocation.sql`：
      `ALTER TABLE memory_records ADD COLUMN revoked_at_ms INTEGER;`（可空，NULL = 未撤销），
      并写明「为何新增迁移而不编辑 0001」（`verify()` 校验迁移文件 SHA-256）
- [x] `crates/nai-storage/src/migration.rs`：`VERSION` 7 → **8**、`SQL: [&str; 7]` → `[&str; 8]` +
      追加 `include_str!`
- [x] `schemas/config/backup.schema.json`：`db_schema_version` 的 `const` 7 → **8**
- [x] `crates/nai-storage/src/tests/knowledge_publication.rs`：样本断言 7 → **8**
      （⚠️ **W51 的回归正是漏了这一处**，四处 MUST 同改）
- [x] TS 生成物同步并**真验**：`cargo xtask schema --check`（W108 侦察实测本机**可跑**，
      本轮不再是 W51 那轮的「不可再生面」）⇒ 实测 `no drift`
- [x] **全仓扫描旧值**：`grep -rn` 确认无 `VERSION = 7` / `db_schema_version.{const":7}` / 断言 `7` 残留

## 3 升级路径 MUST 被实测

- [x] `migration.rs` 测试模块里造一个 **v7** 库（`SQL.iter().take(VERSION-1)`）、`migrate()` 到 v8，
      断言**既有行逐字保留** + 新列可读 + 撤销列默认为 NULL（未撤销）
      ⇒ `version_eight_upgrade_preserves_memory_rows_and_defaults_the_revocation_column_to_null`
- [x] **修 VERSION 7→8 带出的回归**：既有 `version_six_upgrade_...` 用的是 `SQL.iter().take(VERSION-1)`，
      版本号一升就自动前移，实际改测 v7→v8 而**用例名变成不实陈述**、0007 那条升级路径**失去覆盖**
      且零信号 ⇒ 钉住 `const ASSEMBLY_KIND: usize = 7`（MUST NOT 用 `VERSION - 1`）

## 4 存储层：取数与阻断落点

- [x] 新增 `Store::memory_scoped`：**只按 `workspace_id` 过滤**（不按 session 预筛 ⇒ 否则
      `out_of_scope` **失去对象**），带出 `revoked_at_ms`
- [x] 新增 `Store::block_memory_read`：写 `revoked_at_ms`，**只置一次、不覆盖**已撤销行
- [x] `Store::memory_records` **一个字不改**（有测试钉它「已过期记录仍被读回」）
- [x] 两个新入口 MUST NOT 施加 §42.3 的六类判定中的任何一类（判定一次）

## 5 runtime 读取面

- [x] `crates/nai-runtime/src/memory.rs` 新增 `Composition::read_memory(query, policy, now_ms)`：
      取数 → **诚实派生** `ScopedRecord{revoked, permitted, approximate_of}` → 过 `read_records`
- [x] 三条派生各写一处并注明来源：`revoked` ← 撤销列；
      `permitted` ← **该记录所属类别要求的授权是否在场**（观众记忆看记录**自己**的
      `authorization_ref`；其余类别 §42.3 无记录级授权要求 ⇒ 结构蕴含）；
      `approximate_of` ← 恒 `None`（结构上不存在顶替条目，不加列）
      ⚠️ **订正（同轮未提交 ⇒ 就地订正）**：原写「`permitted` ← `permission_grants`」，
      实测该形状会让 `PermissionDenied` **永不可达**——取数入口已要求同一条 `session.manage`，
      条目级再查**同一条** grant ⇒ 二者恒一致 ⇒ 守卫在却无对象可守（明令禁止的形状）。
- [x] 请求级违规与条目级违规分开；`now_ms` 由调用方给（MUST NOT 在库内取墙钟以外的第二处时间源）

## 6 runtime 遗忘落点

- [x] 新增 `Composition::forget_memory`：**先**阻断（写撤销列）→ **再**过 `forget()` 判陈述
- [x] `read_side_blocked` **不是**调用方字段：由「阻断真的发生了」这一事实得出（否则调用方
      可构造恒真 `ForgetOutcome`，判定变装饰）⇒ `ReadSideNotBlocked` 经本入口**结构上不可达**

## 7 测试与非空洞守卫

- [x] 扩 `crates/nai-runtime/tests/memory.rs`：
      越作用域（**未被取数预筛**）／已过期／已撤销／权限不足（**证明可达**）／超配额裁剪可见
- [x] **顺序断言**：同一条记录同时越作用域又过期又撤销又无权限 ⇒ 报的是越作用域
- [x] 非空洞守卫：`MemoryRejection` **穷尽 `match`**（`cover`），每个拒因要么在本文件被构造，
      要么写明由谁构造 ⇒ `NOT_CONSTRUCTED_HERE` 恰为 3 条（`UnknownOperation` /
      `ApproximateSubstitution` / `ReadSideNotBlocked`），另有一个端到端用例断言
      `READ_FORGET_FACE` 十三条**实测观察集合**逐项相等
- [x] 阻断落点用例：阻断后再读 ⇒ 该条进 `refused(Revoked)`；重复阻断 ⇒ 撤销时刻**不变**

## 8 非空洞注入验证（真改文件 + 真跑 + 逐字节还原）

- [x] 生产文件 `sha256sum` 备份（`store.rs` / `memory.rs`）
- [x] I1 派生写死 `revoked: false` ⇒ **2 红**（`blocking_a_record_...`、`...end_to_end`），
      `sha256` 逐字节还原 ✓
- [x] I2 取数改回按 session 预筛 ⇒ **2 红**（`a_record_outside_the_scope_...`、`...end_to_end`；
      实测集合里 `OutOfScope` **整个消失**），`sha256` 逐字节还原 ✓
- [x] I3 阻断改为覆盖写 ⇒ **1 红**（`a_second_block_...`，差值 22ms），`sha256` 逐字节还原 ✓
- [x] 每组 ≥1 红，逐组 `sha256` 逐字节还原；`fmt --check` 干净、18/18 复绿、无 `INJECT` 残留
      ⚠️ **坑**：注入 MUST 用**编辑工具**做，MUST NOT 用 Python 读写往返——Python 会把整文件
      换行符统一（原本 LF 新行与 CRLF 旧行**混排**），`sha256` 还原校验随之失效（I2 曾因此
      只能做内容级核验，已改用编辑工具重做一遍）

## 9 文档与活文档同步

- [x] 新增 `docs/adr/0087-memory-read-surface.md`
- [x] 新增 `docs/reports/2026-10-15-w109-memory-read-surface.md`（§八 CI 段**待推送后回填**）
- [x] 活文档**就地订正**（W107 写的「H04 未接线」「撤销无承载列」「九列」等陈述**当前状态**的句子已过期）：
      `docs/runtime.md`（订正 + 新增 W109 一节）、`docs/storage.md`（九列 → 十列、撤销承载列、
      两个新访问器）、`docs/kernel.md`（边界段）
- [x] `tests/registry.yaml`：H03 / H04 notes 追加 W109 段，**`status` 仍 `pending`**
- [x] `docs/development-plan.md`：新增 W109 段；W107 段**追加勘误段**（原文是**已提交的历史叙述**，
      MUST NOT 就地改——只勘误其**理由**「`revoked` 没有承载列」）
      ⚠️ 判据是「该句在陈述**当前状态**还是**某一轮的历史事实**」：前者就地订正，后者追加勘误

## 10 闭环（流程项，非勾选）

门控 `cargo xtask check --profile full` ⇒ 归档 `openspec archive --yes` ⇒ **两条 validate 都跑**
（归档后 MUST 立刻跑 `--archived`）⇒ 一次 commit + push ⇒ **单次**低频 CI 观测（三件齐）
⇒ 回填报告 §九 ⇒ 记忆收尾。
