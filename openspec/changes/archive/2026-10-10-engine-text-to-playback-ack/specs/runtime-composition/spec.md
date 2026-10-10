## Purpose
把「已审批候选文本 → 语音合成 → 音频块流 → 真实 2D 帧 → 双水位提交 → 实际播放完成 ACK」的可验证终点补进 engine 演练链路，并保持无真实 TTS、无帧槽位、无 keeper、无设备的诚实边界。

## ADDED Requirements
### Requirement: Rehearsal speech-to-playback completion
engine SHALL 在演练脚本显式给出 `render` 段时，把已审批候选对象的文本经语音接缝合成为有界 PCM，按样本窗口切成有界 `AudioBlock` 流，并由 2D 渲染核心以**同一候选对象**产出真实 RGBA 帧计入视频水位。音频水位与首批视频水位都达到后 SHALL 提交，再由演练播放槽按 `(output_id, utterance_id, 预期样本总数)` 发出 ACK。只有「committed=true、actual_local_playback_completed=true、played_sample_count=预期样本总数」的 ACK SHALL 记 `PlaybackAck` 决策条目；其余结局 MUST NOT 记该 kind，MUST NOT 声称观众已听到。

#### Scenario: Approved event reaches a verified playback completion
- **WHEN** 已审批手工事件通过守门与文本生成，且演练播放场景为 `complete`
- **THEN** 决策日志出现一条 `PlaybackAck` 条目，记录 utterance、输出身份、语音/渲染模型标识、采样数与帧数，哈希链校验通过

#### Scenario: Partial, invalid or cancelled playback never logs a PlaybackAck
- **WHEN** 演练播放场景为只部分确认、身份或样本数不符、或已取消
- **THEN** 不出现 `PlaybackAck` 条目，摘要如实标注未完成与 `playback_ack=false`，不产生重复播报

#### Scenario: Injected speech failure halts in order
- **WHEN** 注入使语音合成阶段失败
- **THEN** 引擎先 P0 静音、再撤权、再写 critical 决策日志，随后有界退出，不产生成功结果行

### Requirement: Rehearsal playback boundary is not device certification
演练播放槽是**软件替身**：链路 MUST NOT 写入帧槽位、MUST NOT 涉及 keeper 帧消费、MUST NOT 声称硬件播放或设备认证。其 `PlaybackAck` 记录 MAY 出现在决策日志，但摘要的 `devices` 与 `output_available` 恒为 false，帧只存在于内存。

#### Scenario: Summary never overclaims the output
- **WHEN** 演练播放场景为 `complete` 并因此记录 `PlaybackAck`
- **THEN** 摘要仍标注 `devices=false`、`output_available=false`、`real_tts=false`、`frame_slot=false`、`keeper_consume=false`，不得据此标设备或生产 Ready

### Requirement: Explicit render boundary in the rehearsal script
`render` 段 MUST 为可选且显式：缺失时链路 MUST 止于文本并如实标注 `delivery=none`、`playback_ack=false`；给出时 MUST 是完整闭包（manifest、config、恰三份资产、至少一项许可、非空 output_id、合法播放场景），任一缺失或非法 MUST 拒绝，MUST NOT 猜默认配置或代填身份。

#### Scenario: Absent or incomplete render closure
- **WHEN** 脚本不含 `render` 段，或含 `render` 但资产闭包/许可/输出身份/播放场景不完整或非法
- **THEN** 前者按文本链路运行并如实标注 `delivery=none`；后者在驱动任何会话状态前被拒绝
