# media-basics-timeline Specification

## Purpose
为 nai 媒体链路提供可移植、纯逻辑的样本时轴与录制/ACK 语义：单调媒体时钟、音频块校验、有界 PCM 缓冲、
跨采样率样本映射、水位提交、录制连续性与实际播放完成判定，使媒体时序可验证且不伪造设备完成。

## Requirements

### Requirement: Validated audio blocks with monotonic sequence
系统 SHALL 校验音频块 session/utterance/epoch/seq、采样率、声道、样本格式、sample_start/sample_count 与 end_of_stream；
同 utterance 内 seq 单调、块序号不跳跃，采样率/声道/格式不得中途变化；音频时长按样本数计算，审计 UTC 不当媒体时钟。

#### Scenario: Sequence gap or format change is rejected
- **WHEN** 音频块 seq 非单调重复、缺失、或采样率/声道/格式中途改变
- **THEN** 拒绝该块，不将其写入输出或录制

#### Scenario: end_of_stream is explicit and final
- **WHEN** end_of_stream 前还有块或结束后再写块
- **THEN** 拒绝，完整句段只有在显式结束与样本数一致后才视为完成候选

### Requirement: Monotonic media clock with sample position
系统 SHALL 以单调来源维护媒体时钟（sample_position + sample_rate + 有理 pts），位置只增不减；静音不会使位置倒退；
不同采样率分支的 sample_start 不得直接比较，必须经明确比例映射。

#### Scenario: Clock never rewinds on mute or silence
- **WHEN** 静音/句间插入静音或时钟暂停
- **THEN** sample_position 单调递增或保持，绝不回退

#### Scenario: Cross-rate mapping is explicit
- **WHEN** 模型分支 16k 与输出分支 48k 的样本需要对齐
- **THEN** 只使用按有理比例确定的样本映射，不直接比较两分支 sample_start

### Requirement: Bounded PCM buffering with water-level commit
系统 SHALL 用有界槽位/字节的 PCM 缓冲，写入超限返回容量错误而非静默丢弃；
获准首段音频与首批视频都达到水位才 commit；持续迟到可重复最近帧，持续无法跟上则取消当前句并降级，不允许长时间有声无嘴动。

#### Scenario: Overflow is a capacity error, not silent loss
- **WHEN** 写入超出槽位或字节上限
- **THEN** 返回容量错误，不声称零丢失也不截断入队

#### Scenario: Commit requires both audio and video water level
- **WHEN** 音频水位满足但首批视频未达水位
- **THEN** 不 commit，等待视频或按持续迟到策略取消/降级

### Requirement: Recording with monotonic PTS and continuous clock
系统 SHALL 按单调 PTS 录制而非到达时间；句间补静音/待机保持媒体时钟连续；录制默认不上传，且不以录制文件代替设备验收。

#### Scenario: Recording keeps clock continuous across utterances
- **WHEN** 两句之间有时间间隔
- **THEN** 录制写入确定静音补齐该间隔，PTS 单调不跳跃回退

### Requirement: Completion requires actual playback ACK
系统 SHALL 仅在实际播放完成 ACK（committed=true、actual_local_playback_completed=true、played_sample_count=样本总数）
上报 Completed；生成结束、完整下载或接收都不等同观众已听完；跨故障状态不明任务按取消处理，避免重复播报。

#### Scenario: Incomplete or partial ACK is not completion
- **WHEN** ACK 只确认提交、或样本数小于总数、或未承诺完整播放
- **THEN** 不标 Completed，保持进行中或取消，不重复播报

### Requirement: Playback lane completion binding
播放通道 SHALL 把每个 ACK 绑定到 `(output_id, utterance_id)` 与预期样本总数：未提交的 ACK MUST 保持 Pending 且 MUST NOT 携带非零样本数；已提交但样本数不足 MUST 保持进行中；`actual_local_playback_completed=true` 而样本数不等于预期总数、或身份不符、或未提交即声明完成 MUST 被拒绝；已取消的通道 MUST NOT 被任何后续 ACK 完成，以避免跨故障状态不明时的重复播报。

#### Scenario: Partial progress then completion
- **WHEN** 同一 utterance 先收到已提交的部分样本 ACK，再收到已提交且样本数等于预期总数的完成 ACK
- **THEN** 前次保持进行中，末次才置为 Completed

#### Scenario: Mismatched identity or count is refused
- **WHEN** ACK 的 output_id/utterance_id 不符，或声明完成但样本数与预期总数不同，或未提交即声明完成
- **THEN** 该 ACK 被拒绝，状态不进入 Completed

#### Scenario: Cancelled lane rejects a late complete ACK
- **WHEN** 通道已被取消后再收到完整播放 ACK
- **THEN** ACK 被拒绝且状态保持 Cancelled，不完成

### Requirement: Water-level commit precedes playback
播放通道 SHALL 只在音频水位与首批视频水位都达到后提交；任一水位未达 MUST NOT 提交。提交 MUST NOT 被等同于播放完成，也 MUST NOT 单独产生 `PlaybackAck` 语义。

#### Scenario: Audio only is not committable
- **WHEN** 音频水位已满足但首批视频帧未达水位
- **THEN** 不提交，保持等待（或按持续迟到策略取消/降级），不声明播放完成

### Requirement: Revocable bounded callback audio
系统 SHALL 只从完整 Preplay 复检及有序消费创建有界音频回调对象；回调 SHALL 独立检查 P0、代次与单调期限，不依赖控制线程轮询，且不得生成设备完成 ACK。

#### Scenario: Control thread stalls or owner disappears
- **WHEN** 控制线程停止续租、门控对象析构、换代或 P0 撤权
- **THEN** 旧回调输出静音，且不会因重签租约恢复

#### Scenario: Invalid audio or oversized callback
- **WHEN** PCM 非有限、越界、超预算或目标缓冲超过 10ms
- **THEN** 拒绝准入或清零输出，不将复制样本记为实际播放完成

### Requirement: Explicit native device smoke
系统 SHALL 只在显式设备演练入口打开本机音频和窗口，记录实际回调与窗口更新结果；缺设备或格式不支持 SHALL 明确拒绝，不生成设备完成 ACK。

#### Scenario: Native callback survives stalled window thread
- **WHEN** Windows 演练中主线程暂停超过600ms
- **THEN** 音频回调独立静音，报告观察到的零输出，不把窗口或回调提交当硬件完成

#### Scenario: Emergency mute or device unavailable
- **WHEN** P0 静音发生或输出设备不可用
- **THEN** 回调清零或报告具名不可用，失败不得保留旧成功报告

### Requirement: Isolated approved local media delivery
系统 SHALL 将有界原始媒体与Protobuf授权分离，输出端复检session/epoch/批准哈希/事实/租约；失联600ms内音频静音，不生成硬件完成ACK。

#### Scenario: Root stops renewing
- **WHEN** 根进程退出、EOF或停止续租
- **THEN** keeper独立撤权，不恢复旧音频块，退出或保持静音

#### Scenario: Tampered media or proof
- **WHEN** 文件长度/哈希、候选或审批绑定不符
- **THEN** 拒绝播放且报告明确失败
