# D08 API Provider 实测报告

## 状态

- 已实现：provider-core 异步 HTTP 传输（deadline/cancellation）、错误映射、有界重试（429/5xx/401/403）、
  SSE/二进制流解析、OpenAI-compatible LLM/TTS/Embedding adapter、usage→BudgetCharge 与
  CallBudget（reserve/dispatch/finish）接线。
- Linux/WSL 实测通过：provider 11 项（lib 1 + 集成 10）、全 workspace 122 项 Rust、契约无漂移、
  Node 校验与前端构建。
- 真实冒烟：对 api.openai.com 最小 TTS+Embedding（已授权）。**账户无余额**：embeddings 429
  「no credits remaining」、TTS 403；传输/鉴权/重试/429/403 映射/错误提取对真实端点验证通过，
  付费生成未完成 → Blocked（账户计费），未发生计费调用。LLM 真实流冒烟留待有余额。
- Blocked：Windows GNU provider 交叉检查缺 MinGW C 工具链（aws-lc-sys），与 D06 存储同因。
- NotApplicable：契约生成变更（D08 无新消息）、数据库迁移、常驻服务。

## 修改

新增 `crates/nai-provider-core`（layer=provider）、workspace member、xtask provider suite、CI 步骤；
新增依赖 tokio/reqwest(0.13, rustls, json+stream)/futures-util（dev: tiny_http）；Cargo.lock 更新。
`nai_security::Operation` 增加 `remaining()`。凭据仅从环境读取，未写入仓库。
OpenSpec `d08-api-provider` 已 strict 校验，含真实集成与逻辑分开记录要求。

## 实际检查

| 命令 | 结果 |
|---|---|
| `cargo xtask check --profile portable` | 10 包依赖边界、fmt、Clippy -D warnings、122 项 Rust 测试通过 |
| `cargo xtask test --suite provider` | provider 11 项通过（含 mock 集成） |
| `cargo xtask schema --check` | 27 能力、107 消息；生成无漂移 |
| `pnpm typecheck`、`pnpm build` | 通过 |
| `OPENSPEC_TELEMETRY=0 pnpm exec openspec validate d08-api-provider --strict --no-interactive` | 通过 |
| `OPENAI_API_KEY=*** cargo run -p nai-provider-core --example openai_smoke` | 真实端点 TLS/鉴权/重试/错误映射验证；embeddings 429 无余额、TTS 403；未计费 |
| `cargo check --locked -p nai-provider-core --tests --target x86_64-pc-windows-gnu` | Blocked：aws-lc-sys 缺 x86_64-w64-mingw32-gcc |

全量 122 = contracts 6、kernel 31、budget 9、media 7、hosts 9、SDK 14、security 4、storage 15、
xtask 11、provider 11（本轮新增）。

## 重要验证（mock 集成，无外网）

- SSE 分片 UTF-8/多 data/[DONE] 完整还原；错误对象按 StreamError 终止；[DONE] 前无完整文本不结束。
- 429 Retry-After 在期限内重试成功；超过期限不重试；401/403 永不重试。
- 5xx 有限退避（初值 1 次）后仍不可用返回显式 PROVIDER_UNAVAILABLE。
- 取消/期限中止流并丢弃迟到块；调用失败后账本仍 finish(None) 保留估算。
- TTS 二进制按块+StreamComplete；账本 NeedsReconciliation、actual 为空。
- Embedding usage 精确映射 tokens/prompt_tokens/total_tokens，requests 计数保留，Settled。
- 已派发/已结算的 ACK 重试 acquired=false，不再次调用 Provider；通用 dispatch 拒绝重复派发。

## 失败与修复

- tiny_http 0.12 API（from_listener SSL 参数、Request 非 Clone、recv 循环）适配。
- SSE 测试夹具 JSON 缺右括号；适配器/解码器本身正确，修正夹具并补 usage 事件。
- CallBudget 的 finish 用派发前 revision 导致 CAS 冲突；改为捕获派发后 revision。
- 已结算请求的 dispatch 应从 Reserved 之外的终态返回 Already，而非 Conflict；测试补 ACK 重试路径。
- InMemoryGateway 工作区 id 硬编码与冒烟工作区不一致；加 workspace 参数。
- usage 单位未知会破坏预算；白名单整数单位，未知/浮点/零拒绝。
- Windows GNU 交叉检查因工具链缺失如实记 Blocked。

## 成本/范围与下一项

真实冒烟未产生计费（账户无余额），未写凭据。provider 测试只用本地 mock，CI 不调用真实 API。
价格表、账单导入/对账、多 Provider adapter、真实 LLM 流集成与音轨解码分别属后续 Provider 与 D11 工作。
下一项 D09：知识完整链路（导入、中文 FTS、向量空间、远端契约、原子发布）。
未提交或推送。

## OpenSpec 归档

`d08-api-provider` 完成 8/8 任务，合并 4 条需求为 `openspec/specs/api-provider-transport/spec.md`。
归档目录：`openspec/changes/archive/2026-10-08-d08-api-provider/`。
归档后 `pnpm spec:validate` 与 `openspec validate --archived --no-interactive`（OPENSPEC_TELEMETRY=0）
验证 10 个主规格与 D01–D08 八项归档全部通过。
