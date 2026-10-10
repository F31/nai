# W34 实测报告：设备接入判定骨架（O04 设备侧前置解耦）

- 日期：2026-10-10
- 变更：`openspec/changes/w34-device-admission/`
- 决策：[ADR 0036](../adr/0036-device-admission-boundary.md)；指南：[docs/device-admission.md](../device-admission.md)
- 判据：`tests/registry.yaml` O04

## 结论

O04（安装与设备）的**真值**（受控 Windows 机器 + 真实音频设备 + 真实回退演练）需要外部条件，本轮不可达，O04 **保持 `blocked`**——其 registry
notes 此前只有一句「Windows 安装未执行」。O04 的三条验收（§1329）中**两条已有落点**：安装侧的运行库/平台/依赖闭包/磁盘/凭据检查由
`nai-upgrade-core` 的 `manifest`/`preflight` 覆盖、**升级可回退**由同 crate 的 `plan`（journal、`RollbackReport`、`RecoveryAction`）与
`gates`（`require_rollback_compatible`/`downgrade`）覆盖。

**本轮补的是此前全库没有落点的「设备」一半**——§1424 缺口清单把它写得很直白：**「没有设备与硬件接入的抽象，也没有物理安全联锁。」**
§34.4/§1469/§35.3/INV-23/§8.5 的判据此前没有落点，于是「设备接入不得越界」在执行层等于一句**无从判定**的要求。本轮把口径落进
`crates/nai-kernel/src/device.rs`，几处最容易含糊的地方被钉死：

1. **设备能力靠声明**（§1469「声明动作枚举、取值范围、速率限制」），缺项**指名设备与动作**。
2. **执行类必须安全联锁 + 本地手动覆盖，纯传感不适用**（§1469/§1497）——**否决**把「不适用」读成「不合规」（同 W33 预览未启用）。
3. **物理执行类推导 `risk_3`、默认禁用、需本地确认与独立开关**（§1469、§34.4）。
4. **有效风险 = `max(声明, 推导)`，声明低于推导即拒**（§35.3）——声明方**可以**主动提高风险，但**不能**自我降级。
5. **驱动与 SDK 不进内核进程**（§34.4）落成可判边界，且是**独立声明**（声明内核进程即违规）。
6. **四类事件各有处置且写决策日志**（§1498/§1497）；**未声明设备时不存在设备事件**，故条件化。
7. **「音频设备变更不重复收音」（O04、§8.5）落成 `duplicate_event_opens_second_capture_path`**——判到**结果**（只允许一条有效收音路径），不是「处置了」。
8. **INV-23「实际权限不得超出清单声明与用户授权」落成使用侧审计**（宿主强制执行、不靠自觉），未声明只报一次、不重复报越权。
9. **`readiness` 三态且先取审计**（`Defective` 优先，`Blocked` 判据是审计为空且无声明）。

**本轮不产生任何真实交付**：设备驱动、SDK 宿主、WASAPI 路由、PCM 混音、真实音频设备与安装器都不在此仓库生成。
**不新增 xtask 命令与证据目录**：§30.1 为升级指定 `tests/upgrade`、为 RAG 黄金集指定 `tests/rag-golden`、为 UI 指定 `tests/ui`；
设备接入**没有**证据目录约定，其真值来自**受控 Windows 机器与真实设备**而非仓库载荷，臆造 `tests/device` 属过度绑定。

## 目标行为

| 判据（出处） | 实现 | 关键设计 |
| --- | --- | --- |
| 声明动作枚举/取值范围/速率限制（§1469） | `DeviceDeclaration` / `DeviceAction` | 缺项逐条报出并**指名设备与动作**；空枚举不被当作"已声明" |
| 安全联锁必需（§1469、§1497） | `DeviceDeclaration::safety_interlock` | `Control` 缺失 ⇒ 报出；**纯 `Sense` 不报**（无可联锁的动作） |
| 本地手动覆盖必需（§1497） | `local_manual_override` | `Control` 缺失 ⇒ 报出；**纯 `Sense` 不报**（无可覆盖的执行） |
| 物理执行 `risk_3` + 默认禁用 + 本地确认 + 独立开关（§1469、§34.4） | `physical_execution` / `enabled_by_default` / `local_confirmation` / `independent_switch` | 四项逐条可判；`Sense` 标注物理执行为分类错误 |
| 有效风险 = max(声明, 推导)（§35.3） | `derived_risk()` / `effective_risk()` | 声明低于推导 ⇒ 拒；声明**高于**推导 ⇒ 接受 |
| 驱动与 SDK 不进内核进程（§34.4） | `DeviceHostPlacement` | `KernelProcess` ⇒ 报出；是独立声明故不受是否有设备门控 |
| 四类事件各有处置（§1498） | `DeviceEvent::{Hotplug,Disconnect,Duplicate,Timeout}` / `EventDisposition` | 缺类/未处置逐条指名；**未声明设备时条件化不判** |
| 设备事件写决策日志（§1497） | `EventDisposition::logged` | 未记录 ⇒ 报出并指名事件类 |
| 音频设备变更不重复收音（O04、§8.5） | `keeps_single_capture_path` | `Duplicate` 会开第二条路径 ⇒ 报出；保持单一路径 ⇒ 不报 |
| 实际使用不越声明与授权（INV-23） | `DeviceUse` / `DeviceGrant` | 未声明 ⇒ 报出；已声明未授权 ⇒ 报出；**同一处使用不重复报** |
| readiness 三态（O04） | `HostReadiness` | 先取审计（`Defective`）→ 无声明 ⇒ `Blocked` → 否则 `Ready` |
| 审计纯函数（O04） | `audit_host` | **十六条**稳定标记，逐条可构造，并以「恰为这十六条」断言 |

