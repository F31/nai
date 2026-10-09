# D08 API Provider

## Why
V3.5 第 26 章要求真实 LLM/TTS/Embedding 的流解析、期限/取消、429/有限重试与 usage，且 provider 故障不得由本地模型伪装。D07 提供预算与派发接线，本轮补齐真实 HTTP 传输与 adapter。

## What Changes
- 新增 `nai-provider-core`：异步 HTTP 客户端、deadline/cancellation、有界重试与 SSE/二进制流解析。
- OpenAI-compatible adapter：LLM 流式 chat、TTS 音频、Embedding JSON；usage → BudgetCharge。
- 预算接线：调用前 reserve/dispatch claim、结束/取消后 finish；requests 计数不被 token actual 抹掉。
- 真实冒烟：环境 `OPENAI_API_KEY`（仅环境读取，不写入仓库）跑最小 TTS + Embedding；无凭据时保持 Blocked。
- provider suite、指南/报告与 OpenSpec 归档。

## Capabilities
### New Capabilities
- `api-provider-transport`: HTTP/SSE 传输、有界重试/取消与 usage 接线。

## Impact
新增 provider crate（layer=provider，只依赖 contracts/security/budget/shared），引入 tokio/reqwest/futures-util；
xtask/CI 新增 provider suite。无新常驻服务、自建代理或模型文件。真实计费在冒烟时按最小用量；价格表/账单导入仍属后续。
kernel/media 不依赖 provider-core；媒体字节不经过控制总线，adapter 只回传缓冲引用。
