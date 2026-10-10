## Context

判据 A02 是「仅改配置换 Provider」。要让它成为**可执行**的判据，必须先回答一个问题：替换的**单位**是什么？

引擎当前的调用方拿的是 `&mut dyn TextGenerator` / `&mut dyn SpeechSynthesizer`（`text.rs` / `speech.rs`），两者都是**同步** trait，且只被演练桩实现。真实 HTTP 适配器是**异步**的（reqwest + tokio），而 `apps/engine/src/main.rs` 是 `#[tokio::main]`——pipeline 运行在 tokio 运行时内。

于是有三个候选方案：

| 方案 | 做法 | 否决理由 |
|---|---|---|
| A. 让真实适配器实现同步 trait | 适配器内部持 `Runtime`/`Handle`，在同步方法里 `block_on` | engine 已在运行时内，`Handle::block_on` 会 panic；即使不在，在异步执行器里阻塞也是反模式。**否决**。|
| B. 把接缝改成异步 | trait 方法返回 `Pin<Box<dyn Future>>`（dyn 兼容）或引 `async-trait` | 会级联改动 engine 的同步 pipeline 阶段；且为「加一家供应商」而改动内核调用方，正是 A02 要避免的方向。引 `async-trait` 还是新增依赖。**否决**。|
| C. 按能力切面替换 | 新增 `LlmClient` / `SpeechClient` 枚举切面（异步，直接复用既有异步适配器），由配置决定的 `Protocol` 分派 | 调用方只持有切面、不出现供应商名；替换是配置值变化；engine 的同步桩接缝**原样保留**。**采用**。|

## Goals / Non-Goals

**Goals**
- 凭据呈现方式与供应商静态头**不再写死**在共享传输路径里。
- 文本与语音**各至少两种**线上协议，由配置选择。
- 同一个调用函数在两家协议上行为一致；线上差异**被实测到**（否则测试可能在同一协议上跑了两遍）。
- 供应商的用量字段名在**适配器边界**映射进既有封闭单位表，账本词表不放宽。

**Non-Goals**
- 不接真实付费账号（无余额，`Blocked` 不变）。
- 不改 engine 演练链路（`--event-file` 仍要求 `--rehearsal-provider`）。
- 不引入 mock 框架、`async-trait`、或任何新依赖。
- 不把「换 Provider」的证明升级为外部认证：替换测试证明的是**架构判据**。

## Decisions

### D1 协议决定凭据形态，配置只说协议名

`Protocol::auth()` 与 `Protocol::static_headers()` 是**唯一**来源：`openai` → `Bearer`；`anthropic` → `x-api-key` + `anthropic-version`；`elevenlabs` → `xi-api-key`。配置里写 `protocol: anthropic`，不写头名。

理由：若让配置直接写头名，调用方就能**半描述**一个供应商（写对了头名但配错了路径），那类错误只在线上才暴露。让协议一次性决定一组相关事实，错就错在解析协议名那一步，`Protocol::from_config` 会当场拒绝未知名称——**不落回默认**，因为拼错一个词不该静默地选中另一家供应商。

`HttpConfig` 仍保留 `auth` / `extra_headers` 公开字段：它是**低层传输配置**，`ProviderProfile` 才是**面向配置**的接缝。测试可以直接构造前者以覆盖边界校验。

### D2 替换的判据必须是「调用方看不出」

测试的设计要点在于：**同一个函数** `generate_once` / `speak_once` 分别服务两家协议，二者之间只有 `ProviderProfile`（一个配置值）不同。若只是各写一条路径各测各的，测试会通过但什么都没证明。

因此测试**同时**断言两件相反的事：
- **调用方可见的部分必须相同**（终止块序号、文本拼接结果、chunk 条数下限、格式采样率）；
- **线上必须确实不同**（路径、凭据头名、请求体形状、voice 在体内还是路径内）。

第二类断言是防「假通过」的关键：如果哪天有人把适配器改成共用一个协议，测试会在「线上应当不同」这一侧失败。

