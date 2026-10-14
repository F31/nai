## Why

V3.8 §42.2 把记忆写入钉成一道**来源门槛**：每次写入声明来源、作用域、来源标识与时间；
只有**已批准且已播完**的内容可写为场次记忆；工具返回、外部智能体返回、评论原文与模型草稿
MUST NOT 写成事实或承诺；只有语义记忆可承载权威事实且必须带 §6 的发布引用。

W57 交付了这道门槛（`crates/nai-kernel/src/evaluate/memory.rs` 的 `admit_write`），并同时交付了
`memory_records` 表这一侧的**存储模式**（`migrations/sqlite/0001-foundation.sql:29`，九列，
与 `MemoryRecord` 契约九字段逐字对应）。两半都在，但**中间没有接线**。

实测这条缝（可复现，计数为现测）：

```
$ grep -rn '\badmit_write\b' --include=*.rs crates apps | grep -v /tests/ | grep -v "src/evaluate/memory.rs"
$ grep -rn 'MemoryCandidate\|MemoryPolicy' --include=*.rs crates apps | grep -v /tests/ | grep -v "src/evaluate/memory.rs"
$ grep -rn 'memory_records' --include=*.rs crates apps | grep -v /tests/
crates/nai-storage/src/tests.rs:375
crates/nai-storage/src/tests.rs:432
```

前三条**零命中**：`admit_write`、`MemoryCandidate`、`MemoryPolicy` 在**生产代码里零调用方**。
第四条命中的两处都在测试里，且只是「表在目录里」「新库这张表是空的」两条目录级断言——
**全仓没有任何一处生产读写路径**碰过 `memory_records`。

⇒ 于是一条今天无法判的命题：

> 一次**被判定拒绝**的记忆写入，究竟有没有落库？

判定面不落库（它只返回 `Result<(), MemoryRejection>`），存储面不认识判定。没有任何生产路径
把两者接起来，所以这个问题在 runtime 上**不可表达**，只能靠读源码回答。

⇒ 本轮是「**判定面存在、执行体不存在**」的又一次复现（W103/H15、W104/H02、W105/H18、W106/H01 同型，
本仓**连续第五**次）。与前四轮的落点不同：前四轮补的都是**只追加哈希链**那一侧，本轮补的是
**领域表**那一侧——`memory_records` 已付费（表在 `0001` 里就位，无需新迁移），缺的恰是写入与读回的**执行体**。

## What Changes

- **新增** `crates/nai-runtime/src/memory.rs`（组合根侧）：
  - `Composition::write_memory(&mut self, candidate: &MemoryCandidate, policy: &MemoryPolicy)
    -> Result<MemoryWriteOutcome>`：**先跑 `admit_write`**；判定拒绝 ⇒
    `Ok(MemoryWriteOutcome::Refused(MemoryWriteRefusal))` 且**一列不落库**；判定放行 ⇒
    经既有的单 owner 存储路径把记录写进 `memory_records`，返回 `Ok(MemoryWriteOutcome::Written(记录的九列))`。
  - **判定 + 落库是一处**（与 W103/W104/W105/W106 同形）。理由同前四轮：分开成 `evaluate` 与
    `persist` 两个出口，会把「不许把未放行的写入落库」的责任推给每个调用方——一处遗漏就破坏不变量，
    而没有任何测试看得见。
  - `MemoryWriteOutcome` 恰**两态**：`Written` / `Refused`。这里不是 W106 的四态：H03 只有**一项**判定
    （§42.2 的来源门槛），没有第二侧可折——多造一态会是**没有对应事实的空壳**。
- **新增** `crates/nai-storage/src/store.rs` 的两个访问器（形状照抄 `publish_fact`/`fact`/`record_candidate` 族）：
  - `Store::record_memory(&mut self, op, workspace, actor, record: &MemoryRecord) -> Result<()>`：
    校验作用域标识、查会话权限、写九列；主键冲突 ⇒ 具名 `Conflict`。
  - `Store::memory_records(&self, op, workspace, actor, session) -> Result<Vec<MemoryRecord>>`：按作用域
    读回九列，顺序确定性。**该访问器不施加 §42.3 的读取判定**（撤销／权限／配额、近似顶替），
    它是存储层的取数入口，不是 H04 的读取面——见下方「明确不做」。
- **落库的九列逐字取自被判定审过的输入**：runtime 是**传输**不是**加工**——MUST NOT 由 `content`
  重算任何字段、MUST NOT 补默认值、MUST NOT 夹带 `MemoryCandidate` 上判定没审过的字段
  （`written_at_utc_ms` / `claim` / `publish_ref` 不落库，因为 `memory_records` 的九列里没有它们）。
