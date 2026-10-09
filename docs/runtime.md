# 运行时装配根与子进程监督（W05/W06 后续）

`nai-runtime`（layer `runtime`，ADR 0019）是本轮新增的装配根库：它持有唯一存储 owner、会话与值守租约、
媒体门、审批后端与决策日志，并驱动受监督的媒体子进程。它不是引擎可执行程序——`apps/engine`、
loopback HTTP 服务启动与 broker 装配接线仍待后续里程碑。

## 装配（Composition::prepare）

`Composition::prepare(paths, workspace, actor, profile, bootstrap)` 的固定顺序：

1. `Store::open` 取得唯一存储 owner（第二 owner 打开被拒），包为 `Arc<Mutex<Store>>`。
2. 仅 `bootstrap=true` 时执行可信 owner 管理：建工作区并授予 `config.read/write`、`approval.read/write`、
   `session.manage`。该路径**只对可信 owner 开放**，模型/插件永远不可达；工作区必须不存在（一次性）。
3. `load_config(ConfigKind::Runtime, profile)` 装载经 JSON Schema 校验的运行时配置；缺 `config.read`
   权限或配置非法在装配期显式拒绝。
4. 交叉检查 `media.lease_renew_ms < lease_expire_ms` 与 `runtime.max_active_sessions ∈ [1,1024]`。
5. 构造 `Session`（session_id 即 workspace id）、`MediaGate::with_epoch_floor`、
   `Registry::new(boot_id, Limits)` 与 `ApprovalBackend`（共享 store）。

每次存储/决策访问新建 5s `Operation`（上限 30s，无长租表达），P0 静音经 `EmergencyHandle`
不等数据库锁。

## 控制与值守

- `control(Mute|Takeover)`：先 `gate.emergency().mute()`，再 `Session::revoke`（epoch 递增），
  最后写 critical `cancellation` 决策日志。返回的 epoch 只表示状态变更，**不代表设备已停**。
- 值守包装：operator-only `confirm_duty` 授予、`enforce_duty`/`tick_duty` 到期/撤权，
  均写 critical `duty_lease` 日志；到期执行先静音再撤权再落日志。
- `record` 按 `DecisionKind::criticality()` 分级：critical 失败直接返回错误，telemetry 允许缓冲。
- `rehearsal_lease` 只产出 Rehearsal 模式有界租约（TTL ≤600ms）；生产播放租约需审批与事实，不在此生成。

## 子进程监督（Supervisor）

- 二进制路径由调用方显式传入（`Binaries::validate` 缺失即 `CapabilityNotReady`）；库内不用
  `env!("CARGO_BIN_EXE_…")`，跨包不可见。
- `spawn_keeper`：`--watchdog-drill`，可选 `--frame-drill`（池 FD 经 stderr 继承，与 keeper 约定一致）；
  `arm`/`renew` seq 递增、`remaining_budget_ms ≤250`，仅 Rehearsal 租约。
- `spawn_worker`：`--render-drill`，PCM 原始字节与 protobuf ticket 经文件传入，帧数据不走 JSON/Base64；
  可选 `--ready <path>` 预热标记——worker 先 `Avatar::admit` 写标记后有界等待 reserve ticket
  （冷启动 ~290ms 超过槽位 250ms 非延长预算，故先预热再 reserve）。
- `observe()` 故障处置顺序固定：**静音 → revoke（epoch 递增）→ critical 日志 → standby**；
  日志失败上报但绝不回退静音/撤权，standby 无条件设置。
- `shutdown` kill + 有界回收；非 Linux 显式不可用。

## render-worker（apps/render-worker，layer media）

显式 `--render-drill`；池 FD 经 stderr 继承并读写重开（`/proc/self/fd/2`）；`nai-avatar-2d`
真实像素经 `validate_producer_handle`/`write_frame` 写入池；stdout 仅输出有界
`FrameSlotEvent{write_started|committed|error}`；复检 epoch/取消/期限，失败 error 事件 + 退出码 2。
不产生 PlaybackAck、不声明 Ready、不接设备。

```bash
cargo build -p nai-output-keeper --offline   # supervisor 测试需 keeper 二进制先存在
cargo test -p nai-runtime -p nai-render-worker --offline
```

实测见 [报告](reports/2026-10-09-w05-w06-composition-root.md)：强杀 keeper 后撤权/standby/日志落库且
哈希链校验通过；render-worker 真实像素被 owner commit 接受、consumer 读到 owned 副本；
陈旧 epoch ticket 与篡改 SHA256 均被拒并退休整池。
