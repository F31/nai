# 真实 Provider 接入与证据

两条互补的冒烟路径：**本栈路径**验证我们自己的绑定与适配器对得上真实端点；**端点路径**只验证端点本身，并产出机器可读证据。前者是「我们的代码能不能驱动它」，后者是「它到底答什么」。

```bash
# 本栈：ProviderProfile -> ProviderClient -> 适配器 -> 预算网关
set -a && . ./.env && set +a
cargo run -p nai-provider-core --example live_provider_smoke

# 端点：直接发请求，产出事实记录与真实音频
node --env-file=.env scripts/provider-smoke.mjs

# 凭据边界：git 会发布的内容里不得出现明文凭据
cargo xtask check --secrets
```

## 环境变量

| 变量 | 含义 |
|---|---|
| `NAI_LLM_BASE_URL` | 文本端点基址，含版本段（如 `https://<host>/v1`） |
| `NAI_LLM_API_KEY` | 文本端点凭据 |
| `NAI_LLM_MODEL` | 文本模型标识，**以端点 `/v1/models` 的实际清单为准** |
| `NAI_TTS_BASE_URL` | 语音端点基址 |
| `NAI_TTS_API_KEY` | 语音端点凭据 |
| `NAI_TTS_MODEL` | 语音模型标识 |
| `NAI_TTS_VOICE` | 音色标识，**按端点要求的完整书写形式**填写 |

`.env.example` 只登记变量名，值为空。真实值放本地 `.env`——它被 `.gitignore` 覆盖，因此不在 `check --secrets` 的扫描域内。

## 凭据边界

- 凭据**只经进程环境变量**注入；从未写入文件、日志、文件名或证据。
- `cargo xtask check --secrets` 的扫描域是 **git 会发布的内容**（已跟踪 ＋ 未跟踪但未被忽略）。它**不是**磁盘（否则本机凭据必然自命中而使检查失去意义），也**不只是**暂存集合（否则新增文件在 `git add` 之前逃过检查）。
- 命中时只报**文件、行号与前缀族**，不回显匹配内容——把秘密从私有文件搬进构建日志不是修复。
- 覆盖范围以**已知高熵前缀**为界（`sk-`/`wk-`/`pk-`/`rk-`、`ghp_`/`github_pat_`、`glpat-`、`xox[baprs]-`、`AKIA`、`AIza`、`Bearer <长令牌>`）。**无已知前缀的自定义令牌不在覆盖范围内**，这一点是能力边界，不是承诺。

## 证据

| 文件 | 产出者 | 内容 |
|---|---|---|
| `docs/reports/evidence/live-provider-bound-stack.json` | `live_provider_smoke` 示例 | 经本栈的每条链路：端点、协议名、模型、块数、字符数/音频字节数、终止块是否存在 |
| `docs/reports/evidence/live-provider-facts.json` | `scripts/provider-smoke.mjs` | 逐次请求的 HTTP 状态、网关错误码、用量字段、media type、字节数、SSE 帧统计 |
| `docs/reports/evidence/voice-*.mp3` | 同上 | 真实音频字节 |

证据字段只含事实：`host`、`path`、`method`、`protocol`、`auth_scheme`（**只记方案名**）、`model`、`status`、`gateway_error_code`、`usage_fields`、`bytes`、`media_type`。**不含**凭据、请求头原值或可还原凭据的片段。

## 实测契约事实

这些是实测记录，不是文档假设；端点为 OpenAI 兼容，故都按 `openai` 协议接入（见 ADR 0038 D1）。

- **模型清单以端点为准**：配置里写的模型名可能不在 `/v1/models` 的清单内，此时网关报渠道缺失类错误。
- **可用性是账户状态**：同一端点、同一凭据下，某档模型 2xx 而另一档 403 且报文给出剩余余量（另有档位被免费套餐限流，报文为 `reached the API rate limit for free users`）。逐模型记录，结论不推广。
- **音色标识的书写形式是契约**：不合形式会被端点以显式错误码拒绝；按端点要求书写后返回音频。冒烟不做静默归一化。
- **文本请求的轮次**：`permitted_context` 作为**恰好一条 `user` 轮**发出，适配器不自拟提示文本（ADR 0039）。**系统轮不构成「用户查询」**——严格网关会以 `400 No user query found in messages` 拒绝只有系统轮的请求，故两处冒烟都发 `user` 轮。
- **SSE 帧形状**：`data: <json>` 的增量帧，流以 `data: [DONE]` 这一**非 JSON** 帧终止。适配器消费的正是这个形状。**增量帧的键集随模型而异**：普通模型为 `role`/`content`；**推理模型**另有 `reasoning_content`（先流思考、再流可见 `content`）。证据记**所有增量帧的键并集**，不取单帧采样。
- **推理模型与 token 上限**：可见 `content` 可能为空——若上限只够 `reasoning_content`，流以 `finish_reason: length` 结束而**没有任何可见输出**，此时「链路成功」是**空心**的。冒烟的 `max_tokens` 取 256 以容纳思考（32 时实测为空、256 时有可见输出）。**不**把「可见输出为空」做成失败判据：那是参数与模型的交互，不是连通性。
- **用量字段名随模型而异**：基础为 `prompt_tokens`/`completion_tokens`/`total_tokens`；推理模型另带 `completion_tokens_details`（`reasoning_tokens`/`text_tokens`）与 `prompt_tokens_details`（`cached_tokens`）。映射进同一封闭单位表，**字段名以响应为准**。
- **块数不稳定**：同一请求两次运行的音频块数可不同（实测 12 与 33），字节数量级稳定。**不要**把块数当断言。

## 边界与不覆盖项

- **冒烟不是验收**。它不是 B01 的「100 题集达门槛」，也不是 O01 的「约 10 分钟首句演练语音」；一次成功不改变这两项的登记状态。
- **不覆盖任意形态的密钥**（见上）。
- **不覆盖检索质量**：Recall@5 等指标属于黄金集与知识栈，不属于本路径。
- **不覆盖真实设备与声道**：本栈冒烟取回的是音频字节，不是设备播放结果。
- **不挑选供应商、不替账户充值**；余额不足是外部事实。

## 主机差异

本机 Windows 的证书吊销检查处于离线状态，`curl` 对部分端点报 `CRYPT_E_REVOCATION_OFFLINE`。这是**主机**事实：**不得**据此宣称端点不可用，也**不得**静默吞掉。Rust 侧（reqwest）与 Node 侧的 `fetch` 各有自己的信任链，实测两者均能连上本 ADR 涉及的两个端点。

## 相关

- ADR：`docs/adr/0038-live-provider-evidence-boundary.md`、`docs/adr/0039-provider-message-turn.md`
- 报告：`docs/reports/2026-10-10-w36-live-provider-evidence.md`、`docs/reports/2026-10-10-w37-provider-message-turn.md`
- 传输与接线：`docs/provider.md`
- 黄金集门槛与载荷：`docs/golden-set.md`
