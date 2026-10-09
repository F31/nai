## Why
Slice-0 定义（V3.6 §1.4、§31.2）要求「手工事件 → 模板/LLM → 守门 → TTS API → 2D → 录制」的演练闭环，验收含 R07（决策日志哈希链校验通过 + 用桩 Provider 回放与原决策一致）。逐层构件已实现（kernel `evaluate::{agent,guard}`、knowledge-core、provider-core、media-core、storage 决策日志），但**仓库内不存在任何编排部件**（全仓检索无 Pipeline/orchestrator）；`apps/engine` 只装配了控制面与媒体演练。结果是 R07 取不到判据，A01/R01/R03/S01 也缺少真实进程链路证据。此外，R07 依赖的桩回放被计划推到 W17（R1），与本项 `release_line=slice_0` 冲突，需在本轮就地补齐。

## What Changes
- 在 `apps/engine` 新增 pipeline 模块：手工事件源（显式 `manual` 来源标注）→ 知识检索 → 智能体决策（`kernel::evaluate`）→ 守门四阶段 → 文本生成 → 决策日志落库（哈希链）。
- `nai-provider-core` 新增 `TextGenerator` 接缝与 **rehearsal-only 确定性桩**（离线、可复现、无网络）；既有 HTTP 适配器与 `UsageGateway` 行为不变。
- `apps/engine` 新增演练参数（`--event-file`、`--rehearsal-provider`）与链上故障注入开关（仅演练）；失败路径仍固定为「先 P0 静音 → 撤权（epoch 递增）→ critical 决策日志 → 有界回收」。
- R07 桩回放：以同一手工事件与同一确定性桩重放，比对决策日志与原次一致。
- 新增 ADR 0022 记录编排边界：本轮不新建核心 crate，编排留在 app 层（V3.6 §17.2 核心 crate 清单无 pipeline，且 kernel 不得含业务策略）。

## Capabilities
### New Capabilities
无。
### Modified Capabilities
- `runtime-composition`: engine 的「手工事件 → 守门 → 生成 → 决策日志」编排装配与诚实边界。
- `api-provider-transport`: 新增 `TextGenerator` 接缝与 rehearsal-only 桩的适用范围与禁则。

## Impact
- 代码：`apps/engine`（新增 pipeline 模块与演练参数）、`crates/nai-provider-core`（接缝 + 桩）、`apps/engine/tests`、可能触及 `crates/nai-runtime`（装配注入编排依赖）。
- 规格：`runtime-composition`、`api-provider-transport` 增量；ADR 0022。
- 验收：使 R07 取得桩回放判据；为 A01/R01/R03/S01 增加真实进程链路证据。**是否升 tested 按判据逐条判定**，不因本轮自动升级。
- 依赖：复用现有依赖，无新增外部包、无常驻服务；桩为纯计算，无网络、无凭据。
- 明确不做：真实付费 Provider 调用、TTS→2D→播放 ACK 段（下一轮 `engine-text-to-playback-ack`）、真实设备/窗口/音频、Windows 认证、Tauri protected IPC、手机/远程。
