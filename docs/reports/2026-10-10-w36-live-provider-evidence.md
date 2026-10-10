# W36 实测报告：真实 Provider 接入纪律与真实调用证据

- 日期：2026-10-10
- 变更：`openspec/changes/w36-live-provider-evidence/`
- 决策：[ADR 0038](../adr/0038-live-provider-evidence-boundary.md)；指南：[docs/provider-live.md](../provider-live.md)
- 判据：`tests/registry.yaml` S07 / O01 / B01 / A02

> **勘误（W37）**：本报告的证据与门控基于当时的配置——文本模型 `agnes-3.0-flash`，以及**当时**把 `permitted_context`
> 作为单条 `role: "system"` 发出的 OpenAI 适配器。W37 修正了该角色（改为与 Anthropic 适配器一致的 `user` 轮），
> 并把文本模型标准化为 **`agnes-2.5-flash`**；两处冒烟的 token 上限也由 32 提到 256（推理模型需给可见输出留余量）。
> 证据文件已随之重新生成。本报告的**口径与结论不变**（§30.1、D11、登记状态纪律都成立），仅模型标识与线上请求体角色已更新；
> 详见 [W37 报告](2026-10-10-w37-provider-message-turn.md)。


## 结论

真实凭据到位后，风险从「机制不存在」变成「接入过程会污染仓库、或把事实读错」。本轮把接入纪律固化成**可执行、可检查**的形态：

- **凭据边界**：新增 `cargo xtask check --secrets`。扫描域是**git 会发布的内容**（`git ls-files --cached --others --exclude-standard`）——
  被忽略的本地 `.env` **不在**域内（否则本机无法持有凭据），而**运行期新建、尚未暂存**的文件**在**域内（否则检查只能在错误几乎铸成之后才发现）。
  两条测试各钉住一个方向。实测**全部待发布文件零命中**。
- **接入只是配置**：两家真实端点都是 OpenAI 兼容，`Protocol::OpenAiCompatible` 已同时 `serves` 文本与语音，**`Protocol` 枚举、适配器集合、
  `provider-core` 全部零改动**，只新增一份 `ProviderProfile`。
- **缺凭据必须 Blocked**：`scripts/provider-smoke.mjs` 在任一变量缺失时以 `CAPABILITY_NOT_READY` 非零退出、打印缺失**变量名**、
  **不写出**任何证据或音频；MUST NOT 回退 `RehearsalTextGenerator`/`RehearsalSpeechSynthesizer`（§30.1 点名「用固定素材冒充」）。
- **可用性是账户状态**：同一端点上模型甲 `200`、模型乙 `403`（配额余量为负）、模型丙 `503`（渠道缺失）——证据**逐模型**记录，
  结论**不推广到未实测的模型**。
- **证据只含事实、连凭据前缀都不含**：记录主机名、路径、方法、协议名、凭据**方案名**、状态码、网关错误码、用量字段名、响应字节数与 media type。

**本轮产生真实交付**：两份真实调用证据 JSON 与一段真实合成音频。**但不构成 B01/O01 达标**——B01 仍缺 100 题集与完整评估（保持 `blocked`）、
O01 仍缺约 10 分钟首句演练与约 30 分钟真实输出验证（保持 `pending`）。连通冒烟**不是**验收（D11）。

## 一、端点与账户事实（实测）

| 模型 | HTTP | 网关错误码 | 事实 |
| --- | --- | --- | --- |
| `agnes-3.0-flash` | 200 | — | 有可用通道 |
| `agnes-2.5-flash` | 200 | — | 有可用通道 |
| `agnes-3.0-flash-max` | 403 | `insufficient_user_quota` | 报文余量 `＄-0.000260`（账户状态） |
| `agnes-2.5-pro-alpha` | 403 | `insufficient_user_quota` | 报文余量 `＄-0.000260`（账户状态） |
| `agnes-2.0-flash` | 503 | `model_not_found` | `No available channel for model ... under group enterprise (distributor)`（渠道事实，**不是**凭据无效） |

端点 `/v1/models` 返回 200，真实清单为：

```
agnes-2.5-pro-alpha   agnes-video-2.5-flash  agnes-3.0-flash     agnes-image-2.1-flash
agnes-video-2.5       agnes-image-2.0-flash  agnes-2.5-flash     agnes-3.0-flash-max
agnes-2.5-pro         agnes-2.5-pro-beta     agnes-image-2.5-flash
```

**配置里写的 `agnes-2.0-flash` 不在该清单内**——它不是一个「端点整体不可用」的证据，而是一个「该模型名在本分组下无可用通道」的事实。这正是 D6 的由来。

