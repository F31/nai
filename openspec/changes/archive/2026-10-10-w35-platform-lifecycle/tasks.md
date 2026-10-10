## 1. 立项与骨架

- [x] 1.1 OpenSpec 变更 `w35-platform-lifecycle`（proposal/design/tasks + `platform-lifecycle` 增量），`validate --strict` 通过。
- [x] 1.2 落点确认：放进 `crates/nai-kernel`（`bridge.rs`；kernel 只允许依赖 contracts），不并进既有 `platform.rs`（事件准入，另一面）、不并进 `session.rs`（会话状态机与值守租约，另一实体）、不放进 `nai-plugin-hosts`（IPC/ACL/边界，且会把报告词汇拖进宿主层）、不新开 crate——18 包与依赖方向不变；复用 `golden::Finding`，**不新增令牌类型、不复制实现、零新增依赖**。

## 2. 桥接职责封闭集与 SDK 纪律（P01）

- [x] 2.1 `BridgeDuty::{SdkInit, EventPermissionTranslation, AuthenticatedIpc, Exit}`（`ALL`、`label`）、`BridgeScope{duties, extra_duties}`；`extra_duties` 非空 ⇒ `duty_outside_bridge_scope`，逐条指名。
- [x] 2.2 `SdkTrust{path_pinned, signature_checked, nonce_checked, token_valid}`（`is_complete`/`missing`）、`SdkAvailability::{Verified, UnverifiedExternalDependency}`；`Verified` 而任一项缺失 ⇒ `sdk_loaded_without_trust_check`，逐项指名；`UnverifiedExternalDependency` 时**不判**信任检查。
- [x] 2.3 `CallbackPolicy{enqueues_without_blocking, awaits_llm_or_disk}`；同步等待 ⇒ `callback_blocks_on_llm_or_disk`。
- [x] 2.4 测试：四项之内不报、越界逐条指名；`Verified` 缺签名/路径固定/nonce/令牌逐项报出、齐备不报；`Unverified` 不报信任缺陷；回调同步等待报出、只入队不报。

## 3. 生命周期转换与旧房间播报抑制（P01）

- [x] 3.1 `BridgeTransition::{Started, Exited, Reconnected, RoomSwitched}`（`ALL`、`label`）、`TransitionDisposition{transition, clears_broadcast_authority, logged, leaves_engine_in_standby, restores_previous_auto_authority}`；缺某类 ⇒ `transition_undisposed`；不清授权 ⇒ `transition_keeps_stale_broadcast`；不记录 ⇒ `transition_not_logged`（均指名该类）。
- [x] 3.2 `CurrentConnection{room, connection_generation}`、`BroadcastAuthority{room, connection_generation}`（`covers`）；`BridgeDescriptor::may_broadcast()` 仅当授权覆盖当前连接；不符 ⇒ `stale_room_broadcast_permitted`。
- [x] 3.3 `Exited` 处置未声明引擎转待机 ⇒ `exit_without_standby`；`Reconnected` 处置声明恢复旧自动权限 ⇒ `reconnect_restores_previous_auto_authority`。
- [x] 3.4 测试：缺某类处置逐条报出；不清授权报出；不记录报出；**未声明桥接职责时不报转换缺陷**；陈旧房间授权报出且 `may_broadcast` 为假；一致授权不报且 `may_broadcast` 为真；退出非待机报出；重连恢复旧自动权限报出。

## 4. 能力快照与功能开关（P01 / §349）

- [x] 4.1 `CapabilityState::{Granted, Denied, Unknown}`、`CapabilityGrant{capability, state, read_as_granted}`；`Unknown` 被读作已授予 ⇒ `unknown_capability_treated_as_granted`，指名该能力。
- [x] 4.2 `Feature{name, requires, active, stopped_by_capability_change}`；`active` 而所需能力非 `Granted`（`Unknown` 视为未授予）⇒ `feature_active_without_capability`；`stopped_by_capability_change` 而所需能力仍 `Granted` ⇒ `feature_stopped_despite_capability`（均指名该功能）。
- [x] 4.3 测试：`unknown` 读作已授予报出；功能开着而能力 `denied`/`unknown` 报出、能力 `granted` 不报；功能因能力变更停止而能力仍 `granted` 报出；用户主动关闭（非能力变更）不报。

## 5. 平台中断三态与七类情形（P01 / §119 / §347）

