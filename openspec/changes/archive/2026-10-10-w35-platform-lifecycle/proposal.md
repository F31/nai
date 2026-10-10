## Why

`tests/registry.yaml` 里 **P01**（平台生命周期，R2）是 `blocked`，notes 至今只有一句话：**「官方 SDK/生产宿主未实现（D13）」**。P01 的验收（§1328）有两条：**伴侣启动、退出、重连、切场次不产生旧房间播报**；**权限撤销停止相关功能**。

D13 已交付的是**平台事件准入**（`nai-kernel::platform` 的 `PlatformLink`：可信 root 认证、房间来源、权限快照、连接代次、断线清权限、重连要求新 revision、事件来源/房间/epoch/代次/时间校验）。**但那条线判的是「一个事件能不能进」，不是「桥接进程的生命周期**」——§27.1 与 §347 对后者给了一组很密的判据，此前**全库没有任何落点**（`grep bridge` 在 kernel 内为空）：

- **「桥接只做：SDK 初始化、官方事件与权限转换、与引擎的认证 IPC、退出」（§27.1）** —— 这是一条**封闭职责集**，是一条「只做」的否定式。
- **「伴侣退出时桥接退出，引擎转待机；重连不恢复旧自动权限。需处理多次启动、端口占用、SDK 失联、消息重复、旧房间事件、时间漂移、权限撤销」（§347）** —— 七类**需处理**情形，加两条生命周期不变量（退出转待机、重连不恢复旧自动权限）。
- **「平台能力快照取值 granted、denied、unknown，取最严格交集。缺失能力只关闭对应功能，不自行合成事件」（§349）** —— 正是「权限撤销停止**相关**功能」的精确口径：不是全停，也不是不停。
- **「平台事件中断，自动互动撤权；手工与回放仍可用；重连后不恢复旧自动权限」（§119）** —— 一个三态否定式：撤**自动**、保**手工**、保**回放**。
- **「SDK 回调快速入队，不同步等待 LLM 或磁盘大事务」（§27.1）**。
- **「握手检查路径、签名、nonce 与有效令牌；官方 SDK 库不能从任意路径加载」（§27.1、§1075）**。
- **「没有官方接口文档或 SDK 时返回 `UNVERIFIED_EXTERNAL_DEPENDENCY`；演练走 manual 与 replay，不用 fake connector 冒充真实能力」（§27.1）** —— 该错误码在整个 workspace 内**从未被使用**。

不把这些落成可执行判据，「平台生命周期」在执行层就仍是一句无从判定的要求。

## What Changes

- **桥接职责是封闭四项，越界即报（`BridgeDuty` / `BridgeScope`，P01）**：`SdkInit`/`EventPermissionTranslation`/`AuthenticatedIpc`/`Exit`（§27.1「桥接只做」）。声明四项之外的额外职责 ⇒ `duty_outside_bridge_scope`，逐条指名。
- **四类生命周期转换各有处置、各清授权、各记录（`BridgeTransition` / `TransitionDisposition`，P01）**：`Started`/`Exited`/`Reconnected`/`RoomSwitched`（§1328）。缺某类处置 ⇒ `transition_undisposed`；处置**不清除播报授权** ⇒ `transition_keeps_stale_broadcast`；不记录 ⇒ `transition_not_logged`（均指名该类）。
- **旧房间永不播报（`BroadcastAuthority`，P01）**：授权绑定**房间与连接代次**；`may_broadcast` 仅当授权覆盖当前连接；存在与当前连接不一致的授权 ⇒ `stale_room_broadcast_permitted`（§1328「不产生旧房间播报」）。
- **伴侣退出后引擎转待机（`TransitionDisposition`，P01）**：「退出」转换的处置未声明引擎转待机 ⇒ `exit_without_standby`（§347）。
- **重连不恢复旧自动权限（P01）**：「重连」处置声明恢复旧自动权限 ⇒ `reconnect_restores_previous_auto_authority`（§347、§119）。
- **能力快照取最严格交集、只关闭对应功能（`CapabilityState` / `Feature`，P01）**：`unknown` 视为**未授予**，读作已授予 ⇒ `unknown_capability_treated_as_granted`（§349「取最严格交集」）；功能仍启用而所需能力非 granted ⇒ `feature_active_without_capability`（缺失能力只关闭对应功能）；功能**因能力变更被停止**而所需能力仍 granted ⇒ `feature_stopped_despite_capability`（不越界关闭）。
- **平台中断撤自动、保手工与回放（`Degradation`，P01）**：不撤自动 ⇒ `platform_interruption_keeps_auto_reply`；停手工 ⇒ `platform_interruption_disables_manual`；停回放 ⇒ `platform_interruption_disables_replay`；不记录 ⇒ `degradation_not_logged`（§119、§27.1）。
- **七类情形各有处置且记录（`BridgeCondition` / `ConditionDisposition`，P01）**：`MultipleStarts`/`PortConflict`/`SdkLost`/`DuplicateMessages`/`StaleRoomEvent`/`TimeDrift`/`PermissionRevoked`（§347）。缺处置 ⇒ `condition_undisposed`；不记录 ⇒ `condition_not_logged`（均指名该类）。
- **官方 SDK 只经固定路径与握手校验加载（`SdkTrust`，P01）**：声明 SDK 已核验而缺路径固定/签名/nonce/有效令牌任一项 ⇒ `sdk_loaded_without_trust_check`，逐项指名（§27.1、§1075）；如实声明无 SDK 时**不判**信任检查。
- **无官方 SDK 时如实记 `UNVERIFIED_EXTERNAL_DEPENDENCY`，不冒充（`SdkAvailability`，P01）**：该状态下用 fake connector 冒充真实能力 ⇒ `fake_connector_claimed_as_real_capability`（§27.1）。
- **SDK 回调不同步等待 LLM 或磁盘（`CallbackPolicy`，P01）**：⇒ `callback_blocks_on_llm_or_disk`（§27.1）。
- **readiness 三态（`BridgeReadiness`，P01）**：未声明桥接职责 ⇒ `Blocked`（P01 诚实状态）；有任一违规 ⇒ `Defective` 且**优先**；声明齐备且无违规 ⇒ `Ready`。
- **审计是描述符的纯函数（`audit_bridge`，P01）**：**十九条**稳定标记，每条都可直接构造；干净描述符审计为空。

