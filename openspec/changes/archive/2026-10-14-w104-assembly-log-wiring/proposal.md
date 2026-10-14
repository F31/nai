## Why

V3.11 §41.5 规定上下文装配 MUST 落决策日志，且 MUST 可由**决策日志的片段引用**重建、MUST NOT 依赖当时内存。
W69 把这条判定落成了 `crates/nai-kernel/src/evaluate/assembly.rs` 的 `rebuild_assembly`，并在该模块与
`docs/kernel.md` 里**明确声明**边界：

> 把装配记录追加进 `nai-storage` 的哈希链、以及执行 §47.2 的六类回归（能力包 **H16**），都是 runtime 与
> 证据生产侧的事；故报告 MUST NOT 出现「装配器已接入」「已不依赖当时内存」这类运行期断言。

这句话当时是**诚实**的，问题在于它也是**不可核实**的：

```
$ grep -rn 'rebuild_assembly\|AssemblyRecord\|LoggedFragment' crates/ apps/
crates/nai-kernel/tests/*.rs        ← 只有测试
```

生产代码**零调用方**。于是两条今天**无法被判**的命题，无论仓库里有多少测试：

1. 一次**被判可重建**的装配，会不会**照样**被写进链？（没有调用方 ⇒ 没有答案）
2. 真写进链的那份载荷，能不能**只靠链**重排出同一段片段序列——还是有人顺手只存了指向当时内存的指针？

这与 W103（H15 观测接线）**完全同型**：**判定面存在、执行体不存在**。「有判定面」与「有执行体」是两件事，
前者是后者的必要条件、不是充分条件——H02 的 `notes` 自己也写着「日志真实落盘（`nai-storage` 的只追加
哈希链）与运行时接线未建，本轮交付的是判定面」。

本轮补的就是这一段接线：`rebuild_assembly` 的判定规则一行不改、**目录零漂移**，但**契约并非一个字不加**——
给装配记录一个自己的种类必须同步存储 schema（`decision_log` 的 `kind` CHECK 是与枚举并存的第二个闭世界，
见 What Changes 第一条）。这是**实测发现**、不是预设：首跑即被 `ack.persisted == false` 抓出。

## What Changes

- **新增** `migrations/sqlite/0007-decision-log-assembly-kind.sql`（**本轮实测发现的第二个闭世界**）：
  `0005` 把种类写成 `CHECK(kind IN(…12 个…))`，与 Rust 的 `DecisionKind` 枚举构成**同一件事的两处定义**。
  只改枚举而不改 CHECK ⇒ 插入被 schema 拒绝、**静默落入遥测缓冲**（W104 首跑即复现：`ack.persisted == false`）。
  SQLite 无法就地改 CHECK，故迁移**逐列重建**该表、把既有行**逐字复制**（哈希一并保留）、并重建两个只追加触发器；
  `migration::VERSION` 6 → 7，`schemas/config/backup.schema.json` 的 `db_schema_version` const 同步 6 → 7。
