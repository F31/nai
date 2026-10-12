## Why

V3.9 §51.3 把 harness 的**基础纪律**留在首版门槛（R1）：装配与预算（H01、H02）、记忆读写（H03、H04）、
**观测入决策日志（H15）**、**逐层具名降级（H17）**、模式爬坡（H18）。其中 W56（H01/H02）与 W57（H03/H04）
已在 `crates/nai-kernel/src/evaluate/memory.rs` 落下判定面，**W63（H15/H17）是 R1 这条轨道上唯一完全未开始的工作项**。

现状（实测）：§47.1 定义了五类观测记录、§47.4 定义了六行具名降级表、INV-32 要求「不得静默降级为
空上下文的自由发挥」，但 `crates/` 内**没有任何 harness 观测或逐层降级的判定面**——`grep -rn 'harness' crates/`
零命中判定逻辑。因此「有没有静默降级」今天**只能靠人读代码判断，不能靠测试判断**，这正是 W32–W36 以来
各轮「判定骨架」要解决的那类缺口。

本轮不做容量扩张：**不新增依赖、不新增进程、不新增契约名**（§46.5 的目录零漂移），
只在既有 `evaluate/` 下加一个纯逻辑判定模块与测试，并把它纳入既有 `check --profile full`。

## What Changes

- **新增** `crates/nai-kernel/src/evaluate/harness.rs`：
  - §47.1 的**五类观测**（装配／意图／记忆／工具／缓存）各带必需记录项；记录进日志前 MUST 通过
    明文凭据检查（INV-11）与观众原文脱敏（§5.8），缺项与泄漏一律**具名拒绝**。
  - §47.4 的**六类失败层**各自落到 §12.2 降级顺序（模板 → 已批准缓存 → 人工 → 静音待机）中的确定一级，
    带具名错误码、可见标记与可行动说明；并在上下文失去策略／人设约束时判为**禁止自由发挥**（INV-32）。
- **新增** `crates/nai-kernel/tests/harness.rs`：含**能区分两种实现**的非空洞守卫（见 design.md 第 6 节）。
- **ADDED** `agent-harness` 的两条需求（五类观测可判定；逐层降级具名且有序）。
- **登记**：`tests/registry.yaml` 的 H15／H17 追加 `notes`，说明机制已落地而**验收仍 `pending`**
  （沿用 W57 对 H03/H04 的口径：判定骨架 ≠ 真实路径已接线）。

### Modified Capabilities

- `agent-harness`：新增「五类观测在入日志前可判定」与「逐层降级具名、有序且可见」两条需求。

## Impact

- 代码：`crates/nai-kernel/src/evaluate/{mod,harness}.rs`、`crates/nai-kernel/tests/harness.rs`；零新增依赖。
- 门控：测试进既有 `cargo xtask check --profile full`（`portable` 覆盖 kernel），**不新增第二个门控入口**。
- 文档：`docs/kernel.md`（模块清单 + 具名拒绝数）、`docs/development-plan.md`（W66 条目）、`AGENTS.md`（轮次段）。
- **明确不做**：不把记录真的追加进 `crates/nai-storage/src/decision.rs` 的哈希链（那是 runtime 接线，
  本轮不宣称「已进决策日志」）；不改 `CapabilityView` 的契约字段（不新增契约名）；不改任何验收 `status`；
  不实现意图、缓存、工具循环、MCP、A2A（H05–H14 仍属能力包）。
