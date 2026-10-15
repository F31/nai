## ADDED Requirements

### Requirement: Remote search returns unavailable when retry backoff exceeds remaining budget
远端客户端在重试退避时长不小于当前 `Operation` 剩余预算时 SHALL 立即返回 unavailable，不得等待退避耗尽后被超时中断。

#### Scenario: Backoff that does not fit the remaining budget yields unavailable at once
- **WHEN** 远端返回 429 且 `Retry-After` 大于当前 `Operation` 的剩余预算
- **THEN** 检索结果为 `RetrievalStatus::RetrievalUnavailable`，且立即返回（不耗时等待退避）

### Requirement: Remote search stops retrying after the retry budget is exhausted
远端客户端在尝试次数达到 `max_retries` 上界后 SHALL 停止重试并返回 unavailable，尝试次数 MUST 有界。

#### Scenario: Retry budget exhaustion yields unavailable with a bounded attempt count
- **WHEN** `max_retries = 1` 且远端持续返回可重试状态（429，退避为 0）
- **THEN** 检索结果为 `RetrievalStatus::RetrievalUnavailable`，且尝试次数恰为 2（`max_retries + 1`）

### Requirement: Remote search retries server errors with exponential backoff and ignores Retry-After
远端客户端对 5xx 响应 SHALL 使用指数退避并**忽略** `Retry-After` 头（`Retry-After` 仅用于 429）。

#### Scenario: A 5xx with a large Retry-After is still retried
- **WHEN** 远端首次返回 503 且带 `Retry-After: 100`，随后返回 200，且 `Operation` 剩余预算充足
- **THEN** 客户端仍会重试并最终成功，尝试次数为 2（而非因 100s 退避而直接放弃）

### Requirement: Remote search rejects a response body beyond the read cap
远端客户端 SHALL 拒绝超过 8 MiB 读取上限的响应体并返回 invalid，不得无上限缓冲。

#### Scenario: A response body over 8 MiB is rejected as invalid
- **WHEN** 远端返回一个合法但超过 8 MiB 的响应体
- **THEN** 检索结果为 `Err(RemoteError::Invalid)`（而非被完整缓冲后正常解析）
