## ADDED Requirements

### Requirement: Operation check reports cancellation before the deadline
`Operation::check` SHALL 先判断取消、再判断期限，使得同时被取消且已超期的操作 MUST 报告 cancelled 而非 deadline_exceeded。

#### Scenario: A cancelled and expired operation reports cancellation
- **WHEN** 一个操作既已被取消、其期限也已到达
- **THEN** `check()` 返回 cancelled（不得返回 deadline_exceeded）

### Requirement: The remaining budget never grows as time passes
`Operation::remaining` SHALL 随时间流逝单调不增，期限到达后 MUST 为零。

#### Scenario: Waiting consumes the remaining budget
- **WHEN** 连续两次读取剩余预算并在其间等待
- **THEN** 第二次的剩余预算 MUST 严格小于第一次，且不超过初始预算

### Requirement: stopped is exactly check failure
`Operation::stopped` SHALL 与 `check()` 失败**严格一致**（不引入独立的停止状态判定）。

#### Scenario: Agreement holds in live, cancelled and expired states
- **WHEN** 分别处于存活 / 已取消 / 已超期三种状态
- **THEN** `stopped()` 与 `check().is_err()` 在每种状态下 MUST 相等

### Requirement: Cancellation is shared across cloned operation handles
取消状态 SHALL 在所有克隆句柄间共享，任一克隆句柄上的取消 MUST 使全部句柄进入已停止状态。

#### Scenario: Cancelling through one handle stops every clone
- **WHEN** 克隆出一个操作句柄，并在原句柄上调用取消
- **THEN** 克隆侧也 MUST 观测到已停止，且两侧状态一致
