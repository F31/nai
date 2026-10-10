# 设计：设备接入与安全联锁的判定边界

- 基线：V3.7 §34.4（硬件接入原则）、§1469（`device.sense@1`/`device.control@1`）、§35.3（有效风险）、INV-23（宿主强制执行）、§8.5（只允许一条有效收音路径）、§1261（交付与 CI）、§30.1（测试分层）、§1329（O04 验收）、§1424（缺口清单）
- 判据：`tests/registry.yaml` O04
- 承接：ADR 0034（写实准入 Gate）、ADR 0035（手机监控面）

## 背景

O04（安装与设备）的真值（干净 Windows 安装、真实音频设备变更、真实升级回退）需要外部条件，本轮不可达。但 §1424 明确把**「没有设备与硬件接入的抽象，也没有物理安全联锁」**列为缺口，而 §34.4/§1469/§35.3/INV-23/§8.5 给了一组很密的判据，此前没有落点。要把它们落成代码，必须先回答：

1. 落进哪个 crate？
2. 设备能力靠什么表达，缺什么算缺？
3. 安全联锁与本地手动覆盖是不是**所有**设备都必需？
4. 物理执行类的默认值、推导风险与确认动作怎么表达？
5. 有效风险怎么算，「声明低于推导」怎么拒？
6. 驱动与 SDK 的**放置**怎么判？
7. 四类事件的「明确处置」怎么表达，决策日志怎么挂？
8. 「音频设备变更不重复收音」落在哪？
9. INV-23 的「实际权限不得超出声明与授权」判什么？
10. 未接入真实设备时 readiness 是什么？
11. 审计会不会被判定函数架空？

## 决策

### D1 落 `crates/nai-kernel`（`device.rs`）

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 放进 `nai-media-core`（与 `mixing.rs` 的 `CaptureRegistry` 同住） | 贴近设备与收音 | **否决**：media 层是**媒体装配**，设备接入判定是**策略**（声明、风险、放置、授权），且 media 层会反向依赖 kernel 的报告词汇与风险等级。 |
| B. 放进 `nai-upgrade-core`（与 `preflight` 同住） | 贴近安装 | **否决**：`preflight` 判的是**安装资源是否足够**（磁盘/平台/依赖/凭据），与**设备接入声明**是两回事；塞进去会让「设备可判」依赖一个带 `semver`/storage 的 crate。 |
| C. 放进 `crates/nai-kernel` | 与 `golden.rs`/`realistic.rs`/`monitoring.rs` 同 crate | **采纳**：复用 `golden::Finding`（同一报告词汇），依赖方向不变（kernel 只依赖 contracts）。 |

> §34.4「设备驱动与 SDK 放在 subprocess 或边缘节点，**不进内核进程**」说的是**运行时 I/O 的放置**，与「判定策略放 kernel crate」不矛盾：kernel 判的是**声明与处置**，不打开设备、不枚举硬件。`DeviceHostPlacement` 正是把这条放置边界做成一个可判字段。

### D2 设备能力靠**声明**，缺项指名

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 只报「声明不完整」不指名 | 一个布尔 | **否决**：不知道缺哪一项、哪个动作，修法不同。 |
| B. 要求所有动作都有取值范围与速率限制，缺一即缺陷 | 严格 | **采纳**：§1469「声明动作枚举、取值范围、速率限制」是声明的三个字段。 |
| C. 允许空动作枚举 | 宽松 | **否决**：空枚举等于「什么都没声明」，`no_declared_actions` 是必需的负面判据。 |

### D3 安全联锁与本地手动覆盖是**执行类**的必需项，不是所有设备的

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 所有设备都要求安全联锁与本地手动覆盖 | 一刀切 | **否决**：安全联锁联锁的是**动作**，本地手动覆盖覆盖的是**执行**；对纯传感器没有可联锁/可覆盖的对象。这是把「不适用」读成「不合规」——W33 已就此否决过预览未启用的同类误判。 |
| B. 都不要求 | 宽松 | **否决**：§1469/§1497 明确「安全联锁与本地手动覆盖是必需项」，对执行类设备就是硬要求。 |
| C. `Control` 必须两项齐备；`Sense` 不因此报缺陷 | 条件化 | **采纳**。 |

### D4 物理执行类：推导 `risk_3`、默认禁用、需本地确认与独立开关

§1469「物理执行类默认 risk_3，需要本地确认与独立开关」+ §34.4「物理执行类设备默认禁用，启用需管理员本机确认」合成四个可判字段：`physical_execution`、`enabled_by_default`、`local_confirmation`、`independent_switch`。物理执行类设备 `enabled_by_default` 为真 ⇒ `physical_enabled_by_default`（默认禁用）；缺本地确认/独立开关逐条报出。`Sense` 通道标注物理执行是分类错误 ⇒ `sense_channel_with_physical_execution`。

### D5 有效风险取 **max**，「声明低于推导」即拒

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 用声明风险 | 只信声明 | **否决**：§35.3 正是要防「声明低于推导」的自我降级。 |
| B. 用推导风险 | 只信推导 | **否决**：会丢掉声明方**主动提高**风险（更保守）的信息。 |
| C. `max(声明, 推导)` 为有效风险；`声明 < 推导` ⇒ 拒 | 取较大者 + 拒绝自我降级 | **采纳**：`Control` 推导 `risk_2`、物理执行类推导 `risk_3`（§1469）；`Risk1` 用于纯 `Sense`（只读感知）。 |

