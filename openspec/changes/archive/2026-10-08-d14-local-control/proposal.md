# Proposal

## Why
D14、V3.5 §12/§22 要求操作员看到真实状态并可静音/接管。桌面仍为未连接工程页，本轮建立可测试的显式 loopback 控制面，先闭合读取与撤权操作。

## What Changes
- 控制 OpenAPI 补充设备/序号、会话读取及实际支持操作标记，生成服务校验 schema。
- nai-control-api 提供受 Bearer 鉴权/作用域/权限控制的 status、session read、mute、takeover。
- P0 先调用独立 EmergencyHandle，再尝试 Session 状态更新；ACK 仅表示控制状态执行，不冒充声卡播放/静音 ACK。
- 桌面通过操作员明确输入的 loopback 地址及临时 token 连接，显示真实快照与操作结果；无运行时保持未连接。

## Capabilities
### New Capabilities
- `local-control-surface`: 显式本机控制服务与桌面状态/撤权闭环。
### Modified Capabilities
无。沿用 Session/MediaGate 机制，未实现端点不标成功。

## Impact
新增 shared 层 nai-control-api，依赖 kernel/media-core/contracts；复用已锁定 hyper/hyper-util/http-body-util/tokio/bytes/sha2/jsonschema，不引入新常驻依赖。服务仅显式启动，不改 Local protected IPC 默认；此 HTTP 入口为本机工程运行，非手机/生产 HTTPS 部署。
