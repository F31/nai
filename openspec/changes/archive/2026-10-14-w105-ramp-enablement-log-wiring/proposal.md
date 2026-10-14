## Why

V3.11 §47.3 规定：harness 机制晋升到受监督自动 MUST 过 §47.2 回归，且 MUST 有一条启用记录。
W67 把这条判定落成了 `crates/nai-kernel/src/evaluate/ramp.rs` 的 `evaluate_promotion`，并在该模块的
**模块文档**里明确声明边界：

> **Scope boundary.** This module decides *whether a promotion is allowed*. Enabling a mechanism per
> stage at runtime, and appending the record to the hash chain in `nai-storage`, is wiring and is
> **not** claimed here.

这句话当时是**诚实**的，问题在于它也是**不可核实**的——而且比 W103/W104 那次更刺眼：判定面
**自己**把要落盘的记录算好了，却没人落盘。

```
$ grep -n 'record_to_append' crates/nai-kernel/src/evaluate/ramp.rs
257:    pub record_to_append: Option<EnablementRecord>,
322:    let record_to_append = match (allowed, req.record) {
```

`RampVerdict` **自带** `record_to_append: Option<EnablementRecord>`（`ramp.rs:257`）——判定面已经
把「允许时要追加的那份记录」算成了一个**拥有所有权**的值，`ramp.rs:322` 还专门为它写了构造分支。
而它的**唯一消费者是测试**。于是两条今天**无法被判**的命题：

1. 一条**判定放行**的晋升，它算出来的那份启用记录，会**照样**进链吗？（没有调用方 ⇒ 没有答案）
2. `record_to_append == None` 有两种**必须区分**的原因——①被判**拒绝**（`allowed == false`）；
   ②放行但目标阶段在受监督自动**之前**（「其他阶段不携带启用要求」，`ramp.rs:256` 的原话）。
   今天没有任何运行期形状能区分它们：一个把 ② 折进「拒绝」的实现和一个把 ② 折进「已落日志」的实现，
   **都能通过全部 13 项 kernel 测试**。

这与 W103（H15 观测接线）、W104（H02 装配接线）**完全同型**：**判定面存在、执行体不存在**。
本轮把这半段补上。「有判定面」与「有执行体」是两件事——H18 的 `notes` 自己也写着
「**仍未接线**——按阶段真正启用机制、把记录追加进哈希链是 runtime 工作」。

**与 W104 的一处关键不同**：本轮**不改存储 schema、不加迁移、不动任何契约**——见 What Changes 第二条。

## What Changes

- **新增** `crates/nai-runtime/src/ramp.rs`：
  - `Composition::record_promotion(&PromotionRequest<'_>) -> Result<RampOutcome>`——**判定 + 追加**
    合并为一处（与 W103/W104 同形，理由同：分开两个出口会把不变量的责任推给每个调用方）。
  - **三态**结局 `RampOutcome`（**不是** W103/W104 的两态）：
    - `Logged(DecisionAck)`——判定放行**且**产出了启用记录 ⇒ 追加一条 `approval` 类条目；
    - `NoRecordRequired { from, to }`——判定放行但目标阶段在受监督自动**之前** ⇒ **不追加**，
      且这**不是**拒绝；
    - `Refused(RampRefusal)`——判定拒绝 ⇒ 具名拒绝（逐条拒绝名 + 缺失回归类别 + 凭据泄露字段名），
      **一个字节都不落盘**。
    **为什么必须是三态**：`record_to_append == None` 的两种原因若塌成两态，② 就只能被报成
    「已落日志」（谎：什么都没落）或「已拒绝」（谎：它没被拒）。本项目已记过这个形状——「**「记成什么」
    答不出 ⇒ 先问分类是否少一格**」。
  - **复用既有种类 `DecisionKind::Approval`，不新增种类、不加迁移**：该值在 Rust 枚举
    （`crates/nai-storage/src/decision.rs:31`）与 `migrations/sqlite/0005-decision-log.sql:1` 的
    `kind CHECK(kind IN(…))` 两个闭世界里**都已存在**，故 §47.3 的启用记录**不会**触发 W104 那种
    「枚举改了、表 CHECK 没改」的静默落缓冲。选它而不选 telemetry 类的理由见 design.md §3：
    `criticality()` 把它归 **Critical** ⇒ **写入失败上报错误**，与 §47.3「启用记录已备」的要求同向
    （「要么落地、要么被报告，绝不静默跳过」）。
