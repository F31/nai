## Purpose
把「提交（水位）」与「播放完成（ACK）」在类型与日志上分离，并把播放通道的 ACK 绑定与未完成结局写成可执行、可核验的规则。

## ADDED Requirements
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
