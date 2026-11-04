## ADDED Requirements
### Requirement: Explicit single endpoint connectivity probe
系统 SHALL 仅在显式授权时测试指定LLM或TTS配置，固定模型/音色、预算与期限，禁止转备、重试或播放，并报告实际usage与未知金额。
#### Scenario: Test a backup directly
- **WHEN** 操作者选择备用配置测试
- **THEN** 仅该配置最多一次派发，主配置不调用，报告身份匹配
#### Scenario: Invalid speech or missing key
- **WHEN** 无凭据或TTS返回空/奇数/非PCM内容
- **THEN** 显式not_ready或failed，不产生音画或认证成功
