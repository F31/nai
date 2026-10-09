# 引擎装配：共享会话、真实控制服务与监督闭环

- 日期：2026-10-09
- 变更：`openspec/changes/engine-assembly/`（`runtime-composition` 与 `local-control-surface` 增量）
- 对应 V3.6：composition root 可执行化（承接 W05/W06 装配根与 keeper/render-worker 轮次）
- 验收影响：S09/R01/A01/O02 保持 implemented，notes 补本轮证据与仍缺项（不标 tested）
- 新增 ADR：`docs/adr/0020-engine-assembly-boundary.md`

## 目标与范围

把已实现的存储 owner、共享会话、loopback 控制面与受监督媒体子进程装配为可启动进程
`apps/engine`，并修复「控制面与监督侧各持一份会话」的 epoch 分叉。

明确不做（保持未实现，不冒充）：protected IPC/信号停机、broker 的 UDS 授权接线、keeper 帧投递、
PlaybackAck 生产端、真实窗口/音频设备、Windows 与性能认证。

## 实现

### 共享会话（消除 epoch 分叉）

- `nai-control-api`：`State.session` 改为 `Arc<Mutex<Session>>`，新增
  `ControlApi::with_shared_session`，旧 `new`/`with_approvals` 委托并自行包裹；`status`/`command`/
  `approvals_*` 全部改为 `try_lock`（忙时沿用 409），P0 静音仍在加锁前执行。
- `nai-runtime`：`Composition.session` 改为共享实例并暴露 `session()`/`session_lock()`；
  `control`/`confirm_duty`/`enforce_duty`/`observe`/`rehearsal_lease` 均 `try_lock`，锁失败返回
  `Conflict` 且不回退已执行的静音。
- `rehearsal_lease` 新增陈旧校验：拒绝低于现场会话 epoch 的请求（`STALE_EPOCH`）。

### apps/engine（layer app）

- `--provision <profile.json>`：可信 owner 一次性建工作区、授权、写运行时 profile（存储层校验），退出 0。
- 启动：`Composition::prepare` → `ControlApi::with_shared_session` → `serve` 绑定 loopback；
  stdout 打印 `nai-engine control: http://127.0.0.1:<port>/api/v1`。
- 监督线程（`--keeper` + `--render-worker`）：spawn keeper（floor=现场 epoch）→ `floor+1` 租约 arm →
  每 tick 先续租再 `observe`；控制面撤权后停止续租并记 telemetry；续租失败不静默退出。
- `--media-drill`：`/dev/shm` 私有池 + 预热 render-worker + reserve/原子 ticket + owner commit +
  `FrameConsumer` 读回 owned 像素，落 telemetry（buffer_ref/SHA256/字节数）。
- 停机：`--halt-file` → 撤 token → 静音+撤权+critical 日志 → 有界回收 → HTTP 停 → 退出 0。

### 存储

新增只读 `Store::decision_entries(op, workspace, limit)`（链序返回），供证据断言与后续复盘；不改
追加与哈希链语义。

## 命令与结果

| 命令 | 结果 |
|---|---|
| `cargo test --workspace --offline` | **323 项通过，0 失败**（318 + 引擎 5） |
| `cargo test -p nai-engine --offline` | 5 项通过（真实子进程 + 真实存储） |
| `cargo test -p nai-runtime -p nai-control-api --offline` | 13 + 15 项通过 |
| `cargo xtask check-deps` | 通过（17 个 workspace 包） |
| `cargo xtask check --registry` | 通过 |
| `cargo xtask schema --check` | 无漂移 |
| `cargo xtask test --suite contracts` | Rust/TS/C++ 十三类消息互通通过 |
| `cargo fmt --all --check` / `clippy -D warnings` | 通过 |
| `cargo xtask check --profile portable` | 通过 |
| `pnpm -r typecheck` | 通过 |
| `pnpm test:engine-smoke` | 通过（真实 engine + 桌面 ControlClient） |
| `pnpm test:control-smoke` | 通过（旧冒烟未回归） |

## 测试清单（引擎新增 5 项 + 运行时 1 项）

- `apps/engine/tests/engine.rs`：
  1. 无 token 401；有 token status 200（epoch 0）；mute 后共享会话 epoch=1；halt 后退出 0，
     决策链校验通过且存在 critical `cancellation`（action=mute）。
  2. 引擎监督的 keeper 被 SIGKILL（按 PPid 精确定位）→ 引擎撤权（HTTP 可见 epoch=1）、控制服务仍
     可达、终止后链校验通过且存在含 `keeper` 的 critical 记录与 `armed` telemetry。
  3. HTTP mute → 引擎停止续租（`renewal_halted_after_revoke`），且无 `renewal_failed` 反复重试。
  4. `--media-drill` 真实提交：telemetry 含 `media_drill_committed`、buffer_ref、8 字节像素。
  5. 缺参数与非法 token 权限（0644）均以退出码 2 拒绝，不监听端口。
  第 1 项同时覆盖 `--provision` 真实配置（打印 `nai-engine provisioned` 后退出 0）。
- `crates/nai-runtime/tests/composition.rs`：新增陈旧租约 epoch 被拒断言。

## 未测环境与阻塞

- 优雅停机仅由文件触发：`signal-hook-registry` 不在离线缓存，`unsafe` 被禁（Blocked）。
- broker UDS 授权与 keeper 帧投递未接线；PlaybackAck 生产端、真实窗口/音频设备、Windows 与性能
  认证未执行。

## 后续依赖

protected IPC/信号停机替换 halt 文件；broker 在装配中接线（含 render-worker 侧客户端）；keeper 帧
投递路径；设备里程碑做实 PlaybackAck 与帧率认证。
