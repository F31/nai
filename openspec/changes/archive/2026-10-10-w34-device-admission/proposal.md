## Why

`tests/registry.yaml` 里 **O04**（安装与设备，R1）是 `blocked`，notes 至今只有一句话：**「Windows 安装未执行」**。O04 的验收（§1329）有三条：**干净 Windows 安装且运行库可取得**、**音频设备变更不重复收音**、**升级可回退**。

其中两条已有落点：安装侧的**运行库/平台/依赖闭包/磁盘/凭据**检查由 `nai-upgrade-core` 的 `manifest`（`RuntimeEntry`/`os`/`architectures`）与 `preflight`（平台、内核范围、依赖闭包、runtime 矩阵、磁盘、凭据）覆盖，**升级可回退**由 `plan.rs`（journal、`RollbackReport`、`RecoveryAction`）与 `gates.rs`（`require_rollback_compatible`/`downgrade`）覆盖。

**但「设备」这一半此前全库没有任何落点**，而 §33.1 缺口清单把它列为一条明确的缺失：**「没有设备与硬件接入的抽象，也没有物理安全联锁。」**（§1424）规格却给了它一组很密的判据，不落就会含糊过去：

- **「设备能力声明、安全联锁与本地手动覆盖是必需项；设备事件写入决策日志」（§1497）** —— 这是三条必需的**声明项**，不是一句原则。
- **「设备热插拔、断线、重复与超时要有明确处置；测试使用模拟设备」（§1498）** —— 四类事件各有处置，缺一类就是漏一类。
- **「设备驱动与 SDK 放在 subprocess 或边缘节点，不进内核进程」（§34.4）** —— 这是一条**放置**边界。
- **「物理执行类设备默认禁用，启用需管理员本机确认」（§34.4）** —— 默认值与确认动作都可判。
- **`device.sense@1`/`device.control@1`「声明动作枚举、取值范围、速率限制与安全联锁」（§1469）** —— 声明字段齐备性可判。
- **「有效风险 = max(声明风险, 由 side_effect 与权限推导的风险)；声明低于推导的，上架审核与加载时都拒绝」（§35.3）** —— 声明风险低于推导即拒。
- **「扩展的实际权限不得超出清单声明与用户授权；……设备能力由宿主强制执行，不靠自觉」（INV-23）** —— 实际使用越界可判。
- **「只允许一条有效收音路径，避免 OBS 与伴侣重复收音造成回声」（§8.5）** —— 正是 O04「音频设备变更不重复收音」的判据。

不把这些落成可执行判据，「设备接入」在执行层就仍是一句无从判定的要求。

## What Changes

- **设备能力靠声明，不靠假设（`DeviceDeclaration` / `DeviceAction`，O04）**：`device.sense@1`/`device.control@1` 声明**动作枚举、取值范围与速率限制**（§1469）。动作枚举为空 ⇒ `no_declared_actions`；某动作缺取值范围或速率限制 ⇒ `action_without_value_range` / `action_without_rate_limit`，均指名设备与动作。
- **执行类必须联锁与可本地覆盖（`DeviceDeclaration`，O04）**：`Control` 缺**安全联锁** ⇒ `control_without_safety_interlock`（§1469、§1497）、缺**本地手动覆盖** ⇒ `control_without_local_override`（§1497）。**纯 `Sense` 不因这两项缺省而报缺陷**——无动作可联锁、无执行可覆盖，「不适用」不等于「不合规」。
- **物理执行类默认禁用、需本地确认与独立开关（O04）**：推导 `risk_3`（§1469）；默认启用 ⇒ `physical_enabled_by_default`（§34.4）；缺本地确认 ⇒ `physical_without_local_confirmation`、缺独立开关 ⇒ `physical_without_independent_switch`；`Sense` 通道标注物理执行 ⇒ `sense_channel_with_physical_execution`。
- **有效风险取 max（`RiskClass` / `effective_risk`，O04）**：`Control` 推导 `risk_2`、物理执行类推导 `risk_3`（§1469）；声明风险**低于**推导 ⇒ `declared_risk_below_derived`（§35.3「声明低于推导的，上架审核与加载时都拒绝」），有效风险仍取较大者。
- **驱动与 SDK 不进内核进程（`DeviceHostPlacement`，O04）**：声明为内核进程内 ⇒ `device_driver_in_kernel_process`（§34.4）。
- **四类事件各有处置且写决策日志（`EventDisposition`，O04）**：热插拔/断线/重复/超时缺处置 ⇒ `device_event_undisposed`（§1498）；处置不写决策日志 ⇒ `device_event_not_logged`（§1497）。
- **重复音频设备不开第二条收音路径（O04 / §8.5）**：重复事件的处置会打开第二条有效收音路径 ⇒ `duplicate_event_opens_second_capture_path`——即 O04「音频设备变更不重复收音」。
- **实际使用不越声明与授权（`DeviceUse`/`DeviceGrant`，INV-23）**：使用未声明动作 ⇒ `used_undeclared_device_action`；在已声明范围内但超出用户授权 ⇒ `used_beyond_authorization`；两者不重复报同一处使用。
- **readiness 三态（`HostReadiness`，O04）**：未声明任何设备接入 ⇒ `Blocked`（O04 诚实状态）；有任一违规 ⇒ `Defective` 且**优先于** `Blocked`；声明齐备且无违规 ⇒ `Ready`。
- **审计是描述符的纯函数（`audit_host`，O04）**：**十六条**稳定标记，每条都可直接构造；干净描述符审计为空。