### D6 驱动与 SDK 的**放置**是可判字段

`DeviceHostPlacement::{Subprocess, EdgeNode, KernelProcess}`；`KernelProcess` ⇒ `device_driver_in_kernel_process`（§34.4）。这样「不进内核进程」从一句约定变成一条可判边界。

### D7 四类事件各有处置，且处置挂**决策日志**；但以「声明了设备」为前提

`DeviceEvent::{Hotplug, Disconnect, Duplicate, Timeout}`（§1498 原文四类）；`EventDisposition{handled, logged, keeps_single_capture_path}`。缺某类 ⇒ `device_event_undisposed`；处置不记录 ⇒ `device_event_not_logged`（§1497「设备事件写入决策日志」）。断线对应契约里的 `MediaState::DEVICE_LOST` 语义，本模块只判处置是否存在与是否记录。

**条件化**：未声明任何设备接入时**不存在设备事件**，事件处置 MUST NOT 产生缺陷——否则 O04 尚未接入真实设备的诚实状态会被读成四个缺陷，与 D10「无声明应是 `Blocked`」冲突。这是把「不适用」读成「不合规」的又一例（同 W33 预览未启用、D3 纯 `Sense` 不要求联锁）。而**放置**（D6）是**独立的声明**：声明内核进程本身即违规，故 `device_driver_in_kernel_process` 不随是否有声明门控——这也让 D10 的「`Defective` 优先于 `Blocked`」可被构造与验证。

### D8 「音频设备变更不重复收音」= 重复事件的处置**保持单一路径**

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 复用 `mixing.rs` 的 `CaptureRegistry` | 共享状态 | **否决**：`CaptureRegistry` 判的是**租约时序**（申请/释放/旧租约），设备接入判的是**事件处置**；跨 crate 复用会把 kernel 拖进 media 层。 |
| B. 只声明「处置了重复事件」 | 一个布尔 | **否决**：处置了**不等于**不会开第二条路径。§8.5 的要求是「只允许一条有效收音路径」，必须判到**结果**。 |
| C. `Duplicate` 处置声明「保持单一路径」，否则报出 | 条件化判据 | **采纳**：`duplicate_event_opens_second_capture_path`，即 O04「音频设备变更不重复收音」。 |

### D9 INV-23 判**实际使用**是否越过声明与授权

`DeviceUse{device_ref, action}` 是宿主**实际使用**的能力；`DeviceGrant{device_ref, actions}` 是**用户授权**；声明来自 `DeviceDeclaration.actions`。规则：使用未声明动作 ⇒ `used_undeclared_device_action`；在已声明范围内但未授权 ⇒ `used_beyond_authorization`；**两者不重复报同一处使用**（先判未声明，命中即止）。真实「宿主强制执行、不靠自觉」的运行时拦截属宿主与集成验收，本模块判的是**越界事实可被审计复核**。

### D10 readiness 三态，`Defective` **优先**于 `Blocked`

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 无声明即缺陷 | 一律 Defective | **否决**：O04 尚未接入真实设备是**计划内**的诚实状态（§1261「无法执行就标 Blocked，不写成 CI Passed」）。 |
| B. 无声明即 Ready | 乐观 | **否决**：会把「尚未接入」读成「已就绪」。 |
| C. `Blocked`/`Defective`/`Ready`，**先取审计**（非空即 `Defective`），再按是否有声明（无 ⇒ `Blocked`）否则 `Ready` | 审计优先 | **采纳**：未接入不掩盖越界，越界也不被未接入冲淡。注意 `Blocked` 的判据是**审计为空且无声明**——否则「无声明」会被读成「已就绪」或「有缺陷」。 |

### D11 审计是描述符的纯函数

与 W30/W32/W33 一致：对描述符自身字段做纯函数，报出**全部**命中项，**十六条**标记逐条可在测试里直接构造；干净描述符审计为空；三份构造描述符的审计并集**恰为**这十六条（既证每条可构造，也防未登记标记）。

### D12 不新增 xtask 命令与证据目录

§30.1 未为设备接入指定证据目录（升级为 `tests/upgrade`、RAG 为 `tests/rag-golden`、UI 为 `tests/ui`）。设备接入的真值来自**受控 Windows 机器与真实音频设备**，不是仓库载荷。**臆造** `tests/device` 属过度绑定。待真实设备轨立项后再补载体，届时**无需改本模块判定**。

## 验证

- `cargo test -p nai-kernel --lib` 与 `cargo test -p nai-kernel --test device`（新增用例）。
- `cargo xtask check --profile portable`（含 fmt、workspace clippy、全量测试）通过。
- `cargo xtask check-deps` 仍 18 包、方向不变。
- OpenSpec `validate --all --strict --archived` 全通过。

## 环境边界

判定骨架在 portable（Linux/WSL 逻辑层）可测，测的是**口径**：一份设备接入声明如何判越界、物理执行类如何默认禁用、有效风险如何取较大者、事件处置如何判、重复设备如何不开第二条收音路径、越权使用如何被复核、尚未接入如何表达。**不含**任何真实设备、驱动、SDK、WASAPI 路由、PCM 混音与安装器；因此 O04 保持 `blocked`，本变更不升级其状态。
