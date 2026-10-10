# W37 实测报告：文本请求的轮次呈现（permitted context 是唯一一条 user 轮）

- 日期：2026-10-10
- 变更：`openspec/changes/w37-provider-message-turn/`
- 决策：[ADR 0039](../adr/0039-provider-message-turn.md)；指南：[docs/provider-live.md](../provider-live.md)、[docs/provider.md](../provider.md)
- 判据：`tests/registry.yaml` A02

## 结论

W36 用真实凭据跑两条冒烟时，**经本栈**的文本链路在 `agnes-2.5-flash` 上被 400 拒绝：

```
provider http 400: BadRequestError - {"message":"No user query found in messages."}
```

根因不在模型、不在网关，而在**我们自己的适配器**：`permitted_context` 这个字段，Anthropic 适配器作为 `role: "user"` 发出，
OpenAI 适配器却作为 `role: "system"` 发出——**同一个语义字段，两个适配器给出相反的角色**。W19 把该差异当作「实测到的供应商差异」用测试钉住并写进对照表，
但**记录差异不等于判断对错**。真实网关给出裁决：`agnes-3.0-flash` 容忍 system-only，`agnes-2.5-flash` 直接拒绝「没有用户查询」。

本轮把它统一为 `user` 轮，并把这条性质**写进规格**（新增需求，2 个场景），而不只是改一行代码。

修好角色后暴露出**第二个问题**：本栈冒烟返回 `chunks=1 chars=0`——链路通了却没有可见输出。实测根因是**推理模型与 token 上限的交互**：
被配置的模型先流 `reasoning_content`（思考），再流可见 `content`；上限只够思考时以 `finish_reason: length` 结束且没有可见输出。
32 的上限会让冒烟在**每项检查都通过**的情况下报出**空心成功**。两处冒烟的 `max_tokens` 因此提到 256。

**登记状态不变**：一次连通既不是 B01（100 题集达门槛）也不是 O01（约 10 分钟首句演练）的达标。

## 一、根因（实测）

| 适配器 | 发出的 messages | 结果 |
| --- | --- | --- |
| `adapters/anthropic.rs` | `[{"role": "user", "content": permitted_context}]` | 有注释说明理由 |
| `adapters/openai.rs`（改前） | `[{"role": "system", "content": permitted_context}]` | 无注释 |

同一网关 `apihub.agnes-ai.com`、同一适配器、同一请求体，仅模型不同：

| 模型 | 结果 |
| --- | --- |
| `agnes-3.0-flash` | 200 —— 容忍 system-only |
| `agnes-2.5-flash` | **400** `BadRequestError: No user query found in messages.` |

模型的容忍度不同 ⇒ **system-only 不是普遍合法形状**。

## 二、推理模型与可见输出（实测）

同一端点、同一请求（`只回答两个字：收到`），仅 `max_tokens` 不同：

| `max_tokens` | 可见 `content` | `reasoning_content` 字符数 | `finish_reason` |
| --- | --- | --- | --- |
| 32 | `""`（空） | 67 | **`length`** |
| 256 | `"\n\n收到"` | 542 | `stop` |

一个既产生不出可见输出、又把空输出记为成功的上限，**并没有在测「链路能产出真实结果」**。因此：

- `scripts/provider-smoke.mjs` 与 `examples/live_provider_smoke.rs` 的 `max_tokens` 由 32 提到 **256**，并在源码注释里写明理由（不是只改数字）。
- **不**把「可见输出为空」做成失败判据：那是**参数与模型的交互**，不是连通性故障。

## 三、证据记增量帧的**键并集**

`sse_delta_keys` 原先取**首个 delta** 的键，于是推理模型的 `reasoning_content` 帧完全不被记录——读者会据此以为帧形状里没有推理字段。
改为记**所有增量帧键的并集**（排序）。实测：

| 模型 | `sse_delta_keys` |
| --- | --- |
| 普通模型 | `["content", "role"]` |
| `agnes-2.5-flash`（推理） | `["content", "reasoning_content", "role"]` |

## 四、改动

