## ADDED Requirements

### Requirement: Memory read scope is bound to the current session and does not cross sessions

组合根 SHALL 保证记忆读取的作用域绑定当前场：`scope_matches` SHALL 以 `record.session_id == scope.session_id` 隔离非观众记录、以 `viewer_scoped_id` 隔离观众记录。跨场的查询 SHALL 以具名 `OutOfScope` 拒绝，MUST NOT 就近匹配到别的作用域。

#### Scenario: A query from another session is refused by name

- **WHEN** 一条非观众记忆记录属于 session A，而查询的 scope.session_id 是 session B
- **THEN** 该记录以 `OutOfScope` 进入 `refused`，MUST NOT 被读回

#### Scenario: A viewer memory query for another viewer scope is refused by name

- **WHEN** 一条观众记忆记录的 viewer_scoped_id 是 V1，而查询的 viewer_scoped_id 是 V2
- **THEN** 该记录以 `OutOfScope` 进入 `refused`

### Requirement: Session-scoped memory records survive session end until retention expires

组合根 SHALL 保证场次记忆跨会话保留：记录按 `expires_at_ms` 保留，会话结束 MUST NOT 清空场次记忆。旧场的记录对旧场仍按 session 隔离可读，对新场 `OutOfScope`，但 MUST NOT 被删除。

#### Scenario: Records are not wiped when a session ends

- **WHEN** 一个会话结束、新会话开始
- **THEN** 旧场的记忆记录仍物理保留（按保留期），不因会话结束被清空
