# 报告：engine-broker-frame-delivery（broker 授权单帧投递与有界监督）

- 日期：2026-10-09
- 变更：`openspec/changes/engine-broker-frame-delivery/`
- 基线：V3.6 §26.3、§31（W05/W06/W10）；ADR 0017/0018（帧槽位/broker）、ADR 0020（引擎装配）、ADR 0021（本轮边界与交接勘误）
- 结论：可移植软件工程闭环已实现/实测；**不是**设备、Windows、protected IPC 或 PlaybackAck 验收

## 背景与交接勘误

接管 `engine-assembly` 后核对代码发现：归档 tasks 3.2/3.3/3.4 与冒烟描述把计划写成了已验证。

- 3.2 实际为「撤权后停止续租、不自动重签」（`renewal_halted_after_revoke`），非「epoch 变更后显式重签」。
- 3.3 实际为引擎**进程内**消费；broker UDS 授权与 keeper 帧投递当时**未接线**。
- 3.4 实际为 `--halt-file` 触发，非 SIGINT/SIGTERM；被强杀进程不计优雅停机。

本轮已把 3.3 的缺口补齐，并在归档 tasks 3.2/3.3/3.4、`proposal.md` 与 ADR 0021 中同步更正文字。

## 实现

### broker 与只读句柄（`nai-media-core::frame_slots`）

- `FrameBroker::bind` 建 `/dev/shm` 私有池与 `0600` UDS；最多 2 个唯一 role；`declare_peer` 检查容量与重复 role。
- 授权依据为内核能力证据：声明 pid 存活、uid 等于 pool owner、且 pid **持有 pool 文件**（`/proc/<pid>/fdinfo` 的
  `mnt_id`+`ino`）；UDS 仅作活性触发，连接对端仍未认证（ADR 0018 不升级）。
- `require()` 只复检 `authorized && !retired`（授权在 grant 时一次性建立，撤销经 `disconnect`），
  以支持「producer 写完即退出、root 事后接受 commit」的既有语义。
- `SharedFramePool::consumer_file` 提供只读重开句柄；`read_committed_frame` 读前复检 pool identity、
  session、epoch、producer/consumer、`slot_bytes`、`slot_index`、SHA256 与池状态。
- 新增 `begin_consume`/`release`/`consumer_spawn_handle`。

### keeper 诊断与 engine relay

- `OutputWatchdogStatus` 新增 `consumed_buffer_ref=15`/`consumed_sha256=16`（observability only）。
- keeper `submit_frame` 先经 `MediaGate::rehearsal_frame_permitted` 的 scope 复检（Rehearsal + frame session/epoch/utterance）；
  投递后保留 owned 像素与 SHA256，待机清除；`output_available` 恒 `false`。
- keeper main 支持 `--frame-epoch <n>`（必须 `> floor`）与 `--broker <path>`。
- engine `run_media_drill`：root 声明真实 spawn pid → keeper 只读 FD 触发 UDS → render-worker 继承写 FD、
  `--ready` 预热、reserve/原子 ticket → 读 `write_started`/`committed` → root accept commit（槽 pin Reading）→
  `begin_consume` → `submit_frame(seq)` → 匹配 `consumed_buffer_ref`/`consumed_sha256` → `release` → 写 finish → 回收 worker。
- 预授权/等待期间 `DrillGuard` 每 5ms 续租、`observe()`、复检共享 epoch 与 halt；worker 事件经 reader 线程入有界队列，
  owner 非阻塞轮询（`try_worker_event`），卡读不阻塞期限/停机。250ms 槽预算不延长。
- 失败路径固定：先 P0 静音 → 撤权（epoch 递增）→ critical 决策日志 → 有界回收；`Supervisor` 析构兜底。
- `Supervisor` 用 mpsc + writer/reader 线程承载 keeper 命令与 worker 事件，`shutdown` 先 drop channel 再 `join_bounded`。

## 验证（本轮实际执行）

```
cargo fmt --all
cargo clippy --workspace --all-targets --offline                      # 无告警
cargo test --workspace --offline                                      # 324 项通过
cargo xtask check-deps                                                # 17 包边界通过
cargo xtask check --profile portable                                  # 通过
cargo xtask check --registry                                          # V3.6 S/A/R/O/X + D01–D14，通过
cargo xtask schema --check                                            # 27 能力/112 消息，无漂移
cargo xtask test --suite contracts                                    # Rust/TS/C++ 十三类消息互通
pnpm typecheck                                                        # contracts-ts + desktop
pnpm test:engine-smoke                                                # 真实 engine + 桌面 ControlClient
pnpm test:control-smoke                                               # 真实 loopback 控制
```

关键集成测试（真实子进程）：

- `apps/engine/tests/engine.rs::media_drill_produces_a_committed_frame_through_the_real_worker`
  — 完整 broker+keeper+worker 单帧，校验 `media_drill_committed` bytes/buffer_ref/槽代次。
- `apps/engine/tests/engine.rs::halt_during_media_drill_prewarm_never_commits_a_frame`
  — 预热期 halt 有界退出、不产生 committed 帧、链完整。
- `apps/engine/tests/engine.rs::http_revoke_stops_lease_renewal_without_regranting_output`、
  `killed_keeper_is_halted_by_the_engine_and_the_chain_survives` — 撤权/强杀。
- `crates/nai-runtime/tests/supervisor.rs::{render_worker_writes_real_pixels_and_the_consumer_reads_them,
  render_worker_rejects_a_ticket_from_another_epoch, tampered_commit_hash_is_rejected_and_retires_the_pool,
  killed_keeper_is_revoked_logged_and_stands_by}` — 成功、陈旧 epoch、篡改哈希退休、故障待机。
- `crates/nai-media-core/tests/broker.rs`（7）、`consumer.rs`（9）、`shared_ram.rs`（5）— 授权、只读/边界、
  部分写入强杀、新池绕过旧 FD。

## 边界与残余缺口（不得掩盖）

- **连接对端未认证**：`SO_PEERCRED`/`peer_cred` 在钉住的 rustc 1.96.0 上仍不稳定（E0658），workspace 禁 `unsafe`；
  broker 授权是「声明 pid + uid + 持有 FD」的能力证据，不是对端身份。ADR 0018 结论不变。
- 句柄交付仍为 spawn 继承；SCM_RIGHTS、Windows 命名管道、protected handle broker 正式形态未实现。
- 信号停机仍不可用（`signal-hook-registry` 不在离线缓存），继续用 `--halt-file`。
- keeper 诊断是 observability，**不是** PlaybackAck、**不是** Ready；无真实窗口/音频设备与 Windows 认证。

## 验收状态

`tests/registry.yaml`：S09/R01/R03/A01 维持 implemented，O02 维持 pending，**均不标 tested**。

## 归档

`openspec/changes/archive/2026-10-09-engine-broker-frame-delivery/`；主规格 `runtime-composition` 增量生效。
