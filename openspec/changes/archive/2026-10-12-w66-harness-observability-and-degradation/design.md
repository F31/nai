# Design — W66 harness 观测与逐层具名降级（H15 / H17）

## 1 背景与边界

V3.9 §47.1 与 §47.4 已经把 harness 的观测项与降级表写成了**散文**：五类观测记录、六行具名降级、
INV-32「不得静默降级为空上下文的自由发挥」。但 `grep -rn 'harness' crates/` 在判定逻辑上**零命中**——
也就是说，今天要回答「这次调用有没有静默降级」，只能靠人读代码。本轮把它变成**测试能判**的东西。

W63 是 R1 这条 harness 轨道上**唯一完全未开始**的工作项（W56 交付 H01/H02、W57 交付 H03/H04 的判定面）。

## 2 为什么落在 `evaluate/` 而不是新建顶层模块

`crates/nai-kernel` 里既有两种落法：判定骨架轮（W32–W36、W49）建顶层模块（`device.rs`/`bridge.rs`/
`realistic.rs`/`demo.rs`），而 W57 选择**扩展 `evaluate/memory.rs`**。

本轮选后者的理由是可核实的，不是偏好：

1. `evaluate/` 的模块契约就是「纯逻辑、无 I/O、判定来自受控规则而非模型自报」（见 `evaluate/mod.rs` 头注释），
   观测与降级判定完全符合；
2. `EvaluateError` 与「具名拒绝」这套 house style 就长在这里（`MemoryRejection` 23 个具名拒绝 + `ALL` 常量）；
3. `memory.rs` 的头注释**已经引用了 INV-32**（「every layer failure is named」）——本轮是同一条不变量的
   后半段（观测侧 + 降级侧），放在同一个 `evaluate/` 下才不会出现两个都声称实现 INV-32 的模块。

`docs/kernel.md` 的模块清单是本轮的同步对象之一（W57 的教训：动手前先看有没有人做了一半）。

## 3 为什么不复用 `bridge.rs` 的 `Degradation`

`crates/nai-kernel/src/bridge.rs:330` 已有一个 `Degradation`，但它**只服务 §119 的平台中断**
（字段是 `bridge_lost` / `auto_reply_revoked` / `manual_available` / `replay_available`）。
harness 的降级主体是**调用内部的六层**（§47.4），与平台链路中断是两件事：
一个 MCP 对端不可用（`Peer`）不会让 `bridge_lost` 为真，反过来亦然。
把两者塞进同一个结构会让「这次降级属于哪一类」重新变得不可判定——正是本轮要消除的状态。
故**新建** `harness` 侧的类型，并在 doc-comment 里显式写明与 `bridge::Degradation` 的分工。

## 4 H15：五类观测在入日志前可判定

§47.1 的五层（装配／意图／记忆／工具／缓存）各有一组**必需记录项**，由 `ObservationLayer::required_fields()`
给出，逐项照抄 §47.1 表格，不增不减。判定函数 `evaluate_observation` 输出：

- `loggable: bool` —— 只有全部必需项齐备、且无凭据泄漏、且观众原文已脱敏时才为真；
- `rejections: Vec<ObservationRejection>` —— 每一项都**具名**，沿用 `MemoryRejection` 的扁平枚举 +
  `ALL` 常量 + `label()` 形状；
- `redacted: BTreeMap<String, String>` —— 可落日志的**已脱敏**载荷，不是原样透传。

凭据判定（INV-11）用固定的 `CREDENTIAL_MARKERS` 字面量集合做**值扫描**，不是字段名黑名单。
观众原文与昵称按 §5.8 的保留期脱敏：超期或未脱敏分别具名拒绝。

**诚实边界**：本轮判定的是「这条记录**能不能**进日志、以什么形态进」。
**真正把它追加进 `crates/nai-storage/src/decision.rs` 的哈希链是 runtime 接线，本轮不做**，
故报告与规格里 MUST NOT 出现「已进决策日志」的措辞。

## 5 H17：逐层降级具名、有序、可见

§12.2 的四级（模板 → 已批准缓存 → 人工 → 静音待机）是**输出形态的回退链**。§47.4 给每个失败层指定了
**首选级**，本轮把「首选级 + 可用性」合成为确定落点。