- **新增** `crates/nai-runtime/tests/memory.rs`：放行 ⇒ 恰一行且九列逐字相等；拒绝 ⇒ **零行**且拒绝名在场；
  遍历 `MemoryRejection` 的**写入面八条**做**非空洞守卫**（样本构造函数对拒绝枚举**穷尽 `match`**，
  kernel 加变体而此处未覆盖 ⇒ 编译失败）；读取面访问器逐字往返；观众长期记忆开关关闭时写入面同样关闭
  （PD-31 同一条判定的写侧）。
- **ADDED** `runtime-composition` 两条需求。
- **不新增** `DecisionKind`、**不加迁移**、**不改任何契约**、**不改 `migration::VERSION`**（保持 **7**）：
  `memory_records` 表在 `0001-foundation.sql` 里**已存在**且九列齐备 ⇒ 这是本轮满足
  「可自主推进」的关键——版本号连带面（如 `db_schema_version`、`cargo xtask schema --check` 这条
  宿主机 `EBUSY` 路径）**一个都不会碰到**。
- **登记**：`tests/registry.yaml` 的 H03 追加 `notes`，说明写入面的接线已落地而**验收仍 `pending`**；
  H04 追加 `notes` 记录**具名开放问题**（读取面的接线不在本轮，理由见下）。**不改任何验收 `status`**。

### 明确不做：H04 读取面不在本轮（具名开放问题，MUST NOT 被读作已接）

`read_records(records: &[ScopedRecord], …)` 的条目级门控输入是 `ScopedRecord` 的
`revoked` / `permitted` / `approximate_of` 三个字段。其中：

| 门控输入 | 能否从 `memory_records` 诚实派生 |
|---|---|
| `permitted` | 能（`permission_grants` 是真实存储事实） |
| `approximate_of` | 能（恒 `None`：表中无此概念） |
| **`revoked`** | **不能** —— `memory_records` **没有任何撤销列** |

§42.4 的遗忘链是一条**真实的**状态（`MemoryRejection::ReadSideNotBlocked` 等四条拒因都在描述它）
⇒ 在读取面写死 `revoked: false` 就是**凭空断言一条判定输入**，正是本项目明令禁止的
「**注入判定**而非注入事实」（W94 的形状）。

⇒ H04 的 runtime 接线**留给新的一轮**，并把「如何诚实派生 `revoked`」作为**具名开放问题**
登记进 `tests/registry.yaml` 与报告，**不假装已接**。

## Modified Capabilities

- `runtime-composition`：新增两条需求——
  「被放行的记忆写入落在真实记忆存储，被拒绝的具名且一列不落」、
  「记忆存储的读回是逐列还原的取数入口，不施加读取判定」。
- **不改** `agent-harness`：其 H03/H04 需求正文描述的是**判定**，本就不含任何「落库」声明
  （§42.2 正文只有一句 `MUST NOT 落库`，说的是**拒**的后果，不指定谁去落）。
  该能力下需求正文受 `MAX_REQUIREMENT_TEXT_LENGTH = 500` 约束，追加澄清亦不可能容纳。

## Impact

- 代码：`crates/nai-runtime/src/{lib,memory}.rs`、`crates/nai-runtime/tests/memory.rs`、
  `crates/nai-storage/src/store.rs`；**零新增依赖**（`nai-kernel`、`nai-storage`、`nai-contracts`
  均已是 `nai-runtime` 的既有依赖）。
- **零迁移、零契约改动**：不改 `DecisionKind`、不改 `migrations/**`、不改 `migration::VERSION`（保持 **7**）、
  不改 `schemas/**`、不改任何生成物、不改 `db_schema_version`。
- **权限面零改动**：写入口复用既有权限名 `session.manage`（与同族的 `record_candidate` 一致）
  ⇒ `bootstrap_owner` 的授权表与测试夹具 `grant()` **一字不改**。这一条是**刻意**的：本轮补的是
  「判定 → 执行体」的接线，不是引入新的授权面。
- 存储读取面**零污染**：`memory_records` 的既有读者只有 `crates/nai-storage/src/tests.rs` 的两条
  目录级断言（「表在目录里」「新库为空」），本条改动**不修改**它们；新访问器**不改**任何既有方法的行为。
- 门控：测试进既有 `cargo xtask check --profile full`（`portable` 覆盖 runtime 与 storage），
  **不新增第二个门控入口**；不新增独立 Node 测试文件 ⇒ 无需 `engineering_checks` 登记。
- 文档：`docs/storage.md`（新增记忆表段）、`docs/runtime.md`（新增 W107 段）、`docs/kernel.md`
  （记忆面边界句指向新落点）、`docs/development-plan.md`、`AGENTS.md`、`docs/adr/0085-*.md`、报告、
  `tests/registry.yaml`。
- **明确不做**：不改 `admit_write`／`read_records` 的任何判定规则；不新增种类／迁移／契约／权限名；
  **不接线 H04 读取判定**（具名开放问题）；不新增第二条决策日志写入路径（记忆观测的落链由 W103 的
  `record_observation` 承担，本轮的落点是 `memory_records` 表）；不改任何验收 `status`。
