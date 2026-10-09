# D06 会话、租约和取消

## Why
V3.5 第 3.5、21 章要求会话与媒体独立失效，当前仅有绑定与 IPC。D06 补齐可验证的 portable 机制，为 D11 实际输出提供门控。

## What Changes
- kernel 会话生命周期、epoch、任务取消、命令幂等与安全恢复。
- media-core 本地静音、短租约、播放复检和实际完成 ACK 约束。
- storage 保存会话撤销检查点并使旧审批失效。
- session suite、故障测试、文档与状态记录。

## Capabilities
### New Capabilities
- `session-leases-and-cancellation`: 会话隔离、任务取消、播放租约和独立静音。

## Impact
涉及 kernel/storage、新增 media-core、xtask/CI。复用现有 Protobuf，不更改线协议。无新增常驻服务、第三方依赖或 API 成本。Windows 设备与真实音轨验收留待 D11，未实现的接线保持未开始。
