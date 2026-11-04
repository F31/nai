# model-api-test Specification

## Purpose
为操作员提供模型设置内受预算、可取消且无业务资料传输的单次API响应测试，并区分有限脱敏失败原因。测试复用唯一writer、预算预留和取消结算，显示有界正文与费用状态；结果不授予业务备稿、事实正确或直播输出能力。

## Requirements

### Requirement: Explicit bounded model API test
系统 SHALL 只在操作员确认已保存模型和本次预算后发起一次固定内容API测试，复用权限、预算、deadline、取消及未知费用恢复；不得自动重试或创建业务产品与建议。
#### Scenario: Saved configuration or budget changes
- **WHEN** 用户编辑未保存字段、更换模型或预算不足
- **THEN** 不调用测试，提示保存/确认或具体限制；测试通过不授权后续生成。

### Requirement: Finite response outcomes
系统 SHALL 区分HTTP认证/限流/服务错误、超时、响应格式错误、输出截断及缺正文，并显示有界脱敏正文和费用状态。
#### Scenario: Late or malformed response
- **WHEN** 测试取消、上下文失效或返回缺正文/截断内容
- **THEN** 不显示成功，不恢复已离开的结果，不释放已发送未知费用，不产生可采用候选或直播输出。
