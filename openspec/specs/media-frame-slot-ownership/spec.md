# media-frame-slot-ownership Specification

## Purpose
建立控制与帧数据分离的有界slot所有权、代次隔离与断连撤销前置，验证真实工程数据面而不授予设备或生产IPCReady。

## Requirements

### Requirement: Bounded single-owner frame slots
槽位owner SHALL限制数量/单槽/总bytes，按Free/Writing/Ready/Reading转换；ticket MUST绑定pool/session/epoch/双方身份/slot代次/整份metadata。consumer释放前禁止覆盖，未知身份或陈旧ticket不得推进状态。
#### Scenario: Consumer holds a slot
- **WHEN** consumer已取得Reading票据尚未release
- **THEN** 不得重新reserve该槽；释放后原票据因slot代次变化不能commit/read/release新内容

### Requirement: Commit integrity and retirement fencing
commit/read SHALL校验精确RGBA8长度、声明metadata、SHA256及owner单调期限。超时/断连/数据完整性失败 MUST退休整池，不重用可能被旧peer访问的内存，旧ticket不得重新授权。
#### Scenario: Producer dies during write
- **WHEN** 真实producer在部分写入后被强杀而未commit
- **THEN** 不把partial bytes作为Ready帧；断连后整池退休，新池identity不同，旧FD/票据不能改变新池
#### Scenario: Corrupted shared payload
- **WHEN** 共享载荷被截断或SHA256不匹配
- **THEN** 不返回可消费帧，明确失败并退休整池

### Requirement: Explicit diagnostic backend boundary
Linux tmpfs-file-drill SHALL仅显式启动、使用私有当前owner资源及继承FD、Protobuf元信息通知与独立二进制载荷。MUST不通过JSON/Base64控制总线传帧、不伪造Ready/播放ACK/Windows映射认证；未知后端明确CapabilityNotReady。
#### Scenario: Engineering transfer succeeds
- **WHEN** 演练真实子进程完成RGBA写入且owner验证成功
- **THEN** 只记录owned帧传输与槽位证据，不代表keeper提交、设备播放或正式IPC授权

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

### Requirement: Render worker produces real pixels into the reserved slot

render-worker SHALL 在显式 `--render-drill` 下工作：池 FD 经继承的标准流交付、PCM 以原始字节输入（MUST NOT 经 JSON/Base64 控制总线）、ticket 由 root 预留并校验；像素 MUST 由 2D 插件真实生成并按 ticket 的槽位偏移写入。MUST NOT 产生 `PlaybackAck`、声明 Ready 或授予输出授权。

#### Scenario: Real frame committed
- **WHEN** render-worker 依据合法 ticket 生成并写入像素
- **THEN** 输出 `write_started` 与 `committed` 事件，owner 按其账本接受提交

#### Scenario: Frame rejection
- **WHEN** ticket 过期、epoch 不符或像素长度/hash 与声明不符
- **THEN** 输出 error 事件并非零退出，owner 不接受提交

### Requirement: Producer rechecks epoch, cancel and budget

render-worker SHALL 在渲染与写入前复检 epoch、取消与期限预算；任一不满足 MUST 停止写入并显式失败，不得写入陈旧或超额数据。

#### Scenario: Stale epoch
- **WHEN** ticket 的 epoch 低于当前会话 epoch
- **THEN** 不写入池，报告失败