## Capabilities
### New Capabilities
- `device-admission`：§34.4 硬件接入原则与 §1469 `device.sense@1`/`device.control@1`、§35.3 有效风险、INV-23 宿主强制、§8.5 单一收音路径的判定口径——设备能力声明（动作枚举/取值范围/速率限制）、执行类的安全联锁与本地手动覆盖、物理执行类默认禁用与本地确认与独立开关、有效风险取 max、驱动与 SDK 不进内核进程、四类事件各有处置并写决策日志、重复设备不开第二条收音路径、实际使用不越声明与授权、readiness 三态、描述符纯函数审计。覆盖 registry **O04** 的**设备侧**（安装侧已由 `nai-upgrade-core` 的 `manifest`/`preflight`/`verify` 与 `plan`/`gates` 覆盖，本变更不重复）。

### Modified Capabilities
- 无。既有 `nai-media-core::mixing` 的 `CaptureRegistry` 管的是**收音租约的互斥**（一次只允许一条有效路径、旧租约拒绝释放、默认设备被观察而不被静默跟随），本变更判的是**设备接入声明与事件处置**；两者互补、不重叠，本变更**不改** `mixing.rs`。既有 `nai-kernel::platform`（平台事件门）与 `monitoring`（手机监控面越界）**不改**。既有 `nai-upgrade-core` 的 `preflight`/`manifest`/`plan`/`gates`（安装与回退）**不改**，本变更不重复其判据。

## Impact

- 代码：`crates/nai-kernel/src/` 新增 `device.rs`；`tests/` 新增 `device.rs`；`lib.rs` 增一个 `pub mod`。复用 `golden::Finding`（`Finding::new` 已为 `pub`），**不新增令牌类型**。**不新增 crate，不改 workspace 依赖**（维持 18 包与依赖方向；kernel 仍只依赖 contracts）。
- 规格：新增 capability `device-admission`；新增 `docs/device-admission.md`、ADR 0036、报告 `docs/reports/2026-10-10-w34-device-admission.md`。
- 依赖：**零新增**。仅用 `std`。
- registry：**O04 保持 `blocked`**（干净 Windows 安装、真实音频设备变更与真实升级回退是外部真值），notes 补记 W34 已交付**设备侧**判定机制、**安装侧已由 `nai-upgrade-core` 覆盖**这一事实，并保留 O04 的三条验收口径。不新增编号（registry id 正则为恰好两位数字）。
- 明确不做：不打开任何设备、不枚举硬件、不做真实驱动/SDK 宿主、不做 PCM 混音或回声测试、不接 WASAPI、不改 `mixing.rs`、不改 `preflight.rs`；不做 UI；不引入 `check --profile portable` 的新命令（§30.1 未为设备接入指定证据目录，臆造 `tests/device` 属过度绑定）。

## Non-Goals

- 不用机制替代真实交付：本轮证明的是「设备声明可判、放置边界可判、事件处置可判、重复收音可判、越权使用可判、未接入不被读成缺陷」，**不宣称干净 Windows 安装或真实设备接入已验收**。O04 保持 `blocked`。
- 不替 Windows 安装与真机设备做验收：安装侧的真实运行库取得、真实音频设备变更与真实升级回退都在受控 Windows 机器上执行，无法执行就标 Blocked（§1261「GPU 与真实 SDK 验收在受控机器上单独执行……无法执行就标 Blocked，不写成 CI Passed」）。
- 不把「尚未接入」变成缺陷，也不让「尚未接入」掩盖越界。
- 不重复 `preflight` 的安装检查，也不重复 `CaptureRegistry` 的收音租约互斥：本模块判**声明与处置**，不判**租约时序**。
