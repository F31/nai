## Why
上一轮 `engine-manual-event-pipeline`（ADR 0022）把「手工事件 → 知识 → 智能体 → 守门 → 文本生成 → 决策日志」接通并实测，但链路**止于文本**：`inline delivery="none"`、`playback_ack=false`。Slice-0 的端到端演练闭环（V3.6 §1.4/§31.2）还缺 TTS→2D→播放完成这一段，`media-basics-timeline` 的「双水位提交」与「仅实际播放完成 ACK 才 Completed」两条要求至今只在纯逻辑单测里被验证，从未在真实 engine 进程中被走通；R07 的「完成」判据与 A01 的 2D 像素生产也缺少同一进程内的链路证据。

逐层构件均已实现且可移植：`nai-media-core::timeline`（`AudioBlock` 校验、`UtteranceTimeline` 双水位提交、`AckTracker` 完成判定）、`nai-avatar-2d`（真实 RGBA 像素生产，同一核心由 `apps/render-worker` 装配）、`nai-provider-core`（计量接缝 + 演练桩的先例）。缺的是**把它们在 engine 里按同一已审批候选对象串起来**。

## What Changes
- `nai-provider-core` 新增 `SpeechSynthesizer` 接缝与 **rehearsal-only 确定性语音桩**：样本数只由文本长度与采样率决定（因此预期样本总数可预先算出、可被 ACK 精确匹配），无网络、无凭据；预算/取消/usage 记账与既有 `TextGenerator` 同一纪律。
- `nai-media-core` 新增可移植播放通道 `playback`：以 `UtteranceTimeline` 做双水位提交、以 `AckTracker` 做完成判定，并把 ack 行为收敛为显式的演练场景（完整 / 只部分 / 无效 / 取消）。
- `apps/engine` pipeline 新增可选 `render` 段：文本生成后经语音接缝合成 PCM → 按样本窗口切成有界 `AudioBlock` 流 → `nai-avatar-2d` 产出真实 RGBA 帧并计入视频水位 → 双水位提交 → 演练播放槽按 `(output_id, utterance_id, 样本总数)` 发出 ACK → 仅「committed + actual_local_playback_completed + 样本数相符」记 `PlaybackAck` 决策条目；partial/invalid/cancelled 只记 `Observation` 并如实标注。
- 新增 `--inject speech` 链上故障注入（仅演练），失败仍走「先 P0 静音 → 撤权 → critical 日志 → 有界退出」。
- 新增 ADR 0023 记录本轮边界：演练通道是把渲染核心**嵌入演练**，不写帧槽位、不经 keeper、不声称设备；生产帧路径仍是 `apps/render-worker` + 帧槽位 + keeper。

## Capabilities
### New Capabilities
无。
### Modified Capabilities
- `media-basics-timeline`: 新增播放通道的 ack 绑定/完成判定与「提交不等于播放完成」的显式语义。
- `api-provider-transport`: 新增 `SpeechSynthesizer` 接缝与 rehearsal-only 语音桩的适用范围与禁则。
- `runtime-composition`: engine 演练链路从文本延伸到经实际播放完成 ACK 验证的终点，并保持无设备/无槽位/无 keeper 的诚实边界。

## Impact
- 代码：`crates/nai-provider-core`（speech 接缝 + 桩）、`crates/nai-media-core`（playback 通道）、`apps/engine`（pipeline 扩展、render 段、`--inject speech`、Cargo 依赖新增 `nai-avatar-2d`/`nai-plugin-sdk`，均为 app 层允许方向）、`apps/engine/tests`。
- 规格：三项能力增量；ADR 0023。
- 验收：为 R07 补「经实际播放完成 ACK」的链路判据、为 A01 补同进程内的帧生产证据；**是否升 tested 逐条按判据判定**，不因本轮自动升级。
- 依赖：无新增外部包、无常驻服务；语音桩为纯计算。
- 明确不做：真实 TTS/付费调用、帧槽位写入、keeper 帧消费、真实设备/窗口/音频输出、Windows 认证与帧率验收、Tauri protected IPC。
