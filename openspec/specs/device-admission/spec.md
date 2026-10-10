# device-admission Specification

## Purpose
把 §34.4「硬件接入原则」、§1469 `device.sense@1`/`device.control@1` 与 INV-23 的**判定口径**落成在 portable 下可执行、可测试的骨架：设备能力必须**声明**（动作枚举、取值范围、速率限制、安全联锁），执行类还必须具备**安全联锁与本地手动覆盖**，物理执行类默认禁用且需本地确认与独立开关，有效风险取**声明与推导的较大者**，设备驱动与 SDK **不进内核进程**，热插拔/断线/重复/超时各有明确处置且事件写入决策日志，重复音频设备**不得打开第二条有效收音路径**，实际使用**不得超出声明与用户授权**（由宿主强制执行，不靠自觉）。本能力只判定**一份设备接入声明是否越界**，不打开任何设备、不枚举硬件、不做真实驱动或 PCM 混音；这些真值由外部条件提供，未提供时 readiness 保持 `Blocked`（O04 的诚实状态）。

## Requirements

### Requirement: Device capabilities are declared, never assumed
设备接入 SHALL 通过 `device.sense@1` / `device.control@1` 声明**动作枚举、取值范围与速率限制**（§1469）。声明的动作枚举为空 MUST 被审计报出；某动作未声明取值范围或速率限制 MUST 被审计逐项报出并指名该动作。

#### Scenario: No actions declared
- **WHEN** 一份设备声明不包含任何动作
- **THEN** 审计报出 `no_declared_actions` 并指名该设备

#### Scenario: An action without a value range or rate limit
- **WHEN** 某设备的某动作未声明取值范围或未声明速率限制
- **THEN** 审计分别报出 `action_without_value_range` / `action_without_rate_limit` 并指名该设备与动作

### Requirement: Actuation requires a safety interlock and a local manual override
执行类（`Control`）设备 SHALL 具备**安全联锁**（§1469、§1497）与**本地手动覆盖**（§1497）。缺失 MUST 被审计报出。纯传感类（`Sense`）声明无动作可联锁、无执行可覆盖，其安全联锁与本地手动覆盖的缺省 MUST NOT 被判为缺陷。

#### Scenario: Control without a safety interlock
- **WHEN** 一份 `Control` 设备声明未提供安全联锁
- **THEN** 审计报出 `control_without_safety_interlock`

#### Scenario: Control without a local manual override
- **WHEN** 一份 `Control` 设备声明未提供本地手动覆盖
- **THEN** 审计报出 `control_without_local_override`

#### Scenario: A pure sense declaration is not reported for actuation governance
- **WHEN** 一份 `Sense` 设备声明既无安全联锁也无本地手动覆盖
- **THEN** 审计不报出安全联锁或本地手动覆盖缺陷

### Requirement: Physical execution is risk_3, disabled by default, and locally confirmed
物理执行类设备 SHALL 按 `risk_3` 推导（§1469），MUST NOT 默认启用（§1499），MUST 具备**本地确认**与**独立开关**（§1469）。违反任一条 MUST 被审计报出。`Sense` 通道 MUST NOT 声明物理执行。

#### Scenario: Physical execution enabled by default
- **WHEN** 一份物理执行类设备声明为默认启用
- **THEN** 审计报出 `physical_enabled_by_default`

#### Scenario: Physical execution without a local confirmation or an independent switch
- **WHEN** 一份物理执行类设备声明缺本地确认或缺独立开关
- **THEN** 审计分别报出 `physical_without_local_confirmation` / `physical_without_independent_switch`

#### Scenario: A sense channel claims physical execution
- **WHEN** 一份 `Sense` 通道的设备声明标注为物理执行
- **THEN** 审计报出 `sense_channel_with_physical_execution`

### Requirement: Effective risk is the maximum of declared and derived
有效风险 SHALL 为**声明风险与推导风险的较大者**（§35.3），其中 `Control` 推导为 `risk_2`、物理执行类推导为 `risk_3`（§1469）。声明风险**低于**推导风险 MUST 被审计报出（§35.3「声明低于推导的，上架审核与加载时都拒绝」）。

#### Scenario: Declared risk below derived
- **WHEN** 一份物理执行类设备声明为 `risk_2`（推导为 `risk_3`）
- **THEN** 审计报出 `declared_risk_below_derived`，且有效风险为 `risk_3`

#### Scenario: Declared risk above derived is accepted
- **WHEN** 一份 `Control` 设备（推导 `risk_2`）声明为 `risk_3`
- **THEN** 不报出风险相关缺陷，且有效风险为 `risk_3`

