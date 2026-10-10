## Why

W36 把真实凭据变成真实证据时，用 `scripts/provider-smoke.mjs`（端点级）与
`crates/nai-provider-core/examples/live_provider_smoke.rs`（经本栈）两条路径各跑一次真实调用。端点级两条链路都通；
但**经本栈的文本链路在 `agnes-2.5-flash` 上被拒**：

```
provider http 400: BadRequestError - {"message":"No user query found in messages.", ...}
```

根因不在模型，也不在网关，而在我们自己的适配器：`crates/nai-provider-core/src/adapters/openai.rs` 把
`permitted_context` 作为**单条 `role: "system"` 消息**发出，**没有任何 user 轮**。姊妹适配器
`adapters/anthropic.rs` 把同一个字段作为**单条 `role: "user"` 消息**发出，并在注释里写明了理由
（「Messages API 至少需要一个轮次，且本适配器不添加任何自拟提示文本：permitted context 就是那唯一一条 user 轮」）。

也就是说：**同一个语义字段，两个适配器给出了相反的角色**。W19 把这个差异当作「实测到的供应商差异」用测试钉住、并写进
`docs/provider.md` 的对照表——但 W19 记录的是**差异本身**，不等于**该差异是对的**。真实网关给出了裁决：
`agnes-3.0-flash` 容忍 system-only，`agnes-2.5-flash` 直接以 4xx 拒绝「没有用户查询」。

一条没有 user 轮的 chat-completions 请求**不是普遍合法的**。`permitted_context` 是要被处理的内容（守门放行的查询与证据），
把它整个当作系统指令发出，语义上也不对。本变更把它统一为 **user 轮**。

## What Changes

- **`crates/nai-provider-core/src/adapters/openai.rs`**：`messages[0].role` 由 `system` 改为 `user`，与 Anthropic 适配器一致；适配器仍**不添加任何自拟提示文本**。
- **新增能力规格条款**（`api-provider-transport`）：把「`permitted_context` 作为**恰好一条 `user` 轮**发出」与「请求体 MUST NOT 只含非 user 轮」写成**可检查的需求**，并说明两种文本协议的形状差异**不得延伸到同一语义字段的角色选择**。
- **`crates/nai-provider-core/tests/substitution.rs`**：更新「两种协议请求体形状不同」的断言与注释——差异仍在（路径、`stream_options` 标志、用量字段名），但**不再**包含角色。
- **`docs/provider.md`**：对照表的「请求体」一行由 `messages[system] + stream_options` 改为 `messages[user] + stream_options`。
- **不改** `Protocol` 枚举、`ProviderProfile`、共享传输、流帧解析与用量映射；**不改** Anthropic 适配器（它本就是 `user`）；**零新增依赖**，18 包与依赖方向不变。
- **两处冒烟的 `max_tokens` 由 32 提到 256**（`scripts/provider-smoke.mjs` 与示例）：被配置的模型是推理模型，token 上限若只够思考就以 `finish_reason: length` 结束、**可见输出为空**，冒烟会在每项检查都通过时报出空心成功。改动附注释说明理由。
- **registry**：A02 的 notes 追加 W19 差异中「角色」一项已被修正的说明；B01/O01 状态**不变**（这是一次线上契约修正，不是达标）。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `api-provider-transport`：新增需求「permitted context 以恰好一条 user 轮呈现」（2 个场景）。

## Impact

- 改动一个适配器的一行、更新一处测试断言与注释、更新 `docs/provider.md` 一行、新增一条能力需求。
- 真实链路：`live_provider_smoke` 示例的文本链路在 `agnes-2.5-flash` 上由 400 变为 200；证据 `docs/reports/evidence/live-provider-bound-stack.json` 重新生成。
- `provider-core` 的**调用方可见形状不变**（仍是有序块 + 单一终止块）；变化只在线上请求体的角色字段。
- 现有断言 `a.json()["messages"][0]["role"] == "system"` 会失败——这是本变更**预期**要改的那一处。

## Non-Goals

- 不引入「角色可配置」开关：角色不是供应商差异，而是最小合法形状；把同一语义字段的角色做成配置，会把「哪种角色是对的」变成可配置的正确性。
- 不添加 system 轮（例如空策略或自拟模板）：适配器 MUST NOT 自拟提示文本，加一条系统轮就是发明内容。
- 不改 Anthropic 适配器、不改其余协议路径、不改流帧结构与用量字段名。
- 不据此把 B01/O01 或任何验收项标为达标；一次连通不是验收。
