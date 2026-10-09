# Proposal

## Why
D13、V3.5 §10.2 要求平台失联、旧房间/旧连接事件与权限撤销不能继续驱动真实互动。现有 PlatformEvent 没有 session epoch/连接代次，尚无官方事件作用域与权限入口。

## What Changes
- Protobuf 为 PlatformEvent 增加 optional generation_epoch、connection_generation；权限细分键约定 `platform.events@1/<EventKind protobuf 名称>`。
- 内核增加平台连接准入与官方事件验证；真实房间来源与身份检查由可信 composition root 提供。
- 断线/撤销清除权限；重连要求重新认证、刷新权限版本，不授予自动播报模式。
- 接线测试验证准入后进入已有有界 EventIngress，旧事件在消费前复检被拒绝。

## Capabilities
### New Capabilities
- `platform-event-admission`: 官方事件的连接、作用域、权限及失联拒绝边界。
### Modified Capabilities
无。沿用既有宿主握手、session 和调度能力，不替代实际 SDK。

## Impact
schemas/domain/platform.proto 与生成产物、nai-kernel；无新增依赖或常驻服务。
官方 SDK、账户用途批准、平台宿主启动和网络网关尚未实现；真实集成认证保留 Blocked。
