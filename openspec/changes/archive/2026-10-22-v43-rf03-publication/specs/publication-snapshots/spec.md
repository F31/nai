## Purpose
为V43-RF03建立私有纯内容发布、字段影响复核与持久事实版本撤销基础。精确引用和有期限的影响报告约束发布事务，普通草稿更新保留仍有效旧快照，紧急拒绝意图在审计失败及进程重启后继续生效；发布状态与真实输出完成证据严格分离。
## ADDED Requirements
### Requirement: Publication binds a checked immutable closure
发布 SHALL 核验节目/产品/主持/系列的类型与精确版本/hash闭包、必讲事实绑定、人工确认和有效期；引用快照不可变，只授予明确的私有ContentOnly用途，不授予交付或媒体输出许可。
#### Scenario: Missing or invalid publication input
- **WHEN** 引用缺失、跨工作区、陈旧、类型错误、事实未核对/过期或必讲点无绑定
- **THEN** 发布拒绝，不产生半提交快照/outbox/成功审计。
### Requirement: Reviewed impact cannot outlive its graph watermarks
报告 SHALL 绑定主体、角色revision、输入闭包、图水位、有效期限与确定/可能影响；未知依赖不得视为无影响。发布事务 SHALL 重查全部水位和报告哈希，发布需Admin显式确认。
#### Scenario: Context changes after review
- **WHEN** 报告产生后草稿、角色、会话或交付状态改变，或报告过期
- **THEN** 发布拒绝并要求重新复核，不使用已缓存成功结论。
### Requirement: Revocation persists independently of business audit
紧急撤销 SHALL 先收紧快照许可并持久化独立拒绝意图，再提交业务撤销记录与审计。审计失败或重启不得恢复旧许可；普通草稿编辑不撤销仍有效旧快照；重放与读取每次反映当前有效性。
#### Scenario: Audit fails after revocation intent
- **WHEN** 撤销审计事务失败并重启
- **THEN** 快照仍不可用，历史内容/hash和未知输出范围不被改写为已撤回。
