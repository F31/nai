# 平台事件准入（D13 可移植范围）

`nai-kernel::platform::PlatformLink` 固定 session、epoch、platform 和 app_scope。它消费生成的 PlatformCapabilities/PlatformEvent，不依赖官方 SDK、具体插件或网络。

## 可信宿主责任

composition root 必须完成本地身份/会话令牌校验，验证真实 room_id 来源或已批准映射，取得新权限快照，再调用 `admit(capabilities, verified_room_source, authenticated, context)`。
`authenticated` 是可信调用方结果，不是请求体字段；事件中的来源/authority 字符串本身不证明官方授权。
身份失败、空房间来源、connected=false、缺快照、重用上一权限 revision 或超限输入均拒绝。已连接时第二次准入返回 Conflict，不替换原连接。

- 作用域/ID/引用最长 256 字节；准入消息最多 16 KiB，权限键最多 32 项。
- 权限 valid_until 必须存在，剩余期限 >0 且 ≤120 秒；转换为本地单调期限，墙钟回退不能延长授权。
- 成功返回从 1 递增的连接代次；连接成功不授予 Session 自动模式或媒体租约。
- SDK 断线、权限撤销、房间切换须由可信控制路径立即 `disconnect()`，不等待普通事件队列。再连接需要重新认证、重新核对房间及新权限 revision；root 仍须确保快照新鲜，不仅改 revision 字符串。
- 进程恢复重建 PlatformLink 时必须使用 Session 已撤销的新 epoch，不能重置代次后继续旧会话。

## 官方事件边界

`validate(event, context)` 须在入队前及出队后、事件驱动工作开始前调用。
它验证 SOURCE_OFFICIAL、session/epoch/连接代次、platform/app_scope/room_id/room_source_ref/permission_authority_ref 精确一致。
权限键为 `platform.events@1/<EventKind protobuf 名称>`，例如 `platform.events@1/COMMENT`、`platform.events@1/FOLLOW`；只有 Granted 接受，缺失/Denied/Unknown/未知值拒绝。

事件最多 64 KiB、payload_version=1、ID 非空；剩余 TTL ≤60 秒、接收时间与当前 UTC 偏差 ≤5 秒、可选平台事件时间与接收时间偏差 ≤5 秒。
这些是本轮入口策略，不证明平台时钟可靠；具体 SDK 适配应显式处理时钟差异，不能伪造时间来通过验证。
CallContext 负责 deadline/cancellation；手工/回放来源不得走官方入口，继续由明确演练或人工路径处理。

现有 `scheduler::EventIngress` 负责有界队列/去重/合并，作用域包含 epoch 与连接代次，防止跨重连去重或礼物合并。
本轮没有生产 composition root 自动接线；`tests/platform.rs` 验证准入→入队→断线/重连→消费复检拒绝旧事件。
平台事件通知不是可信控制命令；真实 SDK 必须把断线/撤权先投到可信控制路径。后续异步工作和播放仍复检 Session epoch、审批、事实与媒体租约。

## 验证与剩余工作

```bash
cargo test -p nai-kernel --offline
cargo xtask test --suite contracts
cargo xtask check --profile portable
```

Protobuf 新增 PlatformEvent optional generation_epoch=20 和 connection_generation=21。旧报文可解码，但缺字段时官方准入拒绝；TS 保持 uint64 字符串。
Rust/TS/C++ 互通 fixture 增加平台事件的大整数 epoch/代次。

官方 SDK、真实账户用途准入、平台宿主启动/退出、端口冲突与重连网络处理、无可靠消息 ID 的降级去重、云网关服务尚未实现。
真实 Windows 伴侣/SDK 权限与设备认证未执行，详见 [本轮报告](reports/2026-10-08-d13-platform-ingress.md)。
