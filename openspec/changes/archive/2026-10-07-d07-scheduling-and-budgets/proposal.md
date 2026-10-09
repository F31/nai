# D07 调度与预算

## Why
V3.5 第 22 章要求输入过载先去重/合并/过期、入选后才启动付费请求，并使全局、session、Provider 额度同时成立。D06 提供取消和媒体门控，本轮补齐前置准入与用量账本。

## What Changes
- 有界 EventIngress、CandidateQueue、UtteranceQueue，关键状态独立槽位、过期/epoch 拒绝、冷却/公平与播放债务。
- 共享 budget-core，估算预留、多维额度/并发、已派发取消保留估算、实际结算与幂等对账。
- storage usage_ledger 持久化，重启仍占用未明额度，不自动重新派发。
- 先增量 Protobuf 账本消息与可选配置，再生成三语言消费者；新增 scheduler suite/文档/报告。

## Capabilities
### New Capabilities
- `scheduling-and-budgets`: 有界调度、公平、债务及共享预算账本。

## Impact
kernel、contracts、config schema、storage、新增 budget-core、xtask/CI。
遵循统一内核与 Local 无常驻服务要求；复用现有 std/prost/serde_json/SQLite，无新增第三方版本、后台服务或付费调用。
节目生成/Provider 调用/真实设备接线分别仍属 D10/D08/D11，本轮只提供前置机制与持久账本。
