# 设备接入与安全联锁的判定骨架（O04）

代码：`crates/nai-kernel/src/device.rs`；测试：`crates/nai-kernel/tests/device.rs`；
决策：[ADR 0036](adr/0036-device-admission-boundary.md)。

本模块把 §34.4「硬件接入原则」、§1469 `device.sense@1`/`device.control@1`、§35.3 有效风险、INV-23 宿主强制与 §8.5 单一收音路径的**判定口径**落成在 portable 下可执行、可测试的骨架。它**不打开**任何设备、不枚举硬件、不做真实驱动或 SDK 宿主、不做 PCM 混音——那些由外部条件（受控 Windows 机器与真实音频设备）提供；它负责的是「给定一份**声明的设备接入**，它是否越过声明、风险、放置、处置与授权的边界」。

因此 `tests/registry.yaml` 的 **O04 保持 `blocked`**：§1329 的验收原文是「干净 Windows 安装且运行库可取得；音频设备变更不重复收音；升级可回退」，这些是外部真值。本文档描述的是**判定机制**，不是交付结论。

> **安装侧已另有落点**：O04 三条验收中，**干净 Windows 安装且运行库可取得**由 `nai-upgrade-core` 的 `manifest`（`RuntimeEntry`/`os`/`architectures`/`hosts`）与 `preflight`（平台、内核范围、依赖闭包、runtime 矩阵、磁盘、凭据）覆盖；**升级可回退**由同 crate 的 `plan.rs`（journal、`RollbackReport`、`RecoveryAction`）与 `gates.rs`（`require_rollback_compatible`/`downgrade`）覆盖。本模块补的是此前**没有任何落点**的**设备侧**（§1424 缺口清单原文：**「没有设备与硬件接入的抽象，也没有物理安全联锁。」**）。

## 设备能力靠声明，缺项指名

§1469：`device.sense@1` 与 `device.control@1` **声明动作枚举、取值范围、速率限制与安全联锁**。

| 缺失 | 出处 | 报出 |
| --- | --- | --- |
| 动作枚举为空 | §1469 声明动作枚举 | `no_declared_actions`（指名设备） |
| 某动作未声明取值范围 | §1469 取值范围 | `action_without_value_range`（指名设备/动作） |
| 某动作未声明速率限制 | §1469 速率限制 | `action_without_rate_limit`（指名设备/动作） |

## 执行类必须联锁与可本地覆盖；纯传感不适用

§1469、§1497：**安全联锁**与**本地手动覆盖**是必需项。

| 通道 | 缺安全联锁 | 缺本地手动覆盖 |
| --- | --- | --- |
| `Control` | `control_without_safety_interlock` | `control_without_local_override` |
| `Sense` | **不报** | **不报** |

安全联锁联锁的是**动作**，本地手动覆盖覆盖的是**执行**；纯传感器没有可联锁/可覆盖的对象。对只读传感强制这两项是把**不适用**读成**不合规**——与 W33 预览未启用、本模块事件处置的同类判断一致。

## 物理执行类：risk_3、默认禁用、需本地确认与独立开关

§1469「物理执行类默认 risk_3，需要本地确认与独立开关」+ §34.4「物理执行类设备默认禁用，启用需管理员本机确认」：

| 条件 | 报出 |
| --- | --- |
| `physical_execution && enabled_by_default` | `physical_enabled_by_default` |
| `physical_execution && !local_confirmation` | `physical_without_local_confirmation` |
| `physical_execution && !independent_switch` | `physical_without_independent_switch` |
| `channel == Sense && physical_execution` | `sense_channel_with_physical_execution` |

## 有效风险取 max，「声明低于推导」即拒

§35.3：**有效风险 = max(声明风险, 由 side_effect 与权限推导的风险)；声明低于推导的，上架审核与加载时都拒绝**。§1469：`control` 默认 `risk_2`；**物理执行类**默认 `risk_3`。

| 通道 / 物理执行 | `derived_risk` |
| --- | --- |
| 物理执行（任意通道） | `risk_3` |
| `Control` | `risk_2` |
| `Sense` | `risk_1` |

`declared_risk < derived_risk` ⇒ `declared_risk_below_derived`；`effective_risk = max(声明, 推导)`——声明方**可以**主动提高风险（更保守），但**不能**自我降级。

## 驱动与 SDK 不进内核进程

§34.4「设备驱动与 SDK 放在 subprocess 或边缘节点，**不进内核进程**」。`DeviceHostPlacement::{Subprocess, EdgeNode, KernelProcess}`；`KernelProcess` ⇒ `device_driver_in_kernel_process`。

> 注意区分：这条说的是**运行时 I/O 的放置**。「判定策略放在 kernel crate」与它不矛盾——本模块判的是**声明与处置**，不打开设备。

## 四类事件各有处置、写决策日志、不重复收音

§1498「设备热插拔、断线、重复与超时要有明确处置；测试使用模拟设备」；§1497「设备事件写入决策日志」；§8.5「只允许一条有效收音路径，避免 OBS 与伴侣重复收音造成回声」（= O04「音频设备变更不重复收音」）。

