## Purpose
为语音合成引入与文本生成同纪律的可替换接缝与「仅演练」的确定性实现边界，不改变已验证的 HTTP 传输与 usage 记账行为。

## ADDED Requirements
### Requirement: Speech synthesis seam
`nai-provider-core` SHALL 暴露 `SpeechSynthesizer` 接缝，承载一次语音合成请求（session、utterance、generation epoch、文本、目标采样率、预算、取消标志）与响应（有界 PCM、采样率、usage、模型标识）。接缝 MUST NOT 绕过预算、取消或 usage 记账；非有限、越界或超出长度上界的样本 MUST 被拒绝；已派发的请求 MUST NOT 被重发。既有 HTTP 适配器与 `UsageGateway` 行为 MUST 保持不变。

#### Scenario: Budget or cancellation applies to the speech seam
- **WHEN** 经接缝发出的合成请求超出预算、已被取消或已派发
- **THEN** 调用被拒绝或中止，不产生未记账的合成结果，也不重复派发

#### Scenario: Invalid PCM is refused
- **WHEN** 响应携带非有限值、绝对值超过 1.0 或超出长度上界的样本
- **THEN** 响应被拒绝，不进入后续时轴或渲染

### Requirement: Rehearsal-only deterministic speech stub
确定性语音桩 SHALL 仅在演练与测试路径显式构造，MUST 无网络、无凭据、可复现，且其样本总数 MUST 只由文本与目标采样率决定（有明确上下界），以便预期样本总数可预先核验。桩 MUST NOT 被用于真实集成、付费调用或任何认证结论，MUST NOT 取代 TTS 故障处理。

#### Scenario: Stub cannot be mistaken for real TTS
- **WHEN** 以桩驱动演练
- **THEN** 输出与报告显式标注 rehearsal 来源与桩标识，registry 中依赖真实 TTS 的条目保持原状态，不据此标 Passed
