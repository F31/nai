## Purpose
补齐工程引擎的broker授权/keeper单帧消费装配与有界故障处置，不提升生产IPC或设备验收状态。
## ADDED Requirements
### Requirement: Broker-gated engine frame relay
media-drill SHALL由root声明真实spawn pid并按既有broker的/proc证据验证两端，再reserve/commit/pin/relay/release。consumer MUST只继承只读池FD；keeper MUST复检pool/epoch/live Rehearsal lease/允许utterance并保留owned像素，提供可关联seq/buffer_ref/SHA256的诊断，不产生PlaybackAck或Ready。
#### Scenario: Verified single frame reaches keeper
- **WHEN** 两端验证且worker在槽期限内提交
- **THEN** root接受commit后才转发；匹配keeper诊断前槽仍Reading不可覆盖
#### Scenario: Peer disappears or metadata changes
- **WHEN** peer退出/失去FD或票据与pool/live lease不符
- **THEN** 不成功转发/呈现，撤权或明确待机，不自动恢复
### Requirement: Bounded cancellable media supervision
engine SHALL在预热/授权/事件等待期间持续续租/observe并复检epoch/halt；阻塞worker管道 MUST不阻塞owner期限/停机。媒体失败 MUST先P0静音→撤权→critical记录，再有界回收child；日志失败不得回退静音。
#### Scenario: Worker stalls while preparing
- **WHEN** worker卡住或缺ready/commit，或halt/HTTP撤权到达
- **THEN** 有界退出等待与回收child；HTTP保持可达直到明确halt，不伪造成功帧
## MODIFIED Requirements
### Requirement: Explicit media drill boundary
`--media-drill` SHALL仅使用真实资产、tmpfs与子进程，经broker root声明/proc核验、reserve/commit及keeper读取owned像素；记录telemetry，故障明确记录并撤权。UDS对端仍未认证、句柄仍spawn继承，MUST NOT称为正式protected IPC/设备/PlaybackAck通过。
#### Scenario: Missing tmpfs or asset closure
- **WHEN** 非Linux、资产缺失或worker未按时就绪
- **THEN** 记录失败、撤权与回收child，不成功提交；控制服务可查询失效状态
#### Scenario: Drill commit
- **WHEN** keeper诊断与已接受commit匹配
- **THEN** 记录media_drill_committed含buffer_ref/SHA256/bytes/keeper观察，不声称播放ACK
### Requirement: Honest supervision boundary
Composition库不自行监听HTTP；apps/engine显式装配loopback工程服务。文档 MUST声明无设备/窗口/音频输出/PlaybackAck/生产IPC认证，演练不得表示生产Ready。
#### Scenario: Status reporting
- **WHEN** 装配报告状态
- **THEN** output_available保持false并给出CapabilityNotReady或待机原因，帧消费不提升设备状态
