# ADR 0008：共享预算与有界调度

- 日期：2026-10-07
- 状态：Accepted
- 基线：V3.5 第 22 章；D07

## 决策

kernel/scheduler 只提供每会话有界队列、期限/epoch、公平和债务，付费准入回调由可信 root 提供。
P0 复用 D06 专用媒体路径，不进入调度普通队列或等待预算事务。
关键状态按来源/种类有独立槽位；满载先淘汰低价值点赞，统计丢失。
礼物只有来源/观众/连击标识/文本/数量语义一致时合并：delta 加总（检查溢出），cumulative 取最大值，unknown 不推算。
平台乱序/连击重置的具体样本归 D13；本轮不宣称所有事件有序或零丢失。

P1–P5 与老化阈值共用内核；P1/P2 不受观众冷却限制，普通观众使用有界冷却记录与滚动份额。
话题合并保留旧候选并取更早 TTL，不启动额外生成。债务包括已选中但尚未确认结束的任务，
实际 TTS 时长通过 revise_speech 如实修正，超过限额阻止新增，而非隐藏真实占用。

新增 nai-budget-core 为明确 shared 职责，normal 依赖仅 contracts，kernel 不依赖该库。
root 使用它或 storage owner 完成全局/session/provider 三层金额、用量、并发的原子预留。
整数 u64 最小货币单位与受控整数计量，不使用浮点或自动汇率转换。requests 是尝试准入计数，
实际 token usage 不得抹掉已派发的请求次数。Provider 重试作为独立 attempt 请求预留。

账本是累积预算窗口，不自动按日期清零。预留后派发前可取消释放；派发后取消保持估算和并发；
确认工作结束才释放并发，未知账单保留估算并标 NeedsReconciliation。实际超估算如实记账，
不因达到配额拒绝保存真实消耗；之后拒绝新增。重复账单 ID 幂等，新修订须 revision CAS。

持久派发 claim 必须先提交再调用 Provider；只有 acquired=true 能开始一次请求。
ACK 丢失后重试返回 acquired=false，进程故障后的未知派发不自动重发，不宣称网络 exactly-once。
取消/故障前已真实发生的计费属于待对账，不自动退款或清零。

## 契约与存储

先增量 BudgetCharge/UsageLedgerEntry/BillingState 与可选 runtime/provider 配额配置，再生成三语言契约。
原 schema_version=1 配置仍兼容；缺全局/provider 限额不能被推测为无限预算。
SQLite usage_ledger 不新增列/迁移；estimate_json 使用显式 nai.v1.UsageLedgerEntry 判别与 Protobuf hex
保存完整规范化状态，actual_usage_json 保存可读实际金额/单位（u64 为字符串）。索引/状态与编码对象复检一致。
不兼容旧账本拒绝而非视为零；最大记录数有界，达到上限不删除审计或自动释放未明预留。
批量恢复一次验证总量，避免逐行全量扫描；读取途中检查 deadline/cancellation。

## 证据与范围

portable 106 项 Rust、10 项 Node 契约校验、9 种消息的两条 Rust/TS/C++ 互通路径通过。
kernel/budget GNU Windows 代码与测试编译通过（非原生执行）；storage 仍受 D06 的 MinGW 工具缺失限制。
无第三方新版本、常驻依赖、云资源、模型或付费 API。本轮交付调度/计费机制，
真实 Provider 流/账单归 D08，节目 checkpoint 接线归 D10，实际播放债务回馈归 D11。
