# Design

## Context

见 proposal.md。ControlApi 已持有真实 Session/Store、权限与命令账本，但入口仅接受 HTTP Incoming。
现有壳不持有业务状态；CSP 禁止 loopback。继承管道可由 safe std API 跨平台建立，无公开寻址端点。

## Goals / Non-Goals

接通真实状态、手工事件、审批与预览控制，不复制业务规则。操作者显式指定已配置的数据根/身份/profile/场景。
不自动创建用户授权、不增加设备输出、不把继承能力认证声称为命名管道对端身份认证。

## Decisions

1. Protobuf LocalControlFrame 统一版本/序号/kind/请求身份/TTL，业务 JSON 字节继续按既有 OpenAPI 校验。4 字节大端长度，帧上限 1MiB；所有队列有界。
2. 随机会话凭据只在壳 Rust 内存与继承 stdin 引导中出现，不进命令行、环境、磁盘、WebView、日志。该通道的身份来自可信 spawn 与私有句柄持有，不开放第三方连接；不声称命名管道 ACL/peer PID 已认证。
3. ControlApi 泛化 Body 类型供 Full<Bytes> 与 Incoming 共用；engine 原生模式没有 TcpListener，保留权限、会话 epoch、幂等与控制 ACK 语义。
4. 客户端有界串行 actor、独立子进程句柄与读线程：请求 4 秒期限，超时终止连接/进程而不复发。后台每秒心跳，engine 5 秒未收到完整帧即先 P0 静音、撤权、critical 日志后退出；partial 读与写阻塞不得卡住该故障处置。
5. 关闭窗口隐藏到托盘；托盘退出先停机再退壳，有界回收，不自动重启、不恢复自动权限。UI 不持有 engine 密钥或进程控制接口。无启动参数时仍显示未配置状态。
6. nai-control-ipc 只负责传输与子进程所有权，依赖 contracts，不认识 kernel/Store。engine 做装配，shell 只做宿主。新增 ADR 0130。

## Risks / Trade-offs

- 同用户调试器/恶意宿主不在继承能力边界内 → 如实记录，不把它称作完整 OS 隔离认证。
- 原生构建不在 full → 点名独立 clippy/build/WebView 冒烟；传输与 engine 进程测试在根 workspace。
- 已开始但 ACK 丢失的业务请求结果不确定 → 不重试；终止通道并要求刷新，保持持久化幂等账本。
- 壳明确退出会停机、关窗只隐藏；壳崩溃由心跳租约静音退出。设备仍未接线，不制造硬件 ACK。
