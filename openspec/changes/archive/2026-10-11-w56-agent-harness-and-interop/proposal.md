## Why

用户的指令是：AI 智能体应具备 **Harness 的工程化能力**，结合数字人技术面向专业直播与实时互动，可通过开放接口（MCP、A2A 等）连接，并具备记忆、意图识别、语义缓存、工具调用、Skill 能力扩展、连接器等能力。

对照 V3.7 现状（本轮实测命中数，非印象）：

| 能力 | V3.7 命中 | 性质 |
|---|---|---|
| `Harness` | **0** | 没有「模型之外那套让智能体持续正确」的定义 |
| `A2A` | **0** | 智能体与智能体的互操作完全缺失 |
| `意图识别` | **0** | 且 §5.2 明写「不先做独立分类」——该取舍只对**实时**路径成立，却被写成全局结论 |
| `工具调用` | **1** | §987，且是 SSE 解析措辞（「工具调用增量」），**不是能力定义** |
| `语义缓存` | **3** | §1008 一句纪律、§1480（§34.2 规则 1）、§1646（M04） |
| `MCP` | **4** | §1490「见 35.2」、§1515 表格一行、§1658「MCP 适配调研」、§1873 待确认 |

⇒ 按行业划分 `Agent = Model + Harness`，V3.7 有智能体的**零件**（决策循环、分级路由、守门、技能、知识），但**没有 harness**：给模型看什么、按什么顺序、多少预算没有定义；记忆没有写入门槛与读取过滤；工具循环的终止与失败无定义；缓存的两类（音频 vs 文本）没有分开；MCP 只被当作「技能的一种实现形态」，其 `resources`/`prompts`/`sampling` 与远端服务端信任未处置。

## What Changes

- **V3.8 基线文档**（`docs/直播数字人 + 智能体系统技术方案 V3.8.md`）：新增**第四部分（第 40–48 章，491 行）**，把上述缺口补成可判定的边界与不变量；**§1–§39 内容零改写**，仅 **8 行 1:1 行替换**（标题、副标题、§0.2、§1.4 两行、§16 前导、§17.3 两行）⇒ 总行数不变、行号一一对应，全部既有 `§NNN` 引用继续有效。
- **ADR 0047** 裁决基线由 V3.7 升为 V3.8，并**直接应用 ADR 0042 决策 4 留下的教训**：新验收家族**就地登记**，不用「未登记」表示「未开始」。
- **新能力 `agent-harness`**：8 条规范需求（装配与预算、记忆读写面、意图的最小表示、工具循环、语义缓存分离、MCP 映射、A2A 双向边界、逐层具名降级与观测），共 20 个 WHEN/THEN 场景。
- **`admission-registry` 家族扩展**：验收家族由 `S/R/A/B/O/P/M/X` 扩为含 **`H`**；把规格早已要求、却一直**未实现**的「家族在册断言」真正落地（缺号即失败并指名缺哪个）。
- **注册表**：新增 **H01–H18**（Harness 与开放互操作，全部 `pending`，69 → 87 条）；编号正则、JSON Schema 的 pattern 与描述、`release_lines` 标签同步。
- **契约：不新增契约名**。MCP 的 `tools` 映射为技能（复用已 frozen 的 `skill.invoke@1`）、`resources` 走已 Draft 的 `data.query@1`；`prompts` 与 `sampling` 是配置与授权问题。A2A 的**协议版本与认证方式待验证**（§35.2），先占契约名只会产生**半声明 draft**——§4.1 与 W52 起的能力目录纪律明确禁止它（会让 `stage` 字段失去意义、把「未验证」伪装成「已登记」）。故 A2A 契约待协议验证后随 W62 加入目录，H13/H14 在注册表里 `contract` 留空（沿用 M10/M12 先例）。
- **如实登记一处基线-目录不一致**：§20.6 把 `memory.access@1` 列为保持 Draft，而能力目录已标 `frozen`；V3.8 §42.6/§46.5 以目录为准并登记该不一致，**不改 §20.6 原文**。
- **活文档基线指针同步**：`AGENTS.md`、`README.md`、`docs/{design,engineering,development-plan,cloud,deployment,mixing,multi-session,operations,upgrade,platform-matrix}.md`、`openspec/config.yaml`、`openspec/specs/{decision-log-integrity,duty-lease,platform-matrix}`、`xtask/src/registry.rs`、`apps/desktop/src/main.tsx`。
- **不做**：**不实现任何 harness 代码**（W56–W64 未开始）；**本轮不新增静态门控**（无实现可门控，写了只能是空洞门控）；**不新增契约名、不改能力目录**；不改任何既有 acceptance 的 `status`；不重排 W 编号（V3.7 计划止于 W40，W41–W55 已被实际执行轮次占用 ⇒ 第四部分从 **W56** 起）；不扩张观众长期记忆的字段集；不臆造 MCP/A2A 的协议版本与认证方式。

## Capabilities

### Added Capabilities

- `agent-harness`：智能体在模型之外的工程化运行时边界——上下文装配的固定顺序与分段预算、记忆的写入门槛与作用域过滤、意图作为「封闭枚举 + 置信度 + 依据」且不构成授权、工具循环的有界终止与部分失败标注、语义缓存与 TTS 精确缓存的分离与禁用域、MCP 四概念映射与描述漂移重审、A2A 入站「外部请求」语义与出站委派深度上限、逐层具名降级与观测，以及「边界已定 ≠ 已实现」。

### Modified Capabilities

- `admission-registry`：验收家族清单加入 `H`；家族在册断言由「规格要求但未实现」变为**已实现**（M 18 条与 H 18 条缺号即失败）。

## Impact

- 新增：`docs/直播数字人 + 智能体系统技术方案 V3.8.md`、`docs/adr/0047-design-baseline-v38.md`、`openspec/specs/agent-harness/spec.md`、`docs/reports/2026-10-11-w56-agent-harness-and-interop.md`。
- 修改：`AGENTS.md`、`README.md`、`docs/{design,engineering,development-plan,cloud,deployment,mixing,multi-session,operations,upgrade,platform-matrix}.md`、`docs/adr/0042-*.md`（追加勘误，原文不改写）、`openspec/config.yaml`、`openspec/specs/{admission-registry,decision-log-integrity,duty-lease,platform-matrix}/spec.md`、`schemas/tests/registry.schema.json`、`scripts/contracts/registry.mjs`、`tests/registry.yaml`、`tests/registry.test.mjs`、`xtask/src/registry.rs`、`apps/desktop/src/main.tsx`。
- **能力目录 `schemas/catalog/capabilities.json` 不动**：本轮不新增契约名，故 `crates/nai-contracts/src/generated/catalog.rs` 与全部生成物**零漂移**（CI 的 `cargo xtask schema --check` 据以判定）。
- **依赖与包数不变**：workspace 仍 **19 包**，`check-deps` 仍是 19，**新增外部依赖为零**，**Rust 侧零改动**（仅 `xtask/src/registry.rs` 的一处输出串）。
- **已知代价（如实登记）**：遵守「§1–§39 内容零改写」，§1.4 与 §16 只在**既有行内**增补 H 系列指针、**不新增行**；H01–H18 的完整判据以 §48.5 为准，登记状态以 `tests/registry.yaml` 为准。
- **未做边界**：§40–§48 零实现（H01–H18 全部 `pending`）；MCP/A2A 的协议层事实**未经实测**，本变更只裁决设计与登记，不宣称任何互操作可用。
- **验证**：`cargo xtask check --profile full`（含注册表家族断言与契约目录解析）、`openspec validate --all --strict --archived`；推送后匿名复核 CI 两平台结论。
