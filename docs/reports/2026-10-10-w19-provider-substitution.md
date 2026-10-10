# W19 实测报告：第二家 LLM 与 TTS Provider、替换验证

- 日期：2026-10-10
- 变更：`openspec/changes/w19-provider-substitution/`
- ADR：[0025](../adr/0025-provider-substitution-boundary.md)
- 判据：`tests/registry.yaml` A02「仅改配置换 Provider」、B03「成本对账」
- 指南：[docs/provider.md](../provider.md)

## 摘要

上一轮把「换 Provider 只改配置」（A02）记为 `implemented`、notes 写着「第二家替换测试待建」，W07 也明确
「不做第二家」。本次把它从**声明**变成**可执行判据**：文本与语音各新增一种与既有协议形态不同的真实线上
协议，由**配置**选择，**同一个调用函数**在两家的行为一致，而**线上差异被实测到**。

改动零新增依赖；`nai-provider-core` 全量 **36 passed / 0 failed**（18 lib + 10 provider + 8 substitution）；
`cargo xtask check --profile portable` **整体通过：EXIT=0，64 个测试目标，336 passed / 0 failed**
（基线 321 passed / 63 目标，本轮 +15 项）。

## 背景：判据为何一直悬着

共享传输路径里存在三处「把一家供应商的形态当成通用形态」：

1. `ProviderClient::request` 直接构造 `Authorization: Bearer <secret>`——用别的头承载凭据的供应商进不来，
   除非改共享传输代码（正是 A02 要禁止的）。
2. `Usage.provider_ref` 取的是 **endpoint URL**——换成本地代理或同一供应商的两个区域时，账本分不清谁答的。
3. `OpenAiAdapter` 的路径、请求体、SSE 帧字段与用量字段名同时承担「传输」与「某一家供应商的约定」。

## 改动清单

| 文件 | 改动 |
|---|---|
| `src/client.rs` | 新增 `AuthScheme`（默认 `Bearer` \| `Header{name}`）、`extra_headers`、`provider_ref`；`request()` 按 scheme 构造凭据头；严格校验（头名 RFC 9110 token、禁覆盖传输自管头、禁重复、禁 CR/LF/NUL、有界；`provider_ref` 非空有界） |
| `src/binding.rs`（新） | `Protocol`（`from_config`/`as_config`/`capability`/派生 auth 与静态头）、`ProviderProfile`（不含凭据）、`LlmClient`/`SpeechClient` 能力切面；能力不符显式 `InvalidArgument` |
| `src/adapters/anthropic.rs`（新） | Anthropic Messages：`POST /messages`、`x-api-key` + `anthropic-version`、体 `messages[user]`、SSE 按 JSON `type` 分派、`content_block_delta` 取文本、`input_tokens`/`output_tokens` 映射进封闭单位表 |
| `src/adapters/elevenlabs.rs`（新） | ElevenLabs TTS：`POST /v1/text-to-speech/{voice}?output_format=pcm_{rate}`、`xi-api-key`、体 `text`/`model_id`、裸二进制 PCM、字符成本取 `x-character-count` |
| `src/adapters/openai.rs` | `Usage.provider_ref` 由 endpoint 改为稳定供应商身份 |
| `src/{lib,adapters/mod}.rs` | 导出接缝类型 |
| `tests/provider.rs`、`examples/openai_smoke.rs` | `HttpConfig` 构造点同步更新 |
| `tests/substitution.rs`（新） | 两台本地 `tiny_http` mock，同一驱动函数覆盖两家协议 |

## 实测数字

```
$ cargo test -p nai-provider-core --offline
  Running unittests (lib)      : 18 passed; 0 failed
  Running tests\provider.rs    : 10 passed; 0 failed
  Running tests\substitution.rs:  8 passed; 0 failed
                                  ─────────────────────
                                  36 passed; 0 failed
```

`tests/substitution.rs` 八项：

