# 设计：permitted context 的轮次呈现

## 背景

`permitted_context` 是守门放行后交给模型处理的内容。两个文本适配器对它的**角色选择相反**：

| 适配器 | 发出的 messages | 注释 |
|---|---|---|
| `adapters/anthropic.rs` | `[{"role": "user", "content": permitted_context}]` | 有注释说明：Messages API 至少需一个轮次，且本适配器不添加自拟提示文本 |
| `adapters/openai.rs` | `[{"role": "system", "content": permitted_context}]` | 无注释 |

W19 用测试把该差异钉住并写进 `docs/provider.md`，但那是**记录差异**，不是**判断对错**。

实测裁决（W36 起）：同一网关 `apihub.agnes-ai.com`，

| 模型 | 结果 |
|---|---|
| `agnes-3.0-flash` | 200 —— 容忍 system-only |
| `agnes-2.5-flash` | **400** `BadRequestError: No user query found in messages.` |
| `agnes-2.0-flash` | 不可用（不在 `/v1/models`，429/503） |

同一个适配器、同一份请求体，模型的容忍度不同——说明 system-only **不是**普遍合法形状。

## 决策

### D1 落点：只改 `openai.rs` 一行，与 Anthropic 适配器对齐

`messages[0].role` 由 `system` 改为 `user`。Anthropic 适配器**不动**（它本就是 `user`，且有注释说明理由）。
改后两个文本适配器在**同一语义字段上给出同一角色**，形状差异只剩路径、`stream_options` 标志与用量字段名——
这些是**真实的协议差异**，继续保留并由测试钉住。

### D2 为什么是 `user` 而不是 `system`

三条独立理由，任一条都足以定案：

1. **语义**：`permitted_context` 是**待处理的内容**（守门放行的查询与证据），不是策略指令。把它整个当作系统指令，语义错位。
2. **姊妹适配器**：`anthropic.rs` 已用 `user` 并写明理由（不添加自拟提示文本，permitted context 就是那唯一一条 user 轮）。同一字段两种角色是**内部不一致**。
3. **实测**：严格网关以 400 拒绝「没有用户查询」。一条没有 user 轮的 chat-completions 请求不是普遍合法形状。

### D3 否决：加一条 system 轮 + 一条 user 轮

**否决**。适配器 MUST NOT 自拟提示文本（D5 与 §30.1 的诚实性同源）。凭空加一条系统轮就是**发明内容**：
它既不在契约里，也不由调用方提供，却会被发给供应商并可能影响回答。要么只有一条 user 轮，要么不改。

### D4 否决：把角色做成可配置项

**否决**。角色不是供应商差异，而是**最小合法形状**。做成配置后，「哪种角色是对的」就成了**可配置的正确性**，
调用方必须知道某个网关要不要 user 轮——这违反「调用方 MUST NOT 需要知道供应商特有的形状」。角色是该字段的固定呈现。

### D5 适配器不得自拟提示文本（保持）

本变更**不添加**任何提示文本。请求体只由 `permitted_context`、`model`、`max_output_tokens` 与协议附加标志构成。
这条性质在改动前后都成立，本变更把它写进规格以免未来回归。

### D6 不改共享传输、协议枚举与契约

`Protocol` 枚举、`ProviderProfile`、`ProviderClient`、SSE 解码、用量映射与预算接线**全部不变**。
调用方可见形状不变：仍是有序块序列与单一终止块。变化只在线上的**角色字段**。

### D7 测试如何钉住两个方向

`tests/substitution.rs` 的「一次调用函数、两种协议」测试继续断言**差异存在**（路径 `/chat/completions` vs `/messages`、
`stream_options` 有 vs 无、凭据头 `authorization` vs `x-api-key`），并把角色断言由 `system` 改为 `user`——
**两个适配器都断言 `user`**，使「同一语义字段的角色一致」成为可检查性质，而不只是注释里的意图。

### D8 证据与登记状态

`live_provider_smoke` 示例重新运行，本栈证据 `docs/reports/evidence/live-provider-bound-stack.json` 重新生成
（文本链路恢复为事实记录）。**B01/O01 状态不变**：一次连通既不是题集达标也不是演练达标（延续 W36 D11）。

### D9 冒烟必须给推理模型留出可见输出的余量

修好角色后，本栈冒烟的文本链路返回 `chunks=1 chars=0`——**链路通了却没有可见输出**。实测根因不在适配器：

| `max_tokens` | 可见 `content` | `reasoning_content` 字符数 | `finish_reason` |
|---|---|---|---|
| 32 | `""` | 67 | `length` |
| 256 | `"\n\n收到"` | 542 | `stop` |

被配置的模型是**推理模型**：它先把思考作为 `reasoning_content` 增量流出，再发可见的 `content`。token 上限若只够思考，
就以 `finish_reason: length` 结束、可见输出为空。若冒烟用 32 的上限，它会在**每一项检查都通过**的情况下报出 `chars == 0`——
一个「绿了但什么都没产出」的空心成功，正是本项目要在证据里避免的形状。

因此两处冒烟的 `max_tokens` 由 32 提到 **256**，并在源码注释里写明理由（而非只改数字）。这不是「把参数调到通过」：
冒烟的目的是验证链路**能产出真实结果**，一个既产生不出可见输出、又把空输出记为成功的上限，并没有在测这件事。

**不**把「可见输出为空」做成失败判据：那会把「模型在给定上限内用满预算」误判为链路故障，而它是**参数与模型的交互**，不是连通性问题。

## 风险与边界

- **风险**：个别网关可能偏好 system 轮承载上下文。但 user 轮是 chat-completions 的**普遍合法**形状（`agnes-3.0-flash` 同样接受 user 轮，端点级冒烟已证），
  而 system-only 已被证明**不被普遍接受**。取舍方向明确：从不普遍合法走向普遍合法。
- **边界**：本变更只覆盖**文本**请求体的角色；语音合成不经 messages，不受影响。
- **不新增证据目录约定**：沿用 `docs/reports/evidence/`（W36 已确立，§30.1 未为真实 Provider 证据指定目录）。
