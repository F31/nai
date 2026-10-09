# Spec Delta

## Purpose

实现自动模式的值守租约（V3.6 §21.4、INV-08）：自动互动必须由操作员显式确认的值守租约门控，网络心跳不能续期，到期即停新自动回复并在 600ms 内撤销输出授权，重连不自动恢复（ADR 0015 勘误给出 600ms 初值）。

## ADDED Requirements

### Requirement: Explicitly confirmed duty lease

自动模式的值守租约 SHALL 只能经操作员显式确认授予或续期；心跳或连接事件 MUST NOT 延长租约到期时刻。租约到期判定基于单调时钟。

#### Scenario: Heartbeat does not renew
- **WHEN** 值守租约存续期间收到任意次数心跳
- **THEN** expires_at 保持不变，到期时刻不被延长

#### Scenario: Explicit confirmation renews
- **WHEN** 操作员在到期前显式确认
- **THEN** 租约按新期限续期；无确认则到期失效

### Requirement: Expiry halts auto and revokes output within bounded delay

值守租约到期时 SHALL 立即停止启动新的自动回复；自到期起 600ms 后 MUST 撤销输出授权（epoch 递增、媒体状态 Muted）。自动回复许可 MUST 同时要求会话 Running、模式 AutoSupervised 且值守有效。

#### Scenario: Expiry stops new auto replies immediately
- **WHEN** now 越过 expires_at
- **THEN** auto_reply_permitted 立即为 false，不再启动新自动回复

#### Scenario: Output authorization revoked after 600ms
- **WHEN** now 达到 expires_at + 600ms
- **THEN** 输出授权被撤销（generation_epoch 递增、媒体状态 Muted），旧 epoch 内容不可播放

### Requirement: AutoSupervised requires valid duty lease

切换行为模式到 AutoSupervised 时 MUST 持有效值守租约，否则拒绝。

#### Scenario: Mode change without duty rejected
- **WHEN** 无有效值守租约时请求切到 AutoSupervised
- **THEN** 切换被拒绝，模式保持不变

### Requirement: No automatic restoration

值守租约失效后，除操作员显式确认外 SHALL 不存在任何恢复自动权限的路径；平台或网络重连 MUST NOT 恢复自动权限。

#### Scenario: Reconnect does not restore
- **WHEN** 值守租约已失效且发生平台/网络重连
- **THEN** 自动权限保持关闭，直至操作员重新显式确认
