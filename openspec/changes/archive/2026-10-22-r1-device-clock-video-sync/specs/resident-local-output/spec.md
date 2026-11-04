## ADDED Requirements
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