### D3 用量：映射在边界，缺就不编

Anthropic 报 `input_tokens`/`output_tokens`，OpenAI 报 `prompt_tokens`/`completion_tokens`/`total_tokens`。`usage_charge` 的封闭词表**不放宽**；映射发生在 `adapters/anthropic.rs::canonical_usage`，零字段**省略**而不是发一个 0（一个 0 单位会直接被账本拒绝，不是可用的计费信号）。

对**上游不报用量**的情形（OpenAI 兼容 TTS 不返回 usage），本轮**不**为它编造实际值：保留估算、`actual` 为空。测试按供应商分别断言这一差异，而不是把它压平成一条共同期望——把供应商事实伪装成供应商中立事实，正是上一轮「把压平后的错误文案当成结论」的同类错误。

ElevenLabs 报了 `x-character-count` 就用它；没报就用**发出文本的确定性字符数**（"第一句" = 3）。两者都落地测试。

### D4 语音按句独立

一次调用覆盖恰好一句话：**不**跨调用携带序号、缓冲引用或样本偏移。这样下游不可能把两句缝成一条连续缓冲。测试用一个 `voice-abc`、两次调用（"第一句" / "第二句"），断言两次都从 `seq=0` 起、数据块的 `sample_start` 恒为 0（属于自己的缓冲内偏移）、且两次请求体分别是各自的文本。

采样率是同一类问题的另一半：ElevenLabs 把码率编码在输出格式名里（`pcm_24000`）。供应商命名不了的采样率**必须拒绝**，不能就近取一个——`SpeechResponse::validate` 已经规定合成器不得用未请求的采样率作答，就近取码率会从下面破掉那条规则。测试断言拒绝时**未发出任何请求**、**未产出任何音频块**，且预留已被释放（不是挂着的 reserved）。

### D5 拒绝能力不符，不降级

`ProviderProfile::llm()` / `speech()` 先校验协议是否具备该能力。把语音专用协议绑成 LLM 供应商会**报错**，不落到某个默认供应商。测试断言此时**没有任何请求发出**。

## Risks / Trade-offs

- **两套接缝并存**：同步的演练桩接缝（`TextGenerator`/`SpeechSynthesizer`，带计量包装）与异步的真实适配器切面（`LlmClient`/`SpeechClient`，计量在适配器内经 `CallBudget`）目前是两条梯子。这是本轮**明确接受**的取舍：把它们合成一条需要把 engine 的同步 pipeline 转异步，代价与风险远大于「加一家供应商」本身。已记入后续工作项。
- **`ProviderProfile` 一次只描述一个 endpoint**：因此 OpenAI LLM + ElevenLabs TTS 是**两个** profile。这是刻意的：一个 profile 一种凭据一种呈现方式，避免出现「一个配置同时要求两种凭据头」的不自洽状态。
- **`provider_ref` 语义变更**：既有 `Usage.provider_ref` 由 endpoint 变为稳定身份。现有测试的构造点已同步更新；这是行为变更，已写入 delta spec。
- **未被本轮覆盖**：真实账号、真实 TTS 设备、engine 真实 provider 接线。三者保持 `Blocked`/未开始，不因测试变绿而升级。

## Migration Plan

无数据迁移。`provider_ref` 语义变更只影响新构造的配置：既有配置需显式补 `provider_ref`（`ProviderProfile` 由构造方提供）。

## Validation

```
source /c/Users/32791/.nai-msvc-env.sh
export CARGO_TARGET_DIR=E:/nai-build-target
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=4 cargo run -p nai-xtask --locked --offline -- check --profile portable
```

环境边界：本机为 Windows MSVC 宿主（Rust 1.96.0），`CARGO_BUILD_JOBS` / `RUST_TEST_THREADS` 是 cargo/libtest 原生开关，用于规避宿主提交限制；二者**不写进** portable 检查。`cargo test -p nai-provider-core` 不需要网络（两台 mock 均为本地 `127.0.0.1`）。
