# D14 首轮：本机控制与桌面状态/撤权闭环

- 日期：2026-10-08；环境：Linux/WSL，项目锁定 Rust/Node/pnpm/protoc/libprotobuf。
- 基线：V3.5 §12/§22 与 D14；OpenSpec：`d14-local-control`。
- 结论：显式 loopback 控制 HTTP 服务、桌面客户端与真实 Session 状态/静音/接管闭环已实现实测。正式桌面原生包及 D14 完整预检/审批尚未完成。

## 实现

- 先修改控制 OpenAPI：SessionCommand 必需 device_id/seq、GET session view、部分实现标记与错误响应；生成 TS 和 Rust 校验 schema。生成器扁平化 allOf 后闭合字段，消费者未手写跨语言 DTO。
- shared 层新增 nai-control-api：Bearer 哈希校验、绑定设备/会话权限、令牌期限/撤销、真实状态读取、mute/takeover、CAS/seq/TTL 与有界幂等。
- 独立 EmergencyHandle 先撤销，状态锁使用 try_lock；锁忙/CAS/容量失败仍保持撤权，ACK 只表明控制状态执行。过期 TTL、无效身份/结构/scope 不触发信号。
- 服务只监听 loopback，连接/任务记录最多 16、总期限 5 秒、body 4 KiB、header 缓冲 8 KiB/32 项；CORS 仅本机 5173，query 禁止携带任何参数。
- 桌面从明确输入的本机地址/临时 token 读取状态、发送 mute/takeover；uint64 字符串保精度、未知状态/无效 ACK 拒绝、客户端响应上限与期限、失败不自动重发，断连不 Stop。
- 示例进程创建真实空 Session/MediaGate，不装入 mock Provider/模型/平台。实际进程冒烟使用真实桌面客户端和临时内存测试 token，经动态端口调用并关闭自有进程。

无新增云或自动启动常驻服务；新直接依赖复用仓库已锁定 Hyper/Tokio/bytes/http-body-util。正式 Local protected IPC 默认不变，设计决定见 ADR 0014。

## 实测

| 命令 | 结果 |
|---|---|
| `cargo test -p nai-control-api --offline` | Passed：8 项，包括实际 HTTP 与媒体信号/状态锁隔离 |
| `cargo test -p nai-contracts --offline` | Passed：新增生成控制 schema 继承/闭合字段测试 |
| `pnpm --filter @nai/desktop test` | Passed：5 项客户端测试 |
| `pnpm test:control-smoke` | Passed：真实示例进程、真实桌面 ControlClient、status→mute→takeover，epoch 0→1→2、seq 0→1→2 |
| `cargo xtask test --suite contracts` | Passed：生成无漂移、Node 10 项 schema/API 测试、C++ 实际编译及十类消息双向互通 |
| `cargo xtask check --profile portable` | Passed：12 包依赖边界、fmt、workspace Clippy、222 项 Rust 测试 |
| `pnpm install --frozen-lockfile --offline` | Passed |
| `pnpm typecheck` / `pnpm build` | Passed |
| OpenSpec strict validate（实施前） | Passed |
| 实施后 strict validate / archive / spec:validate / archived validate | Passed：16 个主规格、14 项归档 |

新增 9 项 Rust 测试（8 control + 1 schema），5 项客户端测试，以及实际进程冒烟。
覆盖鉴权/过期/撤销/权限、会话/设备隔离、精确 u64/seq/CAS、同 ID 重试/改内容拒绝、队列记录容量、忙锁时先撤权、未知字段/超大体、TTL、CORS/query 限制、未实现端点、外部绑定拒绝及关闭释放监听。
客户端测试中的 fetch fixture 仅用于 tests；实际冒烟使用真实服务，未据此宣称真实声卡输出。

## 发现与修复

- OpenAPI flow mapping 中含逗号的描述需加引号，已修复后生成通过。
- 旧 validation.mjs 强制全部 API 未实现，更新为严格验证五个已实测操作的局部标记，禁止扩大成功声明。
- 工作区 TS typecheck 曾读到旧 dist 类型；@nai/contracts 的 types export 指向源，避免消费者依赖陈旧构建类型。
- 忙锁测试初版跨 await 持 guard 被 Clippy 拒绝；改为独立线程持锁验证实际 HTTP 撤权。

## 状态与认证边界

- **已实现/实测**：本机控制工程子集与桌面客户端闭环，受限连接与权限/幂等边界。
- **未开始**：完整 preflight/start/stop/mode、候选审批/证据/配置/插件能力视图、生产凭据/配对持久化、WebSocket、正式 Tauri/受保护 IPC。
- **未实测**：浏览器 DOM/人工交互、设备静音 P95、真实声卡/媒体宿主与长播、Windows 原生安装、生产部署认证。
- **Blocked**：Windows 设备/伴侣/SDK/GPU 外部证据仍缺；本轮不授予 T/E/U 或 G0–G3。
- **NotApplicable**：本轮显式 loopback 工程入口不启用手机/远程部署，未声明该能力已实现。

页面 ACK 明确是控制状态，不把内核 atomic mute 或命令 executed 当作硬件 ACK。界面关闭不隐式终止会话；该示例不证明生产媒体进程独立寿命。
指南：[docs/control.md](../control.md)；ADR：[0014](../adr/0014-explicit-local-control-surface.md)。

本轮已归档为 `openspec/changes/archive/2026-10-08-d14-local-control/`，6/6 任务完成，4 条需求合并为 `local-control-surface`。D14 整体未完成，归档仅覆盖本机控制工程子集。
