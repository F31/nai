## Why

W56 把智能体 Harness 写进 V3.8 基线（§40–§48），并把 **H01–H18** 就地登记为 `pending`——**边界、不变量与登记**齐了，
**实现为零**。W57 是这条轨道上的**第一个实现轮**，瞄准 **H03（记忆写入门槛）与 H04（记忆读取与作用域）**。

对照仓库现状（本轮实测，非印象）：`crates/nai-kernel/src/evaluate/memory.rs` **已经存在**且非空——
`working_memory`（只保留已播完、有界）、`session_summary`（确定性去重）、`viewer_record_allowed`（默认关闭、作用域 ID、
用途白名单）、`viewer_record`（字段/保留期/敏感内容），源自 **D10（V3.5 §23.5）**。所以 W57 的诚实落点**不是另起炉灶**，
而是**把 §42.2–§42.6 相对 §23.5 的缺口补齐**：

| §42 条款 | 要求 | 既有代码 | 缺口 |
|---|---|---|---|
| §42.2 | 只有**已批准且已播完**可写为事实；工具/外部/评论/草稿 MUST NOT 写成事实或承诺 | `working_memory` 按 `played_ids` 过滤 | **无「写入来源」维度**——工具返回与模型草稿与正常内容不可区分 |
| §42.2 | 每次写入必带作用域、来源标识、时间与「叙述/权威事实」标注 | `validate_record` 校验 id/内容/保留期 | 无来源标识、无时间、**无叙述/事实标注**、权威事实无 §6 发布引用约束 |
| §42.3 | 先作用域、后时效；过期/撤销/权限不足不得进入；**不得用近似条目顶替** | 仅 `viewer_record` 的字段级校验 | **无读取路径**：无作用域过滤、无撤销、无权限、无近似顶替概念 |
| §42.3 | 记忆段独立配额；超配额按「场次 > 工作 > 观众」固定优先级裁剪 | 无 | 未实现 |
| §42.4 | 四类触发共用删除链：先阻断读取、再异步清除；删除可验证；第三方留存须明示 | 无 | 未实现 |
| §42.6 | 四项操作 `write`/`read`/`forget`/`scopes` 的语义落进一致性套件 | `MemoryRequest.operation` 字段存在但无取值约束 | 未实现 |

⇒ 判定依据不靠叙述：既有实现覆盖的是 §23.5 的**子集**，§42.2 的**来源门槛**、§42.3 的**读取过滤链与配额**、
§42.4 的**删除链**、§42.6 的**四操作语义**在仓库里**零落点**。

## What Changes

- **扩展 `crates/nai-kernel/src/evaluate/memory.rs`**（**不新建模块、不改既有函数签名**），补齐四块：
  1. **写入门槛（H03）**：`WriteSource`（已批准已播完／已批准未播出／工具返回／外部智能体返回／评论原文／模型草稿）、
     `ClaimKind`（叙述／权威事实）、`MemoryCandidate`（记录 + 来源 + 来源标识 + 时间 + §6 发布引用）、
     `admit_write(candidate, policy)`。
  2. **读取面（H04）**：`MemoryScope`、`MemoryQuery`、`ScopedRecord`、`MemoryReadOutcome`（`records` + 逐条具名的 `refused`）、
     `read_records(...)`——按「作用域 → 时效 → 撤销 → 权限 → 配额」的固定链，超配额按「场次 > 工作 > 观众」裁剪且**裁剪可见**。
  3. **遗忘链（§42.4）**：`ForgetTrigger`（TTL／事实失效／权限撤销／用户请求）、`PurgeState`、`ForgetOutcome`、`forget(outcome)`。
  4. **四操作（§42.6）**：`MemoryOperation`（`write`/`read`/`forget`/`scopes`）及其解析。
- **新增具名拒绝类型 `MemoryRejection`**（含 `label()`）：静默丢弃与静默裁剪是主要风险，故**每一种拒绝都必须有名字**
  ——这直接落实 §40 的 INV-32（每层失败须具名）。既有函数继续用 `EvaluateError`，签名不变。
- **新增集成测试目标 `crates/nai-kernel/tests/memory.rs`**：逐条构造每个拒绝名、断言「拒绝名并集**恰为** N 条」、
  正向不变量（合规候选零拒绝）、否定式不变量（未声明 ≠ 违规；未播出 ≠ 过期）、以及**非空洞守卫**。
- **`tests/registry.yaml`**：H03/H04 补 `evidence` 与 W57 notes；**状态保持 `pending`**——判定机制交付 **≠** 验收达标，
  真实记忆子系统（持久化、控制面接线、Pro 跨会话）未建，沿用 W32–W36 判定骨架轮与 A12／O01／O05 的先例。
- **文档**：`docs/kernel.md` 的 D10 记忆条目扩展；`docs/development-plan.md` 追加 W57 轮次记录；
  `docs/reports/2026-10-11-w57-memory-read-write-surface.md`。
- **不做**：**不新增契约名、不改能力目录**（`memory.access@1` 已 `frozen`，本轮是**冻结后语义补齐**）；
  **不使任何既有 `status` 升级**；**不实现持久化存储**（本轮是纯逻辑判定面，无 I/O、无新增依赖）；
  不改 §1–§48 基线正文（V3.8 已定界，W57 是其实验性落地，不改设计）；不新增静态门控之外的检查项。

## Capabilities

### Added Capabilities

（无新能力。本轮为既有 `agent-harness` 能力的**实现轮**。）

### Modified Capabilities

（无。`memory.access@1` 的 `stage`、请求/响应形状与目录条目均不变。）

### Added Requirements

- `agent-harness`：新增 4 条需求（写入的来源门槛、读取的作用域优先与具名拒绝、遗忘的可验证删除链、
  `memory.access@1` 的四项操作语义），把 W56 写下的边界落成**可判定的实现判据**。

## Impact

- 新增：`crates/nai-kernel/tests/memory.rs`、`openspec/changes/w57-memory-read-write-surface/`、
  `docs/reports/2026-10-11-w57-memory-read-write-surface.md`。
- 修改：`crates/nai-kernel/src/evaluate/memory.rs`（**只增不改既有函数**）、`tests/registry.yaml`（H03/H04）、
  `docs/kernel.md`、`docs/development-plan.md`、`openspec/specs/agent-harness/spec.md`（归档时并入新增需求）。
- **能力目录 `schemas/catalog/capabilities.json` 不动** ⇒ `crates/nai-contracts/src/generated/catalog.rs` 与全部生成物
  **零漂移**（CI 的 `cargo xtask schema --check` 据以判定）。
- **依赖与包数不变**：workspace 仍 **19 包**、`check-deps` 仍 **19**、**新增外部依赖为零**；
  新增的集成测试目标使门控的 `Running` 测试目标数 **68 → 69**。
- **未做边界**：H03/H04 **保持 `pending`**；无真实记忆存储、无控制面接线、无 Pro 跨会话门控；
  阈值类取值（保留期上限沿用既有 `MemoryPolicy` 默认，不新造）不在本轮定夺。
- **验证**：`cargo xtask check --profile full`、`openspec validate w57-memory-read-write-surface --strict`、
  `openspec validate --all --strict` 与 `--all --strict --archived` **两条都跑**（W56 教训：两者互不替代）；
  推送后匿名复核 CI 两平台结论。