- **新增** `crates/nai-runtime/tests/ramp.rs`：真实 store 端到端、三态可区分性、载荷**恰为**判定放行的
  那份材料（含「不夹带 `touched`/`passed`」的键集断言）、被拒一字节不落盘且凭据字面量不出现在任何
  已落盘行、九条具名拒绝各自的运行期场景，并含**能区分两种实现**的非空洞守卫（遍历
  `RampRejection::ALL` 八条，穷尽 `match`）。
- **ADDED** `runtime-composition` 的两条需求（放行 ⇒ 落记录；拒绝或非门控阶段 ⇒ 具名且链不动）。
- **MODIFIED** `agent-harness` 的「Harness mechanisms ramp through stages, and supervised automatic mode
  is gated」需求**末句**——W67 写的「按阶段真正启用机制、以及把启用记录追加进只追加哈希链，是 runtime
  接线」在本轮之后仍成立，但需就地 1:1 追加一句指向 `runtime-composition` 的新落点，使该需求 MUST NOT
  被读作「已落链」（**只动这一句**，其余正文与全部 5 个场景逐字不动）。
  ⚠️ 按 W104 归档当场暴露的「需求正文 >500 字符」strict 规则，追加后**必须实测**该需求正文长度
  （改前 **436** 字符），MUST NOT 靠估计。
- **登记**：`tests/registry.yaml` 的 H18 追加 `notes`，说明 runtime 接线已落地面**验收仍 `pending`**
  （「判定之后那一步真的发生」已可测，但六类回归的**执行体**是能力包 H16，且 H18 的
  `automation: manual` 还捆着真实机制启用者）。**不改任何验收 `status`**。

### Modified Capabilities

- `runtime-composition`：新增「放行的晋升把启用记录落进哈希链」与「拒绝或非门控阶段具名且不动链」
  两条需求。
- `agent-harness`：把 W67 那条爬坡需求末句补一句指向 runtime 侧新落点（1:1 行替换）。

## Impact

- 代码：`crates/nai-runtime/src/{lib,ramp}.rs`、`crates/nai-runtime/tests/ramp.rs`；
  **零新增依赖**（`nai-kernel` 与 `nai-storage` 均已是 `nai-runtime` 的既有依赖）。
- **零迁移、零契约改动**：不新增 `DecisionKind` 变体、不改 `migrations/**`、不改
  `migration::VERSION`（保持 **7**）、不改 `schemas/**`、不改任何生成物、不改
  `db_schema_version`。这正是本轮满足「可自主推进」的关键——W104 的 `db_schema_version` 连带面落在
  `TS_GENERATED_SOURCES` 内，其再生入口 `cargo xtask schema --check` 在宿主机 `EBUSY`（本机跑不了），
  只能手工同步一个 token 并交 CI 复核；本轮**不产生**这样的面。
- 存储读取面**零污染**（实测）：`approval` 类条目的既有读者是审批后端，走的是 `approvals` **表**
  （`crates/nai-storage/src/store.rs` 的 `approval_valid` / `list_pending_approvals` / `read_approval`），
  **不**按 `decision_log.kind == 'approval'` 过滤；全仓 `grep '"approval"'` 的其余命中都在
  与本种类无关的枚举（`monitoring` 的能力名、`multiplex` 的通道名、control-api 的 category 串）。
- 门控：测试进既有 `cargo xtask check --profile full`（`portable` 覆盖 runtime），**不新增第二个门控入口**；
  不新增独立 Node 测试文件 ⇒ 无需 `engineering_checks` 登记。
- 文档：`docs/kernel.md`（W67 段的边界句指向新落点）、`docs/runtime.md`（新增 W105 段）、
  `docs/storage.md`（`approval` 类别补一句说明它同时承载 §47.3 的启用记录）、`docs/development-plan.md`、
  `AGENTS.md`、`docs/adr/0083-*.md`、报告。
- **明确不做**：不改 `evaluate_promotion` 的任何判定规则；不新增种类/迁移/契约；不实现 §47.2 六类回归的
  **执行体**（能力包 H16）；不实现「按阶段真正启用机制」的机制侧（属 H16/H01）；不改任何验收 `status`
  （H18 仍 `pending`）。
