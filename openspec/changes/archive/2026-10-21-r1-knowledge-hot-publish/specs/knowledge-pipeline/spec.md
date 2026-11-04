## ADDED Requirements
### Requirement: Knowledge activation is an explicit revoked epoch boundary
普通知识发布 SHALL 不自动改写会话冻结绑定。显式热发布 SHALL 先静音并持久化推进epoch、撤销旧审批与任务，再经新知识准备和持久化确认激活新绑定；旧快照保持不可变，能力/Provider绑定的维护语义不变。
#### Scenario: An active snapshot changes without a hot publication command
- **WHEN** 新快照发布但未执行显式热发布
- **THEN** 会话继续绑定原冻结ID，不查询latest或偷偷换用新证据
#### Scenario: An operator explicitly activates prepared knowledge
- **WHEN** 已撤权代次、准备内容和宿主持久化知识完全匹配
- **THEN** 新人工辅助任务使用新绑定，旧审批不复用，失败不自动恢复旧输出