- [x] 5.1 `Degradation{cause, bridge_lost, auto_reply_revoked, manual_available, replay_available, logged}`；`bridge_lost` 为真时不撤自动 ⇒ `platform_interruption_keeps_auto_reply`、停手工 ⇒ `platform_interruption_disables_manual`、停回放 ⇒ `platform_interruption_disables_replay`、不记录 ⇒ `degradation_not_logged`；**非平台中断不因此报出**。
- [x] 5.2 `BridgeCondition` 取 §347 原文七类（`MultipleStarts`/`PortConflict`/`SdkLost`/`DuplicateMessages`/`StaleRoomEvent`/`TimeDrift`/`PermissionRevoked`）、`ConditionDisposition{condition, handled, logged}`；缺某类 ⇒ `condition_undisposed`；不记录 ⇒ `condition_not_logged`（均指名该类）。
- [x] 5.3 测试：中断不撤自动/停手工/停回放/不记录逐条报出；三态齐备且记录不报；非 `bridge_lost` 降级不报；七类缺处置逐条报出、不记录报出、齐备不报。

## 6. 描述符、readiness 与审计（P01）

- [x] 6.1 `BridgeDescriptor`（scope、transitions、connection、authority、capabilities、features、degradations、conditions、sdk_trust、sdk_availability、uses_fake_connector_claimed_real、callback）与 `BridgeDescriptor::empty()`。
- [x] 6.2 `BridgeReadiness::{Blocked, Defective, Ready}`；`readiness()` 先取审计（非空即 `Defective`），再按是否有桥接职责（无 ⇒ `Blocked`）否则 `Ready`。
- [x] 6.3 `audit_bridge(&BridgeDescriptor) -> Vec<Finding>` 报出全部命中；**十九条**稳定标记逐一可在测试里直接构造；干净描述符审计为空；构造描述的审计并集**恰为**十九条。
- [x] 6.4 `uses_fake_connector_claimed_real` 且 `SdkAvailability::UnverifiedExternalDependency` ⇒ `fake_connector_claimed_as_real_capability`；如实记录不报。
- [x] 6.5 测试：空描述符 `Blocked` 且审计为空；空描述符 + 陈旧房间授权 ⇒ `Defective`（优先于 `Blocked`）；齐备描述符 `Ready`；干净描述符审计为空；逐标记可构造；并集恰为十九条。

## 7. 文档、registry、归档与提交（P01）

- [x] 7.1 新增 `docs/platform-lifecycle.md`（职责封闭集、四类转换与清授权、陈旧房间抑制、退出转待机、重连不恢复旧自动权限、能力快照最严格交集与功能双向、平台中断三态、七类情形、SDK 信任与不冒充、回调不阻塞、readiness、审计标记、与 `platform.rs`/`session.rs` 的边界、未做边界）。
- [x] 7.2 新增 ADR 0037，记录落进 kernel 的理由与 D1–D14 决策的否决方案（并进 platform.rs / 并进 session.rs / 落 plugin-hosts / 只记职责不判越界 / 只判处置面 / 单向判功能 / `unknown` 参与已授予 / 无条件要求握手校验 / 无条件撤自动 / 无声明即缺陷或即就绪 / 臆造证据目录）。
- [x] 7.3 报告 `docs/reports/2026-10-10-w35-platform-lifecycle.md`：实测数字、逐项验收映射（P01，与 D13 平台事件准入的边界）、未覆盖边界与环境边界。
- [x] 7.4 `tests/registry.yaml`：**P01 保持 `blocked`**，notes 补记 W35 已交付判定机制并**划清与 D13 已交付的事件准入**的边界、保留两条验收口径；`docs/development-plan.md` 与 `AGENTS.md` 记录本轮。`check --registry` 通过。
- [x] 7.5 归档 `w35-platform-lifecycle`（归档名 `2026-10-10-w35-platform-lifecycle`，新 capability `platform-lifecycle` 生效），跑 `validate --all --strict`（含 `--archived`）。
- [x] 7.6 全量 `cargo xtask check --profile portable` 通过并留档；提交 + 推送。

## 里程碑切分

本轮为单个可独立验证的切片：职责与 SDK 纪律（§2）、生命周期与播报抑制（§3）、能力快照与功能开关（§4）、平台中断与七类情形（§5）共用同一个 `platform-lifecycle` capability——§6 的审计逐条检查的正是 §2–§5 产出的描述符字段，拆开提交会让审计的第一版无法验证「每条规则都直接可构造」。文档、ADR、registry、归档与门控（§7）为纯声明变更，沿用同一次门控留档。

**顺序纪律**：先跑门控 → 再勾任务项 → 最后归档（`validate --all --archived` 会检查归档 tasks 完成度）。
