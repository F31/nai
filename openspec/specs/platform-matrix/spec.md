# platform-matrix Specification

## Purpose
把「平台」变成仓库里的**一等公民**：一份可判定的**平台集合**与一份可查的**能力 × 平台矩阵**，
让每一项平台相关能力在每一个目标平台上都有**明确的状态**，而不是靠 `#[cfg]` 的沉默。

动机是三项已核实的事实。① 基线 V3.8 全文对 macOS 的命中数为 **0**（随基线订正重新实测，V3.8 亦为 0；只有 Windows 首发、Linux 轻云与 WSL、
iOS/Android 实验）⇒「macOS 覆盖 Standard 与 Pro」是一条**新增需求**，没有可依的验收判据。
② 全仓平台分支只有 **11 处**（`nai-security` 的密钥保护、`nai-storage` 的数据根、`xtask` 的工具链），
其中 `NativeVault` 在非 Windows 上**已经**返回 `CapabilityNotReady`——那是诚实的失败关闭，但**没有任何地方登记过这件事**。
③ §14.1 把「抖音连接器与事件治理」列为 Standard 的核心构件，而 §9.2/§687 说明抖音 SDK 与 Windows 音频是
**Windows 原生目标**；写实的加速路径 §300 也写「Windows NVIDIA CUDA 首发」。⇒ macOS 上的
「Standard」「Pro」**不能照搬 Windows 档位表的内容**。

本能力交付**一份登记与一套校验**，MUST NOT 被读作任何平台已经支持、已经构建或已经可分发。
登记一项能力在某平台**不可得**，是**如实披露**，不是缺陷；只有**不实声明**才是缺陷。

## Requirements

### Requirement: Platforms and target triples are declared, not implied

平台的集合、每个平台的目标三元组与它所属的发布线 SHALL 是一份**显式声明**，MUST NOT 由构建环境或
开发者所在地推断。每个平台 SHALL 有稳定且唯一的小写标识；目标三元组 SHALL 形如
`<arch>-<vendor>-<os>[-<abi>]`；发布线取值 SHALL 限于仓库既有的
`slice_0` / `r1` / `r2` / `r3` / `experimental`。

#### Scenario: A platform is implied by the build host

- **WHEN** 一份平台声明只覆盖构建主机所在的平台，或某平台没有目标三元组
- **THEN** 校验失败并指名该平台

#### Scenario: A release line is invented

- **WHEN** 一个平台声明了仓库既有发布线之外的取值
- **THEN** 校验失败并指名该取值

### Requirement: Every capability is stated on every platform

矩阵 SHALL 对**每一项**已登记的平台相关能力，给出它在**每一个**已声明平台上的状态。
一个能力的平台键集合 SHALL 与平台集合**逐一相等**：少一个平台 MUST 失败——
「这个平台没提」与「这个平台不支持」MUST NOT 被混为一谈。

#### Scenario: A capability silently omits a platform

- **WHEN** 某能力的平台状态缺少一个已声明平台，或多出一个未声明平台
- **THEN** 校验失败，并如实指出两侧差集

### Requirement: Unavailability is explicit and carries a closed reason code

状态 SHALL 取自封闭集合 `supported` / `explicit-unavailable` / `out-of-scope`。
`explicit-unavailable` MUST 携带一个**理由码**，且理由码 MUST 取自插件 SDK 的
`UnavailableReason`（**从源码派生**，不在校验里另立一份常量表）；`supported` MUST NOT 携带理由码。
MUST NOT 存在「不可用但返回成功」的表示。

#### Scenario: An unavailable capability carries no reason

- **WHEN** 一项能力在某平台上标为 `explicit-unavailable` 却没有理由码
- **THEN** 校验失败

#### Scenario: A reason code drifts from the SDK enum

- **WHEN** 矩阵使用了一个 `UnavailableReason` 之外的字符串
- **THEN** 校验失败，并指出该字符串不在源码派生的集合内

#### Scenario: A supported capability claims a reason

- **WHEN** 一项标记为 `supported` 的能力携带理由码
- **THEN** 校验失败

### Requirement: The matrix and the source platform branches agree

矩阵 SHALL 登记每一项能力在源码中的平台分支所在位置；每个被登记的路径 MUST 存在。
反过来，**全仓**含平台条件编译（`cfg(windows)` / `cfg(unix)` / `cfg(target_os = ...)` 等）的文件
SHALL 是这些登记路径的**子集**——出现一处未被登记的平台分支 MUST 失败。
这条约束把「平台假设只增不减」变成**可判定**的事实。

#### Scenario: A platform branch is added without registration

- **WHEN** 源码里新增一处平台条件编译，而矩阵没有对应的能力登记
- **THEN** 校验失败并指名该文件

#### Scenario: A registered path no longer exists

- **WHEN** 矩阵登记了一个不存在的源码路径
- **THEN** 校验失败——悬空引用 MUST NOT 通过

### Requirement: macOS is targeted at Standard and Pro

macOS 的发布线归属 SHALL 同时包含 `r2`（Standard）与 `r3`（Pro）——即这两条发布线的能力水平
在 macOS 上同样成立。这条归属 SHALL 是**数据里的断言**，可被校验读取，MUST NOT 只存在于文档叙述中。

#### Scenario: macOS loses a release line

- **WHEN** macOS 的发布线归属不再同时包含 `r2` 与 `r3`
- **THEN** 校验失败

### Requirement: Windows-bound capabilities are honestly unavailable on macOS

依赖 Windows 原生目标的能力（平台 SDK、音频设备路径、写实加速后端）在 macOS 上
SHALL 被登记为 `explicit-unavailable` 并给出不实声明以外的真实理由；相关文档 MUST NOT 描述
macOS 上存在等价接入路径，除非该路径已在本仓库落地。macOS 的档位内容因此 SHALL 与 Windows 档位表
**分别定义**，MUST NOT 照搬。

#### Scenario: A replacement path is claimed without a landing point

- **WHEN** 一份声明称 macOS 上存在等价能力，而仓库中没有对应的实现落点
- **THEN** 该声明 MUST NOT 成立；能力保持显式不可用

#### Scenario: The platform SDK is unavailable on macOS

- **WHEN** 平台 SDK 能力在 macOS 上的状态被检查
- **THEN** 它必须是 `explicit-unavailable`，理由为「尚无官方接口」，MUST NOT 是 `supported`

### Requirement: Data root and signing are resolved per platform

数据根 SHALL 由壳或操作系统路径 API 按平台解析（§900），MUST NOT 硬编码任一平台的绝对路径。
代码签名与分发 SHALL 由**各平台各自的机制**负责（§1262），MUST NOT 用一个平台的自签名替代
另一平台的机制或既有许可。

#### Scenario: One platform's signing substitutes for another

- **WHEN** 一份声明用某一平台的签名机制代替另一平台要求的机制
- **THEN** 校验失败

### Requirement: Registration is not support

本能力交付**登记与校验**，MUST NOT 使任何验收状态升级，MUST NOT 被读作某平台已构建、
已打包或已可分发。平台的真实支持真值是可运行、可打包、可签名的产物，属各发布线自己的里程碑。

#### Scenario: A registered platform is read as delivered

- **WHEN** 一个平台在矩阵里被登记
- **THEN** 它 MUST NOT 使任何验收状态升级，缺失前置 SHALL 被如实点名

#### Scenario: An unported capability is reported as a defect

- **WHEN** 一项能力在某平台上被如实登记为显式不可用
- **THEN** 它 MUST NOT 被计为缺陷——只有不实声明才是缺陷
