# API Provider 传输与接线

```bash
cargo xtask test --suite provider
cargo test -p nai-provider-core --offline                       # text/speech 接缝与 rehearsal 桩单测
cargo test -p nai-provider-core --test substitution --offline   # 两家协议的替换验证（本地 mock，无外网）
cargo xtask check --profile portable
# 真实冒烟（有凭据时）：OPENAI_API_KEY=... cargo run -p nai-provider-core --example openai_smoke
```

## 传输与取消

`ProviderClient` 绑定 endpoint、模型、price scope、**供应商身份**（`provider_ref`）与凭据
（`Secret`，绝不落日志/仓库）。凭据的**呈现方式**由 `HttpConfig::auth`（`AuthScheme`）决定，共享请求路径
不再写死 `Authorization: Bearer`：`Bearer` 是默认，`Header{name}` 用于 `x-api-key`/`xi-api-key` 一类供应商；
`extra_headers` 承载供应商静态头（如 `anthropic-version`）。详见下节。
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

## 传输接缝：凭据呈现、静态头与供应商身份

「换 Provider 只改配置」曾是悬着的判据（registry A02）。悬着的根因是共享传输路径里**把一家供应商的形态
当成了通用形态**：凭据头写死为 `Authorization: Bearer`、账本用 endpoint 当作供应商身份。本轮把它们逐一
提升为配置事实：

- **`AuthScheme`**（`client.rs`，默认 `Bearer`）：`Bearer` → `Authorization: Bearer <credential>`；
  `Header{name}` → 该头名承载凭据本身（不加前缀、不做编码变换）。`request()` 按 scheme 构造凭据头，
  共享路径对所有供应商相同——新增供应商**不再需要改传输代码**。`Bearer` 保持默认，因此**不声明** scheme
  是「显式的 OpenAI 兼容选择」，而不是静默落空。
- **`extra_headers`**：供应商静态头（当前仅 Anthropic 的 `anthropic-version`）。只承载静态头，**不**承载凭据。
- **`provider_ref`**：供应商的稳定身份，替代原先的 endpoint。同一 endpoint 上的两家供应商、或换掉 endpoint
  的同名供应商，账本都能分辨；它同时进入 `Usage` 记账。

校验在构造 `ProviderClient` 时一次性完成，且是**拒绝而非事后清洗**：头名须为 RFC 9110 token；
不得覆盖传输自管的头（`authorization`/`host`/`content-length`/`content-type`）；不得重复；值不得含
`CR`/`LF`/`NUL`；条数 ≤8、名 ≤64、值 ≤1024；`provider_ref` 非空且 ≤128。任何一条不满足都直接返回
`InvalidArgument`，不会先发一个用不了的凭据位置出去。

## 协议、绑定与替换（`binding.rs`）

`binding.rs` 是**唯一**把「配置选择」变成「具体线上适配器」的地方；kernel 与 pipeline 从不出现供应商名字。

- **`Protocol`**：`openai`（= `openai-compatible`）/ `anthropic`（= `anthropic-messages`）/ `elevenlabs`，
  由 `from_config` 解析配置字符串。未知名称**报错**（`InvalidArgument`），不落回默认——拼错一个词不该静默
  选中另一家供应商。`as_config` 是其逆映射。
- **凭据头与静态头由协议派生**（`Protocol::auth()` / `static_headers()`），配置里只写协议名。若让配置直接写
  头名，就允许调用方**半描述**一个供应商（头名对、路径错），那类错误只在线上才暴露。
- **`ProviderProfile`**：描述一个 endpoint 的全部配置（`provider_ref`/`endpoint`/`model`/`price_revision`/
  `currency`/`timeout`/`max_retries`/`protocol`），**不含凭据**，因此可安全日志、对比与进测试。
  `http_config(credential)` 把协议派生出的 `auth`/`extra_headers` 与凭据合成低层 `HttpConfig`。
- **`LlmClient` / `SpeechClient`**：调用方持有的能力切面（异步枚举，直接复用异步适配器），按协议分派。
  调用方只持有切面、不出现供应商名——替换就是换一个 `ProviderProfile`（配置值）。
- **能力不符显式报错**：`llm()`/`speech()` 先 `require(capability)`。把语音专用协议绑成 LLM 供应商返回
  `InvalidArgument`，**不**降级到别的供应商；测试断言此时**没有任何请求发出**。

### 三家协议对照

| 维度 | `openai`（OpenAI 兼容） | `anthropic`（Messages） | `elevenlabs`（TTS） |
|---|---|---|---|
| 能力 | LLM + Speech | 仅 LLM | 仅 Speech |
| 凭据头 | `Authorization: Bearer` | `x-api-key` | `xi-api-key` |
| 静态头 | — | `anthropic-version: 2023-06-01` | — |
| 文本路径 | `POST /chat/completions` | `POST /messages` | — |
| 语音路径 | `POST /audio/speech` | — | `POST /v1/text-to-speech/{voice}?output_format=pcm_{rate}` |
| 请求体 | `messages[system]` + `stream_options` | `messages[user]`（无 `stream_options`） | `text` / `model_id` |
| voice 位置 | 请求体 `voice` | — | **路径段** |
| 文本流分帧 | `choices[0].delta.content` | `content_block_delta`（`delta.text`） | — |
| 用量字段 | `prompt_tokens`/`completion_tokens`/`total_tokens` | `input_tokens`/`output_tokens` | 响应头 `x-character-count` |
| 语音形态 | 二进制 PCM | — | 裸二进制 PCM |

