# ADR 0039：文本请求的轮次呈现——permitted context 是唯一一条 user 轮

- 日期：2026-10-10
- 状态：Accepted
- 基线：V3.7 §30.1（诚实性：不得用桩或空结果冒充真实产出）、ADR 0038（真实 Provider 接入边界，D9「契约差异以实测为准」、D11「冒烟不是验收」）、W19（第二家替换：协议差异实测并钉住）
- 变更：`openspec/changes/w37-provider-message-turn/`
- 判据：`tests/registry.yaml` A02 的 notes；`crates/nai-provider-core/tests/substitution.rs`

## 背景

W36 用真实凭据跑两条冒烟时，**经本栈**的文本链路在 `agnes-2.5-flash` 上被拒：

```
provider http 400: BadRequestError - {"message":"No user query found in messages."}
```

根因在我们自己的适配器。`permitted_context` 是守门放行后交给模型处理的内容，而两个文本适配器对它的**角色选择相反**：

| 适配器 | 发出的 messages | 注释 |
|---|---|---|
| `adapters/anthropic.rs` | `[{"role": "user", "content": permitted_context}]` | 有：Messages API 至少需一个轮次，且适配器不添加自拟提示文本 |
| `adapters/openai.rs` | `[{"role": "system", "content": permitted_context}]` | 无 |

W19 把这个差异当作「实测到的供应商差异」用测试钉住、写进 `docs/provider.md` 的对照表。但**记录差异不等于判断对错**。实测给出裁决（同一网关 `apihub.agnes-ai.com`，同一适配器、同一请求体）：

| 模型 | 结果 |
|---|---|
| `agnes-3.0-flash` | 200 —— 容忍 system-only |
| `agnes-2.5-flash` | **400** `No user query found in messages.` |

模型的容忍度不同，说明 **system-only 不是普遍合法形状**。

## 决策

### D1 文本适配器把 `permitted_context` 作为**恰好一条 `user` 轮**发出

`openai.rs` 的 `messages[0].role` 由 `system` 改为 `user`，与 `anthropic.rs` 对齐。适配器**不添加任何提示文本**——不补系统轮，也不加模板前缀。

### D2 为什么是 `user`

1. **语义**：`permitted_context` 是**待处理的内容**（守门放行的查询与证据），不是策略指令。整段当作系统指令是语义错位。
2. **内部一致**：姊妹适配器已用 `user` 并写明理由；同一字段两种角色是内部不一致。
3. **实测**：严格网关以 400 拒绝「没有用户查询」。一条没有 user 轮的 chat-completions 请求不是普遍合法形状。

### D3 否决「system 轮 + user 轮」

适配器 **MUST NOT** 自拟提示文本。凭空加一条系统轮就是**发明内容**：它不在契约里、不由调用方提供，却会被发给供应商并可能影响回答。要么只有一条 user 轮，要么不改。

### D4 否决「角色可配置」

角色不是供应商差异，而是**最小合法形状**。做成配置后，「哪种角色是对的」变成**可配置的正确性**——调用方必须知道某个网关要不要 user 轮，违反「调用方 MUST NOT 需要知道供应商特有形状」。角色是该字段的固定呈现，由规格钉住。

### D5 两种协议的形状差异**不得**延伸到同一语义字段的角色

差异继续保留并由测试钉住的是**真实协议差异**：路径（`/chat/completions` vs `/messages`）、协议附加标志（`stream_options` 有 vs 无）、凭据头（`authorization` vs `x-api-key`）、用量字段名。**角色不在其中**。测试改为**两个适配器都断言 `user`**，使「同一语义字段角色一致」成为可检查性质，而不只是注释里的意图。

### D6 冒烟必须给推理模型的可见输出留出余量

修好角色后，本栈冒烟返回 `chunks=1 chars=0`——链路通了却**没有可见输出**。实测根因（同一端点）：

| `max_tokens` | 可见 `content` | `reasoning_content` 字符数 | `finish_reason` |
|---|---|---|---|
| 32 | `""` | 67 | `length` |
| 256 | `"\n\n收到"` | 542 | `stop` |

被配置的模型是**推理模型**：先流 `reasoning_content`（思考），再流可见 `content`。上限只够思考时，流以 `finish_reason: length` 结束且**没有可见输出**——冒烟会在**每项检查都通过**的情况下报出空心成功，正是本项目要在证据里避免的形状。

因此两处冒烟的 `max_tokens` 由 32 提到 **256**，并在源码注释里写明理由。**不**把「可见输出为空」做成失败判据：那是**参数与模型的交互**，不是连通性故障。

### D7 证据记增量帧的**键并集**，不取单帧采样

`sse_delta_keys` 原先取首个 delta 的键，于是推理模型的 `reasoning_content` 帧完全不被记录，读者会以为帧形状里没有推理字段。改为记**所有增量帧键的并集**（排序）——事实应当覆盖整个流，而不是它的第一帧。

### D8 契约与共享组件不变

`Protocol` 枚举、`ProviderProfile`、`ProviderClient`、SSE 解码、用量映射与预算接线**全部不变**；调用方可见形状不变（有序块 + 单一终止块）。变化只在线上请求体的**角色字段**。18 包与依赖方向不变，零新增依赖。

### D9 登记状态不变

一次连通**不是** B01（100 题集达门槛）或 O01（约 10 分钟首句演练）的达标。修好角色只是让文本链路**可通**，不改任何验收项状态（延续 ADR 0038 D11）。

## 被否决的替代方案

| 方案 | 否决理由 |
|---|---|
| 保持 `system`，把 LLM 换成容忍 system-only 的模型 | 把「形状是否合法」外包给模型容忍度；换一个网关就再次翻车 |
| 加一条空的/模板化的系统轮 | 适配器自拟提示文本 = 发明内容（D3） |
| 角色做成 `ProviderProfile` 配置项 | 可配置的正确性（D4） |
| 把空可见输出判为失败 | 把参数与模型的交互误判为链路故障（D6） |
| 让 `sse_delta_keys` 保持单帧采样 | 证据漏掉推理帧，形状描述不实（D7） |

## 后果

- **正向**：同一语义字段在两个协议下角色一致；严格网关不再拒；`provider-core` 的单测把「角色一致」钉成可检查性质。
- **成本**：`docs/provider.md` 的对照表与 W19 报告需带勘误；W36 报告与证据需重新生成。
- **风险**：个别网关可能偏好系统轮承载上下文。取舍方向明确——从**不被普遍合法**走向**普遍合法**（`agnes-3.0-flash` 同样接受 user 轮，端点级冒烟已证）。

## 相关

- 指南：[docs/provider-live.md](../provider-live.md)、[docs/provider.md](../provider.md)
- 报告：[W37](../reports/2026-10-10-w37-provider-message-turn.md)、[W36](../reports/2026-10-10-w36-live-provider-evidence.md)、[W19](../reports/2026-10-10-w19-provider-substitution.md)
- 上位 ADR：[0038 真实 Provider 接入边界](0038-live-provider-evidence-boundary.md)
