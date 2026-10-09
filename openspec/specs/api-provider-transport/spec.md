# api-provider-transport Specification

## Purpose
为 nai 提供真实的 Provider HTTP 传输、SSE/二进制流解析、deadline/cancellation、有界重试和 usage 预算接线，使未入选事件不启动付费请求、已派发请求如实入账，且 Provider 故障显式可见。

## Requirements

### Requirement: Bounded HTTP transport with deadline and cancellation
系统 SHALL 使用每 Provider 独立认证，把任务期限转成本地单调截止并约束连接/读取，取消/超时 SHALL 丢弃迟到数据且不返回正常完成。SSE SHALL 解析多行 data、分片 UTF-8、事件边界、[DONE] 与服务端错误；二进制流 SHALL 按块回传缓冲引用。

#### Scenario: Late and cancelled stream
- **WHEN** 读取中途期限到期或取消令牌置位
- **THEN** 后续块被丢弃，结果以 Cancelled/DeadlineExceeded 结束且不标 Completed

#### Scenario: Fragmented SSE event
- **WHEN** 单个 data 事件被多个 TCP 块分片且含多字节 UTF-8
- **THEN** 事件仍被完整还原，[DONE] 前无完整文本不被当作结束

### Requirement: Bounded retry without double-play
401/403 SHALL 不自动重试；429 SHALL 遵循 Retry-After 且不超过任务期限；5xx/连接错误 SHALL 采用有限指数退避 + jitter，初值最多 1 次额外尝试并按配置可调。重试仍不可用 SHALL 返回显式 PROVIDER_UNAVAILABLE。已提交/已入账的请求 SHALL 不因重试重复派发或重复计费。

#### Scenario: Retry-after exceeds deadline
- **WHEN** 服务端返回 429 且 Retry-After 超出任务剩余期限
- **THEN** 不再重试并以 RESOURCE_EXHAUSTED/PROVIDER_UNAVAILABLE 结束

#### Scenario: Auth failure
- **WHEN** 返回 401/403
- **THEN** 不重试并保持 UNAUTHENTICATED/PERMISSION_DENIED，不冒充授权通过

### Requirement: Honest usage and budget wiring
成功/取消的请求 SHALL 保留尝试计数与真实单位；Provider 返回的 token/用量 SHALL 转为整数 BudgetCharge，TTS 或未知用量 SHALL 保留估算并标待对账。派发 claim SHALL 在 I/O 前持久化，重复确认 acquired=false；结束/取消 SHALL 调用 finish，实际超估算如实入账且阻止超额准入。

#### Scenario: Usage from embeddings
- **WHEN** Embedding 响应含 prompt_tokens/total_tokens
- **THEN** 单位 tokens 与 requests 计入 BudgetCharge，price_revision 保留，不丢弃请求计数

#### Scenario: TTS without usage
- **WHEN** TTS 成功返回音频但无 usage 字段
- **THEN** 以 NeedsReconciliation 结束并保留估算，不伪造零账单或双扣

### Requirement: Provider isolation and explicit failure
provider-core SHALL 只依赖 contracts/security/budget/shared，不依赖 kernel/media/宿主；凭据以不泄露方式传入，日志与 Debug 不得含密钥。网络/连接/5xx SHALL 映射为可重试 PROVIDER_UNAVAILABLE；未配置凭据 SHALL 显式 Unavailable/Blocked，不以本地模型或假数据标 Passed。

#### Scenario: Credential missing
- **WHEN** 未提供凭据时发起 Provider 调用
- **THEN** 返回显式 Unavailable/Unauthenticated，不尝试本地降级伪装

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
