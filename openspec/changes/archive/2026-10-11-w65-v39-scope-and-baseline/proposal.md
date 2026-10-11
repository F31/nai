## Why

用户转来一份对 V3.8 的外部审视（五维批评：产品定位、过度设计、功能优先级倒挂、组件数失控、「轻量 / 敏捷」自相矛盾），
并给出自己的反批评：**「首版范围膨胀、产品验证不足、文档一致性下降」的批评成立，部分技术判断不准确**。
两份主张都被逐条取证核实（`docs/reports/2026-10-11-v38-review-adjudication.md`）：产品向批评成立；技术向的若干判断不成立
（`181.9` 在全文 **0 命中**、§14.3 并非「开播中只暂存」、六进程里的 `control-server` 标注为可选、§23.2 是 **32 张**表而非 44）。

核实过程中还查出**双方都没发现**的 7 处问题（D1–D7），其中两处是活文档矛盾：`docs/control.md` 写死的 ADR 篇数、
以及 `openspec/specs/spec-driven-delivery/spec.md` 的**基线指针仍停在 V3.5**（自 V3.6 起即不成立，ADR 0042 与 ADR 0047
两轮都记为遗留而不订正）。

要修的不是「文档没意识到」——§18.1 早已把「首版交付面过宽」登记为**高**风险。要修的是**登记之后没有围绕它重排资源**：
**登记解决「可以说清」，不解决「按什么顺序做」**。

## What Changes

- **V3.9 基线文档**（`docs/直播数字人 + 智能体系统技术方案 V3.9.md`）：新增**第五部分（第 49–57 章）**——
  §49 定位与版本减法纪律（INV-33 / INV-34、威胁 T-25）、§50 三个维度拆开（发布阶段 / 产品档位 / 部署配置）、
  §51 发布线语义与能力包、§52 平台准入独立成轨、§53 基础讲品循环前移 R1、§54 产品视角、
  §55 决策日志责任再限定、§56 规范冲突订正、§57 V3.9 版本说明。
  **前 2366 行就地订正 42 行**：24 个 hunk、**全部 1:1 行替换、零增删行** ⇒ 行号一一对应，全部既有 `§NNN` 引用继续有效。
- **本仓库第一次不再以「纯追加」处理升基线**：前两轮（V3.6→V3.7、V3.7→V3.8）纯追加的前提是「无矛盾可订正」；
  一旦已声明内容与已实现/已登记内容互相矛盾，继续追加只是把矛盾登记第二遍。V3.9 §56.2 因此确立一般规则：
  **活文档就地订正、历史记录保留原文 + 追加勘误段**，并给就地订正加硬约束（必须 1:1 行替换）。
- **ADR 0048** 裁决基线由 V3.8 升为 V3.9，并记录本次先例变更；ADR 0047 与 ADR 0042 追加勘误段（原文不改写）。
- **新增第六条发布线 `capability_pack`**：`tests/registry.yaml` 中 **29** 条（M01–M18、H05–H14、H16）由 `r1`/`r2`
  改为 `capability_pack`；收缩后 `slice_0` **10** + `r1` **36** + `r2` **9** + `capability_pack` **29** + `r3` **1** +
  `experimental` **2** = **87**（**只改归属，不加条目**）。
- **门控增量（本版唯一的机器可判定部分）**：`scripts/contracts/registry.mjs` 增**能力包集合断言**——
  集合 MUST 恰为上述 29 条、MUST NOT 有归属外的 id 被标为能力包、**MUST NOT 有任一能力包 id 落在 `slice_0`/`r1`/`r2`**
  （即 **INV-33**）；失败时**指名 id 与当前线**。`schemas/tests/registry.schema.json` 的**两处**硬编码（`release_lines`
  的 `required` 与 `.properties`、`acceptance.release_line.enum`）同改。`tests/registry.test.mjs` 以**三种注入**证其非空洞。
- **规范订正 C1–C9**（§56.1）：§4.5↔§1.4 的 wasm、§20.6 的 `memory.access@1` 阶段、§47.6 的 W57「冻结」不实、
  §47.6 的 W64 跨线、§48.5 的 A 行缺 A12–A14、§48.6 的轮次范围（W41–W55 含不存在的 W45）、§14.1↔§1.4 的档位矩阵，
  以及两处活文档矛盾（`docs/control.md` 的 ADR 篇数、`spec-driven-delivery` 的 V3.5 基线指针）。