用量字段名在**适配器边界**映射进既有封闭单位表（`adapters/anthropic.rs::canonical_usage`），
**不放宽**账本词表；零字段**省略**而非发 0（0 单位会被账本拒绝，不是可用计费信号）。
ElevenLabs 的字符成本优先用响应头 `x-character-count`，缺失时用**发出文本的确定性字符数**。

### 替换验证（`tests/substitution.rs`）

`tests/substitution.rs` 用两台本地 `tiny_http` mock（`127.0.0.1`，**无外网**）把 A02 变成可执行判据。
要点是**同一个驱动函数** `generate_once` / `speak_once` 分别服务两家协议，二者之间只有 `ProviderProfile`
（一个配置值）不同。因此测试**同时**断言两件相反的事：

- **调用方可见的部分必须相同**：终止块序号、文本拼接结果、chunk 条数下限、格式采样率一致；
- **线上必须确实不同**：路径、凭据头名、请求体形状、voice 在体内还是路径内。

第二侧是防「假通过」的关键——若哪天有人把适配器改成共用一个协议，测试会在「线上应当不同」这一侧失败。
用量按供应商**分别**断言，不压平成一条共同期望：ElevenLabs 报 `characters`；OpenAI 兼容 TTS **不报** usage，
故其 `actual` 为空、**不编造**一个计费数字。

### 语音按句独立与采样率拒绝

一次合成调用覆盖**恰好一句话**：**不**跨调用携带序号、缓冲引用或样本偏移，因此下游不可能把两句缝成一条
连续缓冲。测试用一个 `voice-abc`、两次调用（"第一句" / "第二句"），断言两次都从 `seq=0` 起、数据块
`sample_start` 恒为 0（属于自己的缓冲内偏移）、两次请求体分别是各自的文本、且各自结算一条 usage
（各以自己的 `request_id` 入账，不被并成一次计费调用）。

采样率是同一类问题的另一半：ElevenLabs 把码率编码在输出格式名里（`pcm_24000`），`SUPPORTED_RATES` 只含
`8000/16000/22050/24000/44100`。供应商**命名不了**的采样率一律**拒绝**，不就近取一个码率——
`SpeechResponse::validate` 已规定合成器不得用未请求的采样率作答，就近取会从下面破掉那条规则。
非单声道同样显式拒绝。测试断言拒绝时**未发出任何请求**、**未产出任何音频块**，且预留已被**释放**
（不是挂着的 `Reserved`）。voice 身份成为路径段，因此先校验其不得改变请求路径形状
（不得含 `/`、`?`、`#`、`..` 或超长）。

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

三个适配器各自承载**一家供应商的线上约定**，共享传输一视同仁：`OpenAiAdapter`（llm_stream / tts_stream /
embeddings）、`AnthropicAdapter`（`messages_stream`）、`ElevenLabsAdapter`（`tts_stream`）。差异（路径、
凭据头、请求体形状、SSE 分帧方式、用量字段名、voice 在体内还是路径内）**只**存在于适配器与 `binding.rs`
的协议派生里，调用方持有的 `LlmClient`/`SpeechClient` 切面看不到它们。

provider-core 只依赖 contracts/security/budget-core，不依赖 kernel/media/宿主；媒体字节仅以缓冲引用回传。
media layer 禁止依赖 provider（deps.rs 已校验）。新增 Provider = 一个同形 adapter + `Protocol` 的一个 arm
（含派生 auth 与静态头），**不改**共享传输、kernel 或 pipeline；并由 `tests/substitution.rs` 的
「调用方可见一致 + 线上确实不同」两侧断言守住——只加一个不改变线上形状的适配器会被该测试挡下。

## 真实集成状态

真实冒烟对 api.openai.com 发起最小 TTS + Embedding（已授权少量额度）；实测账户无余额：
embeddings 429「You have no credits remaining」、TTS 403。传输/TLS/鉴权/重试/429/403 错误映射与
错误提取均对真实端点验证通过，但付费生成未完成，**不标记 Passed**。LLM 真实流冒烟留待账户具备
余额且成本允许时执行。Windows GNU 交叉检查缺 MinGW C 工具链而阻塞，非原生认证。

**本轮（W19）把「仅改配置换 Provider」从声明变成可执行判据**：文本与语音各已至少两种真实线上协议，
由配置选择，同一调用函数在两家的行为一致而线上差异被实测到。但替换测试证明的是**架构判据**，
**不是**外部认证——真实付费账号（无余额，`Blocked` 不变）、真实 TTS 设备、engine 的真实 provider
接线均未接入。价格表/账单导入与真实音轨解码仍属后续任务。
