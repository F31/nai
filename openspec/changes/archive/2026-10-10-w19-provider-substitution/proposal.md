## Why
`tests/registry.yaml` 把 **A02「仅改配置换 Provider」** 记为 `implemented`，notes 写着「单 Provider 已实现；第二家替换测试待建」。规格 V3.6 W19 也把「第二家 LLM 与 TTS Provider、替换测试」列为工作项，且 W07 当时明确「不做第二家」，把它留给 W19。也就是说：**这个判据一直是悬着的**——没有任何证据表明「换 Provider 只改配置」成立。

悬着的具体原因是代码里存在**三处把单一供应商的形态当成了通用形态**：

1. **传输层写死了一种凭据呈现方式**：`ProviderClient::request` 直接构造 `Authorization: Bearer <secret>`，其余部分无法表达。任何用别的头部承载凭据（`x-api-key`、`xi-api-key`）的供应商都进不来，除非改共享传输代码——那正好是 A02 要禁止的。
2. **没有任何供应商身份**：`Usage` 的 `provider_ref` 取的是 **endpoint URL**。换成本地代理或同一供应商的两个区域时，账本会把两次不同的供应商当成同一条记录，替换之后就分不清谁答的。
3. **调用方只认识一种线上协议**：`OpenAiAdapter` 的路径、请求体形状、SSE 帧字段与 usage 字段名同时承担了「传输」和「某一家供应商的约定」两个职责。第二家协议要么复制这层逻辑，要么去改调用方。

另外还有一个**容易做错**的地方：真正困难的部分不是「再加一个适配器」，而是「换掉之后调用方看不出来」。如果替换测试只是分别跑两条各自的路径，它能通过却什么也没证明。

本轮的意图：把「换 Provider」从一句声明变成一条**可执行的判据**——同一个调用函数、只换配置值，两家线上协议都能跑通，且**线上差异被实测到**（否则测试可能只是在同一协议上跑了两遍）。

## What Changes
- **传输接缝去硬编码**（`client.rs`）：新增 `AuthScheme`（`Bearer` | `Header{name}`）、`extra_headers`、`provider_ref`。凭据呈现方式成为配置的一部分，共享请求路径对所有供应商完全相同。校验严格：头名须为 RFC 9110 token、不得覆盖传输自管的头（`authorization`/`host`/`content-length`/`content-type`）、不得重复、不得含 CR/LF/NUL、有界（≤8 条、名 ≤64、值 ≤1024）。`Bearer` 保持默认，因此 **不声明** scheme 是「显式的 OpenAI 兼容选择」而不是静默落空。
- **`Usage.provider_ref` 改为供应商身份**，不再取 endpoint。同一 endpoint 上的两个供应商、或换掉 endpoint 的同一供应商，账本都能分辨。
- **第二家 LLM 协议**（`adapters/anthropic.rs`）：Anthropic Messages。路径 `/messages`、`x-api-key` + `anthropic-version`、请求体 `system`/`messages` 分离、SSE 帧由 JSON `type` 字段区分（`content_block_delta` 而非 `choices[0].delta`）、用量字段为 `input_tokens`/`output_tokens`。供应商字段名**在适配器边界**映射进既有封闭单位表（`prompt_tokens`/`completion_tokens`/`total_tokens`），**不**放宽账本词表。
- **第二家 TTS 协议**（`adapters/elevenlabs.rs`）：ElevenLabs。路径 `/v1/text-to-speech/{voice}`（**voice 在路径里**，不在请求体）、`xi-api-key`、请求体 `text`/`model_id`、响应为裸二进制 PCM、字符成本走响应头 `x-character-count`。采样率由输出格式名承载：**供应商命名不了的采样率一律拒绝，不静默改码率**。voice 身份成为路径段，因此先校验其不得改变请求路径形状。
- **配置驱动的绑定接缝**（`binding.rs`）：`Protocol`（`openai` / `anthropic` / `elevenlabs`）由配置字符串解析，未知名称**报错而不是落回默认**；凭据头与供应商静态头**由协议派生**，配置里只写协议名；`ProviderProfile` 只描述一个 endpoint（不含凭据，可安全日志/对比）；`LlmClient` / `SpeechClient` 是调用方持有的能力切面，按协议分派，调用方不出现任何供应商名字。协议被问到它没有的能力时**显式报错**，不静默降级到别的供应商。
- **替换验证集成测试**（`tests/substitution.rs`）：两台本地 mock 端点（无外网），**同一驱动函数**分别对两家协议执行文本生成与语音合成，断言：线上差异确实存在（路径、凭据头名、请求体形状）、调用方可见结果形状一致、供应商字段名都落进同一封闭单位表、以及**语音按句独立**（序号不跨句续接、`sample_start` 恒为本次缓冲内偏移、每句各自结算一条 usage）。

## Capabilities
### New Capabilities
无。

### Modified Capabilities
- `api-provider-transport`：凭据呈现与供应商静态头 MUST 由协议派生且不得写死在共享传输路径；供应商身份 MUST 与 endpoint 分离并进入 usage；文本与语音各 MUST 至少存在两种线上协议且由配置选择；供应商未报告的用量 MUST NOT 被编造；语音 MUST 按句独立且不跨句续接。

## Impact
- 代码：`crates/nai-provider-core/src/{client,lib}.rs`、新增 `src/binding.rs`、`src/adapters/{anthropic,elevenlabs}.rs`、`src/adapters/openai.rs`（`provider_ref`）、`tests/provider.rs`（构造点更新）、`examples/openai_smoke.rs`（构造点更新）、新增 `tests/substitution.rs`。
- 规格：`api-provider-transport` 增量；`docs/provider.md` 增补；ADR 0025；报告 `docs/reports/2026-10-10-w19-provider-substitution.md`。
- 依赖：**零新增**（新增测试复用既有 `tiny_http` dev-dependency）。
- registry：A02 补证据（替换测试已建），B03 补「同一封闭单位表覆盖两家协议」证据；**不**升级产品项为 `tested`——没有真实付费账户、没有真实 TTS 设备，替换测试证明的是**架构判据**，不是外部认证。
- 明确不做：不接真实付费账号（无余额，Blocked 不变）；不改 engine 演练链路（其 `--event-file` 仍要求 `--rehearsal-provider`）；不引入 `async-trait` 或运行期桥接（engine 在 tokio 运行时内，同步接缝上做 `block_on` 会 panic，故按能力切面而非同步 trait 替换）。
