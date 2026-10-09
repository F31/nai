# ADR 0006：插件宿主、IPC 与隔离边界

- 日期：2026-10-07
- 状态：Accepted
- 对应：D05；V3.5 第 20 章

## 决策

1. IPC 帧为 4 字节小端长度 + Protobuf body，上限 64KiB，分配前先校验长度。
   读取函数复用 contracts 的期限/取消检查点，正确区分干净 EOF（帧边界，Ok(None)）
   与部分头/帧体 EOF（错误）。多帧合并与逐字节 partial read 均可还原。
2. 首个帧必须是 Handshake；服务端经 IdentityValidator 校验身份材料（可选 OS peer
   凭据）后协商版本交集并回显 nonce，客户端校验 nonce 回显。握手后所有帧为 Envelope，
   复用 `nai_contracts` 的 validate/negotiate，不新增协议消息。
3. AccessPolicy 以 session/principal/capability/operation 为粒度，分派前检查；
   空/未知一律拒绝，ACL 由可信 composition root 提供，不读取插件自报身份。
4. `nai-plugin-sdk` 提供 `Plugin` trait（object-safe、Send+Sync、同步）、
   `PluginContext`（期限/取消/预算/获准配置/credential refs，只由 host 注入）、
   `CapabilityClient`（握手后 invoke/cancel，带 chunk 上限）。异步流升级留给 D06 的执行工作。
5. `nai-plugin-hosts` 提供 `CapabilityHost`（serve 循环：握手、信封校验、协议主版本核对、
   取消前置拒绝、ACL 分派、结果 data/complete/error 明确结束）、`AccessPolicy`、
   `IdentityValidator`、`verify_within_root`（规范化后必须在根内，拒绝 `..`/绝对逃逸/
   符号链接/重解析点逃逸）与 `scrub_env`（移除 token/secret/password/credential/apikey）。
6. NativeBoundary：无经认证的 OS 沙箱（AppContainer/受限身份/Job Object）证据时，
   不可信原生子进程显式 `CAPABILITY_NOT_READY`（Unavailable），不以同用户进程冒充沙箱。
   认证证据与集成属于 D13/D18；`verify_subprocess_sandbox` 当前始终拒绝并允许测试断言。
7. OsPeer（pid/uid/gid）在 Unix 域套接字经 peer_cred 提取；TcpStream 无 OS 身份时传 None，
   IdentityValidator 必须显式决定是否接受，None 不视为已认证。
8. 取消以 request_id 记录：先收到 CancelRequest 后再收到同 request_id 调用，分派前拒绝
   CANCELLED。同步分派不抢占进行中计算，这是 D06 异步取消的职责；CapabilityResult
   必须带明确 StreamEnd，EOF 不是正常完成。

## 验证与范围

hosts/sdk 23 项测试、portable 42 项 Rust 测试、三语言契约互通、前端与 OpenSpec strict 通过；
Windows GNU 交叉编译通过（非原生执行）。Windows 命名管道 peer PID、真实 Named Pipe、
AppContainer/Job Object、受限身份与文件/网络/凭据限制仍未实测（Blocked）。
不可信 inproc 插件仍无原生故障隔离；D05 不声称已提供 P0 静音或宿主资源治理。
