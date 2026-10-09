# Design

## 依赖方向与登记

- 新增 `crates/nai-runtime`，`[package.metadata.nai] layer = "runtime"`（`xtask/src/deps.rs` 已预登记该路径与层）。
  `runtime` 层无外部名禁令；可依赖 kernel 与具体实现，**kernel 不得依赖 runtime**（由 check-deps 强制）。
- 新增 `apps/render-worker`，`deps.rs` 已把该路径硬编码为 `media` 层；`media` 禁 provider 与 `plugins/llm-|tts-|gateway-`，
  依赖 `plugins/avatar-2d`（plugin 层）与 `nai-media-core` 合法。
- 两个新包都必须加入根 `Cargo.toml` 的 `members` 与 `default-members`，否则 portable profile 不构建它们。
- 不新增第三方依赖：runtime 用既有 `nai-*`（kernel/storage/control-api/media-core/security/contracts/plugin-sdk）+ `prost`/`serde_json`；
  render-worker 用 `nai-avatar-2d`/`nai-media-core`/`nai-plugin-sdk`/`nai-contracts`/`prost`。runtime 不引入 tokio/hyper（HTTP 启动留待 `apps/engine`）。

## composition root（crates/nai-runtime）

- `Composition::prepare(paths: DataPaths, workspace, actor, profile_name) -> Result<Composition>`：
  1. `Store::open`（单 owner，`db/owner.lock` 独占）→ `Arc<Mutex<Store>>`；
  2. 工作区引导：`create_workspace` + `set_permission(config.read/…)` 仅在显式 `bootstrap` 参数下执行，且注释声明这是可信 owner 管理，不得由模型/插件触发；
  3. 读取并校验 runtime 配置（`load_config(ConfigKind::Runtime, profile_name)`，schema 与 media/budget 交叉校验在 storage 内）；
  4. 由配置构造 `Session::new(workspace, session)`、`MediaGate`（用配置的 `lease_renew_ms/lease_expire_ms`，`with_epoch_floor`）、
     `Registry::new(boot_id, Limits)` 与 `ApprovalBackend::new(store.clone(), workspace, actor)`。
- 决策日志写入统一走 `Composition::record(kind, session, payload, trace_id)`：内部按 storage 的 `DecisionKind::criticality()`
  分级（critical 失败即返回错误，telemetry 允许进缓冲），每次用短 `Operation::new(5s)`（`Operation` 上限 30s，无长租表达）。
- 会话控制统一走 `Composition::control(action)`：先媒体侧 `gate.emergency().mute()` 再 `Session::revoke`
  （保持 ADR/既有约定「先撤权再落库」），随后写 critical 日志；ACK 语义不变（不代表设备已停）。
- 值守：`confirm_duty/enforce_duty/tick_duty` 包一层并在授予、到期、撤权时写 `duty_lease` critical 日志。

## 子进程监督（crates/nai-runtime::supervisor）

- `Supervisor` 持有 keeper 与 render-worker 的 `Child`，stdio 走 `nai_plugin_sdk::frame`（4 字节 LE 前缀 + Protobuf，≤64KiB）。
- `spawn_keeper(session, floor, frame_spec)`：`Command::new(env!("CARGO_BIN_EXE_…"))` 不可用于库内，二进制路径由调用方
  （测试/未来 engine）解析并显式传入；启动参数沿用 `--watchdog-drill <session> <floor> [--frame-drill …]`，
  池 FD 经 `.stderr(Stdio::from(file))` 继承（与既有 keeper 约定一致）。
- `arm(lease)/renew(lease)`：构造 `OutputWatchdogCommand`（seq 递增、`remaining_budget_ms ≤250`）投递；
  仅 Rehearsal 租约（与 keeper 一致）。
- `poll(deadline)`：非阻塞读取状态帧，更新 `lease_valid`/`presenting`/`standby_reason`/`epoch_floor`；
  读取失败或超界按故障处理。
- **故障处置（本轮核心）**：`observe()` 检测子进程退出/被强杀或状态读取失败 →
  1. `gate.emergency().mute()`（先静音，不等数据库/模型），
  2. `Session::revoke`（epoch 递增，旧任务与审批失效），
  3. 写 critical 决策日志（kind `cancellation`，payload 含子进程名、退出状态、epoch、原因），
  4. `standby = Some(StandbyReason::Retired | LeaseLost …)` 并拒绝继续投递。
  顺序与「先撤权、后落库」一致；日志写入失败不得让媒体侧静音回退。
- `shutdown()`：kill + 有界 wait 回收，避免僵尸与残留 FD。

## render-worker（apps/render-worker，显式 --render-drill）

- 生产装配形态（非设备验收）：读取 root 提供的**池 FD（stderr 继承，`/proc/self/fd/2` 只读重开用于校验）**、
  `--pcm <path>`（原始 PCM 字节，不走 JSON/Base64）与 `--ticket <path>`（root 已 reserve 的 `FrameSlotTicket` protobuf）；
- 用 `nai-avatar-2d` 按样本窗口生成真实 RGBA 像素，`write_frame` 写入池、`validate_producer_handle` 校验偏移；
- 向 stdout 输出 `FrameSlotEvent{write_started|committed|error}` 帧；
- 复检 epoch（`ticket.generation_epoch`）、取消与期限（`IoBudget`），失败即发 error 事件并以非零退出；
- 不产生 `PlaybackAck`、不声明 Ready、不接触设备。

## 测试

- runtime 单元/集成：配置装载与校验（非法配置拒绝、缺权限拒绝）；bootstrap 权限路径；decision-log 分级
  （critical 失败上报、telemetry 缓冲）；control 路径先静音再撤权并写日志；duty 授予/到期/撤权写日志。
- runtime ↔ keeper 真实子进程：spawn keeper → arm 租约 → 状态 `lease_valid` 可见；**强杀 keeper** →
  `observe()` 撤权（epoch 递增）、standby、critical 日志落库、`verify_decision_chain` 通过。
- runtime ↔ render-worker 真实子进程：reserve ticket → spawn render-worker（池 FD 继承）→ 收到 `write_started`/`committed`
  → owner `commit` 接受 → keeper 侧 `FramePath` 消费到 owned 像素（presenting）；篡改 SHA256 的负例被拒并退休整池。
- 依赖方向：`cargo xtask check-deps` 必须保持通过（kernel 不依赖 runtime；runtime 不引入 plugin→plugin 直接依赖）。
- 明确不做的验证：HTTP 控制服务启动、broker 装配接线、真实设备/窗口/音频、性能与长播。

## 风险

- 子进程二进制路径需调用方显式提供（跨包 `CARGO_BIN_EXE` 不可用）：以「显式路径 + 存在性校验」处理，缺失时返回明确错误而非静默跳过。
- 装配后仍无真实设备：所有「播放/静音完成」表述必须停留在软件状态，不得升级为设备证据。
