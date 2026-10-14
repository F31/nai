## Why

V3.11 §47.1 规定 harness 的五类观测「进入只追加的哈希链决策日志之前」必须先被判定。W66 已把这条判定
落成 `crates/nai-kernel/src/evaluate/harness.rs` 的 `evaluate_observation`，并在模块文档里**明确声明**
「Appending it to the actual hash chain in `nai-storage` is runtime wiring and is **not** claimed here」。
也就是说：**判定面存在，执行体不存在**。

现状（实测）：`grep -rn 'evaluate_observation\|ObservationRecord' crates/ apps/` 在生产代码里**零命中**，
只有 `crates/nai-kernel/tests/*.rs` 命中。于是「一条被拒的记录会不会照样进日志」「进日志的载荷是不是
脱敏后的那一份」今天**只能靠人读代码判断，不能靠测试判断**——与 W54（桌面壳）、W68（讲品循环）同型的
「缺失对象」缺口。R1 的验收项 H15 因此仍停在 `pending`。

本轮补的就是这一段接线，且**只补接线**：判定逻辑一行不改，契约一个字不加，目录零漂移。

## What Changes

- **新增** `crates/nai-runtime/src/observation.rs`：
  - `Composition::record_observation(&ObservationRecord, &ObservationPolicy)`——**判定 + 追加**合并为一处。
  - 判定为可落日志 ⇒ 追加一条 `DecisionKind::Observation` 条目，载荷**恰为**判定输出的已脱敏材料
    （层标签 + 脱敏字段 + 脱敏观众原文），MUST NOT 携带任何未经判定的原始字段。
  - 判定为不可落日志 ⇒ 返回**具名拒绝** `ObservationRefusal`（层标签 + 逐条拒绝名 + 脱敏材料），
    **一个字节都不落盘**。
  - 载荷形状由 `crate::observation` 一处定义，追加走装配根既有的单 owner `record()` 与逐次调用预算，
    **不开第二条写入路径**。
- **新增** `crates/nai-runtime/tests/observation.rs`：真实 store 端到端、五类各一条、拒绝不追加、
  凭据字面量不出现在任何已落盘行、并含**能区分两种实现**的非空洞守卫（见 design.md §5）。
- **ADDED** `runtime-composition` 的两条需求（可落日志 ⇒ 追加脱敏载荷；不可落日志 ⇒ 具名拒绝且链不动）。
- **MODIFIED** `agent-harness` 的「Harness observations are decidable before they reach the log」需求末句
  ——W66 写的「真正追加进哈希链是 runtime 接线」在本轮之后**不再成立**，就地 1:1 行替换为指向
  `runtime-composition` 的新落点（**只改这一行**，其余四段场景逐字不动）。
- **登记**：`tests/registry.yaml` 的 H15 追加 `notes`，说明 runtime 接线已落地面**验收仍 `pending`**
  （判据「真的落进了链且链校验通过」已可测，但 H15 还捆着五类的**真实产生者**，那是 H01/H02/H03/H04/B07
  的活）。**不改任何验收 `status`**。

### Modified Capabilities

- `runtime-composition`：新增「可落日志的观测记录才进哈希链」与「被拒的观测记录具名且不动链」两条需求。
- `agent-harness`：把 W66 那条需求末句从「真正追加以外都不算完成」改为指向 runtime 侧新落点。

## Impact

- 代码：`crates/nai-runtime/src/{lib,composition,observation}.rs`、`crates/nai-runtime/tests/observation.rs`；
  **零新增依赖**（`nai-kernel` 与 `nai-storage` 均已是 `nai-runtime` 的既有依赖）。
- `composition.rs` 的唯一改动是把 `next_trace` 的可见性从私有放宽到 `pub(crate)`——**没有第二份实现**，
  也不新增第二个 trace 生成器（沿用 ADR 0027「一处定义」）。
- 门控：测试进既有 `cargo xtask check --profile full`（`portable` 覆盖 runtime），**不新增第二个门控入口**。
- 文档：`docs/runtime.md`、`docs/kernel.md`（指向接线落点）、`docs/development-plan.md`、`AGENTS.md`。
- **明确不做**：不改 `evaluate_observation` 的任何判定规则；不新增契约名（`/upgrades/*` 那类动 OpenAPI 的
  活本轮避开，因为再生路径 `cargo xtask schema --check` 在宿主机 `EBUSY`、本机验不了）；
  不实现观测的**产生者**（装配/意图/记忆/工具/缓存的真实调用点属 H01/H02/H03/H04/B07）；
  不改任何验收 `status`（H15 仍 `pending`）。
