## ADDED Requirements

### Requirement: Explicit approved local playback
系统 SHALL 保留不播放的文件预览，并允许操作者显式播放同一已批准候选；缺设备后端、陈旧代次、过期/未批准或事实变化 SHALL 拒绝，不自动重复付费合成。

#### Scenario: Approved candidate reaches local device
- **WHEN** 操作者在Windows选择本机播放已批准候选
- **THEN** 同一批准文本经TTS与真实2D输出交给keeper，界面只报告设备提交，不冒充硬件完成

#### Scenario: Revoke or unsupported target
- **WHEN** 静音、审批失效、事实变化或平台不支持
- **THEN** 拒绝或有界停止本机输出，重复命令不重新合成
