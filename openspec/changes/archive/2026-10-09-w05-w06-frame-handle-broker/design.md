# Design

## 边界与不变量

- 本轮仍是 ADR 0017 的工程演练边界：Linux-only、显式启动、有复制、非 mmap/零拷贝，不授予设备、Ready 或播放 ACK。
- **句柄交付仍是 spawn 继承**：`std` 不提供 SCM_RIGHTS/额外 FD 传递，新增依赖（rustix/nix）未经审查，故本轮不引入。
  broker 的职责是**授权**：只有内核证据支持的 root 声明身份（见下节）才能取得该 pool 的 producer/consumer 授权，
  未授权连接不产生任何 ticket。SCM_RIGHTS/Windows 命名管道句柄传递明确未实现。
- 身份证据取自内核（`/proc` 的存在性、uid 与 fdinfo），不来自对端自报字段；连接对端本身不被认证（见下节残余缺口）。
- 单 owner 不变：pool owner 与 ticket 账本仍唯一在 root/broker 进程；keeper 只做消费者侧只读。

## broker（nai-media-core::frame_slots::broker，Linux-only）

- **身份证据的现实边界（实测）**：`UnixStream::peer_cred()`/`SO_PEERCRED` 在钉住的 rustc 1.96.0 上仍属不稳定
  feature（E0658，issue #42839），且本 workspace 禁止 unsafe、不引入新依赖。因此**连接对端不被认证**：
  UDS 连接只是活性触发，身份来自 `/proc` 内核证据——声明的 pid 必须存在、属 pool owner uid，并持有该 pool 文件
  （`mnt_id`+`ino` 匹配 `/proc/<pid>/fdinfo`）。这验证的是**能力持有**（谁拿到了 root 交付的 FD），
  不是 IPC 对端身份。该降级为 Blocked 项，写入报告与 ADR 0018，不得表述为完整 IPC 认证。
- `PeerRole { Producer, Consumer }`；`PeerIdentity { role, pid: i32, uid: u32 }` 由 root 在 spawn 后声明（root 知道自己启动的 pid）。
- `FrameBroker::bind(private_dir, spec, peers)`：
  - 复用 ADR 0017 的私有目录校验（`/dev/shm` 下、canonical、owner uid、`0600`）与 pool 创建（文件/ inode 派生新 pool identity）；
  - 在私有目录内绑定 UDS `<pool_id>.sock`，权限 `0600`，`Drop` 时 unlink；
  - `listener.set_nonblocking(true)`，accept 循环有期限与次数上限（`MAX_ACCEPT_ATTEMPTS = 8`），不无界阻塞。
- `poll_grants(deadline)`：accept → `peer_cred()` → 必须 `uid == owner_uid` 且 `pid` 等于某条声明身份的 pid；
  匹配则记录对应 role 的授权；不匹配、uid 不符、pid 为 None、超时或超出次数上限 → 关闭连接并返回明确错误
  （`PermissionDenied` / `DeadlineExceeded`），**不产生授权**。
- 控制操作全部经授权门控：`reserve/commit/producer_file` 要求 producer 已授权，`read_frame` 要求 consumer 已授权，
  否则 `PermissionDenied`（"peer identity not verified"）。`disconnect(role)` 撤销该 role 授权并按现有语义退休整池。
- 非 Linux：所有入口 `CapabilityNotReady`，不伪造后端。

## consumer（nai-media-core::frame_slots::consumer）

- 从现有 `SharedFramePool::read_frame` 中抽出**不依赖 owner 账本**的读路径：
  `read_committed_frame(file, spec, consumer, ticket, commit, budget) -> Vec<u8>`，校验 ticket、
  精确 RGBA8 长度、池文件长度、分块读、SHA256；`SharedFramePool::read_frame` 改为复用该函数（owner 账本检查保留在外层）。
- `FrameReader { file, spec }`：消费者侧只读视图（keeper 通过继承 FD 打开池文件）。
- `FrameConsumer { session_id, min_epoch, last_delivered_epoch }`：
  `submit(event, lease_valid, reader, now) -> ConsumeOutcome`，规则：
  1. `lease_valid == false` → `Standby(LeaseLost)`，**不读像素**；
  2. `write_started` → `Standby(NoFrame)`；`error` 变体 → `Standby(Retired)`（peer 报错视为数据面不可用）；
  3. `committed`：ticket 校验失败 → `Standby(IntegrityFailure)`；`epoch <= min_epoch` 或低于已投递 epoch → `Standby(StaleEpoch)`；
     读失败（长度/哈希/超时）→ `Standby(IntegrityFailure)` 或 `Timeout`；成功 → `Delivered(frame)` 并记录 epoch。
- 不产生 `PlaybackAck`，不授予输出授权。

## keeper（apps/output-keeper）

- `FramePath { reader, consumer, standby: Option<StandbyReason>, presenting: bool }`：帧消费与待机状态。
- `Watchdog` 集成：新增 `Action::SubmitFrame(FrameSlotEvent)`（provisional `OutputWatchdogCommand` 新 oneof 字段 8）：
  沿用 seq/期限/会话校验；`MediaGate` 租约不 live 时直接 `Standby(LeaseLost)` 且不读像素；成功投递置 `presenting=true`。
- `poll`：租约失效且此前 presenting → `presenting=false`、`standby=LeaseLost`、逻辑静音通道保持（keeper 无设备，只表达为状态）。
- `status`：新增 `presenting` 与 `standby_reason`（`OutputWatchdogStatus` 字段 13/14），`output_available` 仍恒 false。
- `--frame-drill` 启动模式：池文件经 `stderr`（fd 2）继承（`std` 仅支持标准流传递 FD），keeper 通过 `/proc/self/fd/2`
  安全重开为只读 `File`；命令/事件仍走既有 stdin 帧通道。未给 `--frame-drill` 时不启用任何帧路径。

## 契约变更（provisional）

- `OutputWatchdogCommand` 增 `FrameSlotEvent submit_frame = 8`；`OutputWatchdogStatus` 增 `bool presenting = 13`、`string standby_reason = 14`。
- 仅新增字段，不改既有字段号/类型；`cargo xtask schema` 重新生成 Rust/TS/C++，互通测试按生成物校验。

## 测试

- media-core broker（单元/进程内）：未授权时 reserve/commit/producer_file/read_frame 全部拒绝；role 不匹配、pid 不符、超出 accept 次数、期限耗尽均无授权；授权后 producer 写入 + consumer 读回成功。
- media-core broker（真实子进程）：drill 子进程连接 socket 并被接受（pid 匹配）→ 取得授权并完成写入；另一用例声明**错误 pid** → 子进程连接被拒且无授权、pool 无提交。
- media-core consumer：lease false 不读像素；write_started/error/stale epoch/篡改 SHA256/池长度变化 → 对应 Standby 原因；成功路径返回 owned 像素；`read_committed_frame` 与 `SharedFramePool::read_frame` 行为一致（回归既有测试）。
- keeper：`--frame-drill` 真子进程写入后 `SubmitFrame` → presenting；租约到期/撤销 → standby(LeaseLost) 且不再投递；池退休 → standby(Retired)；`output_available` 始终 false。

## 风险

- 身份校验只覆盖控制授权；持有继承 FD 的恶意 peer 仍可直接写像素（ADR 0017 已声明，supervisor 须 kill/wait）。
- 同 uid 的第三方进程无法通过 pid 声明伪装，但 root 声明的 pid 若被复用（pid 回绕）存在理论窗口：本轮以"授权在进程生命周期内一次性建立 + disconnect 撤销"收敛，残余风险记录在报告。
