## 1. 规格与依赖
- [x] 1.1 核对基线/契约并严格校验 OpenSpec 工件。
- [x] 1.2 引入 provider-core 依赖并登记 workspace/layer 规则，无契约生成变更。
## 2. 实现与测试
- [x] 2.1 实现 HTTP 传输、deadline/cancellation 与错误映射及 mock 测试。
- [x] 2.2 实现 SSE/二进制流解析与 LLM/TTS/Embedding adapter 及分片/错误测试。
- [x] 2.3 实现有界重试（429/5xx/401/403/期限）与预算接线（reserve/dispatch/finish）测试。
## 3. 交付
- [x] 3.1 加入 provider suite/CI，执行 portable/契约/前端与真实冒烟（有凭据）。
- [x] 3.2 更新 ADR/指南/计划/报告/工程入口并严格校验，归档。

## Workflow follow-up
验证后归档并校验全部规格与归档任务；真实集成与逻辑测试分开记录。
