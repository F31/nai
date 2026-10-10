## 1. 立项与骨架

- [x] 1.1 OpenSpec 变更 `w34-device-admission`（proposal/design/tasks + `device-admission` 增量），`validate --strict` 通过。
- [x] 1.2 落点确认：放进 `crates/nai-kernel`（`device.rs`；kernel 只允许依赖 contracts），不放进 `nai-media-core`（媒体装配，且会反向依赖 kernel 报告词汇）、不放进 `nai-upgrade-core`（安装资源检查，非设备声明）、不新开 crate——18 包与依赖方向不变；复用 `golden::Finding`，**不新增令牌类型、不复制实现、零新增依赖**。

## 2. 声明词汇与动作（O04）

- [x] 2.1 `DeviceChannel::{Sense, Control}`、`RiskClass::{Risk1, Risk2, Risk3}`（`Ord`，供 `max`）、`ValueRange{min_milli, max_milli}`、`DeviceAction{name, range, rate_limit_per_min}`、`DeviceDeclaration{device_ref, channel, physical_execution, enabled_by_default, local_confirmation, independent_switch, actions, safety_interlock, local_manual_override, declared_risk}`。
- [x] 2.2 审计 `no_declared_actions`、`action_without_value_range`、`action_without_rate_limit`（后两条指名设备与动作）。
- [x] 2.3 测试：空动作枚举 → 报出并指名；某动作缺取值范围/速率限制 → 逐条报出并指名；齐备时不报。

## 3. 安全联锁、本地覆盖与物理执行（O04）

- [x] 3.1 `Control` 缺安全联锁 ⇒ `control_without_safety_interlock`；缺本地手动覆盖 ⇒ `control_without_local_override`。
- [x] 3.2 物理执行类：`enabled_by_default` ⇒ `physical_enabled_by_default`；缺本地确认 ⇒ `physical_without_local_confirmation`；缺独立开关 ⇒ `physical_without_independent_switch`；`Sense` 标注物理执行 ⇒ `sense_channel_with_physical_execution`。
- [x] 3.3 测试：`Control` 缺两项逐条报出；**纯 `Sense` 缺两项不报**；物理执行类默认启用/缺确认/缺开关逐条报出；`Sense` 标注物理执行报出。

## 4. 有效风险与放置（O04）

- [x] 4.1 `derived_risk()`（`Control`→`Risk2`、物理执行类→`Risk3`、`Sense`→`Risk1`）、`effective_risk() = max(declared, derived)`；声明低于推导 ⇒ `declared_risk_below_derived`。
- [x] 4.2 `DeviceHostPlacement::{Subprocess, EdgeNode, KernelProcess}`；`KernelProcess` ⇒ `device_driver_in_kernel_process`。
- [x] 4.3 测试：物理执行类声明 `risk_2` → 报出且有效风险为 `risk_3`；`Control` 声明 `risk_3` → 不报且有效风险 `risk_3`；驱动在内核进程 → 报出。

## 5. 事件处置与决策日志（O04 / §8.5）

- [x] 5.1 `DeviceEvent::{Hotplug, Disconnect, Duplicate, Timeout}`、`EventDisposition{event, handled, logged, keeps_single_capture_path}`；缺某类 ⇒ `device_event_undisposed`；不记录 ⇒ `device_event_not_logged`（均指名事件类）。
- [x] 5.2 `Duplicate` 处置会打开第二条有效收音路径 ⇒ `duplicate_event_opens_second_capture_path`（O04「音频设备变更不重复收音」、§8.5）。
- [x] 5.3 测试：缺「超时」处置 → 报出并指名；「断线」不记录 → 报出并指名；「重复」开第二条路径 → 报出；「重复」保持单一路径且已记录 → 不报。

## 6. 实际使用与授权（INV-23）

- [x] 6.1 `DeviceUse{device_ref, action}`、`DeviceGrant{device_ref, actions}`；使用未声明动作 ⇒ `used_undeclared_device_action`；已声明但未授权 ⇒ `used_beyond_authorization`；**同一处使用不重复报**（先判未声明，命中即止）。
- [x] 6.2 测试：未声明动作 → 报出并指名；已声明未授权 → 报出并指名；未声明且未授权 → 只报未声明；声明+授权内使用 → 不报。

## 7. readiness、审计与文档、提交（O04）

- [x] 7.1 `HostReadiness::{Blocked, Defective, Ready}`；`DeviceHost::readiness()` 先取审计（非空即 `Defective`），再按是否有声明（无 ⇒ `Blocked`）否则 `Ready`。
- [x] 7.2 `audit_host(&DeviceHost) -> Vec<Finding>` 报出全部命中；**十六条**稳定标记逐一可在测试里直接构造；干净描述符审计为空；三份构造描述符审计并集**恰为**十六条。
- [x] 7.3 新增 `docs/device-admission.md`（声明词汇、执行类联锁与覆盖、物理执行类、有效风险、放置边界、事件处置与决策日志、重复收音、越权使用、readiness、审计标记、未做边界）。
- [x] 7.4 新增 ADR 0036，记录落进 kernel 的理由与十二项决策的否决方案（落 media-core / 落 upgrade-core / 只报不完整 / 所有设备都要联锁 / 用声明或用推导风险 / 放置不可判 / 只报「处置了」 / 复用 CaptureRegistry / 判授权而非使用 / 无声明即缺陷或即就绪 / 臆造证据目录）。
- [x] 7.5 报告 `docs/reports/2026-10-10-w34-device-admission.md`：实测数字、逐项验收映射（O04，安装侧回指 `nai-upgrade-core`）、未覆盖边界与环境边界。
- [x] 7.6 `tests/registry.yaml`：O04 **保持 `blocked`**，notes 补记 W34 已交付**设备侧**判定机制、**安装侧已由 `nai-upgrade-core` 覆盖**并保留三条验收口径；`docs/development-plan.md` 与 `AGENTS.md` 记录本轮。`check --registry` 通过。
- [x] 7.7 归档 `w34-device-admission`（归档名 `2026-10-10-w34-device-admission`，新 capability `device-admission` 生效），跑 `validate --all --strict`（含 `--archived`）。
- [x] 7.8 全量 `cargo xtask check --profile portable` 通过并留档；提交 + 推送。

## 里程碑切分

本轮为单个可独立验证的切片：声明词汇与动作（§2）、联锁/覆盖/物理执行（§3）、风险与放置（§4）、事件处置与决策日志（§5）、使用与授权（§6）共用同一个 `device-admission` capability——§7 的审计逐条检查的正是 §2–§6 产出的描述符字段，拆开提交会让审计的第一版无法验证「每条规则都直接可构造」。文档、ADR、registry、归档与门控（§7）为纯声明变更，沿用同一次门控留档。

**顺序纪律**：先跑门控 → 再勾任务项 → 最后归档（`validate --all --archived` 会检查归档 tasks 完成度）。
