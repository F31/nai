## ADDED Requirements
### Requirement: Bounded health aware capability routing
系统 SHALL 分别为LLM/TTS选择健康供应商，主故障达到阈值后熔断、冷却后有界试探，每次请求固定模型/音色/预算身份。
#### Scenario: Primary fails after dispatch
- **WHEN** 主供应商调用失败且达到阈值
- **THEN** 本请求不跨供应商重发，后续显式请求选择健康备用，费用归属实际供应商
#### Scenario: Recovery and cancellation
- **WHEN** 主供应商冷却到期或调用因操作者取消结束
- **THEN** 到期主供应商可单次试探，成功后恢复；取消不增加供应商失败计数
