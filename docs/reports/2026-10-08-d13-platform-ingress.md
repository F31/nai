# D13 平台事件入口实测报告

- 日期：2026-10-08；环境：Linux/WSL，项目锁定 Rust/Node/pnpm/protoc/libprotobuf。
- 基线：V3.5 §10.2 与 D13；OpenSpec：`d13-platform-ingress`。
- 结论：可移植平台连接/官方事件准入与调度接线测试已实现；真实 SDK 与网络网关仍未实现。

## 实现

1. 先修改 `schemas/domain/platform.proto`：PlatformEvent 新增 optional generation_epoch=20、connection_generation=21；由 xtask 生成 Rust/TS/C++，未手改生成产物。
2. `nai-kernel::platform::PlatformLink`：固定 session/epoch/platform/app_scope，可信 root 认证与真实房间来源、完整有效权限快照准入；单调权限期限、连接代次递增、断线清除权限、重连要求新 revision。
3. 官方事件校验来源/房间/authority/epoch/代次、逐类权限 Granted、事件版本/长度/TTL/时钟偏差、CallContext deadline/cancellation。
4. 调度去重和合并 scope 纳入 epoch/连接代次，避免旧连接吞掉新事件；消费前重新验证连接状态。
5. Rust/TS/C++ 互通增加平台事件 fixture，大整数 epoch/代次保持精确，旧事件可解码但缺字段不能通过官方入口。

无新增依赖、服务或平台 SDK。没有自动模式或输出授权入口。

## 实测结果

| 检查 | 结果 |
|---|---|
| `cargo test -p nai-kernel --offline` | Passed：42 lib + 2 evaluate + 7 platform + 8 scheduler，共 59 项 |
| `cargo xtask test --suite contracts` | Passed：Rust、10 项 Node schema/API 测试、实际 C++ 编译与双向互通（10 类消息） |
| `cargo xtask schema --check` | Passed：生成无漂移；contracts suite 中也复检 |
| `cargo xtask check --profile portable` | Passed：11 包依赖边界、fmt、workspace Clippy、213 项 Rust 测试 |
| `pnpm typecheck` / `pnpm build` | Passed |
| OpenSpec strict validate（实施前） | Passed |
| 实施后 strict validate / archive / spec:validate / archived validate | Passed：15 个主规格、13 项归档 |

本轮新增 8 项 Rust 测试：7 项平台状态机/事件/队列接线测试，1 项 Protobuf epoch 存在性与大整数代次测试。
覆盖断线重连、重复准入失败不覆盖有效连接、旧/缺 epoch、旧房间/代次/来源、权限缺失/Denied/Unknown、未知枚举、时间偏差/TTL、超限、权限到期、调用取消与过期、跨连接去重及排队消费复检。
实现过程中纠正契约错误码和生成 map 的实际类型，并简化测试闭包数组以通过 Clippy；最终检查全部通过。

## 真实状态与边界

- **已实现/实测**：平台事件可移植准入逻辑、生成契约与演练测试接线。
- **未开始**：官方 SDK 插件、生产 composition root 平台宿主接线、真实启动/退出/端口冲突处理、网关服务与无可靠平台消息 ID 的降级策略。
- **Blocked**：真实平台账户/用途批准、官方权限及 Windows 直播伴侣/SDK 环境证据。
- **未执行**：真实平台 SDK 认证、设备/直播输出、完整 T/E/U。

测试身份、平台、房间、权限都是明确测试 fixture，不冒充真实平台授权。`authenticated` 必须是可信 root 的独立判断，不能直接映射客户端字段；root 须保证权限来源与新鲜度。
连接恢复不授予自动播报；断线须走可信 disconnect 控制路径，生产媒体静音仍由独立租约与 Session/MediaGate 协调，逻辑成功不表示硬件 ACK。
详细调用规则见 [平台指南](../platform.md)。

本轮已归档为 `openspec/changes/archive/2026-10-08-d13-platform-ingress/`，5/5 任务完成，4 条需求合并为 `platform-event-admission`。归档的是平台事件准入范围，D13 SDK/网关整体仍未完成。
