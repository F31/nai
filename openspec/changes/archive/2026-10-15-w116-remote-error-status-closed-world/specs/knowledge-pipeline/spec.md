## ADDED Requirements

### Requirement: Remote error Unauthorized maps to RetrievalUnauthorized
`RemoteError::status()` SHALL 把 `RemoteError::Unauthorized` 映射为 `RetrievalStatus::RetrievalUnauthorized`。

#### Scenario: Unauthorized maps to RetrievalUnauthorized
- **WHEN** 取 `RemoteError::Unauthorized.status()`
- **THEN** 返回 `RetrievalStatus::RetrievalUnauthorized`

### Requirement: Remote error Unavailable maps to RetrievalUnavailable
`RemoteError::status()` SHALL 把 `RemoteError::Unavailable` 映射为 `RetrievalStatus::RetrievalUnavailable`。

#### Scenario: Unavailable maps to RetrievalUnavailable
- **WHEN** 取 `RemoteError::Unavailable.status()`
- **THEN** 返回 `RetrievalStatus::RetrievalUnavailable`

### Requirement: Remote error Invalid collapses to RetrievalUnavailable
`RemoteError::status()` SHALL 把 `RemoteError::Invalid` 映射为 `RetrievalStatus::RetrievalUnavailable`（非显然坍缩：不存在独立的 `RetrievalInvalid` 分支）。

#### Scenario: Invalid collapses to RetrievalUnavailable
- **WHEN** 取 `RemoteError::Invalid.status()`
- **THEN** 返回 `RetrievalStatus::RetrievalUnavailable`

### Requirement: Remote error Timeout and Cancelled collapse to RetrievalTimeout
`RemoteError::status()` SHALL 把 `RemoteError::Timeout` 与 `RemoteError::Cancelled` 都映射为 `RetrievalStatus::RetrievalTimeout`（两个不同语义的错误坍缩进同一状态）。

#### Scenario: Timeout and Cancelled collapse to RetrievalTimeout
- **WHEN** 取 `RemoteError::Timeout.status()` 与 `RemoteError::Cancelled.status()`
- **THEN** 两者都返回 `RetrievalStatus::RetrievalTimeout`
