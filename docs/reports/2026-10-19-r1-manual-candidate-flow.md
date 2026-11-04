# R1 手工提问 → API 候选 → 人工审批

日期：2026-10-19。变更：`openspec/changes/r1-manual-candidate-flow/`。

## 本轮解决的问题

上一轮补的是操作者工作台（最终文本展示、升级成本/事务查询），但真实主链路仍是
`apps/engine/src/pipeline.rs` 里的演练脚本：它固定构造 `RehearsalTextGenerator`，审批来自脚本，
不会产生可审批的存储候选。于是「输入一个问题 → 看真实回答 → 批准/拒绝」这条最基本的 R1 用户路径
在仓库里走不通。

本轮把它接通，止于**待审批候选**；没有宣称 TTS、播放、设备或平台认证。

## 实际能完成的操作

1. 可信 owner 用 `--provision <runtime.json> --operator-config <manual-session.json>` 导入显式场景：
   知识文档、知识范围与快照、Provider 端点/模型/凭据环境变量名、提示词片段与初始预算。
2. 启动 `nai-engine --operator-config <manual-session.json>`，桌面连接控制地址。
3. 在桌面输入问题；引擎实际检索已发布知识、核验 `local-only`，按既有装配器生成上下文，
   经 `ProviderProfile → ProviderClient → 适配器` 调用配置的 API。
4. 最终文本经输入/证据/输出守门后，以同一 cue 哈希写入 `utterances`，出现在现有待审批列表；
   操作者批准或拒绝，批准只表示允许该候选进入后续媒体阶段，本轮不播放。
5. 静音/接管可在 Provider 卡住时先撤权；在途任务随后以 `STALE_EPOCH` 结束，不留可批准候选。

## 实现

- `apps/engine/src/operator.rs`（layer app）：唯一编排点。
  - `OperatorLane::new` 持有真实 Store/共享 Session；环境变量仅用于读取 `Secret`，不写入配置或日志。
  - `provision` 导入并发布知识快照、创建持久会话；权限使用既有 `knowledge.import`/`knowledge.read`/
    `budget.manage`/`session.manage`。
  - `generate`：实际 `retrieve_session_knowledge` → `remote_egress_targets` 复核 local-only →
    `plan_assembly` 九段/裁剪/预算 → `record_assembly` 与 `record_observation` →
    输入/证据守门 → API → 输出守门 → 提交前复检 epoch/TTL/停机 → `record_candidate`。
  - 预算走持久化 `reserve_usage → dispatch_usage → update_usage`；请求在 dispatch 之后不再重发；
    失败/撤权时按 `Reserved→Cancel`、`Dispatched→Finish(None)` 结算。
  - 生成任务容量为 1（有界、无排队），事件历史 200；控制命令沿用 256 条幂等账本。
  - 重启时恢复持久会话并执行撤权/换代，旧候选不会在新会话中可批准。
- `crates/nai-control-api`：新增 `ManualEventSurface` 接缝，接线既有
  `POST /sessions/{id}/manual-events` 与 `GET /sessions/{id}/events`；沿用权限、scope、epoch、
  revision、seq、TTL 与幂等账本。ACK 只表示任务准入。
- `schemas/openapi/control.yaml`：两条既有路由标记为 `implemented-loopback`；操作闭世界同步登记。
- `schemas/operator/manual-session.schema.json`：本地场景清单（1 MiB 上限，未知字段拒绝）。
- `apps/desktop`：问题输入、任务轮询、事件与候选刷新；迟到响应、换代、候选缺失与失败均具名；
  不自动重发，不把 ACK 当生成或播放。审批仍使用上一轮接通最终文本的列表。
- `tests/ui/manual-question.test.mjs`：真实 `ControlClient` 驱动，验证命令字段、单次发送、
  失败不重试、换代/失效拒收、迟到响应抑制、4 KiB 输入上限与 503 具名错误。

## 验证

在 Linux/WSL 本机：

```bash
cargo test -p nai-engine --test engine operator_flow -- --nocapture   # 5 passed
cargo test -p nai-control-api                                         # 50 passed
cargo clippy -p nai-engine -p nai-control-api --all-targets -- -D warnings
cargo xtask check --profile full                                      # EXIT=0
node scripts/frontend-gate.mjs                                        # 6 steps passed
```

`check --profile full` 现测：**84 个 Running 目标 + 18 个 Doc-tests = 102 个测试目标，
1193 passed / 0 failed**；Node 侧 **427 项通过 / 0 失败**（含本文件新增 6 项与 UI 84 项），
fmt/clippy/前端六步均通过。归档后复跑同一命令仍为 **EXIT=0**，计数逐项相同。

归档与规格校验：变更已移至 `openspec/changes/archive/2026-10-19-r1-manual-candidate-flow/`；
`openspec validate --all --strict` 为 **43/43**，`--all --strict --archived` 为 **153/153**。

进程级测试覆盖：

- 两个问题分别批准、拒绝；同一命令重试返回原 ACK，不产生第二次 Provider 请求；
  最终文本、哈希、证据数进入审批列表，决策链包含 assembly/observation/approval 且无 playback ACK。
- Provider 阻塞时静音立即成功；在途任务以 `STALE_EPOCH` 结束、审批列表为空；
  换代后的新问题仍能生成候选。
- 过期任务以 `DEADLINE_EXCEEDED` 结束且无候选；陈旧 epoch 命令被拒。
- `local-only` 文档与无检索命中时**不发起** Provider 调用；输出守门拦截时同样无候选。
- 重启后旧候选不可批准，且重启不会重放已结算的请求；新问题使用新候选哈希。

## 边界与未做

- 进程内 HTTP Provider 是**本地协议测试替身**，只验证线上请求形状、预算与失败路径；
  **不是真实付费 API 认证**，也不证明任何具体供应商的账户可用性。
- 没有浏览器 E2E、Windows 音频/窗口、TTS、帧槽、keeper 消费或播放 ACK；批准不等于播出。
- 场景知识来自可信 owner 的显式 provision，不新增远端知识编辑面；没有提供系统密钥库配置 UI，
  凭据只从调用进程的环境变量读取。
- `manual-events` 仍是 loopback 工程控制面，不是生产 protected IPC/TLS；桌面壳尚未构建。
- `tests/registry.yaml` 的验收状态未提升。O02/O05 仍为 pending；真实账户、设备与平台证据未取得。

## 下一步

第二条产品主线：把「已批准且仍有效的同一文本」接入 API TTS → 有界 PCM → 2D 渲染 → 输出端，
并使完成 ACK 来自实际输出端；同时保留「批准与播放文本同一哈希」的复检。