### Requirement: Device drivers and SDKs stay out of the kernel process
设备驱动与 SDK SHALL 位于 subprocess 或边缘节点，MUST NOT 进入内核进程（§34.4）。声明为内核进程内 MUST 被审计报出。

#### Scenario: Driver declared in the kernel process
- **WHEN** 设备接入声明其驱动与 SDK 位于内核进程
- **THEN** 审计报出 `device_driver_in_kernel_process`

### Requirement: Hotplug, disconnect, duplicate and timeout each have an explicit disposition
设备**热插拔、断线、重复与超时**SHALL 各有明确处置（§34.4）。任一事件类无处置 MUST 被审计报出并指名该事件类。**未声明任何设备接入时不存在设备事件**，本项 MUST NOT 产生缺陷。

#### Scenario: An event class is undisposed
- **WHEN** 已声明设备接入，但未声明对「超时」的处置
- **THEN** 审计报出 `device_event_undisposed` 并指名 `timeout`

#### Scenario: No device is declared
- **WHEN** 未声明任何设备接入
- **THEN** 审计不报出任何事件处置缺陷

### Requirement: Device events are written to the decision log
设备事件 SHALL 写入**决策日志**（§34.4）。某事件类的处置不记录事件 MUST 被审计报出并指名该事件类。

#### Scenario: An event disposition does not log
- **WHEN** 「断线」事件的处置不写入决策日志
- **THEN** 审计报出 `device_event_not_logged` 并指名 `disconnect`

### Requirement: A duplicate audio device never opens a second live capture path
重复音频设备 MUST NOT 产生第二条**有效收音路径**（§8.5「只允许一条有效收音路径，避免 OBS 与伴侣重复收音造成回声」；O04「音频设备变更不重复收音」）。重复事件的处置若会打开第二条有效收音路径 MUST 被审计报出。

#### Scenario: A duplicate opens a second capture path
- **WHEN** 「重复」事件的处置允许打开第二条有效收音路径
- **THEN** 审计报出 `duplicate_event_opens_second_capture_path`

#### Scenario: A duplicate keeps a single capture path
- **WHEN** 「重复」事件的处置保持单一路径且已记录
- **THEN** 不报出重复收音相关缺陷

### Requirement: Actual device use never exceeds declaration and authorization
扩展实际使用的设备能力 MUST NOT 超出**清单声明**与**用户授权**（INV-23），且由**宿主强制执行，不靠自觉**。使用未声明的设备动作 MUST 被审计报出；在已声明范围内但超出用户授权 MUST 被审计报出，且两种缺陷 MUST NOT 重复报出同一处使用。

#### Scenario: Using an undeclared device action
- **WHEN** 宿主实际使用了某未声明的设备动作
- **THEN** 审计报出 `used_undeclared_device_action` 并指名该设备与动作

#### Scenario: Using beyond authorization
- **WHEN** 宿主使用了已声明但未被用户授权的设备动作
- **THEN** 审计报出 `used_beyond_authorization` 并指名该设备与动作

#### Scenario: Use within declaration and authorization
- **WHEN** 宿主只使用已声明且已授权的设备动作
- **THEN** 不报出使用相关缺陷

### Requirement: Readiness is Blocked until a host is declared, and Defective outranks it
审计非空时 readiness MUST 为 `Defective`，且该判定 MUST 优先。审计为空且未声明任何设备接入时 readiness MUST 为 `Blocked`（O04 诚实状态：干净 Windows 安装与真实设备尚未接入），MUST NOT 记为缺陷。审计为空且已声明设备接入时 readiness 为 `Ready`。

#### Scenario: No device host declared
- **WHEN** 未声明任何设备接入，且无任何违规
- **THEN** readiness 为 `Blocked` 且审计为空

#### Scenario: Defect outranks the undeclared host
- **WHEN** 声明了设备接入，但驱动被声明为位于内核进程
- **THEN** readiness 为 `Defective`，不得因「尚未接入真实设备」而掩盖该缺陷

#### Scenario: A declared host with no violations
- **WHEN** 设备接入声明齐备且无任何违规
- **THEN** readiness 为 `Ready`

### Requirement: Host audit is a pure function of the descriptor
审计 SHALL 是对所声明设备接入字段的**纯函数**，报出全部命中项，使每条规则都能被直接构造与验证。

#### Scenario: A clean host audits empty
- **WHEN** 直接构造一份满足全部约束的设备接入描述
- **THEN** 审计为空

#### Scenario: Every finding is constructible
- **WHEN** 直接构造一份含某违规的描述
- **THEN** 审计报出对应的稳定标记，无需经由任何真实设备或运行时测量