| 用例 | 断言 |
|---|---|
| `the_same_caller_generates_text_through_two_different_wire_protocols` | 同一 `generate_once` 对 OpenAI 兼容与 Anthropic；文本结果一致；线上路径/头名/体形状不同 |
| `vendor_specific_usage_fields_land_in_the_same_canonical_units` | 两家用量字段名都落进 `prompt_tokens`/`completion_tokens`/`total_tokens`，且按 `provider_ref` 入账 |
| `the_same_caller_speaks_through_two_different_speech_protocols` | 同一 `speak_once` 对 OpenAI 兼容与 ElevenLabs；chunk 形状一致；voice 一在体内、一在路径内 |
| `each_sentence_is_synthesized_into_its_own_sequence` | 两次调用均从 `seq=0` 起、`sample_start` 恒 0、各结算一条 usage |
| `a_provider_that_reports_no_character_cost_still_bills_a_real_count` | 缺 `x-character-count` 时用发出文本的确定性字符数（"第一句"=3） |
| `a_rate_the_provider_cannot_produce_is_refused_before_any_request` | 48 kHz 被拒；无请求发出、无音频块、预留已释放 |
| `a_provider_is_refused_for_a_capability_it_does_not_have` | 语音专用协议绑为 LLM / 文本专用协议绑为 Speech 均报错且无请求 |
| `the_transport_shape_is_a_function_of_the_protocol_alone` | 配置换成不同协议只改变派生值，调用方对象不变 |

## 线上差异证据（防「同一协议跑两遍」）

替换测试**同时**断言「调用方可见一致」与「线上确实不同」。后者是防假通过的关键：

| 观测 | OpenAI 兼容 | Anthropic | ElevenLabs |
|---|---|---|---|
| 文本路径 | `POST /chat/completions` | `POST /messages` | — |
| 语音路径 | `POST /audio/speech` | — | `POST /v1/text-to-speech/voice-abc?output_format=pcm_24000` |
| 凭据头 | `authorization: Bearer <secret>` | `x-api-key: <secret>`（无 `authorization`） | `xi-api-key: <secret>`（无 `authorization`） |
| 静态头 | — | `anthropic-version: 2023-06-01` | — |
| 请求体 | `messages[0].role == "system"` + `stream_options.include_usage` | `messages[0].role == "user"`，无 `stream_options` | `text` + `model_id`，无 `voice` |
| voice 位置 | 体 `voice` | — | 路径段 |

> **勘误（W37）**：上表「请求体」一行记录的 `messages[0].role == "system"` 是**当时实现的写照，并非供应商差异**。
> 真实网关证明该形状不被普遍接受（严格网关以 400 `No user query found in messages` 拒绝无 user 轮的请求），
> W37 已把 `openai` 适配器的角色改为 `user`，与 `anthropic` 适配器一致。差异仍在，但**不含角色**。

若有人把适配器改成共用一个协议，测试会在「线上应当不同」一侧失败。

## 未覆盖边界（不得掩盖）

- **真实付费账号**：账户无余额，`Blocked` 不变；本轮全部在本地 mock，**无外网**。
- **真实 TTS 设备**：未接入；替换测试证明的是**架构判据**，不是设备/音频认证。
- **engine 真实 provider 接线**：未做；`apps/engine --event-file` 仍要求 `--rehearsal-provider`。
- **两套接缝并存**：同步演练桩 trait（`TextGenerator`/`SpeechSynthesizer`）与异步适配器切面
  （`LlmClient`/`SpeechClient`）是两条梯子，合成一条需把 engine 同步 pipeline 转异步，记入后续工作项。
- **`provider_ref` 语义变更**：由 endpoint 变为稳定身份，属行为变更，已写入 delta spec。

## 门控与命令

```
source /c/Users/32791/.nai-msvc-env.sh
export CARGO_TARGET_DIR=E:/nai-build-target
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=4 cargo run -p nai-xtask --locked --offline -- check --profile portable
```

结果 `EXIT=0`，末行原文 `portable: engineering foundation checks passed; device/SDK/GPU/install
certification not performed`；64 个测试目标（48 测试二进制 + 16 doc-test），**336 passed / 0 failed**。

环境边界：本机为 Windows MSVC 宿主（Rust 1.96.0）。`CARGO_BUILD_JOBS`/`RUST_TEST_THREADS` 是 cargo/libtest
原生开关，用于规避宿主提交限制，**不写进** portable 检查。另因宿主增量编译会话目录锁残留（`os error 5`）
导致 `LNK1104` 无法写输出 exe，本次运行以 `CARGO_INCREMENTAL=0` 规避；这是宿主事实，非产品缺陷，
同样不写进检查。`cargo test -p nai-provider-core` 不需要网络（两台 mock 均为本地 `127.0.0.1`）。
`registry` 双向校验、`validate --all --strict`（24 主规格）通过；归档校验 29 项通过。

## registry 状态（不越权升级）

A02/B03 补证据，**不**升级为 `tested`：没有真实付费账户、没有真实 TTS 设备，替换测试证明的是**架构判据**，
不是外部认证。真实付费链路保持 `Blocked`。
