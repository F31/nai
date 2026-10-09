## Purpose
建立控制与帧数据分离的有界slot所有权、代次隔离与断连撤销前置，验证真实工程数据面而不授予设备或生产IPCReady。
## ADDED Requirements
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