| 失败层 | 具名错误码 | 首选级 | 拒绝调用 | 继续时丢弃 | 影响既有会话 |
|---|---|---|---|---|---|
| 装配（不可裁剪段缺失） | `HARNESS_ASSEMBLY_FAILED` | 模板 | **是** | — | 否 |
| 记忆读失败 | `HARNESS_MEMORY_READ_FAILED` | 模板 | 否 | 记忆 | 否 |
| 意图不可用／低置信 | `HARNESS_INTENT_UNAVAILABLE` | 人工 | 否 | 意图 | 否 |
| 工具失败 | `HARNESS_TOOL_FAILED` | 模板 | 否 | 工具结果 | 否 |
| 缓存不可用／失效 | `HARNESS_CACHE_UNAVAILABLE` | 人工 | 否 | 缓存条目 | 否 |
| MCP/A2A 对端不可用 | `HARNESS_PEER_UNAVAILABLE` | 人工 | 否 | 该对端能力 | **不受影响**（§38.4 精神） |

回退规则：从首选级起沿 §12.2 顺序向下取**第一个可用**的级；静音待机恒可用。
装配层额外置 `refuses_call = true`——§47.4 的「拒绝调用」是绝对的，不因落点变化而解除。

`visible` 恒为真：**不存在不可见的降级**。这是「无静默降级」的第一半。

## 6 「不得自由发挥」怎么变成可判定（INV-32 的第二半）

只断言「有错误码」是空洞的——一个实现可以给出错误码，然后把空上下文丢给模型让它自由发挥。
故本轮加一条**跨字段**判定 `assert_no_free_form`：当本次调用存在任一降级，且装配后上下文**失去策略或人设约束**
（`has_policy_constraint == false || has_persona == false`）时，判为违规，调用 MUST NOT 继续生成。
违规名同样是具名的。

## 7 非空洞守卫（四组注入，缺一不可）

沿用 W54/W55 的做法：零新增依赖、进既有 `check --profile full`、带能区分两种实现的注入样本。

1. **五类完整性**：`OBSERVATION_LAYERS` 恰为 5、`label()` 互不相同、每层 `required_fields()` 非空且互不相同；
   删掉任一类 ⇒ 失败。
2. **凭据扫描不是字段名黑名单**：值里含 `sk-…` ⇒ 拒；字段名含 `key` 但值不含任何 marker ⇒ **必须放行**。
   只做前者会被「见到 key 就拒」的假实现骗过。
3. **降级落点有序**：装配/记忆/工具在模板可用时落**模板**，意图/缓存/对端落**人工**；
   模板不可用且人工可用时，工具失败落**人工**；全部不可用时落**静音待机**。
   ⇒ 把一切映射到同一级的实现必然失败。
4. **无静默降级**：存在降级且上下文失去策略约束 ⇒ 判违规；仅给错误码而仍继续生成 ⇒ 同样判违规。

## 8 验证与主机限制

- `cargo xtask check --profile full`（含 fmt / clippy / workspace 测试 / 前端步骤）。
- `openspec validate` **两条口径都跑**：`--all --strict`（规格）与 `--all --strict --archived`（已归档变更）。
- `cargo xtask schema --check` 在本机**跑不了**（`generate.mjs` 的 `spawnSync(process.execPath)` 报 `EBUSY`，
  managed 与 system node 均复现）——本轮**不改动任何生成器输入**（`schemas/catalog/capabilities.json`、
  `schemas/{config,plugin}/*.schema.json`），故以结构性理由取证，并由 CI 的独立步骤兜底。

## 9 未做与外部阻塞

- 不把记录接进真实决策日志哈希链（runtime 接线）；
- 不改 `CapabilityView` 契约字段（不新增契约名，目录零漂移）；
- 不改任何验收 `status`：H15/H17 仍 `pending`，只追加 `notes`；
- 意图、缓存、工具循环、MCP、A2A（H05–H14）与黄金集（H16）仍属能力包，本轮不实现；
- 三处阈值（意图置信度、缓存相似度、上下文利用率）仍待本场景回放实测。
