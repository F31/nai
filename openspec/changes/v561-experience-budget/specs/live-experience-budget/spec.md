## ADDED Requirements

### Requirement: 首声按完整体验计时
就绪后首声 SHALL 以提交到设备首样本≤3000ms验收；TTS首PCM≤800ms SHALL 仅作优化目标，冷启动准备单独记录。文本已备和包含LLM生成两类 SHALL 显式区分，未测阶段不得推定通过。
#### Scenario: 已备文本首声
- **WHEN** 就绪后提交已备文本且设备时间线和宿主提交观察完整
- **THEN** 计算提交首声的保守上界，标注未包含LLM；不得以1700ms首PCM直接判完整首声通过。

### Requirement: 单槽预取的连续体验
系统 SHALL 保持单TTS、至多一个未来句及有界源/帧缓存，播放当前句时准备下一句；连续句间实际设备gap目标≤300ms。缓存 SHALL NOT 用来掩盖长期生成慢于播放或伪造连续证据。
#### Scenario: 源验证迟于前句设备完成
- **WHEN** ready_count为0但后句已有可播放分块
- **THEN** 以真实设备首尾重算停顿，不只凭ready_count判定体验；若gap超300ms仍报告未达标。
#### Scenario: 时间线不完整
- **WHEN** 缺少设备样本时刻、提交观察或完成样本不一致
- **THEN** 拒绝授首声或连续通过，保留失败原因，不填零。

### Requirement: 短程证据与安全边界
有限连续体验 SHALL 记录真实调用数、单窗口/单设备、逐句样本和停止结果，SHALL NOT 授30分钟或OBS通过。停止、接管和旧epoch门 SHALL 不因3秒预算改变而放宽。
#### Scenario: 四句完成
- **WHEN** Windows真实四句全部完成
- **THEN** 只形成短程证据，人工听感、长稳、LLM链路及插问未执行时保留pending。
