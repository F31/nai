# Spec Delta

## Purpose

在 ADR 0017 的工程演练边界内补上「保护式句柄签发」：帧数据的 producer/consumer 授权必须来自 OS 层身份校验（`SO_PEERCRED`），而不是对端自报；未授权进程不得取得该 pool 的任何 ticket 或控制操作。

## ADDED Requirements

### Requirement: Kernel-evidence peer authorization

broker SHALL 只对满足内核证据的声明身份授予 pool 授权：声明的 pid 存在、其 uid 等于 pool owner uid，且该 pid
MUST 持有该 pool 文件（`/proc/<pid>/fdinfo` 的 `mnt_id`+`ino` 匹配）。授权 MUST NOT 取自对端消息字段。
accept SHALL 有期限与次数上限。
`SO_PEERCRED`/`peer_cred` 在钉住工具链上不稳定（E0658），故**连接对端本身不被认证**；MUST 在代码注释、ADR
与报告中明确声明该残余缺口，不得表述为完整 IPC 认证或设备授权。

#### Scenario: Declared holder connects
- **WHEN** root 声明的 pid 对应进程存在、属 owner uid 且持有 pool 文件，并且存在连接触发
- **THEN** 该 role 获得授权，可取得 ticket 并提交帧

#### Scenario: Undeclared or foreign identity
- **WHEN** 声明 pid 不存在、uid 不符或不持有 pool 文件
- **THEN** 不产生任何授权、ticket 或提交，连接被关闭

### Requirement: Authorization-gated control operations

取得授权前，`reserve`、`commit`、`producer_file`、`read_frame` MUST 拒绝（PermissionDenied），且不得推进槽位状态；`disconnect` SHALL 撤销对应 role 授权并退休整池。

#### Scenario: Unverified consumer reads
- **WHEN** consumer 未通过身份校验即请求读取
- **THEN** 拒绝且不返回像素、不产生 Ready 证据

### Requirement: Consumer-side standby reasons

消费者 SHALL 在租约无效时不读取像素并报告 `LeaseLost`；`write_started` 报告 `NoFrame`；peer 报错报告 `Retired`；ticket 校验、长度或 SHA256 失败报告 `IntegrityFailure`；epoch 不满足报告 `StaleEpoch`。成功消费 MUST 返回 owned 像素且 MUST NOT 产生 `PlaybackAck`。

#### Scenario: Lease lost before consumption
- **WHEN** 消费者收到帧事件但租约无效
- **THEN** 不读取共享载荷，返回 `Standby(LeaseLost)`

#### Scenario: Epoch regression
- **WHEN** 提交事件的 epoch 不高于已投递/最低要求 epoch
- **THEN** 返回 `Standby(StaleEpoch)`，不投递

### Requirement: Broker-boundary honesty

实现与文档 MUST 声明句柄交付仍为 spawn 继承，SCM_RIGHTS 与 Windows 命名管道句柄传递未实现；授权校验 MUST NOT 被表述为完整 IPC 认证或设备授权。

#### Scenario: Capability absence on other platforms
- **WHEN** 在非 Linux 平台调用 broker 入口
- **THEN** 返回 `CapabilityNotReady`，不伪造后端或成功