- **新增** `crates/nai-runtime/src/assembly.rs`：
  - `Composition::record_assembly(&AssemblyRecord, &[LoggedFragment])`——**判定 + 追加**合并为一处。
    判定为**可重建** ⇒ 追加一条 `DecisionKind::Assembly` 条目，载荷**恰为**判定放行的那一份材料
    （按进入顺序的片段 `id`/`source_ref`/**正文**、模型版本、提示词版本），MUST NOT 退化成指针。
    判定为**不可重建** ⇒ 返回**具名拒绝** `AssemblyRefusal`（逐条拒绝名 + 三类问题 id），
    **一个字节都不落盘**。
  - `Composition::rebuild_assembly_from_log(&AssemblyRecord, limit)`——**只读真实哈希链**：读回
    `assembly` 条目、按其自身顺序解出被引用片段，再交给既有的 `rebuild_assembly` 判定，
    结果映射为拥有所有权的 `AssemblyRebuild`。判定是**调用**既有判定函数，不是复写第二份。
- **新增** `crates/nai-runtime/tests/assembly.rs`：真实 store 端到端、往返重建、正文真落盘、
  顺序判据（跨调用写两条 assembly 使 `RebuildOrderDiffers` 可触发）、两类具名拒绝、无装配可重建情形、
  超大载荷为 `Err`，并含**能区分两种实现**的非空洞守卫（见 design.md §5）。
- **ADDED** `runtime-composition` 的两条需求（可重建 ⇒ 追加完整片段材料；不可重建 ⇒ 具名拒绝且链不动）。
- **MODIFIED** `agent-harness` 的「Every assembled fragment is sourced, scoped and reconstructible」需求末句
  ——W69 写的「重建 MUST **只**消费日志」在本轮之后仍成立，但需就地 1:1 追加一句指向 `runtime-composition`
  的新落点，使该需求 MUST NOT 被读作「已落链 / 已在运行期重建」（**只动这一句**，其余场景逐字不动）。
- **登记**：`tests/registry.yaml` 的 H02 追加 `notes`，说明 runtime 接线已落地面**验收仍 `pending`**
  （判据「真的落进了链且能只靠链重建」已可测，但 H02 的 `automation: integration` 还捆着真实装配器产生者，
  那是 H01/H16 的活）。**不改任何验收 `status`**。

### Modified Capabilities

- `runtime-composition`：新增「可重建的装配记录才进哈希链」与「不可重建的装配记录具名且不动链」两条需求。
- `agent-harness`：把 W69 那条需求末句补一句指向 runtime 侧新落点（1:1 行替换）。

## Impact

- 代码：`crates/nai-storage/src/decision.rs`（**+1 枚举变体** `DecisionKind::Assembly`）、
  `crates/nai-storage/src/migration.rs`（`VERSION` 6 → 7、`SQL` 数组 +1 段、新增升级测试）、
  `crates/nai-runtime/src/{lib,assembly}.rs`、`crates/nai-runtime/tests/assembly.rs`；
  **零新增依赖**（`nai-kernel` 与 `nai-storage` 均已是 `nai-runtime` 的既有依赖）。
- 存储：**新增迁移** `migrations/sqlite/0007-decision-log-assembly-kind.sql`（无法就地改 CHECK ⇒ 逐列重建
  `decision_log`、逐字复制既有行、重建索引与两个只追加触发器）。迁移与升级路径由
  `migration::tests::version_six_upgrade_preserves_the_decision_chain_and_accepts_the_assembly_kind`
  **本机实测**（v6 库上先证旧 CHECK 拒绝 `assembly`，升级后证既有行逐字保留、新种类被接受、触发器仍在）。
- 契约：`db_schema_version` 随 `migration::VERSION` 同步 ⇒ `schemas/config/backup.schema.json`（const 6 → 7）、
  `packages/contracts-ts/src/generated/config/backup.ts`（生成物，6 → 7）、
  `tests/contracts/schema.test.mjs`（样本 6 → 7，并把「紧邻上一版」负例由 5 改为 6）、
  `crates/nai-storage/src/tests/knowledge_publication.rs`（清单断言 6 → 7）。
  ⚠️ **如实披露**：`db_schema_version` 在 `TS_GENERATED_SOURCES` 内，其再生入口
  `cargo xtask schema --check`（→ `scripts/contracts/generate.mjs`，内部 `spawnSync(process.execPath)`）
  在宿主机 `EBUSY`、**本机跑不了**。生成物由**手工同步一个 token**（当前仓内
  `backup.ts: db_schema_version: 6;` 与 schema const `6` 逐字对应，该不变量由已提交的一对确认），
  由 CI 的 `schema --check` 复核。
- `DecisionKind` 枚举本身**不在任何 `schemas/` 契约里**（`grep` 空结果）⇒ 新增变体不改任何契约名。
- 门控：测试进既有 `cargo xtask check --profile full`（`portable` 覆盖 runtime），**不新增第二个门控入口**。
- 文档：`docs/kernel.md`（把 W69 的「runtime 与证据生产侧」边界指向新落点）、`docs/storage.md`（telemetry
  种类枚举 1:1 加 `assembly`；迁移段与 `db_schema_version` 的**陈旧值 5 一并就地订正为 7**）、
  `docs/development-plan.md`、`AGENTS.md`、`docs/adr/0082-*.md`、报告。
- **明确不做**：不改 `rebuild_assembly` 的任何判定规则；不新增契约名；不实现装配器的**真实产生者**
  （喂给模型、执行 §47.2 六类回归属 H01/H16）；不改任何验收 `status`（H02 仍 `pending`）。
