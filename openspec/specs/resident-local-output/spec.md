# resident-local-output Specification

## Purpose
为Windows本机采集保持会话内稳定的输出窗口，在逐句重新授权和独立失联静音的前提下连接审批内容与真实音画设备，为后续直播录制提供可验证输出增量。

## Requirements

### Requirement: 会话常驻输出窗口
系统 SHALL 在同一引擎会话内复用受监督的 Windows 输出进程及窗口，每句独立校验审批、哈希、事实版本、epoch和短租约，句间静音待机。
#### Scenario: 两句独立批准内容
- **WHEN** 操作员依次播放两个不同已批准候选
- **THEN** 同一 keeper PID 和窗口完成两次提交，句间不重复调用Provider或重播
#### Scenario: 控制失联
- **WHEN** 私有管道关闭或播放租约不再续期
- **THEN** 音频撤权，未完成内容不能报告硬件完成，输出不会自动恢复
#### Scenario: 有界输入
- **WHEN** 单句超过20秒、帧超过64MiB或队列溢出
- **THEN** 拒绝并保持静音，不截断成成功
#### Scenario: 播放中人工撤权
- **WHEN** 播放中静音或人工接管使当前epoch失效
- **THEN** keeper撤销当前句并回静音待机，确认超时则被回收，旧续租不能自行重播

### Requirement: 音频会话与许可待机图
系统 SHALL 在常驻keeper中只打开一条输出stream，逐句使用独立授权与计数，句间/撤权回到合法中性形象和零音频。
#### Scenario: 两句与撤权
- **WHEN** 两句已批准内容依次播放并发生人工撤权
- **THEN** stream打开次数仍为1，旧句停止且待机图为已准入素材生成的闭嘴图，不自动重播
#### Scenario: 素材许可不可用
- **WHEN** 生成待机图时素材许可或任务期限检查失败
- **THEN** 拒绝生成，不以旧候选画面冒充合法待机图
#### Scenario: 父进程失联的待机保留
- **WHEN** 已安装合法中性待机图后父进程控制管道EOF
- **THEN** 停止旧语音并保留静音待机窗口至本地关闭或会话期限，不恢复旧授权或重播

### Requirement: 有界分帧文件读取
系统 SHALL 对v2逐帧文件按已绑定摘要校验，每帧最多16MiB、单句最多512MiB/20秒，并限制帧数和描述符大小。
#### Scenario: 正常分辨率超64MiB
- **WHEN** 合法v2帧文件超过64MiB但未超单句配额
- **THEN** 能逐帧读取而不把整句RGBA装入内存
#### Scenario: 读取前篡改
- **WHEN** 文件验证后某帧被修改或截断
- **THEN** 该帧展示前校验失败，当前语音停止并回静音待机

### Requirement: Explicit neutral output prewarm has no speech authority
桌面 SHALL 提供显式静默预热，复用当前引擎准入的合法闭嘴素材，打开常驻窗口与单条静音音频stream；MUST NOT 调用Provider、构造候选审批或签发语音租约。媒体字节不得进入控制总线。预热忙碌、设备不可用、许可/摘要/期限/epoch失效时 SHALL 如实失败，不以控制ACK表示就绪。
#### Scenario: Silent preparation succeeds
- **WHEN** 有效会话显式预热且设备确认零语音样本、单stream和匹配待机摘要
- **THEN** 界面才显示静默待机，后续真实审批播放复用该进程和stream，不产生硬件完成ACK
#### Scenario: Tampered or revoked preparation
- **WHEN** 媒体描述符/像素被篡改或预热时epoch变化
- **THEN** 拒绝该任务并有界回收未确认输出，不恢复语音或自动重试
#### Scenario: Replay and concurrent preparation
- **WHEN** 相同命令重放或已有语音任务占用输出
- **THEN** 重放不再次执行，忙碌不进入无界等待；立即静音仍可执行

### Requirement: Device clock admission before completion integration
The implementation SHALL validate device position using the same stream identity and a stable nonzero clock frequency, converting position to frames with checked integer arithmetic. Regressions, frequency changes and positions beyond submitted frames SHALL be rejected. QPC and elapsed time SHALL NOT substitute device position.
#### Scenario: Invalid cursor
- **WHEN** a clock regresses, changes frequency or exceeds committed frames
- **THEN** it is rejected without producing completion evidence.
#### Scenario: Explicit native admission
- **WHEN** the Windows admission test is explicitly invoked
- **THEN** it drives only bounded silent buffers, observes actual clock progress and verifies stopped position without producing PlaybackAck or advancing the program.

### Requirement: Resident audio submission requires actual device position
The resident backend SHALL maintain a single safe WASAPI stream and derive played frames from IAudioClock position and frequency. Successful writes SHALL bind each cue tail to absolute stream frames and a denial-only revocation/epoch/deadline fence. Only subsequent valid device position at or beyond that tail SHALL produce the cue played count. Buffers and queues SHALL be bounded, and P0 SHALL remain independent of model/storage locks.
#### Scenario: Copied or predicted time alone
- **WHEN** samples are copied but device position has not reached their committed tail
- **THEN** full device progress is not reported and submission is not successful.
#### Scenario: Cancellation or replacement
- **WHEN** revocation, expiry, epoch change or cue failure invalidates the pending tail
- **THEN** later device progress cannot produce its receipt or transfer it to a new cue.

### Requirement: Host verifies native progress identity
The private status SHALL carry optional device played frames, an observation flag and backend identity. Both host success exits SHALL require exact descriptor identity and independently expected full counts from the known backend. Missing or predicted-only evidence SHALL be rejected. Actual audio progress SHALL NOT create full A/V PlaybackAck or automatic program advancement.
#### Scenario: Old or forged status
- **WHEN** backend identity, observed flag or expected full played count is missing or mismatched
- **THEN** the host rejects successful submission.

### Requirement: Video submission follows actual cue audio device position
本机常驻画面 SHALL 按当前cue实际设备播放位置选帧，不按缓冲复制位置、墙钟或QPC选帧。位置 MUST 单调且不超过已复制与独立预期总帧数；窗口提交前 SHALL 复检取消/租约/健康，失败不登记成功尾帧。
#### Scenario: Audio has been copied ahead
- **WHEN** copied已到尾而device played仍在中间
- **THEN** 画面仅选择device played对应帧，不提前输出尾帧成功证据。
#### Scenario: Revoke or stream regression
- **WHEN** 读取像素后撤权，或设备位置回退/越界
- **THEN** 不提交授权语音画面或成功尾帧，输出转静音待机/失败。

### Requirement: Host verifies cue video tail submission independently
本机submitted SHALL 同时包含匹配描述符、完整实际音频进度、独立预期最后视频帧索引及对应设备总帧数。窗口计数或旧缺省字段 MUST NOT 替代尾帧证据；尾帧提交 SHALL NOT 产生完整音画PlaybackAck或自动节目进度。
#### Scenario: Missing or mismatched tail
- **WHEN** 索引/设备位置/提交标志缺失或与宿主预期不符
- **THEN** 宿主与人工确认拒绝该成功报告。
