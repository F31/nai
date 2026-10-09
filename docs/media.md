# 媒体基础与样本时轴（D11 内核部分）

`nai-media-core::timeline` 是可移植、纯逻辑的媒体时序内核，仅依赖 contracts；
Windows 独立窗口/音轨与设备不在这里实现。

```bash
cargo test -p nai-media-core
cargo test -p nai-output-keeper
cargo xtask check --profile portable
```

## 音频块与时轴

- `audio::AudioBlock`：session/utterance/epoch、seq、采样率、声道、格式、sample_start/count、end_of_stream；
  同句内 seq 单调 +1、采样率/声道/格式不变，end_of_stream 是终块（其后不可再写）。
- `timeline::UtteranceTimeline`：累计音频样本与视频帧；首段获准音频 + 首批视频（默认 8 帧）双水位才 `commit`；
  持续迟到返回 `Late`，由调用方取消当前句并静音/降级。
- 时长按样本数，审计 UTC 不是媒体时钟；PCM 到达时间不等于内容时间。

## 时钟、缓冲与重采样

- `clock::MediaClock`：位置只增不减，`insert_silence/advance_ms` 保连续；`pts(tb_num,tb_den)` 为有理表达。
- `clock::map_cross_rate / duration_in_output`：16k 模型帧 → 48k 输出帧的精确比例映射，禁止直接比较异率 sample_start。
- `ring::PcmRing`：有界槽位/字节，超限返回 `Capacity`，不静默丢弃；`read_up_to` 按 FIFO。
- `resample::resample_linear_i16`：确定性线性插值，输出长度有上界，超限报错。

## 录制与 ACK

- `record::RecordingSink`：以媒体时钟位置单调写入；句间间隔写确定静音补齐，`gaps_written` 透明；`MemoryRecorder` 为测试实现。
- `ack::AckTracker`：`Pending → Committed → Playing → Completed`；仅 `committed && actual_local_playback_completed && played==expected` 标 Completed，
  部分/矛盾 ACK 报 InvalidAck，取消后不再接受迟到 ACK。

集成测试 `tests/timeline.rs` 覆盖 16k TTS → 48k 输出 → 双水位 commit → 实际 ACK → 连续录制全程。
真实 Windows 窗口/音轨、虚拟摄像头/音频设备与录制文件装配仍为设备里程碑，见 [D11 报告](reports/2026-10-08-d11-media-basics.md)。

## 输出看门狗演练（W05 前置）

`apps/output-keeper --watchdog-drill <session> <trusted-floor>` 是显式、仅演练的独立进程（详见
[报告](reports/2026-10-08-w05-w10-output-watchdog.md) 与 [ADR 0016](adr/0016-output-watchdog-drill-boundary.md)）：
复用 `MediaGate` 独立 10ms 轮询租约，私有继承 stdio 传输 Protobuf 工程命令，输入 16×64KiB 有界、
帧期限 250ms、仅 Rehearsal 租约；输出始终 `output_available=false`、backend_error=CAPABILITY_NOT_READY，
不消费 PCM/帧，不产生播放 ACK。真实子进程强杀约 10ms 使软件租约失效并撤权、停止续租约 578ms 到期、
partial 卡读按期关闭、keeper 重启须更高 epoch floor（`MediaGate::with_epoch_floor` 提供无授权 floor）。
这些仅证明演练软件边界，不表示真实 engine/worker 或设备故障处理，正式窗口/音轨/媒体协调仍待后续。

## 2D 像素生产核心（W06 前置）

`plugins/avatar-2d` / `nai-avatar-2d` 已实现分层RGBA8素材准入、样本时轴RMS嘴型与有界逐帧合成。
`cargo test -p nai-avatar-2d --offline` 可执行；参考 [2D指南](avatar-2d.md) 与 [实测报告](reports/2026-10-09-w06-avatar-2d-core.md)。
返回owned像素与FrameMetadata，不授权输出或标实际播放完成；当前没有render-worker/共享内存/keeper消费生产接线，Windows窗口/设备与实时帧率未执行。

## 帧槽位所有权与Linux tmpfs-file演练（W05/W06 前置）

`nai-media-core::frame_slots` 提供有界、单owner的帧槽位状态机与显式Linux工程后端（ADR 0017）：
- 单owner `Free→Writing→Ready→Reading→Free`；ticket绑定pool identity/session/epoch/双方身份/slot代次/整份metadata，`slot_index`只是偏移索引。
- 上限16槽、8MiB/槽、64MiB/池；reserve非延长期限≤250ms；RGBA8长度/SHA256/metadata在commit与read复检；release前不可覆盖，ABA/重复/未知身份拒绝。
- 超时、断连、读取取消或完整性失败退休整池，不复用旧内存；新池必须新identity。
- Linux后端名为 `tmpfs-file-drill`：私有 `/dev/shm` 目录（当前owner、0700）、unlink的0600文件、继承FD、safe分块`read_at/write_at`；有复制，不声明mmap/零拷贝。非Linux显式 `CapabilityNotReady`。
- `nai-shared-frame-drill` 需显式 `--shared-frame-drill`，stdin为继承数据FD、stdout仅有界Protobuf事件；MUST NOT经JSON/Base64控制总线传帧。

