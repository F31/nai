# 报告：engine-manual-event-pipeline（手工事件 → 决策日志链路与桩回放）

- 日期：2026-10-09
- 变更：`openspec/changes/engine-manual-event-pipeline/`
- 基线：V3.6 §1.4、§31.2（Slice-0）；ADR 0022（编排边界）
- 结论：Slice-0 手工事件链路的可移植软件闭环已实现/实测；**不是**真实 Provider、播放 ACK、设备或 Windows 认证

## 背景

Slice-0 要求「手工事件 → 模板/LLM → 守门 → TTS API → 2D → 录制」演练闭环，验收含 R07（决策日志哈希链
校验通过 + 用桩 Provider 回放与原决策一致）。逐层构件（`kernel::evaluate`、knowledge-core、provider-core、
media-core、storage 决策日志）均已实现，但**全仓不存在任何编排部件**，`apps/engine` 此前只装配控制面与
媒体演练，因此 R07 取不到判据，A01/R01/R03/S01 也缺少真实进程链路证据。

## 实现

### Provider 文本接缝与 rehearsal 桩（`crates/nai-provider-core/src/text.rs`）

- `TextRequest`（request/provider/session/utterance 标识、`generation_epoch`、prompt、输出上界、剩余预算、
  取消标志）与 `TextResponse`（文本、模型标识、usage）。
- `TextRequest::validate()`：拒空/不完整标识、空/超长 prompt（`MAX_PROMPT_CHARS = 8192`）、非法输出上界
  （`MAX_OUTPUT_CHARS = 4096`）、已取消（Cancelled）、`remaining_budget_ms == 0`（DeadlineExceeded）。
- `generate_with_budget`：`CallBudget::start`（reserve → dispatch）→ 仅 `dispatched()` 为真才调用生成器；
  返回 `Already` 即 Conflict 而**不重发**；`usage_charge(currency, price_revision, usage, 1)` 后
  `finish(actual)`。既有 HTTP 适配器与 `UsageGateway` 行为不变。
- `RehearsalTextGenerator`（`MODEL_REF = "rehearsal-deterministic-v1"`）：纯计算、无网络、无凭据、可复现，
  仅在演练/测试路径显式构造。
- `client.rs` 两处构造器（`unavailable`/`invalid_argument`）改为 `pub` 供跨 crate 使用。
- 6 项单测通过。

### engine 编排（`apps/engine/src/pipeline.rs`）

链路：手工事件 → 归一化/去重 → 知识检索 → 智能体决策（`kernel::evaluate::agent`）→
守门四阶段（`kernel::evaluate::guard`）→ 文本生成 → 决策日志哈希链。

- 脚本输入 `Script`：`session_id`/`workspace_id`/`knowledge_revision`/`top_k`/`session`
  （operator、`duty_ttl_ms`、mode、lifecycle）/`knowledge`/`events`/`approvals`。`load_script` 拒绝非 `manual`
  来源、未知 kind、重复 id、缺失 session/workspace 与越界语料/队列。
- 检索：复用 `nai-knowledge-core::VectorSpace` 的真实余弦排序，但嵌入为**精确词表索引**（非哈希技巧）——
  修正了「哈希碰撞把未命中变成命中」的真实缺陷，`knowledge_no_hit` 是真实可核验事实。
- 审批绑定：`preview_candidates` 离线算出候选对象哈希（`candidate_hash`），守门 `PreplayChecks.approval_valid`
  绑定**确切对象哈希**；审批是构造一致而非约定一致。
- 每事件在首次对外调用前复检 session/epoch/值守；守门缺失/未通过/陈旧 epoch 一律不外发。
- 两遍执行：pass 1 编排 + 落库；pass 2 以 `Injection::None` 重放并比对决策日志 canonical 行
  （`replay_matched`）；再 `verify_chain`（`chain_verified`）。
- 失败顺序：`ordered_halt` 先 P0 静音 → 撤权（epoch 递增）→ critical 日志；存储失败先 halt 再 persist
  （**不回退**已执行的静音）。
- CLI：`--event-file <script.json>`（需 `--rehearsal-provider`）、
  `--inject <none|knowledge|guard|generate|storage>`（仅演练，且需 `--event-file`）。
