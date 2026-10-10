# API Provider 传输与接线

```bash
cargo xtask test --suite provider
cargo test -p nai-provider-core --offline    # 含 text/speech 接缝与 rehearsal 桩单测
cargo xtask check --profile portable
# 真实冒烟（有凭据时）：OPENAI_API_KEY=... cargo run -p nai-provider-core --example openai_smoke
```

## 传输与取消

`ProviderClient` 绑定 endpoint、模型、price scope 与 Bearer 凭据（`Secret`，绝不落日志/仓库）。
每次请求把 `Operation` 剩余时间转成本地单调截止；reqwest `.timeout(remaining)` 约束整次请求，
流式读取循环之间调用 `op.check()`，取消/超时即丢弃迟到数据并以 CANCELLED/DEADLINE_EXCEEDED 结束，
不返回正常完成。未配置凭据时返回显式 UNAUTHENTICATED，不尝试本地降级。

错误映射：

| 来源 | 结果 | 是否重试 |
|---|---|---|
| 401/403 | UNAUTHENTICATED / PERMISSION_DENIED | 否 |
| 429 | RESOURCE_EXHAUSTED，跟随 Retry-After | 是（限期限内） |
| 连接/超时/5xx | PROVIDER_UNAVAILABLE，指数退避+jitter | 是（初值 1 次，可配置） |
| 取消/期限 | CANCELLED / DEADLINE_EXCEEDED | 否 |

最终错误会读取有界响应体并尽量提取 `error.message` 用于诊断，但不包含认证材料。

## SSE 与二进制流

`SseDecoder` 把原始字节按 `\n` 切行（UTF-8 多字节不含 0x0A，跨块安全），累积 `data:` 行、
空行发事件、`[DONE]` 结束，多行 data 与分片 UTF-8 完整还原。`collect_llm` 把
`choices[0].delta.content` 变为有序 LlmChunk，服务端 `error` 对象按 StreamError 终止，
final chunk 的 `usage` 转入账本（Settled）。未在 [DONE] 前出现的完整文本不当作结束。

TTS 使用 `/audio/speech`，二进制按块回传 `buffer_ref`/格式/序号，末尾 StreamComplete；
样本计数与解码由 D11 负责，adapter 不声称样本级精确。PCM/媒体字节不进入 JSON/Base64 控制总线。

## 预算接线

`UsageGateway`（reserve/dispatch/finish）由 composition root 实现到 D07 的 Store 持久账本，
测试用 `InMemoryGateway`。`CallBudget::start` 先 reserve 再 dispatch，`dispatched()==true` 才允许
Provider I/O；已派发/已结算的请求返回 acquired=false，绝不重复调用。结束、错误或取消都要调用
`finish(actual_or_None)`；TTS/未知 usage 用 None 保留估算并标 NeedsReconciliation，
重复账单幂等，修订需 revision CAS。

usage JSON 只接受白名单整数单位：requests/tokens/prompt_tokens/completion_tokens/total_tokens/
characters/audio_seconds/images；未知/浮点/零单位拒绝。requests 是尝试计数，不被 token-only 抹掉。

## 文本生成接缝（TextGenerator）与 rehearsal 桩

`nai-provider-core::text` 暴露一次文本生成的接缝：`TextRequest`（request/provider/session/utterance 标识、
`generation_epoch`、prompt、输出上界、剩余预算、取消标志）与 `TextResponse`（文本、模型标识、usage）。
`validate()` 拒绝不完整标识、空/超长 prompt（上界 `MAX_PROMPT_CHARS = 8192`）、非法输出上界
（`MAX_OUTPUT_CHARS = 4096`）与已取消请求；`remaining_budget_ms == 0` 返回 DEADLINE_EXCEEDED。
生成器自身**不得**做记账，调用方一律经 `generate_with_budget`。

`generate_with_budget(generator, gateway, entry, request, price_revision)` 走既有预算路径：
`CallBudget::start` 先 reserve 再 dispatch，仅 `dispatched()` 为真才调用生成器；已派发请求返回
`Already`（Conflict）而**不重发**；usage 经 `usage_charge` 按精确 `price_revision` 记账后 `finish` 结算。
接缝不绕过预算、取消或 usage 记账。

**rehearsal-only 确定性桩**：`RehearsalTextGenerator`（`model_ref = "rehearsal-deterministic-v1"`）
是纯计算、无网络、无凭据、可复现的桩，只在演练与测试路径显式构造。它 MUST NOT 用于真实集成、付费调用
或任何认证结论，也 MUST NOT 取代 API 故障处理；真实生成仍走 `OpenAiAdapter` 的 llm_stream。

## 语音合成接缝（SpeechSynthesizer）与 rehearsal 桩

`nai-provider-core::speech` 暴露与文本接缝**同纪律**的一次合成接缝：`SpeechRequest`
（request/provider/session/utterance 标识、`generation_epoch`、文本、目标采样率、voice 绑定、剩余预算、
取消标志）与 `SpeechResponse`（PCM、采样率、模型标识、usage）。`validate()` 拒绝空文本/空 voice、
不支持的采样率（`8000..=48000`）、超长文本（`MAX_SPEECH_CHARS = 4096`）与已取消请求；
`remaining_budget_ms == 0` 返回 DEADLINE_EXCEEDED。响应校验采样率一致、长度有界
（`MAX_SPEECH_SAMPLES = 240_000`）、样本有限且 `|s| <= 1.0`。

`synthesize_with_budget(synthesizer, gateway, entry, request, price_revision)` 与文本路径同形：
reserve → dispatch → 合成 → 校验响应 → `usage_charge` → `finish`；已派发请求返回 `Already`（Conflict）
而**不重发**。既有 HTTP 适配器与 `UsageGateway` 行为不变。

**rehearsal-only 确定性语音桩**：`RehearsalSpeechSynthesizer`
（`model_ref = "rehearsal-speech-deterministic-v1"`）是纯计算、无网络、无凭据、可复现的桩。它的样本
总数**只由文本与采样率决定**——`clamp(字符数 × 60ms, 400ms, 5000ms) × sample_rate / 1000`——因此
「预期样本总数」可以在合成之前独立算出，播放完成 ACK 才能被核验，而不是回读运行结果自证。
波形由文本的 FNV-1a 种子经 xorshift 生成（幅度 0.8）。该桩 MUST NOT 用于真实集成、付费调用或任何
认证结论，也 MUST NOT 取代 TTS 故障处理；真实 TTS 仍走 `OpenAiAdapter` 的 tts_stream。

```bash
cargo test -p nai-provider-core --offline    # text 6 项 + speech 5 项 + usage 等，共 11 项单测
```

## 适配器与 Provider 隔离

`OpenAiAdapter` 提供 llm_stream / tts_stream / embeddings。provider-core 只依赖
contracts/security/budget-core，不依赖 kernel/media/宿主；媒体字节仅以缓冲引用回传。
media layer 禁止依赖 provider（deps.rs 已校验）。新增 Provider 需同类 adapter 与 conformance。

## 真实集成状态

真实冒烟对 api.openai.com 发起最小 TTS + Embedding（已授权少量额度）；实测账户无余额：
embeddings 429「You have no credits remaining」、TTS 403。传输/TLS/鉴权/重试/429/403 错误映射与
错误提取均对真实端点验证通过，但付费生成未完成，**不标记 Passed**。LLM 真实流冒烟留待账户具备
余额且成本允许时执行。Windows GNU 交叉检查缺 MinGW C 工具链而阻塞，非原生认证。
价格表/账单导入、多 Provider 适配、真实音轨解码属于后续任务。
