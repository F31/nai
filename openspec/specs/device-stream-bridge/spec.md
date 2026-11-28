# device-stream-bridge Specification

## Purpose
将同一流式语音源交独立认证音频设备，并按真实设备时钟呈现有界GPU帧；分别验证源完整性、工程提交和人工可听可见的同步效果。

## Requirements

### Requirement: 外部PCM源完整性
外部源 SHALL 有界且复检源样本及EOS摘要，断开/超限必须取消，不额外调用模型。
#### Scenario: 通道断开
- **WHEN** 未收到合法EOS的外部源断开
- **THEN** 任务终止且不得报告播放完成。

### Requirement: 设备主时钟与真实帧
窗口 SHALL 仅按认证设备时钟选择已验证且已入队帧，任务取消或时钟过期显示安全帧。
#### Scenario: 有限预缓冲单句
- **WHEN** 一次真实TTS输出进入GPU、帧入调度器且满足配置预缓冲
- **THEN** 同源音频交独立设备播放，报告实际played样本及窗口提交，不授连续实时通过。