- 5 项单测 + 单行 stdout `nai-engine pipeline: {summary}`。

### 诚实边界

summary 显式 `rehearsal:true`、`real_provider:false`、`devices:false`、`playback_ack:false`、
`output_available:false`；决策日志分录 payload `delivery:"none"`。链路止于文本，**未**渲染、**未**投递、
**未**播放。

## 验证（本轮实际执行）

| 检查 | 结果 |
|---|---|
| `cargo fmt --all` | clean |
| `cargo clippy --workspace --all-targets`（经 `check --profile portable`） | 通过 |
| `cargo test -p nai-provider-core --lib` | 6 passed |
| `cargo test -p nai-engine --lib` | 通过 |
| `cargo test -p nai-engine --test pipeline` | 7 passed |
| `cargo test --workspace --locked --offline --no-fail-fast` | 296 passed / 14 failed（既有平台差异，见下） |
| `cargo xtask check-deps` | 17 包边界通过 |
| `cargo xtask check --profile portable` | 通过 |
| `cargo xtask check --registry` | 通过 |
| `cargo xtask schema --check` | **未执行**（本机平台限制） |
| `cargo xtask test --suite contracts` | **未执行**（本机平台限制） |
| `pnpm typecheck` | **未执行**（本机无 pnpm） |
| `pnpm test:engine-smoke` / `test:control-smoke` | **未执行**（本机平台限制） |

关键集成测试（`apps/engine/tests/pipeline.rs`，真实 engine 子进程）：

- 通过守门的手工事件写入可校验决策日志，哈希链校验通过 + 回放一致（`chain_verified`/`replay_matched`）。
- 守门不通过 / 无知识命中 / 陈旧 epoch 均不外发（`guard_output`/`knowledge_no_hit`/`stale_epoch`）。
- 缺失审批在生成前被守门拦截。
- 第二个进程重放产出一致的日志 digest。
- 注入生成失败 / 知识失败 → 有序 halt 且无成功结果行；注入存储失败 → **不回退**已执行的静音。

## 平台边界（如实）

本机为 Windows（非 WSL）环境，`node` 进程**无法创建任何子进程**——`spawnSync(process.execPath, [...])`
返回 `EBUSY`，且与沙箱、目标可执行文件、工作卷无关：

```bash
node -e "const{spawnSync}=require('node:child_process');console.log(spawnSync(process.execPath,['-e','1']).error?.code)"  # => EBUSY
```

后果：`scripts/contracts/generate.mjs`（`spawn` ts-proto + rustfmt）无法运行，故 `schema --check` 与
`test --suite contracts` 未执行；`engine-smoke`/`control-smoke` 同样因 `spawn` 不可用未执行；本机无 `pnpm`，
前端 `typecheck` 未执行。**本轮未修改任何 `schemas/`**（`git status` 无 schemas 变更），xtask catalog 校验通过
（`27 capabilities; 112 protobuf messages`），故契约面无回归风险。以上为环境限制而非代码缺陷，记录以备在具备
可用 node/pnpm 的环境复跑。

`cargo test --workspace` 的 14 项失败**全部为既有测试**（本轮未修改其所在文件），失败点集中在 Windows 平台差异：

- `apps/output-keeper/tests/faults.rs`（6）：命名管道/FIFO（`ERROR_PIPE_BUSY`）。
- `apps/engine/tests/engine.rs`（3）：`/proc/<pid>` 探测 keeper 子进程、`0600` 权限位断言。
- `crates/nai-storage/src/tests*.rs`（5）：`fs::rename` 覆盖语义（`filesystem operation failed`）及相关断言。

本轮新增测试（provider text 6、engine pipeline 7）全部通过。`nai-storage::snapshot` 的 `fs::rename` Windows
缺陷与上述 Linux-only 假设不在本轮范围。

## 验收状态

`tests/registry.yaml`：R07 取得**桩回放判据**、S01 补真实进程链路证据，**均保持 implemented 不自动升级**
（播放 ACK 生产端与真实设备仍缺）。

## 归档

`openspec/changes/archive/2026-10-10-engine-manual-event-pipeline/`；主规格 `runtime-composition` 与
`api-provider-transport` 增量生效。ADR 0022。