### 语音端点契约

| 音色标识书写形式 | HTTP | media type | 字节 | 网关错误码 |
| --- | --- | --- | --- | --- |
| `alex`（裸名） | 400 | `application/json` | 53 | `20047` `Invalid voice.` |
| `FunAudioLLM/CosyVoice2-0.5B:alex`（端点要求的完整形式） | 200 | `audio/mpeg` | 非空 | — |

**契约差异只能靠实测发现**（D9）：冒烟**不**对失败输入做静默归一化或重试，4xx 连同错误码如实记录，且**不**被读作「端点不可用」。

## 二、接入纪律的四条实测输入（F1–F4）

| # | 事实 | 含义 |
| --- | --- | --- |
| F1 | 文本网关 `/v1/models` 返回 200，其真实模型清单**不含**配置里写的模型名 | 配置里的模型名可能不存在于端点 |
| F2 | 同一端点上模型甲 2xx、模型乙 403（报文含**剩余余量为负**） | 可用性是**账户状态**，不是端点的性质 |
| F3 | 语音端点以**显式错误码**拒绝不合书写形式的音色标识；按端点要求书写后返回音频 | 契约差异只能靠实测发现 |
| F4 | 本机对文本端点报证书吊销检查失败（`CRYPT_E_REVOCATION_OFFLINE`，schannel 已知行为），对语音端点不报 | **主机环境**差异，**不是**端点事实 |

F4 在证据里记为**主机环境**注记；MUST NOT 据此宣称端点不可用，也 MUST NOT 静默吞掉。本机冒烟因此对文本端点需 `--ssl-no-revoke`。

## 三、证据产出

两份证据均为**纯事实记录**，不含凭据前缀、不含 `Authorization` 头值、不含可还原凭据的片段。

`docs/reports/evidence/live-provider-facts.json`（`scripts/provider-smoke.mjs`，端点级冒烟）：

| 面 | 关键事实 |
| --- | --- |
| 文本（非流式） | `status=200`；`usage_fields=[completion_tokens, prompt_tokens, total_tokens]`；`finish_reason=stop` |
| 文本（流式 SSE） | `status=200`；`sse_data_lines=5`；`sse_terminator_done=true`（`[DONE]` 终止符作为契约事实记录，不进 JSON 解析）；`sse_unparsable_frames=0`；`sse_delta_events=4`；`sse_delta_keys=[role, content]` |
| 语音 | `status=200`；`media_type=audio/mpeg`；字节数取自响应体长度；`produced_audio=true` |

`docs/reports/evidence/live-provider-bound-stack.json`（`crates/nai-provider-core/examples/live_provider_smoke.rs`，**经本栈**）：
文本经 `ProviderProfile → ProviderClient → 适配器 → 预算网关` 得 `chunks=2 chars=2 terminal_chunk=true`；语音得 `chunks=33` 且落真实音频字节。
这条路径回答的是端点级冒烟回答不了的问题——**配置的端点经我们自己的装配能不能走通**。

音频产物：`docs/reports/evidence/voice-FunAudioLLM-CosyVoice2-0-5B.mp3`（真实字节，非桩、非占位、非固定素材）。

## 四、目标行为（八条需求 → 可检查形态）

| 需求 | 可检查形态 |
| --- | --- |
| 新端点是配置而非协议 | `Protocol` 枚举与适配器集合零改动；一份 `ProviderProfile` 接入；同一端点两能力共享同一 `endpoint` 与同一凭据 |
| 凭据只来自进程环境 | `check --secrets` 在「git 会发布的内容」上零命中；被忽略 `.env` 不在域、未暂存新文件在域 |
| 缺凭据即 Blocked | 冒烟以 `CAPABILITY_NOT_READY` 非零退出、不写文件；不回退桩 |
| 可用性是账户状态 | 逐模型记录状态/错误码/余量；结论只覆盖实测模型；渠道缺失记作渠道事实 |
| 证据只有事实 | 字段固定；无凭据前缀、无请求头值、无请求行原文 |
| 音频是真实字节 | 字节数取自响应体、media type 取自响应头；空体/错误报文判为未产出证据 |
| 契约差异以实测为准 | 4xx 连同错误码记录；不静默归一化、不吞掉 |
| 冒烟不是验收 | 冒烟成功不改动 B01/O01 的登记状态 |

## 五、测试

`xtask/src/secrets.rs`（`cargo test -p nai-xtask --lib` 内，共 **8 项**）：

