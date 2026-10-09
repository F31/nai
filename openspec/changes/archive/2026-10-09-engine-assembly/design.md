# Design: engine-assembly

## 1. 共享会话（消除 epoch 分叉）

问题：`ControlApi` 与 `Composition` 各自持有 `Session` 实例。HTTP 静音/接管只推进控制面那份的
epoch，监督侧租约与决策日志用另一份——撤权后 keeper 租约 epoch 不失效，违反 V3.6 复检不变量。

决策：单一 `Arc<Mutex<Session>>`。

- `nai-control-api`：`State.session` 改为 `Arc<Mutex<Session>>`；新增
  `ControlApi::with_shared_session(shared, emergency, access, approvals)`；既有 `new`/`with_approvals`
  委托之（自行包裹）。锁一律 `try_lock`，忙时返回既有 409 `Conflict`（P0 静音走 `EmergencyHandle`，
  不经此锁）。
- `nai-runtime`：`Composition.session` 字段改为 `Arc<Mutex<Session>>`，新增
  `shared_session()` getter；`control`/`confirm_duty`/`enforce_duty`/`observe` 内部 `try_lock`，
  锁失败返回 `Conflict` 且**不影响已执行的静音**。`prepare` 签名不变（内部自建共享会话）。

## 2. apps/engine（layer `app`）

显式参数（无隐式启动）：

```
nai-engine --data-root <dir> --workspace <id> --actor <id> --profile <name> [--provision <profile.json>]
           --token-file <path> [--control-port <u16>] [--halt-file <path>]   # 0 = 临时端口
           [--keeper <path> --render-worker <path>]
           [--media-drill --pcm <path> --avatar <manifest> --avatar-config <config>
            --asset <id=path>... --license <id>... [--slot-bytes <n>] [--time-base-den <n>]]
```

- `--provision <profile.json>`：可信 owner 一次性配置——建工作区、授予运行时权限、写入并校验运行时
  profile，随后退出 0（不启动服务）。装配路径由此变成「先 provision，再启动」，运行时不再需要
  bootstrap 分支。
- token 从 `0600` 文件读取（32–256 字符，规则复用 `Access::new`）；权限全集
  （runtime.read/session.read/media.mute/session.takeover/approval.read/approval.write）；TTL 1h。
- 优雅停机由 `--halt-file <path>` 显式触发并有界轮询：`tokio::signal` 需要
  `signal-hook-registry`，该 crate 不在离线缓存内，而 crate 内 `unsafe` 被禁用，因此本轮不用信号；
  被强杀进程不计为优雅停机（后续接 protected IPC/信号时替换）。
- 启动顺序：`Composition::prepare` → `ControlApi::with_shared_session`（approvals 来自装配）→
  绑定 `127.0.0.1:port`（非 loopback 由 `serve` 拒绝）→ stdout 打印一行
  `nai-engine control: http://127.0.0.1:<port>/api/v1`（机器可解析，冒烟脚本依赖此行）。
- 监督循环（独立线程，`--keeper` 与 `--render-worker` 成对给出）：spawn keeper
  （`--watchdog-drill`，floor = 现场会话 epoch）→ 以 `floor+1` 签发 Rehearsal 租约并 arm →
  每 tick 先续租、再 `observe()`。**控制面撤权后停止续租**（记 telemetry
  `renewal_halted_after_revoke`），不重新签发：重新授予输出权限需要显式操作者决定，绝不自动发生。
  续租失败同样不静默退出，而由 `observe()` 走静音→撤权→critical 日志→待机。故障后 **不自动重启** 输出。
- 租约签发规则：`rehearsal_lease` 拒绝低于现场会话 epoch 的请求（`STALE_EPOCH`），epoch 0 仍需显式
  非零（keeper 要求租约 epoch 严格大于其 floor）。
- `--media-drill`（Linux）：引擎建池（`/dev/shm` 私有目录 tmpfs-file 后端）→ spawn render-worker
  （`--ready` 预热后 reserve + 原子写 ticket）→ 读 `write_started`/`committed` → owner commit →
  `FrameConsumer` 读回 owned 像素并在决策日志记 telemetry（`buffer_ref`/SHA256/字节数）。
  - **偏差（如实）**：broker 的 UDS 授权未接线（render-worker 无 broker 客户端），keeper 本轮只承担
    watchdog 监督、未接帧投递路径；二者都在各自既有测试与 media-core 内覆盖。任一环节失败显式报错，
    不静默降级。
- 停机：halt 文件 → 撤 token（`Access::revoke`）→ P0 静音 + 撤权 + critical `cancellation` 日志 →
  监督线程 stop（有界回收子进程）→ HTTP watch 停机 → 进程退出码 0。
- 新增只读 `nai_storage::Store::decision_entries`（链序、限流），供引擎测试与复盘读取证据；不改变
  追加/哈希链语义。

## 3. 冒烟（scripts/engine-smoke.mjs）

仿 `control-smoke.mjs`：临时数据根 + 内存生成 token 写临时文件（0600），spawn 真实
`target/debug/nai-engine`，解析 stdout 地址行，用桌面 `ControlClient` 实测：status（lifecycle/media_state/
epoch=0）→ 无 token 401 → mute 后 epoch=1 → takeover/approval 列表 → SIGTERM 优雅退出。
`package.json` 增加 `test:engine-smoke`（先 `cargo build -p nai-engine --offline`）。

## 4. 边界（写入 ADR 0020）

engine 是**工程装配可执行**，不是生产形态：无 protected IPC、连接对端未认证（ADR 0018）、
无 PlaybackAck 生产端、无真实设备；`--media-drill` 仍是显式演练。验收编号状态不变。
