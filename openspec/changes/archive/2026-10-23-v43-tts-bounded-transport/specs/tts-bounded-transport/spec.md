## ADDED Requirements
### Requirement: 有界TTS音频传输
适配器 SHALL 使用有界HTTPS合成、禁止重试与跳转，取消或超时后丢弃全部音频；仅接受经独立格式校验的24kHz单声道PCM16 WAV，不将音频装入JSON控制消息。
#### Scenario: 有效合成
- **WHEN** 返回完整且符合上限的WAV
- **THEN** 输出Rust owned PCM16与采样率，不自动播放，不推断usage或设备完成
#### Scenario: 取消或无效音频
- **WHEN** 请求取消、期限耗尽、HTTP失败或音频格式及长度不符
- **THEN** 返回有限错误分类和保守派发状态，不返回部分音频或供应商错误正文
### Requirement: 字符费用估算
策略 SHALL 对已知单价使用有界Unicode字符数和整数向上取整估算，未知价格不得当零。
#### Scenario: 未配置价格
- **WHEN** TTS单价未知
- **THEN** 拒绝估算，后续任务不能绕过预算执行
