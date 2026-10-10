# ADR 0025：Provider 替换的接缝边界——按能力切面替换、凭据形态由协议派生、用量在适配器边界映射且不编造

- 日期：2026-10-10
- 状态：Accepted
- 基线：V3.6 §1.4（R1 能力预设）与 §31 工作项 W19；承接 ADR 0015（基线与验收注册表）、ADR 0024（工程校验回路）
- 变更：`openspec/changes/w19-provider-substitution/`
- 判据：`tests/registry.yaml` A02「仅改配置换 Provider」、B03「成本对账」

## 背景

`tests/registry.yaml` 把 A02 记为 `implemented`，notes 写着「单 Provider 已实现；第二家替换测试待建」，
W07 也明确「不做第二家」。也就是说这条判据**一直悬着**：没有任何证据表明「换 Provider 只改配置」成立。

悬着的根因是共享路径里**把一家供应商的形态当成了通用形态**，共三处：

1. `ProviderClient::request` 直接构造 `Authorization: Bearer <secret>`。用别的头承载凭据的供应商
   （`x-api-key`、`xi-api-key`）进不来，除非改共享传输代码——那正是 A02 要禁止的。
2. `Usage.provider_ref` 取的是 **endpoint URL**。换成本地代理或同一供应商的两个区域时，账本会把两家
   当成同一条记录，替换之后分不清谁答的。
3. `OpenAiAdapter` 的路径、请求体形状、SSE 帧字段与用量字段名同时承担「传输」与「某一家供应商的约定」
   两个职责；第二家协议要么复制这层逻辑，要么去改调用方。

真正的难点不是「再加一个适配器」，而是「换掉之后调用方看不出来」——如果替换测试只是分别跑两条各自路径，
它能通过却什么都没证明。

## 决策

### D1 替换的单位是**能力切面**，不是同步 trait

引擎调用方当前拿的是同步 trait `&mut dyn TextGenerator` / `&mut dyn SpeechSynthesizer`，且只被演练桩实现；
真实 HTTP 适配器是异步的（reqwest + tokio），而 `apps/engine` 是 `#[tokio::main]`，pipeline 跑在运行时内。

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 让真实适配器实现同步 trait | 适配器内持 `Runtime`/`Handle`，同步方法里 `block_on` | **否决**：engine 已在运行时内，`Handle::block_on` 会 panic；即便不在，异步执行器里阻塞也是反模式。 |
| B. 把接缝改成异步 trait | 返回 `Pin<Box<dyn Future>>` 或引 `async-trait` | **否决**：会级联改动 engine 的同步 pipeline 阶段，为「加一家供应商」而改内核调用方正是 A02 要避免的；引 `async-trait` 还是新增依赖。 |
| C. 按能力切面替换 | 新增异步 `LlmClient` / `SpeechClient` 枚举切面，直接复用既有异步适配器，由配置决定的 `Protocol` 分派 | **采用**：调用方只持切面、不出现供应商名；替换是配置值变化；engine 的同步桩接缝**原样保留**。 |

同步桩接缝（`text.rs` / `speech.rs`）与异步适配器切面（`binding.rs`）**并存**，是本轮明确接受的代价：
合并成一条需要把 engine 的同步 pipeline 转异步，风险远大于「加一家供应商」本身。

### D2 凭据形态与静态头由协议派生，配置只说协议名

`Protocol::auth()` 与 `Protocol::static_headers()` 是**唯一**来源：`openai` → `Bearer`；
`anthropic` → `x-api-key` + `anthropic-version`；`elevenlabs` → `xi-api-key`。配置写 `protocol: anthropic`，
不写头名。

若让配置直接写头名，调用方就能**半描述**一个供应商（头名对、路径错），那类错误只在线上才暴露。
让协议一次性决定一组相关事实，错就错在解析协议名那一步：`Protocol::from_config` 遇未知名称**报错**，
**不落回默认**——拼错一个词不该静默选中另一家供应商。

`HttpConfig` 仍保留 `auth` / `extra_headers` 公开字段：它是**低层传输配置**；`ProviderProfile` 才是
**面向配置**的接缝（不含凭据，可安全日志/对比/进测试）。

### D3 拒绝能力不符，不降级

`ProviderProfile::llm()` / `speech()` 先校验协议是否具备该能力。把语音专用协议绑成 LLM 供应商**报错**
（`InvalidArgument`），不落到某个默认供应商；测试断言此时**没有任何请求发出**。

### D4 用量：映射在适配器边界，缺就不编

Anthropic 报 `input_tokens`/`output_tokens`，OpenAI 报 `prompt_tokens`/`completion_tokens`/`total_tokens`。
`usage_charge` 的封闭词表**不放宽**；映射在 `adapters/anthropic.rs::canonical_usage`，零字段**省略**
而不是发一个 0（0 单位会被账本拒绝，不是可用计费信号）。

对**上游不报用量**的情形（OpenAI 兼容 TTS 不返回 usage），**不**为它编造实际值：保留估算、`actual` 为空。
测试按供应商**分别**断言这一差异，而不把它压平成一条共同期望——把供应商事实伪装成供应商中立事实，
与 ADR 0024 记下的「把压平后的错误文案当成结论」是同类错误。

### D5 替换测试必须同时证明「一致」与「不同」

`tests/substitution.rs` 用两台本地 mock（无外网），**同一驱动函数**分别服务两家协议，二者只差一个
`ProviderProfile`。测试**同时**断言两件相反的事：调用方可见的部分**必须相同**（终止块序号、文本拼接结果、
chunk 条数下限、格式采样率）；线上**必须确实不同**（路径、凭据头名、请求体形状、voice 在体内还是路径内）。
第二侧是防「假通过」的关键——把适配器改成共用一个协议会让它在「应当不同」一侧失败。

### D6 语音按句独立，采样率不可就近取

一次合成调用覆盖**恰好一句话**，不跨调用携带序号、缓冲引用或样本偏移，因此下游不可能把两句缝成连续缓冲。
ElevenLabs 把码率编码在输出格式名里（`pcm_24000`），供应商**命名不了**的采样率一律**拒绝**、不就近取——
`SpeechResponse::validate` 已规定合成器不得用未请求的采样率作答。拒绝时**不发出请求、不产出音频块**，
且释放预留（不留下挂着的 `Reserved`）。

## 后果

- **A02 从声明变为可执行判据**：文本与语音各已至少两种真实线上协议，由配置选择；同一调用函数在两家行为
  一致而线上差异被实测到。替换的代价从「改传输/内核代码」降为「换一个 `ProviderProfile`」。
- **`provider_ref` 语义变更**：由 endpoint 变为稳定身份。既有测试构造点已同步更新；这是行为变更，已写入
  delta spec。同一 endpoint 上的两家供应商、或换掉 endpoint 的同名供应商，账本都能分辨。
- **零新增依赖**：替换测试复用既有 `tiny_http` dev-dependency；未引 `async-trait`、未引 mock 框架。
- 代价（如实）：**两套接缝并存**（同步演练桩 trait + 异步适配器切面）是两条梯子，合并留作后续工作项；
  `ProviderProfile` 一次只描述一个 endpoint，因此 OpenAI LLM + ElevenLabs TTS 是**两个** profile。
- 残余缺口（不得掩盖）：真实付费账号（无余额，`Blocked` 不变）、真实 TTS 设备、engine 的真实 provider
  接线（其 `--event-file` 仍要求 `--rehearsal-provider`）均未接入。替换测试证明的是**架构判据**，
  **不是**外部认证；A02/B03 **不**因测试变绿升级为 `tested`。
