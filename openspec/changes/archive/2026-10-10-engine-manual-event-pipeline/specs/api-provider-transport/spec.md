## Purpose
为文本生成引入可替换接缝与「仅演练」的确定性实现边界，不改变已验证的 HTTP 传输与 usage 记账行为。

## ADDED Requirements
### Requirement: Text generation seam
`nai-provider-core` SHALL 暴露 `TextGenerator` 接缝，承载一次文本生成请求（utterance 上下文、预算、取消/期限）与响应（文本、usage、模型标识）。既有 HTTP 适配器与 `UsageGateway` MUST 保持行为不变，且接缝 MUST NOT 绕过预算、取消或 usage 记账。

#### Scenario: Budget or cancellation applies to the seam
- **WHEN** 经接缝发出的生成请求超出预算或已被取消
- **THEN** 调用被拒绝或中止，不产生未记账的生成结果

### Requirement: Rehearsal-only deterministic stub
确定性生成桩 SHALL 仅在演练与测试路径显式构造，MUST 无网络、无凭据、可复现，并 MUST 在文档与报告中标注 rehearsal-only。桩 MUST NOT 被用于真实集成、付费调用或任何认证结论，也 MUST NOT 取代 API 故障处理。

#### Scenario: Stub cannot be mistaken for a real provider
- **WHEN** 以桩驱动演练
- **THEN** 输出与报告显式标注 rehearsal 来源，registry 中依赖真实 Provider 的条目保持原状态，不据此标 Passed
