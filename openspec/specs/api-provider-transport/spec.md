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

### Requirement: Protocol-derived credential presentation
供应商的凭据呈现方式与静态头 MUST 由所配置的协议派生，MUST NOT 写死在共享请求路径中。未知协议名 MUST 被拒绝，MUST NOT 落回某个默认供应商。调用方 MUST NOT 需要知道任何供应商特有的头名。

#### Scenario: Two providers present their credentials differently
- **WHEN** 以两种分别使用 `Authorization: Bearer` 与供应商自有凭据头的协议各发一次请求
- **THEN** 各请求携带的凭据头名与协议一致，且**不**同时携带另一协议的凭据头

#### Scenario: A misspelled protocol never selects a default vendor
- **WHEN** 配置中的协议名无法识别
- **THEN** 绑定被拒绝，不发出任何请求

### Requirement: Provider identity is separate from the endpoint
用量记录中的供应商身份 MUST 与 endpoint 地址分离。同一 endpoint 上的两个供应商、或 endpoint 改变后的同一供应商，MUST 能在账本中分辨。

#### Scenario: Identity survives an endpoint change
- **WHEN** 同一供应商身份先后指向两个不同 endpoint
- **THEN** 其用量记录归属同一供应商身份，而不是被记成两家

### Requirement: Two wire protocols per capability, selected by configuration
文本生成与语音合成 MUST 各至少存在两种线上协议适配器，且由配置选择。适配器 MUST 在自身边界内承载该协议的路径、请求体形状、流帧结构与用量字段名；调用方 MUST 通过能力切面调用，MUST NOT 出现供应商名。

#### Scenario: One caller, two text protocols
- **WHEN** 同一个文本生成调用函数先后以两种不同文本协议运行
- **THEN** 两处都返回相同形状的有序块序列与单一终止块，而实际请求的路径与请求体形状不同

#### Scenario: One caller, two speech protocols
- **WHEN** 同一个语音合成调用函数先后以两种不同语音协议运行
- **THEN** 两处都返回相同形状的块序列与终止块，而实际请求的路径与请求体形状不同

### Requirement: Capability mismatch is refused, never degraded
把不具备某能力的协议绑定为该能力时 MUST 显式报错，MUST NOT 降级到其它供应商或本地替身，且 MUST NOT 发出请求。

#### Scenario: Speech-only provider asked for text
- **WHEN** 以语音专用协议构造文本生成客户端
- **THEN** 返回显式错误，且该端点未收到任何请求

### Requirement: Vendor usage maps into the closed canonical units
供应商特有的用量字段名 MUST 在适配器边界映射为既有封闭单位，账本词表 MUST NOT 因此放宽。用量为零的字段 MUST 被省略而非发出零单位。供应商未报告用量时 MUST NOT 为其编造实际值。

#### Scenario: Two vendors, one unit vocabulary
- **WHEN** 两家供应商分别以各自的字段名报告 token 用量
- **THEN** 两者都记入同一组规范单位，且各自归属自己的供应商身份

#### Scenario: A vendor that reports nothing is not credited with a number
- **WHEN** 某供应商在成功响应中不返回任何用量
- **THEN** 该次调用保留估算、不产生实际计费值，且该差异按供应商分别断言而不是被压平成共同期望

### Requirement: Speech is synthesized per sentence and never stitched
一次语音合成调用 MUST 只覆盖一个句子，MUST NOT 跨调用携带序号、缓冲引用或样本偏移。目标采样率 MUST 是供应商能产出的采样率，否则 MUST 拒绝且 MUST NOT 静默替换为另一码率。

#### Scenario: Two sentences stay two sequences
- **WHEN** 先后以两个句子各发起一次合成
- **THEN** 第二次不延续第一次的序号，各数据块的样本起点属于其自身缓冲，且两次各自结算一条用量

#### Scenario: An unproducible sample rate is refused before any request
- **WHEN** 请求的采样率不被该供应商支持
- **THEN** 调用被拒绝，不发出请求、不产出音频块，且预留被释放而非保持 reserved

### Requirement: The permitted context is presented as one user turn

文本适配器 MUST 把 `permitted_context` 作为**恰好一条 `role: user` 的消息**发出，且 MUST NOT 自拟任何提示文本
（既不加系统轮，也不加模板前缀）。文本生成请求 MUST NOT 只含非 `user` 轮：系统轮不构成「用户查询」，
部分网关会以 4xx 拒绝该形状，故 system-only 请求 MUST NOT 被构造。

两种文本协议的请求体形状差异 MUST 限于路径、协议附加标志与用量字段名，MUST NOT 延伸到**同一语义字段的角色选择**——
`permitted_context` 在两种协议下的角色 MUST 一致。

#### Scenario: Both text protocols send the context as one user turn
- **WHEN** 同一个文本生成调用函数先后以两种不同文本协议运行
- **THEN** 两处请求体都把 `permitted_context` 作为唯一一条 `user` 轮发出，且都不含适配器自拟的提示文本

#### Scenario: No system-only text request is constructed
- **WHEN** 构造一次文本生成请求
- **THEN** 请求体的 `messages` 中存在 `role: user` 的消息，且不存在因适配器自拟而产生的额外轮次
