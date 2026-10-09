# Proposal

## Why

D04 只完成内核登记/绑定，尚无实际传输和进程边界。V3.5 第20章要求有界 framing、握手/ACL 先于能力授权、以及仅从验签目录加载并拒绝路径逃逸。本轮落实这些传输与宿主约束，供后续会话/媒体使用真实进程。

## What Changes

- SDK 增加有界二进制 framing：4字节小端长度 + Protobuf body，先限长再分配。
- 实现握手协商、nonce/身份材料核对及协商版本；ACL 在能力授权前检查。
- 实现 `CapabilityHost` 会话循环、`CapabilityClient.invoke/stream/cancel` 与最小 `Plugin`/`PluginContext`。
- 实现宿主生命周期抽象：inproc 可编译审核模块；subprocess 无已验证原生沙箱时显式 Blocked/Unavailable，提供环境清理辅助。
- 实现路径守卫：规范化后在允许根内，拒绝穿越/符号链接/重解析点逃逸；仅从已验签目录加载。
- 新增 plugin-hosts crate、host/IPC suite、契约与文档，真实宿主/平台 SDK 认证保留 Blocked。

## Capabilities

### New Capabilities

- `plugin-hosts-and-ipc`：有界帧传输、握手/ACL、宿主生命周期与路径隔离边界。

### Modified Capabilities

无；复用既有 Envelope/Handshake/CapabilityInvocation/Result，不增删 v1 字段。

## Impact

影响 plugin-sdk、新增 `crates/nai-plugin-hosts`、xtask/CI 与文档。SDK/hosts 增加 std+contracts 依赖，无新增常驻服务或 API 调用。本轮 Ready/健康仍是宿主观测，不冒充设备、平台或最终授权。