- **不做**：**不删除任何能力**（§57.1 明写「本版砍掉的：无」——本版是**范围重排**，由 INV-34 保证被降级能力的边界保留）；
  **不整体移出第四部分**；**不删 workspace 作用域**；**不删决策日志**；**不停止内核开发**；
  **不新增契约名、不改能力目录**（§56.3，目录零漂移）；**不改任何 acceptance 的 `status`**；
  **不重排 W 编号**（第五部分从 **W65** 起）；**不实现任何代码**（Rust 侧零改动）。

## Capabilities

### Modified Capabilities

- `admission-registry`：发布线集合加入 `capability_pack`；**新增**需求把「首版门槛与能力包互不相交」变成机器可判定的断言
  （能力包集合恰为 29 条、无归属外 id 被标为能力包、无能力包 id 落在 `slice_0`/`r1`/`r2`，即 INV-33）；
  **修改**需求把 `capability_pack` 纳入家族/发布线口径。
- `agent-harness`：**新增**需求钉住 R1 保留的 harness 基础纪律（H01–H04、H15、H17、H18）与能力包（H05–H14、H16）的分界，
  判据为「一个确定性的上下文装配函数能不能满足它？」，并规定降级不删边界（INV-34）、启用即验收、
  加装沿用同一条升级与迁移图（MUST NOT 有第二条安装路径）。
- `decision-log-integrity`：**新增**需求限定日志的三个宣称——哈希链能发现**部分**改动而非构成防篡改证明；
  **确定性回放**与**重调在线模型**是两件事、MUST 分开验收；允许事务状态表 + 快照 + 关联日志的**混合**投影。
- `spec-driven-delivery`：**修改**需求，规格中的基线指针**不再写死版本号**（C9）。

## Impact

- 新增：`docs/直播数字人 + 智能体系统技术方案 V3.9.md`、`docs/adr/0048-design-baseline-v39.md`、
  `openspec/specs/` 中由本变更归档生成/更新的四条规格、`docs/reports/2026-10-11-w65-v39-scope-and-baseline.md`、
  `docs/reports/2026-10-11-v38-review-adjudication.md`。
- 修改：`AGENTS.md`、`README.md`、`docs/{design,engineering,development-plan,cloud,deployment,kernel,mixing,multi-session,operations,platform-matrix,upgrade,control}.md`、
  `apps/desktop/src/main.tsx`、`openspec/config.yaml`、
  `openspec/specs/{admission-registry,agent-harness,decision-log-integrity,duty-lease,platform-matrix,spec-driven-delivery}/spec.md`、
  `schemas/platform-matrix/platforms.json`、`schemas/tests/registry.schema.json`、`scripts/contracts/registry.mjs`、
  `tests/registry.yaml`、`tests/registry.test.mjs`、`xtask/src/registry.rs`、`docs/adr/0042-*.md` 与 `docs/adr/0047-*.md`（追加勘误，原文不改写）。
- **能力目录 `schemas/catalog/capabilities.json` 不动**：本轮不新增契约名，`crates/nai-contracts/src/generated/` 与全部生成物**零漂移**。
- **依赖与包数不变，Rust 侧零改动**（仅 `xtask/src/registry.rs` 的一处输出串由 V3.8 改 V3.9）。
- **已知代价（如实登记）**：本轮**首次**改动既有条款（§1–§48 中的 42 行），因此提供的是
  「24 hunk / 42 行 / 全 1:1 / 零增删」这一**可复核的**不变量，而不是「只增不改」这句更容易的话。
- **未做边界**：能力包与 R1 的 36 条**均零实现**（M01–M18、H05–H14、H16 全部 `pending`，W58–W62、W64 未开始）；
  §52 的三条平台准入命题**均未取得证据**，本轮只把它们拆成独立门槛，**不宣称任何一条已成立**；
  §54.2 的用户画像**未经访谈**。
- **验证**：`cargo xtask check --profile full`（含注册表能力包集合断言、契约目录解析、`tests/contracts`、`tests/platform`、
  `tests/ui` 与前端门控）、`openspec validate --all --strict` 与 `--all --strict --archived`；推送后匿名复核 CI 两平台结论。
