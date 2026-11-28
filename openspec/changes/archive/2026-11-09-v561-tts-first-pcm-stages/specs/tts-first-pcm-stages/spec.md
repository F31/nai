## ADDED Requirements

### Requirement: 首PCM诊断不改变合成语义
阶段观察器 SHALL 保留原生成器、异常和单飞行为，不记录合成文本/音频/密钥，不强制CUDA同步；记录有界并区分Python墙钟与GPU计时。

#### Scenario: 原合成失败
- **WHEN** 原factory或模型阶段抛出异常
- **THEN** 异常原样传播，仅记录无正文失败状态，不将部分音频当完成或自动重试。

#### Scenario: 解释联合首包
- **WHEN** 收集一次真实联合请求及资源记录
- **THEN** 依据首token/声学/前端实际证据给结论，证据不足处保留未知，不声称低延迟通过。
