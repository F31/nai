# ADR 0085：§42.2 记忆写入判定接上真实记忆存储（H03 runtime 接线）

- 状态：已接受
- 日期：2026-10-14
- 轮次：W107
- 关联：[ADR 0084](0084-assembly-judgement-wiring.md)、[ADR 0083](0083-ramp-enablement-log-wiring.md)、
  [ADR 0082](0082-assembly-log-wiring.md)、[ADR 0081](0081-observation-log-wiring.md)
  （H01/H18/H02/H15 的同型接线，本轮的直接前驱）；
  [ADR 0067](0067-viewer-memory-single-answering-point.md)（观众记忆开关的单一应答点，本轮的写侧半边）、
  [ADR 0027](0027-trace-id-single-definition.md)（一处定义）

## 背景

V3.8 §42.2 把记忆写入钉成一道**来源门槛**：

> 每次记忆写入 SHALL 声明写入来源（已批准且已播完／已批准未播出／工具返回／外部智能体返回／评论原文／
> 模型草稿）、作用域、来源标识、时间，以及「叙述还是权威事实」的标注。只有**已批准且已播完**的内容
> 可写为事实……缺作用域、来源标识或时间的写入 MUST 被具名拒绝。

W57 交付了这道门槛的**判定面**（`crates/nai-kernel/src/evaluate/memory.rs` 的 `admit_write`），
W57 的迁移同时把 `memory_records` 表建进 `0001-foundation.sql`（九列，与 `MemoryRecord` 契约九字段
逐字对应）。两半都在，**中间没有接线**：

```
$ grep -rn '\badmit_write\b' --include=*.rs crates apps | grep -v /tests/ | grep -v "src/evaluate/memory.rs"
（空）
$ grep -rn 'MemoryCandidate\|MemoryPolicy' --include=*.rs crates apps | grep -v /tests/ | grep -v "src/evaluate/memory.rs"
（空）
$ grep -rn 'memory_records' --include=*.rs crates apps | grep -v /tests/
crates/nai-storage/src/tests.rs:375
crates/nai-storage/src/tests.rs:432
```

前两条**零命中**；第三条命中的两处都在测试里，且只是**目录级**断言（「表在目录里」「新库这张表为空」）。
于是两条今天**无法被判**的命题：

1. 一次被**判定拒绝**的记忆写入，在 runtime 上**会不会**落库（不是「判定面说该拒」，而是「真有执行体不落」）？
2. 被**放行**的写入，到底落进了哪张表——哈希链，还是记忆表？

这与 W103（H15）、W104（H02）、W105（H18）、W106（H01）**完全同型**，是**判定面存在、执行体不存在**
在本仓连续**第五轮**现身。与前四轮**不同的一点**：前四轮的执行体都是**只追加哈希链**，本轮是**领域表**。

## 决策

### 决策 1：执行体落在 `memory_records` 表，不是决策日志

§42.2 的拒因后果原文是「MUST NOT **落库**」（`agent-harness` 的
`An approved but unplayed candidate is offered as a fact` 场景），指向领域表。

并且记忆**观测**落链**已经**由 W103 的 `Composition::record_observation` 承担（`agent-harness` 的
「五类观测」需求明写其落链由 `runtime-composition` 的「A judged-loggable harness observation reaches
the hash chain」承担）。本轮若也写一条决策日志条目，就是**同一件事的第二处定义**（W88：一个东西
MUST NOT 两处定义），且需要一个新的 `DecisionKind` 变体 ⇒ 一次迁移。

⇒ 一次写入、一条路径、落 `memory_records`。

### 决策 2：判定与落库是**一处**入口

`Composition::write_memory(&mut self, candidate, policy) -> Result<MemoryWriteOutcome>` 先跑
`admit_write`，放行才落库。拆成 `admit` 与 `persist` 两个出口，会把「不许把未放行的写入落库」的责任
推给每个调用方——一处遗漏就破坏不变量，而没有任何测试看得见。理由与 W103–W106 一致。

### 决策 3：`MemoryWriteOutcome` 恰**两态**

H03 只有**一项**判定（§42.2 的来源门槛），`admit_write` 返回 `Result<(), MemoryRejection>` ⇒ 只有
一个「放行 / 不放行」的轴。故 `Written` / `Refused` 两态。

W106 的 `AssemblyOutcome` 是四态，因为 H01 有**两项**判定（可调用 × 可重建）。此处多造第三态会是
**没有对应事实的空壳**——即 W92 教训「『记成什么』答不出 ⇒ 先问分类是否少一格」的**反向**应用：
分类在此是**恰够**的，多一格就是**无对象的分类**。

### 决策 4：判定**一次**，存储层不重跑，也不代答开关

`Store::record_memory` 只做存储层自己的事：`id()` 校验作用域标识、`access()` 查会话权限、
写九列、主键冲突映射成具名 `Conflict`。它 **MUST NOT** 重跑「来源可信吗 / 是事实还是叙述」——
那是 `admit_write` 的判定，重跑就是「一条开关答两次」（W88），两处口径会分家。观众长期记忆开关
同理：写入面**调用**判定的结果，**不自己复写** `if !policy.viewer_memory_enabled`（ADR 0067）。

### 决策 5：九列**逐字**取自被判定审过的输入，多一列都不造

`MemoryRecord` 九字段与 `memory_records` 九列逐字对应（`viewer_scoped_id` → `viewer_scope`、
`expires_at_utc_ms` → `expires_at_ms`），故**零迁移**。

判定还审了 `source` / `claim` / `source_ref` / `written_at_utc_ms` / `publish_ref`，但 §42.2 把它们
定为**判定输入**而非持久化字段，表中无对应列 ⇒ **不落库**。为「看起来更完整」多造列，就是给这张表
第二处定义。