- **命中**：前缀加足额长度的合成凭据被报出，带**行号**与**前缀族**；匹配文本**从不**被携带、打印或存储。
- **阈值**：同前缀但长度低于阈值的哨兵（如 `sk-live-must-not-leak`，前缀后 18 字符）不命中，达到阈值即命中。
- **前缀族**：`sk-`/`wk-`/`pk-`/`rk-`/`ghp_`/`github_pat_`/`glpat-`/`xox[baprs]-`/`AKIA`/`AIza`/`Bearer` 各自覆盖或形状校验（含 `AKIA` 小写正文**不**命中）。
- **不误伤**：散文、占位符、`token: "..."` 这类合法词汇不被命中。
- **自命中规避**：扫描器源码自身**不自命中**（`assert!(scan_text(include_str!("secrets.rs")).is_empty())`）。
- **扫描域**（两个方向）：被忽略的本地状态（`.env`、`.workbuddy/`）**不在**域内，而 `Cargo.toml` 等在域内；
  **一个新建、尚未暂存的探针文件在域内**（`a_new_file_is_scanned_before_it_is_staged`）。
- **工作区自身干净**：对真实工作区跑扫描为零命中。

## 六、实跑

```
cargo xtask check --secrets
credential scan passed: 672 files git would publish, no prefixed credential material

cargo xtask check --registry
admission registry check passed (V3.6 acceptance + D01-D14 mapping)

cargo xtask check-deps
dependency boundaries passed: 18 workspace packages (normal/build/dev/target declarations)

cargo test -p nai-xtask --lib
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

node --env-file=.env scripts/provider-smoke.mjs
text: status=200
text(stream): status=200
speech: status=200
both links produced a real result
EXIT=0

cargo xtask check --profile portable
EXIT=0；82 个测试目标（65 Running + 17 Doc-tests）；675 passed / 0 failed；LNK=0
portable: engineering foundation checks passed; device/SDK/GPU/install certification not performed
```

> **勘误（W38 复核）**：「82 个测试目标（65 Running + 17 Doc-tests）」**多算了一个**——`65` 里含 cargo 打印的
> `Running ...nai-xtask.exe check --profile portable` 包装行，它不是测试目标。按 `Running <test target>` 精确重数为
> **64 + 17 = 81**（`675 passed / 0 failed` 不变）。不改写正文，见 [W38 报告](2026-10-10-w38-desktop-approval-surface.md)。

前端门控（`apps/desktop`、`packages/contracts-ts`）：

```
tsc --noEmit（contracts-ts）          EXIT=0
tsc --noEmit（desktop）               EXIT=0
vite build（desktop）                 ✓ 29 modules transformed; ✓ built in 3.58s
node --experimental-strip-types --test tests/*.test.mjs（desktop）
# tests 5  # pass 5  # fail 0
```

目标数 82（W36 未新增测试目标，密钥扫描 8 项落在既有 `nai-xtask` lib 目标内）；通过项 667→**675**（+8）。

## 七、依赖边界

- 落 `xtask/src/secrets.rs`（tool 层，无依赖限制）；`lib.rs` 仅 +2 行（`pub mod secrets;` 与一条 `["check", "--secrets"]` 分发臂）。
- **`provider-core` 零改动**：`Protocol`、`ProviderProfile`、`LlmClient`、`SpeechClient` 全部不变；两家端点各只差一份 `ProviderProfile`。
- **零新增外部依赖**：扫描器仅用 `std` 与 `git`；`provider-smoke.mjs` 仅用 Node 22 内建 API。
- 18 包与依赖方向不变（`check-deps` 通过）。

## 八、未覆盖边界

- **不挑选供应商、不保存凭据、不替账户充值**：账户余额与渠道由账户状态决定，不在本变更可控范围。
- **不覆盖任意形态的密钥**：扫描只覆盖**已知高熵前缀**；未知前缀的令牌**不在覆盖范围**，不得据此宣称「仓库无任何密钥」。
- **日志/诊断包/崩溃转储的扫描仍未建**：S07 保持 `implemented` 不升级。
- **冒烟不是验收**：B01 保持 `blocked`（100 题集与完整评估未接）、O01 保持 `pending`（约 10 分钟首句演练与约 30 分钟真实输出验证仍需人工）。
- **不新增证据目录约定**：§30.1 已为 RAG/媒体/升级/UI/性能指定目录，**未为真实 Provider 证据指定目录**；音频与事实记录落 `docs/reports/evidence/`，与既有报告同处 `docs/reports` 之下。
- **真实凭据只存在本地未跟踪 `.env` 与进程环境中**：不进仓库、不进证据、不进日志。
