# 设计

新增 `crates/nai-provider-core`（layer=provider），只依赖 contracts/security/budget-core（均 shared/contracts）。
引入 tokio、reqwest（rustls 默认、json/stream）、futures-util；无新增常驻服务或代理。
凭据由 composition root 通过 `nai_security::Secret` 传入，`Debug`/日志 redact，禁止写仓库。

## 传输与取消

`Operation`（deadline+cancelled）来自 nai-security。每次请求剩余时间转为 reqwest `.timeout(remaining)`，
读取循环之间检查 `op.check()`；取消置位或期限到期即停止消费并丢弃迟到块。
sleep 重试以 50ms 步进轮询 `op.stopped()`，绝不越过任务期限。

错误映射：401/403→UNAUTHENTICATED/PERMISSION_DENIED（不重试）；429→RESOURCE_EXHAUSTED；
连接/超时/5xx→PROVIDER_UNAVAILABLE（可重试）；取消/期限→CANCELLED/DEADLINE_EXCEEDED。

## SSE 与二进制流

字节流缓冲按 `\n` 切行（UTF-8 多字节不含 0x0A，切行安全），累积 `data:` 行，空行发事件，`[DONE]` 结束。
chunk 内的多行 data 与分片 UTF-8 完整还原。TTS 音频按块回传 `buffer_ref`/格式/序号，样本计数由 D11 解码器计算，
adapter 不声称样本级精确。媒体字节不进入 JSON/Base64 控制总线。

## 重试

Policy 含 max_retries（0=不重试）。429 解析 Retry-After（秒），取 min(值, 剩余期限)；5xx/连接错误
指数退避 `base*2^n + jitter`，初值 1 次额外尝试（配置可调）。任何重试前复检 op.check。
重试后仍不可用→PROVIDER_UNAVAILABLE；不会把同一请求重复派发。

## usage 与预算接线

Provider JSON usage（prompt_tokens/total_tokens 等）→ `BudgetCharge{currency, cost_minor, units{requests:1,tokens:N}, price_revision}`。
TTS/无 usage→Finish(None) 保留估算标 NeedsReconciliation。价格表/账单导入不是 D08 范围（未来实现），
cost_minor 在价格解析接入前如实记 0 且保留 units/price_revision，不伪造已定价金额。

`UsageGateway` trait（reserve/dispatch/finish）由 root 实现到 Store；`CallBudget` 包装：
reserve→dispatch(acquired=true 才发 I/O)→finish(结束或取消时 actual=None)。重复确认 acquired=false 不重发。
requests 计数不得被 token-only actual 抹掉（D07 已在 Book 校验）。TTS 已提交句段不得因重试再播。

## 验证

provider suite：mock（tiny_http）本地服务器覆盖 SSE 分片/多 data/[DONE]/错误、二进制流、
429 Retry-After 在/超期限、401/403 不重试、5xx 有限退避、取消/期限丢弃、usage 映射与 budget 接线顺序。
portable 全量、契约生成无漂移、前端构建、OpenSpec strict。
真实冒烟：`cargo run --example openai_smoke`，仅在环境提供 OPENAI_API_KEY 时运行一次最小 TTS+Embedding，
打印字节/维度/usage 计数，不打印密钥；无凭据环境保持 Blocked。CI 不自动执行真实调用。
Windows GNU 交叉检查受 D06 存储工具限制；provider 依赖 rustls 亦缺 MinGW C 工具链，如实记录不标通过。
