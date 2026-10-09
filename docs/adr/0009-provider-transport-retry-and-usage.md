# ADR 0009：Provider 传输、重试与用量接线

- 日期：2026-10-07
- 状态：Accepted
- 基线：V3.5 第 26 章；D08

## 决策

新增 `nai-provider-core`（layer=provider），只依赖 contracts/security/budget-core。
凭据经 `nai_security::Secret` 由 composition root 注入，日志/Debug 一律 redact，不写仓库。
每个请求把 `Operation` 期限转成本地单调截止：reqwest `.timeout(剩余)`，读取循环之间 `op.check()`；
取消/超时丢弃迟到字节，永不返回正常完成。

错误映射：401/403→UNAUTHENTICATED/PERMISSION_DENIED（不重试）；429→RESOURCE_EXHAUSTED（跟随 Retry-After，
不超过任务期限）；连接/超时/5xx→PROVIDER_UNAVAILABLE（有限指数退避+jitter，初值 1 次额外尝试，可配置）。
任一重试前复检 op.check。最终错误读取有界响应体并解析 `error.message`，不泄露认证材料。

SSE 解码按字节 `\n` 切行（UTF-8 多字节不含 0x0A），累积 `data:` 行、空行发事件、`[DONE]` 结束；
多行 data 与分片 UTF-8 完整还原，服务端 error 对象按 StreamError 终止。TTS 音频二进制按块回传
`buffer_ref`/格式/序号，样本计数由 D11 解码器计算；PCM/媒体字节不进入 JSON/Base64 控制总线。

OpenAI-compatible adapter 映射 chat-completions（SSE）、audio/speech（二进制）、embeddings（JSON）。
Provider JSON usage 经白名单整数单位（requests/tokens/prompt_tokens/completion_tokens/total_tokens/
characters/audio_seconds/images）转 BudgetCharge；未知/浮点/零单位拒绝。TTS 不返回 usage，
账本保持 NeedsReconciliation 估算，不伪造零账单或双扣。

派发使用 D07 的 DispatchClaim 语义：仅 acquired=true 才启动 I/O；ACK 丢失重试返回 acquired=false，
不重新调用 Provider。结束/取消都必须调用 finish(actual_or_None)。重复账单幂等，修订 revision CAS。
LLM 流 usage 落在 final chunk（include_usage），实测映射到 Settled。

## 验证与边界

provider 单元/集成测试用本地 mock（tiny_http）覆盖 SSE 分片/多 data/[DONE]/错误、二进制流、
429 Retry-After 在/超期限、401/403 不重试、5xx 有限退避、取消/期限丢弃、usage 映射与预算顺序。
真实冒烟对 api.openai.com 的 embeddings 与 TTS 发起最小请求（用户已授权少量额度）；实测账户无余额：
embeddings 429「no credits remaining」、TTS 403，传输/鉴权/重试/429/403 错误映射/错误提取均对真实
端点验证通过，但付费生成因账户计费未完成，如实记 Blocked 而非 Passed。未产生计费调用。
LLM 流路径本轮未做真实冒烟（token 成本较大），与 TTS 共用传输路径，风险已由 mock 覆盖。
Windows GNU 交叉检查缺 MinGW C 工具链（aws-lc-sys）而阻塞，与 D06 存储同因，未标通过。
真实 Provider usage/账单导入/价格表/多 Provider 适配属于后续任务；真实音轨解码/播放属 D11。
