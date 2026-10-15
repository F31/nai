## ADDED Requirements

### Requirement: Session-ended memory is retained until its retention expires

存储层 SHALL 保证场次记忆跨会话保留：记忆记录按 `expires_at_ms` 保留，MUST NOT 因会话结束被清空。只有超过保留期的记录方可过期移除。

#### Scenario: No session-end purge exists

- **WHEN** 一个会话结束
- **THEN** 存储层不触发任何清空记忆记录的动作；旧场记录仍可按其 session 隔离读取（直至保留期）