| 文件 | 改动 |
| --- | --- |
| `crates/nai-provider-core/src/adapters/openai.rs` | `messages[0].role`：`system` → `user`，补注释说明「permitted context 是那唯一一条 user 轮，适配器不自拟提示文本」 |
| `crates/nai-provider-core/tests/substitution.rs` | 注释与断言更新：差异仍断言（路径、`stream_options`、凭据头），**角色改为两个适配器都断 `user`** |
| `scripts/provider-smoke.mjs` | `max_tokens` 32→256（附注释）；`sse_delta_keys` 改记键并集 |
| `crates/nai-provider-core/examples/live_provider_smoke.rs` | `max_output_tokens` 32→256（附注释） |
| `docs/provider.md` | 对照表「请求体」：`messages[system]` → `messages[user]` |
| `docs/reports/2026-10-10-w19-provider-substitution.md` | 加**勘误**注记（角色一行是当时实现的写照，非供应商差异） |
| `docs/reports/2026-10-10-w36-live-provider-evidence.md` | 加**勘误**注记（该轮基于 `agnes-3.0-flash` 与 `system` 角色） |
| 新增 | `docs/adr/0039-provider-message-turn.md`、本报告 |
| 规格 | `api-provider-transport` 新增需求「permitted context 以恰好一条 user 轮呈现」（2 场景） |

**不改** `Protocol` 枚举、`ProviderProfile`、`ProviderClient`、SSE 解码、用量映射与预算接线；**不改** Anthropic 适配器（本就是 `user`）。
**零新增依赖**，18 包与依赖方向不变。

## 五、实测

```
cargo test -p nai-provider-core --offline
36 passed / 0 failed（lib 18 + provider 10 + substitution 8）

node --env-file=.env scripts/provider-smoke.mjs
text: status=200
text(stream): status=200
speech: status=200
both links produced a real result            EXIT=0

cargo run -p nai-provider-core --example live_provider_smoke
text ok: endpoint=https://apihub.agnes-ai.com/v1 protocol=openai model=agnes-2.5-flash chunks=3 chars=4 terminal=true
speech ok: endpoint=https://api.siliconflow.cn/v1 protocol=openai model=FunAudioLLM/CosyVoice2-0.5B bytes=47743 chunks=25
live provider smoke passed                   EXIT=0
```

对比改前：本栈文本链路由 `400 No user query found in messages` 变为**200 且有可见输出**。

证据文件（`docs/reports/evidence/`）已随之重新生成：`live-provider-facts.json`（端点级）、`live-provider-bound-stack.json`（经本栈）、`voice-*.mp3`（真实音频字节）。

门控：

```
cargo xtask check --secrets
credential scan passed: 680 files git would publish, no prefixed credential material

cargo xtask check --registry
admission registry check passed (V3.6 acceptance + D01-D14 mapping)

cargo xtask check-deps
dependency boundaries passed: 18 workspace packages (normal/build/dev/target declarations)

cargo xtask check --profile portable
EXIT=0；82 个测试目标（65 Running + 17 Doc-tests）；675 passed / 0 failed

tsc --noEmit（contracts-ts / desktop）  EXIT=0
vite build（desktop）                    ✓ 29 modules transformed
node --experimental-strip-types --test tests/*.test.mjs（desktop）  # tests 5  # pass 5  # fail 0
```

目标数与通过项均与 W36 持平（82 / 675）：本变更**未新增测试目标**，只改了既有断言的角色值。
（首轮门控曾因示例可执行文件被占用报 `LNK1104`，移除被锁文件后一次通过——本机既知现象，与代码无关。）

> **勘误（W38 复核）**：上文的「82 个测试目标（65 Running + 17 Doc-tests）」**多算了一个**——`65` 里含 cargo 打印的
> `Running ...nai-xtask.exe check --profile portable` 包装行，它不是测试目标。按 `Running <test target>` 精确重数为
> **64 + 17 = 81**（`675 passed / 0 failed` 不变）。不改写正文，见 [W38 报告](2026-10-10-w38-desktop-approval-surface.md)。

## 六、未覆盖边界

- **不引入「角色可配置」开关**：角色是最小合法形状，不是供应商差异；做成配置会把「哪种角色是对的」变成可配置的正确性。
- **不添加系统轮**：适配器 MUST NOT 自拟提示文本，加一条系统轮就是发明内容。
- **不改 Anthropic 适配器、不改其余协议路径、不改流帧结构与用量字段名**。
- **不改任何验收项状态**：B01 保持 `blocked`（题集与完整评估未接）、O01 保持 `pending`（演练仍需人工）。
- **余额/套餐仍由账户决定**：`agnes-2.0-flash` 当前不可用（不在 `/v1/models`，被免费套餐限流），本变更不解决账号问题。