## Capabilities
### New Capabilities
- `platform-lifecycle`：§27.1 桥接封闭职责与 SDK 加载/回调纪律、§347 伴侣生命周期与七类需处理情形、§349 能力快照最严格交集与「只关闭对应功能」、§119 平台中断撤自动保手工与回放、§1328 P01 验收的判定口径——职责封闭、四类转换处置与清授权与记录、陈旧房间播报抑制、退出转待机、重连不恢复旧自动权限、能力快照三态与功能开关两个方向、平台中断三态、七类情形处置、SDK 信任检查、`UNVERIFIED_EXTERNAL_DEPENDENCY` 诚实性与不冒充、回调不阻塞、readiness 三态、描述符纯函数审计。覆盖 registry **P01**。

### Modified Capabilities
- 无。既有 `nai-kernel::platform`（`PlatformLink`，D13）判的是**事件准入**（来源/房间/authority/epoch/代次/时间/权限），本变更判的是**桥接进程生命周期与播报抑制**；两者互补、不重叠，本变更**不改** `platform.rs`。既有 `nai-kernel::session`（会话生命周期状态机、duty lease、revoke）判的是**会话**与**值守租约**，本变更判的是**伴侣桥接**；两者互补，本变更**不改** `session.rs`。既有 `nai-kernel::device`（O04 设备接入）与 `monitoring`（O03 手机监控面）**不改**。

## Impact

- 代码：`crates/nai-kernel/src/` 新增 `bridge.rs`；`tests/` 新增 `bridge.rs`；`lib.rs` 增一个 `pub mod`。复用 `golden::Finding`（`Finding::new` 已为 `pub`），**不新增令牌类型、不复制实现**。**不新增 crate，不改 workspace 依赖**（维持 18 包与依赖方向；kernel 仍只依赖 contracts）。
- 规格：新增 capability `platform-lifecycle`；新增 `docs/platform-lifecycle.md`、ADR 0037、报告 `docs/reports/2026-10-10-w35-platform-lifecycle.md`。
- 依赖：**零新增**。仅用 `std`。
- registry：**P01 保持 `blocked`**（官方 SDK、生产 composition root 平台宿主接线、真实启动/退出/端口冲突与网关是外部真值），notes 补记 W35 已交付判定机制、并**划清与 D13 已交付的平台事件准入**（`nai-kernel::platform`）的边界。不新增编号（registry id 正则为恰好两位数字）。
- 明确不做：不启动伴侣、不加载官方 SDK、不建网络连接、不推流、不做真实握手、不做网关与回调服务、不改 `platform.rs`/`session.rs`、不做 UI；不引入 `check --profile portable` 的新命令（§30.1 未为平台生命周期指定证据目录，臆造 `tests/lifecycle` 属过度绑定）。

## Non-Goals

- 不用机制替代真实交付：本轮证明的是「职责封闭可判、四类转换与清授权可判、陈旧房间播报可判、退出转待机可判、重连不恢复自动权限可判、能力快照与功能开关可判、平台中断三态可判、七类情形处置可判、SDK 信任与不冒充可判、未接入不被读成缺陷」，**不宣称伴侣启动/退出/重连/切场次已验收，也不宣称官方 SDK 或生产宿主已就绪**。P01 保持 `blocked`。
- 不替 D13 复判事件准入：事件来源/房间/权限/epoch/代次/时间校验仍在 `PlatformLink`；本模块判**生命周期**，不重复其判据。
- 不替真实平台做验收：真实 SDK 认证、账户/用途批准、Windows 直播伴侣环境与端口冲突处置无法执行就标 Blocked（§1261「GPU 与真实 SDK 验收在受控机器上单独执行……无法执行就标 Blocked，不写成 CI Passed」）。
- 不把「尚未接入」变成缺陷，也不让「尚未接入」掩盖越界或冒充。
- 不把「桥接只做四项」读成「桥接什么都做不了」：本模块判的是**声明越界**，不否定桥接转发事件与转换权限本身。
