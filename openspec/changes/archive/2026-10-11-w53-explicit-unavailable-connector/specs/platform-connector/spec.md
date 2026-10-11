# Platform Connector

## Purpose

把 §9.4「快手、视频号放入同一平台插件目录，在核实官方接口前保留**显式 Unavailable 实现**，不宣称可上线」、
§17.2「尚未取得官方接口的连接器**只保存契约、准入说明与显式 Unavailable 实现，不放伪造 SDK**」与
§1344「外部依赖缺失时：实现显式 Unavailable，报告 Blocked……不得用 mock 通过真实验收」落成在 portable 下
可执行、可测试的**接缝**与**实物**：连接器的身份、可用性与准入说明是**可判定的声明**；不可用连接器的
**每一项操作只能以具名失败结束，绝不返回合成的成功**；「不放伪造 SDK」是**从包自身清单读出的结构事实**，
不是一句承诺；**新增连接器不改内核**（A03）由依赖方向门控强制，不是靠人记得。
本能力只判定**一份连接器声明**，不加载任何平台 SDK、不发网络请求、不启动伴侣、不推送音视频流。

## ADDED Requirements

### Requirement: An unverified platform connector is explicitly unavailable, never a fake

尚未核实官方接口的平台连接器 SHALL 以**显式 Unavailable 实现**交付，MUST NOT 被表述为可上线，
MUST NOT 内嵌或转发任何**伪造的官方 SDK**（§9.4、§17.2、§1344）。连接器的可用性 SHALL 是**声明的一部分**，
取值限定为 `Verified { interface_revision }` 或 `Unavailable { reason }`，其中 `reason` 取自**封闭集合**。
声明 `Verified` 而缺少接口修订号 MUST 被拒绝——「已核实」是一项有依据的主张，不是一个形容词。

#### Scenario: Verified without an interface revision

- **WHEN** 连接器声明 `Verified` 但未给出所依据的官方接口修订号
- **THEN** 校验以具名错误拒绝，该连接器不得被读作可用

#### Scenario: An unverified connector stays declared unavailable

- **WHEN** 连接器的官方接口尚未核实
- **THEN** 其可用性为 `Unavailable` 且携带封闭集合内的原因，且没有任何声明把它读作可上线

### Requirement: Unavailable operations fail by name and never fabricate success

可用性为 `Unavailable` 的连接器，其**每一项**桥接操作（§27.1 的四项职责）SHALL 以稳定错误结束——
`CAPABILITY_NOT_READY`，且 `details["reason"]` 为 `UNVERIFIED_EXTERNAL_DEPENDENCY`（§27.1 的原文术语）——
MUST NOT 返回成功，MUST NOT 返回**合成的事件、能力快照或生命周期结果**。
该规则 MUST NOT 是空洞的：可用连接器的同一组操作 SHALL 存在一条可成功的路径，
否则「全部失败」只证明了接缝恒失败。

#### Scenario: Every operation of an unavailable connector fails by name

- **WHEN** 逐一调用不可用连接器的四项桥接操作
- **THEN** 每一项都以 `CAPABILITY_NOT_READY` 与 `UNVERIFIED_EXTERNAL_DEPENDENCY` 失败，且没有任何一项返回成功

#### Scenario: The rule is not vacuous

- **WHEN** 直接构造一个 `Verified` 的连接器替身（仅存在于测试内）
- **THEN** 同一组操作存在成功路径，证明「全部失败」是声明的性质而非接缝的恒假

### Requirement: A connector declares its identity and its admission notes

连接器 SHALL 声明**稳定身份**（连接器 id 与平台 id）与**非空准入说明**；同一目录下的不同平台 MUST 具有
**互异**的平台 id。准入说明 SHALL 覆盖 §13.1 的平台合规核对项与 §9.3 的接入节奏事项；缺失任一必需核对项
MUST 被拒绝并指出所缺项。（§17.2「只保存契约、准入说明与显式 Unavailable 实现」）

#### Scenario: Two platforms in one directory

- **WHEN** 同一连接器目录同时登记两个平台的显式 Unavailable 连接器
- **THEN** 两者的平台 id 互异且各自稳定，身份可被逐一取回

#### Scenario: An admission note misses a compliance check

- **WHEN** 准入说明缺少 §13.1 的某一核对项
- **THEN** 校验以具名错误拒绝，并指出缺失的核对项

### Requirement: No fake SDK is a structural fact, not a promise

声明 `Unavailable { NoOfficialInterface }` 的连接器 MUST NOT 依赖任何平台 SDK。该性质 SHALL 可**从包自身的
清单**机器校验：出现 SDK 形态的依赖名 MUST 失败并指名该依赖。（§17.2「不放伪造 SDK」）

#### Scenario: An SDK-shaped dependency appears

- **WHEN** 该连接器的包清单出现一个 SDK 形态的依赖
- **THEN** 结构检查失败并指名该依赖

#### Scenario: Only the declared seam dependencies

- **WHEN** 该连接器只依赖契约与插件接缝
- **THEN** 结构检查通过

### Requirement: Adding a connector never changes the kernel

平台连接器 SHALL 位于内核之外，且内核 MUST NOT 依赖任何平台连接器；**平台连接器 MUST NOT 依赖内核**
（A03「新增连接器不改内核」；§17.5「kernel 不依赖具体插件」）。两项方向 SHALL 由依赖方向门控强制，
MUST NOT 依赖人工记得。

#### Scenario: The kernel reaches a connector

- **WHEN** 内核依赖某个平台连接器
- **THEN** 依赖方向门控失败并指名两侧

#### Scenario: A connector reaches the kernel

- **WHEN** 一个平台连接器依赖内核
- **THEN** 依赖方向门控失败——接缝本身已是充分的，依赖内核意味着接缝有缺
