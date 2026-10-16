## ADDED Requirements

### Requirement: The assembly root answers the activation gate from its real session
装配根 SHALL 用**自己持有的真实会话**回答升级激活门（`ActiveSessionGate`），MUST NOT 依赖 rehearsal 计数器或缓存计数；「活跃」MUST 定义为**在播**（lifecycle 为 Running 或 Draining）。

#### Scenario: On-air defers, off-air allows
- **WHEN** 装配根的真实会话被推进到 Running（或 Draining）
- **THEN** 激活门返回 deferred（活跃数为 1），`require_idle` 以 capability_not_ready 拒绝
- **WHEN** 该会话被推进到 Stopped
- **THEN** 激活门返回 allowed

### Requirement: An unobservable session fails the gate closed
装配根无法观测会话状态（会话锁忙或中毒）时 SHALL 返回错误，MUST NOT 报告「无活跃会话」。

#### Scenario: A busy session lock is reported, not degraded to zero
- **WHEN** 会话锁在激活门被调用时处于忙（竞争已就位）
- **THEN** 激活门返回 conflict 错误，而非 allowed
