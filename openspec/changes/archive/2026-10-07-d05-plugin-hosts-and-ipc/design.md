# Design

## Context

D04 已提供 Registry/Admission/权限与冻结绑定，但没有字节传输和进程边界。V3.5 第 20 章要求帧边界先限长、握手/ACL 先于能力授权、仅从验签目录加载并拒绝逃逸。当前 Linux/WSL 无 Windows 命名管道或原生沙箱，原生子进程隔离未认证。

## Goals / Non-Goals

**Goals:** 可移植有界 framing、握手/ACL、宿主生命周期抽象、加载/路径隔离与诚实的原生沙箱边界。

**Non-Goals:** 真实 Named Pipe/AppContainer/Job Object 集成、异步并发调度、会话/租约/取消业务执行、远程/WASM 宿主实现。

## Decisions

1. 帧 = 4 字节小端长度 + Protobuf body，默认上限 64KiB。任何 read_exact 前先校验长度；读操作携带 deadline/cancellation 检查点。这是可移植字节流约束，不绑定具体 IPC 传输。
2. 首个帧必须是 Handshake；成功应答 HandshakeAck 含协商版本与 nonce 回显。握手后所有帧都是 Envelope，复用 D02 的 validate_envelope/negotiate，不新增协议消息。
3. `nai-plugin-hosts`（layer=hosts）提供 CapabilityHost、AccessPolicy、IdentityValidator、加载守卫与 NativeBoundary；`nai-plugin-sdk` 提供 Plugin trait、PluginContext、CapabilityClient 与 frame 编解码。hosts 只依赖 contracts/sdk，kernel 不依赖 hosts/sdk，满足 AGENTS 依赖边界。
4. AccessPolicy 以 peer/session/capability/operation 为键，先检查再分派；ACL 判定属于可信装配，不读取插件自报身份。OsPeer（pid/uid/gid）仅在 Unix 域套接字通过 peer_cred 提取；TcpStream 提供 None，IdentityValidator 必须据此决定是否接受，不把 None 当成已认证身份。
5. Plugin trait 使用 object-safe 同步签名（describe/validate_config/start/health/suspend/resume/stop/invoke），invoke 通过回调 sink 输出 CapabilityResult；流/异步升级留给 D06 在合适 executor 上做，不强行 async trait 使其失去 object safety。
6. PluginContext 提供期限、取消、时钟、预算、获准配置与 credential refs；host 从独立 owner 注入，不信任模型字段。context 是插件访问受限资源的唯一入口，不直接暴露原生凭据句柄。
7. 取消以 cancellation_id 记录：先记录 CancelRequest 后再分派匹配请求即拒绝 CANCELLED；同步分派不承诺抢占进行中计算，真正异步取消属 D06。EOF 不是正常完成，CapabilityResult 必须带明确 StreamEnd。
8. 加载守卫 `verify_within_root` 规范化并拒绝 `..`、绝对逃逸与符号链接/重解析点逃逸。NativeBoundary 在无认证 OS 沙箱证据时返回显式 CapabilityNotReady/Unavailable；子进程 env 清理移除密钥类变量，只保留受控引用。
9. 测试使用 UnixStream 对（unix）与 TcpStream 回环（可移植）验证真实字节流；`ChunkedReader` 构造 partial/coalesced/超长/EOF 场景；Windows 原生 peer 身份提取与命名管道不在 Linux 验证，作为 Windows 验收项 Blocked。

## Risks / Trade-offs

- 同步 invoke 无法强抢占长任务 → 用 cancellation_id 拒绝后续与检查点，预留 D06 异步取消；文档不宣称抢占已实现。
- TcpStream 无 OS peer 凭据 → IdentityValidator 对 None 依策略拒绝；真实本地总线用 Unix/Named Pipe 提供身份。
- NativeBoundary 始终 Unavailable 会阻塞原生子进程 → 这是诚实默认；认证沙箱（AppContainer/受限身份）在 D13/D18 验证后按显式证据放行。
- 进程内无故障隔离 → 文档与报告明确 inproc 不隔离崩溃/OOM；宿主边界按声明执行，Windows/GPU/设备验收单列。

## Migration Plan

无数据迁移。新增 workspace member `crates/nai-plugin-hosts`，CLI 增加 `cargo xtask test --suite hosts`；契约不变，无消费者破坏。

## Verification

`cargo xtask test --suite hosts`（SDK + hosts）、portable、生成无漂移及 contracts suite、前端、OpenSpec strict、Windows GNU 交叉编译（非原生认证）。
