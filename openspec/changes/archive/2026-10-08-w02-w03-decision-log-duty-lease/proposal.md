# Proposal

## Why

V3.6 §5.8 要求统一的只追加哈希链决策日志（INV-19，验收 R07，Slice-0 退出项），当前仓库没有 decision_log 表与链式校验；§21.4 要求自动模式由值守租约门控（INV-08），当前内核只有会话/媒体租约，没有值守租约实现（grep 零匹配）。ADR 0015 已裁决：日志缓冲满时丢弃遥测段、关键提交先持久化；值守租约失效后 600ms 内撤销输出授权。本轮落地这两个 Slice-0 前置机制。

## What Changes

- 新增迁移 `migrations/sqlite/0005-decision-log.sql`：`decision_log` 表（只追加触发器、prev_hash/entry_hash 哈希链、criticality 分级），按 workspace 独立链。
- nai-storage 新增决策日志 API：`append_decision`（critical 直接持久化且失败即报错；telemetry 失败进有界缓冲、满则丢弃最旧并告警）与 `verify_decision_chain`（逐条重算校验）。
- nai-kernel 新增值守租约：操作员显式确认授予/续期，心跳不续期；到期即停新自动回复，600ms 后撤销输出授权（epoch 递增）；切 AutoSupervised 必须持有效值守租约；重连不恢复。
- 决策日志的记录链含 playback_ack 类别，但本轮无生产端（无 output-keeper/真实播放 ACK），该段保持 Draft，不冒充完整 R07。
- 明确不做：桩 Provider 回放一致性（W17）、决策日志与运行时的接线（composition root 尚不存在）、故障域强杀进程测试（无 keeper/worker 进程可杀，留 D17）。

## Capabilities

### New Capabilities

- `decision-log-integrity`: 只追加哈希链决策日志的存储、分级缓冲与链校验。
- `duty-lease`: 自动模式值守租约的授予、到期处置与恢复复核语义。

### Modified Capabilities

无。不改既有契约字段；`Session::change_mode` 增加值守前置检查属于内核行为收紧，不改 proto。

## Impact

- 新增 1 个迁移文件、nai-storage 决策日志模块与测试、nai-kernel duty 模块与测试；无新增 crate、进程或常驻依赖。
- `tests/registry.yaml`：R07 由 pending 更新为 implemented（注明回放/ACK 缺口）；S01 证据补充值守租约实测。
- docs/storage.md、docs/session.md（或新增小节）、docs/development-plan.md、AGENTS.md 同步。
