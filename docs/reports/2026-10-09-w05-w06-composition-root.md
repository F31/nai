# W05/W06 装配根与 render-worker 演练（composition root）

- 日期：2026-10-09
- 变更：`openspec/changes/w05-w06-composition-root/`（新主规格 `runtime-composition`，`media-frame-slot-ownership` 增量）
- 对应 V3.6：W05/W06 的独立 render-worker 生产装配与 composition root 接线（软件层）
- 验收影响：R01/R03/A01/S01 保持 implemented，notes 补本轮证据与仍缺项（不标 tested）
- 新增 ADR：`docs/adr/0019-composition-root-boundary.md`

## 目标与范围

把既有存储/会话/媒体门/审批/决策日志与受监督媒体子进程装配为软件监督闭环：
`crates/nai-runtime`（layer `runtime`）+ `apps/render-worker`（layer `media`）。

明确不做（保持未实现，不冒充）：`apps/engine` 可执行与 loopback HTTP 控制服务启动、broker 在装配中的
接线、真实窗口/音频设备、PlaybackAck 生产端、性能与 Windows 设备认证。

## 实现

### crates/nai-runtime

- `Composition::prepare`：`Store::open` 唯一 owner → 显式一次性 `bootstrap`（可信 owner 建工作区+授权，
  模型/插件不可达）→ `load_config` 经 schema 校验并交叉检查租约间隔/会话上限 → 装配
  `Session`/`MediaGate::with_epoch_floor`/`Registry`/`ApprovalBackend`。每次存储访问新建 5s
  `Operation`；P0 静音经 `EmergencyHandle` 不等数据库锁。
- `control(Mute|Takeover)`：先静音 → `Session::revoke`（epoch 递增）→ critical `cancellation` 日志；
  返回 epoch 不代表设备停止。
- 值守包装：operator-only `confirm_duty` 与 `enforce_duty`/`tick_duty` 到期/撤权均写 critical
  `duty_lease`；`record` 按 `DecisionKind::criticality()` 分级（critical 失败即错误，telemetry 可缓冲）。
- `rehearsal_lease` 只产出 Rehearsal 有界租约（TTL ≤600ms）。
- `Supervisor`：二进制路径显式传入（缺失即 `CapabilityNotReady`）；`spawn_keeper`
  （`--watchdog-drill`，可选 `--frame-drill` 池 FD 经 stderr 继承）、`arm`/`renew`（seq 递增、
  ≤250ms 预算、仅 Rehearsal）、状态帧解析、`shutdown` kill+有界回收。
- `observe()` 故障处置：静音 → 撤权 → critical 日志 → standby；日志失败上报但绝不回退静音。

### apps/render-worker

显式 `--render-drill`；池 FD 经 stderr 继承并读写重开（`/proc/self/fd/2`）；`--pcm` 原始字节、
`--ticket` protobuf；`nai-avatar-2d` 真实像素经 `validate_producer_handle`/`write_frame` 写入；
stdout 仅有界 `FrameSlotEvent{write_started|committed|error}`；复检 epoch/取消/期限，失败退出码 2。
`--ready <path>` 预热标记：冷启动 ~290ms（含 `Avatar::admit` 一次性 jsonschema ~194ms）超过槽位
250ms 非延长预算，故先 admit 写标记、有界等待 reserve ticket，reserve→commit 只覆盖写入窗口。

### 与设计稿的两处偏差（已记录）

1. reserve/spawn 排序改为「预热 → reserve → commit」（原因如上）。
2. 子进程测试置于 `crates/nai-runtime/tests/`：`env!("CARGO_BIN_EXE_…")` 跨包不可见，
   `-Z bindeps` 在钉住工具链不稳定（已核实）；keeper 二进制需先行构建（与 `frame_drill.rs` 既有约定一致）。

## 命令与结果

| 命令 | 结果 |
|---|---|
| `cargo test -p nai-runtime -p nai-render-worker --offline` | 15 项通过，0 失败 |
| `cargo test --workspace --offline` | 318 项通过（303+15），0 失败 |
| `cargo xtask check-deps` | 通过（16 个 workspace 包） |
| `cargo xtask check --registry` | 通过 |
| `cargo xtask schema --check` | 无漂移 |
| `cargo xtask test --suite contracts` | Rust/TS/C++ 十三类消息互通通过 |
| `cargo fmt --all --check` | 通过 |
| `cargo clippy --workspace --all-targets --offline -- -D warnings` | 通过 |
| `cargo xtask check --profile portable` | 通过 |

## 测试清单（新增 15 项，全部真实子进程/真实存储）

- `tests/composition.rs`（9）：第二 owner 拒绝、缺 profile/缺 config.read 权限/非法配置装配期拒绝、
  bootstrap 先行、control 顺序与日志、duty 授予/到期/撤权日志、critical 失败上报与 telemetry 缓冲。
- `tests/supervisor.rs`（4，Linux 真实子进程）：arm 后 `lease_valid`；**强杀 keeper** → epoch 递增 +
  standby(Retired) + `ack.persisted` + `verify_decision_chain` 通过；render-worker 真实像素被 owner
  commit 接受且 `FrameConsumer` 读到 owned 副本；陈旧 epoch ticket 拒签（error 事件+非零退出）；
  篡改 SHA256 被拒并退休整池。
- `apps/render-worker/tests/usage.rs`（2）：`--render-drill` 与精确资产闭包缺失时显式拒绝。

## 未测环境与阻塞

- `apps/engine` 可执行、loopback HTTP 服务启动、broker 装配接线：未实现。
- 真实窗口/音频设备、PlaybackAck 生产端、性能与 Windows 认证：Blocked（WSL 不验收）。
- 连接对端未认证（SO_PEERCRED Blocked，ADR 0018）；keeper 二进制需先行构建（bindeps 不稳定）。

## 后续依赖

`apps/engine` 装配 loopback HTTP 与 supervisor 主循环；broker 接入装配；设备里程碑做实 PlaybackAck
与帧率认证。
