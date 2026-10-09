# platform-event-admission Specification

## Purpose
为平台宿主提供独立可验证的官方事件准入边界，确保事件绑定真实来源的房间、当前会话 epoch 和当前连接代次，失联或权限撤销后拒绝旧数据；保持 SDK 认证、自动播报授权与媒体静音的独立责任，不把可移植逻辑视为真实平台接入。

## Requirements

### Requirement: Trusted scoped connection admission
官方连接 SHALL 需要可信宿主身份判断、验证过的房间来源及完整有效权限快照，固定 session/platform/app_scope/epoch。缺失、过大、不兼容或未认证输入 MUST 拒绝；准入失败不得修改有效连接。

#### Scenario: Missing room or authentication
- **WHEN** 身份判断失败、room_id/来源缺失或权限快照过期
- **THEN** 不建立连接或授予事件消费权限

### Requirement: Explicit reconnect without authority inheritance
断线/撤销 SHALL 立即清除本地事件权限；重新准入 SHALL 使用新的权限 revision 与递增连接代次。连接成功 MUST 不授予自动播报或媒体输出权限。

#### Scenario: Reconnect with cached permission
- **WHEN** 断线后尝试复用旧权限 revision 或旧连接事件
- **THEN** 拒绝；新快照重新准入后仅接受新代次事件

### Requirement: Current official event authorization
官方事件 SHALL 精确匹配 session、epoch、连接代次、platform、app_scope、房间与来源、权限 authority。事件类型权限键 `platform.events@1/<EventKind protobuf 名称>` MUST 明确 Granted；未知类型/状态或未声明权限拒绝。手工/回放不得伪装官方来源。

#### Scenario: Unsupported or stale event
- **WHEN** 事件缺 epoch、旧连接/房间、非官方来源或 FOLLOW 未获授权
- **THEN** 拒绝事件，不合成缺失能力

### Requirement: Bounded temporal validation and consumption recheck
事件与权限输入 SHALL 有长度和数量上限，事件 TTL、版本及时间偏差须验证；期限与取消 SHALL 在调用入口/出口检查，权限用本地单调期限复检。排队事件 MUST 在消费前再次通过当前连接检查。

#### Scenario: Disconnect after enqueue
- **WHEN** 有效官方事件进入有界队列后断线或权限被撤销
- **THEN** 消费复检拒绝旧事件，不凭缓存继续真实互动