| 事件类 | 缺处置 | 未记录 | 重复收音 |
| --- | --- | --- | --- |
| `Hotplug`/`Disconnect`/`Duplicate`/`Timeout` | `device_event_undisposed`（指名） | `device_event_not_logged`（指名） | 仅 `Duplicate`：`duplicate_event_opens_second_capture_path` |

- 「处置了」**不等于**「不会开第二条路径」——§8.5 的要求是**结果**（只允许一条有效收音路径），故判到 `keeps_single_capture_path`。
- **条件化**：未声明任何设备接入时**不存在设备事件**，这些规则一律不产生缺陷（否则 O04 尚未接入真实设备的诚实状态会被读成四个缺陷）。
- 断线对应契约里的 `MediaState::DEVICE_LOST` 语义；本模块只判处置是否存在与是否记录。

## 实际使用不越声明与授权（INV-23）

INV-23：**扩展的实际权限不得超出清单声明与用户授权；……设备能力由宿主强制执行，不靠自觉。**

| 情况 | 报出 |
| --- | --- |
| 使用了声明里没有的设备动作 | `used_undeclared_device_action`（指名设备/动作） |
| 动作已声明，但未被用户授权 | `used_beyond_authorization`（指名设备/动作） |
| 既未声明也未授权 | **只报未声明**（同一处使用不重复报） |

真实「宿主强制执行、不靠自觉」的运行时拦截属宿主与集成验收；本模块判的是**越界事实可被审计复核**。

## readiness 三态

| 条件（按序） | `HostReadiness` |
| --- | --- |
| 审计非空（任一违规） | `Defective` |
| 审计为空且未声明任何设备接入 | `Blocked` |
| 审计为空且已声明设备接入 | `Ready` |

**先取审计**——未接入不掩盖越界（`Blocked` 的判据是**审计为空且无声明**）。`Blocked` 是 O04 现在**诚实**的阻塞状态，不写成缺陷（§1261「无法执行就标 Blocked，不写成 CI Passed」）。

## 审计是描述符的纯函数

`audit_host(&DeviceHost)` 对声明字段做纯函数，报出**全部**命中项，使每条规则都能被直接构造：

| token | 含义 |
| --- | --- |
| `device_driver_in_kernel_process` | 驱动/SDK 被声明在内核进程 |
| `no_declared_actions` | 动作枚举为空（指名设备） |
| `action_without_value_range` | 动作未声明取值范围（指名设备/动作） |
| `action_without_rate_limit` | 动作未声明速率限制（指名设备/动作） |
| `control_without_safety_interlock` | 执行类缺安全联锁 |
| `control_without_local_override` | 执行类缺本地手动覆盖 |
| `declared_risk_below_derived` | 声明风险低于推导风险 |
| `sense_channel_with_physical_execution` | 传感通道标注物理执行 |
| `physical_enabled_by_default` | 物理执行类默认启用 |
| `physical_without_local_confirmation` | 物理执行类缺本地确认 |
| `physical_without_independent_switch` | 物理执行类缺独立开关 |
| `device_event_undisposed` | 某事件类无明确处置（指名事件类） |
| `device_event_not_logged` | 事件处置未写决策日志（指名事件类） |
| `duplicate_event_opens_second_capture_path` | 重复设备打开第二条有效收音路径 |
| `used_undeclared_device_action` | 使用了未声明的设备动作（指名设备/动作） |
| `used_beyond_authorization` | 使用超出用户授权（指名设备/动作） |

`every_finding_token_is_constructible` 断言这**十六条**恰好等于一份最大违规描述的审计并集——既保证每条可构造，也保证没有未被登记的额外标记。

## 未做边界

- **不含任何真实交付**：设备驱动、SDK 宿主、WASAPI 路由、PCM 混音、回声测试、真实音频设备枚举都由外部提供。测试里的描述符是**构造出来的**，只为证明越界口径与 readiness 派生可执行。
- **不重复 `preflight` 的安装检查**：运行库/平台/依赖闭包/磁盘/凭据由 `nai-upgrade-core::preflight` 判；本模块不重复。
- **不重复 `CaptureRegistry` 的租约互斥**：`nai-media-core::mixing` 的 `CaptureRegistry` 判的是**收音租约时序**（申请/释放/旧租约拒绝、默认设备被观察而不被静默跟随）；本模块判的是**事件处置**。两者互补，本模块**不改** `mixing.rs`。
- **不新增 xtask 命令与证据目录**：§30.1 为升级指定 `tests/upgrade`、为 RAG 黄金集指定 `tests/rag-golden`、为 UI 指定 `tests/ui`；**设备接入没有证据目录约定**，其真值来自**受控 Windows 机器与真实音频设备**，不是仓库载荷。**臆造** `tests/device` 属过度绑定。待真实设备轨立项后再补载体，**无需改本模块判定**。
- 注册表 **O04 保持 `blocked`**：判定机制（W34，设备侧）已交付并在 notes 中保留；安装侧另有 `nai-upgrade-core` 覆盖。
