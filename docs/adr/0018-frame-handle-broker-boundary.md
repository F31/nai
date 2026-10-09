# ADR 0018：帧句柄 broker 边界与连接对端未认证的残余缺口

- 日期：2026-10-09
- 状态：Accepted
- 基线：V3.6 §26.3、§3.4；承接 ADR 0017（帧槽位所有权与 tmpfs-file 演练边界）
- 变更：`openspec/changes/w05-w06-frame-handle-broker/`

## 决策

在 ADR 0017 的工程演练边界内新增**授权 broker**（`nai_media_core::frame_slots::broker`，Linux-only）与
keeper 侧帧消费/待机路径（`apps/output-keeper`），并明确二者都没有达到生产 IPC 与设备验收：

1. **授权依据是内核能力证据，不是连接对端身份**。`UnixStream::peer_cred()`/`SO_PEERCRED` 在钉住的
   rustc 1.96.0 上仍是不稳定 feature（E0658，rust-lang/rust#42839）；workspace 禁止 `unsafe`、本轮不引入
   新依赖（rustix/nix 未经审查）。因此 broker 采用：声明的 pid 必须存在、uid 等于 pool owner uid，且
   **持有该 pool 文件**（`/proc/<pid>/fdinfo` 的 `mnt_id`+`ino` 匹配）。UDS 连接只作活性触发。
   - 这验证的是「谁拿到了 root 交付的 FD」这一**能力持有**，不是「连接来自哪个进程」。
   - 连接对端未被认证：能连上 socket 的进程只影响授权时机（触发 `poll_grants`），不能直接取得 ticket；
     ticket 与提交仍由持有 owner 账本的 root/broker 进程签发。
2. **句柄交付仍是 spawn 继承**。SCM_RIGHTS、Windows 命名管道句柄传递、protected handle broker 的正式形态
   均未实现；broker 只是授权与账本门控。
3. **accept 有界**：单次 `poll_grants` 最多 8 次 accept、有期限；socket 位于 `0600` 私有目录，可达范围限于
   pool owner uid。同 uid 进程已在信任边界内（ADR 0017 已声明）。
4. **keeper 帧消费与待机**：租约、epoch、池状态与完整性任一不满足 → 待机（`LeaseLost`/`StaleEpoch`/
   `Retired`/`IntegrityFailure`/`Timeout`/`NoFrame`），不投递、不产生 `PlaybackAck`，
   `output_available` 恒 `false`；帧路径仅 `--frame-drill` 显式启用。
5. 帧数据仍不经控制总线：池文件经继承的标准流交付，keeper 只读重开（`/proc/self/fd/2`）。

## 后果

- 未授权进程无法取得任何 ticket 或推进槽位状态；未声明/不存在的 pid、非 owner uid、不持有 pool FD 的 pid
  永不获授权（有真实子进程测试覆盖）。
- 残余缺口（不得掩盖）：连接对端未认证；同 uid 且已持有 FD 的进程本就在能力集合内；pid 复用存在理论窗口
  （本轮以「授权一次性建立 + `disconnect` 撤销」收敛）；8 次 accept 预算内可被同 uid 的无效连接占用。
- 后续正确路径：工具链稳定 `peer_cred` 或经审查引入 SCM_RIGHTS 能力后，改为对端认证 + 真正句柄传递；
  在此之前不得把本 broker 表述为完整 IPC 认证或设备授权。
- 独立 render-worker 生产装配、PlaybackAck 生产端、真实窗口/音频设备与性能认证仍待后续里程碑。