### 决策 6：存储层**不**加写入面的时效规则

`record_candidate` 会在存储层拒掉已过期候选；`record_memory` **不**做这一步。§42.3 把时效判定放在
**读取**面（`Expired` 是条目级拒因），写入面没有这条规则。在存储层加一条判定面从未声明的规则，
会让「判定面声明了什么」与「存储层实际做了什么」变成两个口径。

### 决策 7：写入面权限名复用 `session.manage`（不新增 `memory.write`）

本轮补的是「判定 → 执行体」的接线；新增权限名要连带改 `Composition::bootstrap_owner` 的授权表与
测试夹具 `grant()`，那是一处**与判定无关**的授权面扩张。记忆记录本就是**会话作用域**内的产物，
复用语义亦成立。⚠️ 这是**选择**，不是「本来如此」——如实披露在报告与 `docs/storage.md`。

### 决策 8：H04 读取面**不在本轮**，登记为具名开放问题

`read_records` 的条目级门控输入是 `ScopedRecord` 的 `revoked` / `permitted` / `approximate_of`。
`permitted` 可由 `permission_grants` 诚实派生、`approximate_of` 恒 `None`，
但 **`revoked` 在 `memory_records` 里没有任何承载列**——§42.4 的遗忘链是一条**真实状态**
（`ReadSideNotBlocked` 等四条拒因都在描述它）。

⇒ 在读取面写死 `revoked: false` 就是**凭空断言一条判定输入**，正是本项目明令禁止的「注入**判定**
而非注入**事实**」（W94 的形状）。故 H04 的接线留给新的一轮，并把「如何诚实派生 `revoked`」登记为
具名开放问题，**不假装已接**。

配套：`Store::memory_records` 是**取数入口**（逐列还原、按作用域过滤），MUST NOT 施加 §42.3 的
任何条目级判定，MUST NOT 被读作 H04 已接线。

### 决策 9：非空洞守卫覆盖**写入面八条**，并穷尽 `match`

`refusal_sample(rejection)` 对 `MemoryRejection`（23 变体）**穷尽 `match`**：写入面的八条各给一个
样本，其余十二条显式 `return None`。kernel 新增变体而此处未决定归属 ⇒ **编译失败**，不静默丢覆盖。
守卫断言「可达的恰为这八条」（`WRITE_FACE`），并断言全部拒绝**零落库**。

## 后果

- **闭合**：`admit_write` 从零生产调用方变为有一处，且该处**必然**先判定后落库。「一次被拒的记忆写入
  会不会落库」从「只能读源码回答」变成**可测**（9 项运行期用例、七组注入共 **16 红**）。
- **零迁移、零契约改动**：不改 `DecisionKind`、不改 `migrations/**`、`migration::VERSION` 保持 **7**、
  不改 `schemas/**`、不改任何生成物、不改 `db_schema_version`。这是本轮满足「可自主推进」的关键：
  版本号连带面（`cargo xtask schema --check` 这条宿主机 `EBUSY` 路径）**一处都没碰**。
- **零新增依赖**：`nai-kernel` / `nai-storage` / `nai-contracts` 均已是 `nai-runtime` 的既有依赖。
- **权限面零改动**：`bootstrap_owner` 与测试夹具 `grant()` 一字不改（决策 7）。
- **明确留下的缝（如实登记）**：
  - **H04 读取面未接线**（决策 8），登记在 `tests/registry.yaml` 与报告；
  - `MemoryCandidate` 的五个判定输入字段**不落库**（决策 5）——这是设计，不是遗漏，登记在
    `docs/storage.md`；
  - **权限名复用** `session.manage`（决策 7）——是选择，登记在报告。
- **不变**：`admit_write` / `read_records` 及其它 kernel 判定规则**一行未改**；H03 与 H04 的 registry
  `status` **保持 `pending`**（本轮交付的是 runtime 接线，不是验收达标）。

## 替代方案

1. **只加 `Store::record_memory`，runtime 侧不加入口** —— 判定仍零调用方，「不许把未放行的写入落库」
   的责任落到每个调用方，缝原样保留。
2. **runtime 侧拆成 `evaluate_memory` 与 `persist_memory` 两个出口** —— 同决策 2 的理由。
3. **落库前在存储层重跑一遍来源判定** —— 一条开关答两次。
4. **同时接线 H04 读取面** —— `revoked` 无来源，只能凭空断言（决策 8）。
5. **新增 `memory.write` 权限名** —— 取舍见决策 7。
6. **新增 `DecisionKind::Memory` 并同时落链** —— 需要一次迁移，且与 W103 的记忆观测落链构成同一件事
   的第二处定义（决策 1）。
7. **`MemoryWriteOutcome` 用三态** —— 只有一项判定，第三态无对应事实（决策 3）。

## 关联

- 实现：`crates/nai-runtime/src/memory.rs`、`crates/nai-storage/src/store.rs`
  （`record_memory` / `memory_records`）、`crates/nai-runtime/src/lib.rs`
- 测试：`crates/nai-runtime/tests/memory.rs`（9 项 + 非空洞守卫）
- 规格：`openspec/specs/runtime-composition/spec.md`（W107 新增两条需求）
- 报告：`docs/reports/2026-10-14-w107-memory-write-gate-wiring.md`
- 活文档：`docs/runtime.md`、`docs/storage.md`、`docs/kernel.md`、`docs/development-plan.md`、`AGENTS.md`
- 登记：`tests/registry.yaml`（H03 追加接线说明、H04 追加开放问题；**两条 `status` 未改**）
