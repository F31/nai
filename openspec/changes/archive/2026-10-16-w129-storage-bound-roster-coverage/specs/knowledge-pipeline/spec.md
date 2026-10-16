## ADDED Requirements

### Requirement: A candidate's evidence-reference bound is separately attributable
一条候选引用的证据条数 SHALL 受一个声明上限约束；恰为该上限时 MUST 被接纳，越界 MUST 以 `resource_exhausted` 具名拒绝，且该拒绝 MUST 与候选依赖条数、候选编码两条上限**可分别归因**。

#### Scenario: The bound falls on the reference count alone
- **WHEN** 提交一条证据引用恰为声明上限、依赖为零、编码远低于上限的候选
- **THEN** 写入被接受
- **WHEN** 提交一条证据引用比上限多一条、其余均在上限内的候选
- **THEN** 以 `resource_exhausted` 具名拒绝，理由指向证据引用上限
