# ADR 0021：引擎 broker 帧投递接线与交接勘误——不升级对端认证状态

- 日期：2026-10-09
- 状态：Accepted
- 基线：V3.6 §26.3、§31；承接 ADR 0017/0018（帧槽位/broker 边界）、ADR 0019/0020（装配根/引擎）
- 变更：`openspec/changes/engine-broker-frame-delivery/`

## 决策

`apps/engine` 的媒体演练从「引擎进程内直接 `FrameConsumer` 读回」升级为 **broker 门控的真实
render-worker→keeper 单帧投递**，同时保持 ADR 0018 的对端认证结论不变：

1. **root 声明 spawn pid，broker 按内核能力证据授权**。engine 以 `FrameBroker::bind` 建池并
   以 `PeerIdentity{role,pid,uid}` 声明 producer（render-worker）与 consumer（output-keeper）；
   授权依据仍是「pid 存在 + 属 pool owner uid + 持有 pool 文件（`/proc/<pid>/fdinfo` 的
   `mnt_id`+`ino`）」，UDS 仅作活性触发。**连接对端未被认证**：`SO_PEERCRED`/`peer_cred` 在钉住的
   rustc 1.96.0 上仍不稳定（E0658），workspace 禁 `unsafe`、未引入新依赖。
2. **consumer 只继承只读 FD**。keeper 经 stderr 继承池 FD，只读重开；每次读取复检
   pool/session/epoch/双方 role/stride/slot 边界/SHA256。
3. **keeper 诊断不产生 PlaybackAck**。`OutputWatchdogStatus` 新增
   `consumed_buffer_ref`/`consumed_sha256` 诊断字段（observability only），待机时清除；
   `output_available` 恒 `false`。帧诊断**不是** Ready，也**不是**播放完成。
4. **等待期持续续租/observe/复检**。预热、授权、事件等待均按固定 tick 续租、`observe`、
   复检 live epoch 与 halt，槽位 250ms 预算不延长；worker 事件经 reader 线程入有界队列，owner
   非阻塞轮询，卡读不阻塞 halt/timer。
5. **失败顺序固定**：主错误路径先 P0 静音→撤权（epoch 递增）→critical 决策日志→有界回收子进程；
   `Supervisor` 析构兜底。日志失败不回退静音。
6. **临时停机方式保留**。信号停机仍不可用（`signal-hook-registry` 不在离线缓存、禁 `unsafe`），
   继续用 `--halt-file`。

## 勘误（engine-assembly 归档）

交接核对发现归档 tasks 3.2/3.3/3.4 与冒烟描述把**未完成/未实现**写成了已验证，本 ADR 与本次
规范增量同步修正：

- 3.2：实际为「撤权后停止续租、不自动重签」（`renewal_halted_after_revoke`），**非**「epoch 变更后显式重签」。
- 3.3：实际为引擎进程内消费；broker UDS 授权与 keeper 帧投递**当时未接线**。本次变更完成该接线。
- 3.4：实际为 `--halt-file` 触发，**非** SIGINT/SIGTERM；被强杀进程不计优雅停机。
- 冒烟 `scripts/engine-smoke.mjs` 不涉及信号停机，文字已更正。

本 ADR 记录的新接线**不升级** ADR 0018 的认证状态：句柄交付仍为 spawn 继承，未实现 SCM_RIGHTS 与
Windows 命名管道。

## 后果

- 真实 engine+broker+keeper+worker 单帧 reserve→commit→Reading pin→relay→keeper 诊断→release 可实测；
  worker 卡读/预热期 halt、HTTP 撤权、只读 FD、陈旧 metadata/epoch 故障回归由新测试覆盖。
- 保持 A01/R01/R03/S02 为 implemented，**不标 tested**。
- 残余缺口（不得掩盖）：protected IPC/信号停机、broker 对端认证、SCM_RIGHTS/Windows 句柄传递、
  PlaybackAck 生产端、真实设备/窗口帧率与 Windows 认证。
