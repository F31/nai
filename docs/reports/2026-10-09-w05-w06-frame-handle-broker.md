# W05/W06 帧句柄授权 broker 与 keeper 帧消费/待机

- 日期：2026-10-09
- 变更：`openspec/changes/w05-w06-frame-handle-broker/`（主规格 `media-frame-slot-ownership`、`output-watchdog-process` 增量）
- 对应 V3.6：W05（output-keeper）、W06（render-worker 数据面），承接 ADR 0017
- 验收影响：A01、R03 保持 implemented，notes 补本轮证据与仍缺项（不标 tested）
- 新增 ADR：`docs/adr/0018-frame-handle-broker-boundary.md`

## 目标与范围

补齐 ADR 0017 明确遗留的两项：帧句柄的**授权门控**（谁可以取得 producer/consumer 授权）与 keeper 的
**帧消费/待机切换**路径。

明确不做（保持未实现，不冒充）：SCM_RIGHTS/Windows 命名管道句柄传递（仍为 spawn 继承）、mmap/零拷贝、
独立 render-worker 生产装配、PlaybackAck 生产端、真实窗口/音频设备、性能与设备认证。

## 实现

### 契约（provisional，仅新增字段）

`schemas/domain/media.proto`：`OutputWatchdogCommand.submit_frame = 8`（携带 `FrameSlotEvent`）、
`OutputWatchdogStatus.presenting = 13`、`standby_reason = 14`。`cargo xtask schema` 重新生成
Rust/TS/C++ 与 JSON Schema/OpenAPI，`schema --check` 无漂移；未手改生成物。

### media-core

- `frame_slots::shared_ram::read_committed_frame(...)`：抽出不依赖 owner 账本的像素读路径
  （ticket/pool identity、精确池长度与 RGBA8 长度、分块读、SHA256），`SharedFramePool::read_frame` 复用，
  owner ledger 检查保留在外层；既有槽位测试全部保持通过。
- `frame_slots::broker`（Linux-only，非 Linux `CapabilityNotReady`）：`PeerRole`/`PeerIdentity`/`FrameBroker`，
  `bind/poll_grants/reserve/commit/producer_file/producer_spawn_handle/read_frame/disconnect`；
  UDS `<pool_id>.sock`（`0600`、Drop unlink、nonblocking、单次最多 8 次 accept 且有期限）；
  未授权时全部控制操作拒绝且不推进槽位。
- `frame_slots::consumer`：`FrameReader`、`FrameConsumer::submit`、`StandbyReason` 七值、
  `ConsumeOutcome::{Delivered,Standby}`；`IoBudget::starting_at` 让读预算锚定事件时刻，陈旧事件不能借用新鲜 IO 时间。

### keeper

- `FramePath` + `Watchdog::with_frames`、`Action::SubmitFrame`：沿用 seq/期限/会话校验；租约不 live 直接
  `Standby(LeaseLost)` 且不读像素；成功投递 `presenting=true`；`poll` 在租约失效且此前 presenting 时切待机。
- `status` 暴露 `presenting`/`standby_reason`，`output_available` 恒 `false`。
- `--frame-drill <pool-id> <producer> <consumer> <slots> <slot-bytes>`：池文件经 **stderr（fd 2）** 继承，
  keeper 通过 `/proc/self/fd/2` 只读重开；未显式启用时不建立帧路径，提交帧请求被拒绝。

## 身份证据的现实边界（本轮最关键的诚实声明）

设计原意是 `SO_PEERCRED` 校验连接对端，**实测不可行**：`UnixStream::peer_cred()`/`UCred` 在钉住的
rustc 1.96.0 上仍是不稳定 feature（E0658，rust-lang/rust#42839），而 workspace 禁止 `unsafe`、本轮不引入新依赖：

```
error[E0658]: use of unstable library feature `peer_credentials_unix_socket`
  --> /tmp/peer_probe.rs:1:38   (rustc 1.96.0)
```