## 测试

`cargo test -p nai-kernel --test device`：**31 passed / 0 failed**。

| 组 | 覆盖 |
| --- | --- |
| 声明与动作（4） | 空动作枚举被指名报出；缺取值范围/缺速率限制**各报一条且不误报另一条**并指名 `设备/动作`；齐备动作不报；`ValueRange::is_well_formed` 边界 |
| 联锁与覆盖（3） | `Control` 缺两项逐条报出；**纯 `Sense` 缺两项不报**（否决「不适用⇒不合规」）；`Control` 两项齐备且干净 |
| 物理执行（5） | 默认启用/缺本地确认/缺独立开关**逐条命中且不互相误报**；`Sense` 标注物理执行被报出；完全治理的物理设备审计为空 |
| 风险与放置（4） | 推导风险随通道与物理执行（含 `Sense`+物理 ⇒ `risk_3`）；声明低于推导 ⇒ 报出**且有效风险为 `risk_3`**；声明高于推导 ⇒ 不报；内核进程放置 ⇒ 报出，subprocess/edge ⇒ 不报 |
| 事件与决策日志（5） | 缺「超时」处置 ⇒ 指名；`handled=false` ⇒ 报出；「断线」未记录 ⇒ 指名；「重复」开第二条路径 ⇒ 报出；「重复」保持单一路径 ⇒ 不报 |
| 使用与授权（4） | 未声明动作 ⇒ 指名；已声明未授权 ⇒ 指名；**既未声明也未授权只报未声明一次**；声明+授权内使用不报 |
| readiness（3） | 空 host 审计为空且 `Blocked`；空 host + 内核进程放置 ⇒ `Defective`（优先）；干净声明 host ⇒ `Ready` |
| 审计完备（2） | 干净描述符审计为空；一份最大违规描述的审计并集**恰为十六条** |

## 实跑

```
$ cargo xtask check --profile portable
... 64 test binaries + 17 doc-test groups, 623 passed / 0 failed
portable: engineering foundation checks passed; device/SDK/GPU/install certification not performed
EXIT=0

$ cargo xtask check-deps
dependency boundaries passed: 18 workspace packages (normal/build/dev/target declarations)

$ cargo xtask check --registry
# pass 1 / # fail 0
admission registry check passed (V3.6 acceptance + D01-D14 mapping)
```

门控目标数由 W33 的 **80** 升至 **81**（64 Running + 17 Doc-tests）：新增一个集成测试目标 `tests/device.rs`；通过项由 592 升至 **623**（+31）。
本轮**先跑预链**（`cargo test --workspace --no-run` + 按报错路径定向清理被锁 exe）**5 轮收敛**，随后门控一次通过、`LNK=0`——
宿主对新生成 exe 的瞬时锁，与代码无关。

## 依赖边界

- 落 `nai-kernel`（只依赖 contracts），复用 `golden::Finding`（同一报告词汇），**不新增令牌类型**。
- `check-deps` 仍 **18 包**、方向不变；**外部依赖零新增**（仅 `std`）。
- 不新增 crate；不改 workspace 组成。

## 未覆盖边界

- **不含任何真实交付**：设备驱动、SDK 宿主、WASAPI 路由、PCM 混音、回声测试、真实音频设备枚举与安装器都由外部提供。测试中的描述符是**构造的**，
  只证明越界口径与 readiness 派生可执行。
- **不重复 `preflight` 的安装检查**：运行库/平台/依赖闭包/磁盘/凭据由 `nai-upgrade-core::preflight` 判；O04 安装侧另有落点，本模块不重复。
- **不重复 `CaptureRegistry` 的租约互斥**：`nai-media-core::mixing` 的 `CaptureRegistry` 判**收音租约时序**（一次只允许一条有效路径、旧租约拒绝释放、
  默认设备被观察而不被静默跟随）；本模块判**事件处置**。两者互补，本模块**不改** `mixing.rs`。
- **不加载体**：待真实设备轨立项后再补 xtask 命令与证据目录，**无需改本模块判定**。
- 注册表 **O04 保持 `blocked`**：notes 保留三条验收口径，记明**设备侧判定机制（W34）已交付**、**安装侧已由 `nai-upgrade-core` 覆盖**。
