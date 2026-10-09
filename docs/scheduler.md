# 有界调度与预算

```bash
cargo xtask test --suite scheduler
cargo xtask test --suite contracts
cargo xtask check --profile portable
```

## 调度

`nai_kernel::scheduler::EventIngress` 普通队列、关键状态槽位和去重表均有容量。
权限/连接/房间状态先于普通事件出队，房间结束优先；关键消息只保留每来源/种类最新收到的状态。
这不是平台时间序列证明，平台乱序需由真实 connector 校验。普通队列满时可淘汰已有点赞，
去重表满时拒绝新普通事件，不提前淘汰仍在 TTL 内的去重记录。`Metrics` 记录 overflow、dropped_likes、expired、duplicate、merged。
delta/cumulative 礼物仅按明确同来源/同观众/同 combo/同文本合并；unknown 保留原始语义，不累计数量或推算价值。

`Scheduler` 绑定 session/epoch；每个 Candidate 有 P1–P5、viewer/topic、估计 speech_ms、单调 deadline。
root 从场景配置构造 TTL（初值 QA 15s、欢迎 5s、感谢 20s，可调），不把网络收到时间当授权。
同观众/主题/优先级合并保留旧 ID 与更早截止；返回被合并 ID，由 root 维护事件关联。
`Policy` 明确容量、待播毫秒数、冷却、滚动窗口份额与 starvation 阈值；无随机选择或礼物价值永久优先。
普通任务等待足够久提升到 P2，P1 人工仍优先。冷却和份额约束可继续阻止任务；不承诺所有配置下无条件播报。
份额在窗口填满后执行且在存在其他可选观众时防垄断；P1/P2 人工/关键任务不使用观众冷却账。

`select` 在期限/债务/容量/公平检查后只对最终候选调用 admission 回调，拒绝准入时队列/债务不变。
回调不得直接调用付费 Provider，应只预留额度。返回 Selection 才能进入后续生成流程。
`revise_speech` 用实际时长修正债务，超限如实保留并阻止新增；`finish` 只在实际播放/取消结束确认后释放。
deadline 到期不会自动释放尚未结束的任务债务。D06 静音/epoch 撤销之后调用 scheduler.revoke 丢弃旧句。

## 全局共享账本

`nai_budget_core::Book` 是单 workspace 共享 owner，不为每 session 独立创建“全局余额”。
可信 root 必须显式提供全局、每 session、每 Provider 的 Ceiling；未知 scope/单位/币种拒绝。
金额为 u64 最小货币单位，用量为受控整数单位。如 requests/tokens/characters/speech_ms/tool_steps；
Provider/GPU 并发放在 concurrent map，不累计为货币。适配器负责把真实 usage 转成对应计量，不能任意把 TTS 转为 Token。
requests 是尝试准入计数，实际 usage 更新必须保留；额外 Provider attempt 使用新请求 ID 独立预留。

账本转换：

- reserve → Reserved：同 ID/同估算返回已有记录，改 session/epoch/估算冲突。
- Reserved cancel → Released：明确未派发，仅释放预留，不伪造零账单。
- Dispatch → Dispatched；cancel 只加取消标记，保留估算与忙资源。
- Finish(Some actual) → Settled：确认任务结束并记实际量。
- Finish(None) → NeedsReconciliation：确认结束释放并发，但费用/用量保持估算。
- Reconcile → Reconciled：实际量替换旧记账；相同账单 ID/内容幂等，不同内容冲突；后续修订须 revision CAS。

实际超估算仍入账，之后超额度的新请求拒绝。整数溢出/非法单位失败不部分更新。
Book.restore_all 验证完整恢复集合，保持未明额度；不会自动重发请求。

## 持久化与派发

root 授予实际 actor `budget.manage` 后使用 `Store::reserve_usage`/`budget_book`/`update_usage`。
SQLite 事务同时校验 scope/全局限额、写账本与审计。使用既有表，无列迁移；未知格式保持拒绝。
所有数值编码保留 u64 精度，新增消息已通过 Rust/TS/C++ 往返。

**Provider I/O 必须使用 `Store::dispatch_usage`，且仅 `DispatchClaim.acquired=true` 时启动。**
claim 在网络调用前持久提交，重复确认/重启后的同一请求返回 acquired=false 或终态拒绝。
`update_usage(Change::Dispatch)` 被拒绝，避免没有独占 claim 的派发。
预算预留、claim 与开始 I/O 之间都需要复检 D06 TaskToken/deadline；取消/超时不能启动新请求。
若 claim 已持久化而派发状态不明，保持估算等待对账，不自动重试或退款。
远端取消/忙 GPU 不能仅靠客户端标记就证明结束；Finish 的结束确认由真实 adapter/worker 提供。

配置新增字段可选以兼容旧配置，缺 global/provider ceiling 时新付费能力须显式未就绪，不能按无限预算处理。
总量按本工作区的累积窗口管理，不自动按天清零或删除已提交账单。限额降低/旧 scope 不完整/记录数超限
都保持明确拒绝；重启需保留历史 scope 的登记。人工预算调整与历史归档不能静默丢失审计。
当前每次写入批量恢复有界 ledger，适用于 Local owner；尚未增加云共享/Redis 等服务。

## 状态边界

此轮提供可测试的队列/准入/持久账本。真实 LLM/TTS/Embedding 流、计费、429/取消及账单核对属 D08；
节目内容/checkpoint 属 D10；实际设备播放时长/结束 ACK 的 root 接线属 D11。
Windows kernel/budget GNU 编译通过；原生安装、设备与存储 C 工具链验收仍未完成。