因此实现改用 `/proc` 内核证据：声明的 pid 必须存在、uid 等于 pool owner uid、且**持有该 pool 文件**
（`mnt_id`+`ino` 匹配 `/proc/<pid>/fdinfo`）；UDS 连接只作活性触发，**连接对端未被认证**。

- 这验证的是「谁拿到了 root 交付的 FD」这一**能力持有**，不是「连接来自哪个进程」。
- 因此本 broker **不是**完整 IPC 认证、**不是**设备授权，也未实现正式句柄传递。该缺口已写入
  ADR 0018、`docs/media.md`、registry notes 与报告，并按 Blocked 记录（待工具链稳定或经审查引入 SCM_RIGHTS 能力）。
- 残余风险：同 uid 且已持有 FD 的进程本就在能力集合内；pid 复用存在理论窗口（以「授权一次性建立 +
  `disconnect` 撤销」收敛）；8 次 accept 预算内可被同 uid 无效连接占用（socket 目录 `0600`，可达范围限于 owner uid）。

## 实际运行的命令与结果

| 命令 | 结果 |
|---|---|
| `cargo test --workspace --offline` | **303 项通过 / 0 失败**（上一轮 283，本轮 +20：broker 7、consumer 9、keeper 3、keeper 单元 1） |
| `cargo test -p nai-media-core -p nai-output-keeper --offline` | 通过（70 项） |
| `cargo xtask schema --check` | 无漂移（Rust/TS/C++/JSON Schema/OpenAPI） |
| `cargo xtask test --suite contracts` | 通过（十三类消息三语言编译与双向互通） |
| `cargo xtask check --registry` | 通过 |
| `cargo xtask check-deps` | 通过（14 个 workspace 包） |
| `cargo xtask check --profile portable` | 通过（fmt、clippy `-D warnings`、依赖边界、全量测试） |
| `pnpm -r typecheck` | 通过（contracts-ts、desktop） |
| `pnpm exec openspec validate w05-w06-frame-handle-broker --strict --no-interactive` | valid |

测试清单（节选关键项）：

- broker：`unverified_peer_cannot_touch_slots`、`wrong_declared_pid_is_never_granted`、`accept_budget_is_bounded`、
  `deadline_without_connection_is_explicit`、`verified_producer_writes_and_consumer_reads`、
  `real_subprocess_declared_pid_is_granted_and_writes`、`real_subprocess_wrong_pid_is_refused_without_commit`。
- consumer：`invalid_lease_never_reads_and_reports_lease_lost`、`epoch_at_or_below_floor_or_last_delivery_is_stale`、
  `tampered_sha256_is_an_integrity_failure`、`changed_pool_length_is_an_integrity_failure`、
  `expired_read_budget_is_a_timeout`、`delivery_returns_owned_pixels_and_records_epoch_without_ack`、
  `foreign_pool_identity_is_an_integrity_failure`、`peer_error_event_is_retired`、`write_started_and_missing_event_are_no_frame`。
- keeper（真实子进程）：`real_frame_is_presented_then_lease_loss_is_standby`、
  `peer_error_switches_keeper_to_retired_standby`、`non_frame_drill_mode_rejects_frame_submission`。

## 未测环境与外部阻塞

- 连接对端认证：Blocked（`peer_cred` 不稳定；引入 SCM_RIGHTS 需新依赖与安全审查）。
- SCM_RIGHTS / Windows 命名管道句柄传递、mmap/零拷贝：未实现。
- 独立 render-worker 生产装配、composition root、PlaybackAck 生产端：未实现。
- 真实窗口/音频设备、帧率、P95 延迟与 2 小时/8 小时长播：未执行（Windows/设备里程碑）。
- 性能：临时池文件读写有复制，本轮未做吞吐/延迟基准，不宣称实时能力。

## 后续依赖

- 下一主线：独立 render-worker 生产装配与 composition root（把 broker/keeper/审批/决策日志接线到可运行 root）；
  工具链可用后补连接对端认证。
