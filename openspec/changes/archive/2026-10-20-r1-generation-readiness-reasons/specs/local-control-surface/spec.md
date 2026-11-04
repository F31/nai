## ADDED Requirements
### Requirement: Actionable manual generation failures
手工生成失败 SHALL 保留错误码并给出受控阶段原因，不得将无知识命中、知识外发拒绝和空模型输出混淆，也不得自动绕过知识守门或重试付费请求。
#### Scenario: No matching knowledge
- **WHEN** 问题在当前发布知识中无命中
- **THEN** 说明未找到可用证据且未调用LLM，不产生候选
#### Scenario: Local only knowledge
- **WHEN** 命中知识不允许远端外发
- **THEN** 明确外发授权拒绝，不调用Provider、不自动更改权限