```bash
cargo test -p nai-media-core --offline
cargo test -p nai-avatar-2d --offline
```

实测见 [报告](reports/2026-10-09-w05-w06-frame-slots.md)：真实继承FD生产者提交、readerowned副本、部分写入强杀整池退休、新池绕过旧FD。独立render-worker/keeper生产装配、protected handle broker、命名管道身份与Windows映射仍未实现；本后端不替代正式共享内存或设备验收。

## 帧句柄授权 broker 与 keeper 帧消费（W05/W06 后续）

`nai_media_core::frame_slots::broker`（Linux-only，ADR 0018）对 pool 的 producer/consumer 授权做门控：

- 授权依据是**内核能力证据**：声明的 pid 存在、uid 等于 pool owner uid、且持有该 pool 文件
  （`/proc/<pid>/fdinfo` 的 `mnt_id`+`ino`）；`reserve/commit/producer_file/read_frame` 未授权一律拒绝，
  `disconnect` 撤销并退休整池。UDS（`<pool_id>.sock`，`0600`，Drop unlink）只作活性触发，
  单次最多 8 次 accept 且有期限。
- **连接对端未被认证**：`SO_PEERCRED`/`peer_cred` 在钉住的 rustc 1.96.0 上仍不稳定（E0658），
  本 workspace 禁 `unsafe` 且未引入新依赖。句柄交付仍是 spawn 继承，SCM_RIGHTS 与 Windows 命名管道未实现。
  不得把该 broker 表述为完整 IPC 认证或设备授权。
- `frame_slots::consumer` 的 `FrameConsumer` 消费已提交帧并给出待机原因：`LeaseLost`（不读像素）、
  `NoFrame`、`Retired`、`StaleEpoch`、`IntegrityFailure`、`Timeout`、`NotReady`；成功返回 owned 像素，
  **不产生 PlaybackAck**。读路径 `read_committed_frame` 与 owner 侧 `SharedFramePool::read_frame` 共用。

`apps/output-keeper --watchdog-drill <session> <floor> --frame-drill <pool-id> <producer> <consumer> <slots> <slot-bytes>`
在显式启用时建立帧路径：池文件经 **stderr（fd 2）** 继承并由 keeper 只读重开（`/proc/self/fd/2`），
命令与帧事件走既有 stdin 帧通道（`OutputWatchdogCommand.submit_frame`）。有效租约下投递置 `presenting=true`；
租约失效、epoch 陈旧、peer 报错或池退休 → 切待机并停止投递；`output_available` 恒 `false`。

```bash
cargo test -p nai-media-core -p nai-output-keeper --offline
```

实测见 [报告](reports/2026-10-09-w05-w06-frame-handle-broker.md)：未授权/错误 pid/超预算均无授权，
真实子进程被接受后写入并读回，keeper 在真实子进程帧后 presenting、租约丢失与池退休切待机。
独立 render-worker 生产装配、PlaybackAck 生产端、真实窗口/音频设备与性能认证仍未实现。

## 装配根与 render-worker 演练（W05/W06 后续）

`nai-runtime`（ADR 0019，详见 [运行时指南](runtime.md)）把存储/会话/媒体门/审批/决策日志装配为
唯一 owner 的 `Composition`，并以 `Supervisor` 驱动 keeper 与 render-worker 真实子进程：

- 控制与故障处置顺序固定：先 P0 静音，再撤权（epoch 递增），再 critical 决策日志；
  日志失败绝不回退静音，standby 无条件设置。ACK 不代表硬件停止。
- `apps/render-worker --render-drill`：池 FD 经 stderr 继承，`--pcm` 原始字节与 `--ticket`
  protobuf 经文件传入（帧不走 JSON/Base64）；`nai-avatar-2d` 真实像素写入槽位，stdout 仅
  `FrameSlotEvent`；复检 epoch/取消/期限，失败 error 事件并非零退出。冷启动超 250ms 槽位预算，
  故 `--ready` 预热标记先 admit 再等有界 reserve ticket。
- 不产生 PlaybackAck、不声明 Ready、不接设备；非 Linux 显式 `CapabilityNotReady`。

```bash
cargo build -p nai-output-keeper --offline   # supervisor 测试需 keeper 二进制先存在
cargo test -p nai-runtime -p nai-render-worker --offline
```

实测见 [报告](reports/2026-10-09-w05-w06-composition-root.md)。`apps/engine` 可执行、
loopback HTTP 服务启动、broker 装配接线、真实设备与性能认证仍未实现。
