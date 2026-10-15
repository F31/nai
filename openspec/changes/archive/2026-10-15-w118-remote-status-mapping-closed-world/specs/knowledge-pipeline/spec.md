## ADDED Requirements

### Requirement: Remote search maps partial body status to RetrievalPartial
远端检索回包 `status` 为 `partial` 时，`parse_reply` SHALL 把结果状态映射为 `RetrievalStatus::RetrievalPartial`。

#### Scenario: partial body status maps to RetrievalPartial
- **WHEN** 远端 `/search` 返回 HTTP 200 且 body `status` 为 `"partial"`
- **THEN** 检索结果状态为 `RetrievalStatus::RetrievalPartial`

### Requirement: Remote search maps timeout body status to RetrievalTimeout
远端检索回包 `status` 为 `timeout` 时，`parse_reply` SHALL 把结果状态映射为 `RetrievalStatus::RetrievalTimeout`。

#### Scenario: timeout body status maps to RetrievalTimeout
- **WHEN** 远端 `/search` 返回 HTTP 200 且 body `status` 为 `"timeout"`
- **THEN** 检索结果状态为 `RetrievalStatus::RetrievalTimeout`

### Requirement: Remote search maps unavailable body status to RetrievalUnavailable
远端检索回包 `status` 为 `unavailable` 时，`parse_reply` SHALL 把结果状态映射为 `RetrievalStatus::RetrievalUnavailable`。

#### Scenario: unavailable body status maps to RetrievalUnavailable
- **WHEN** 远端 `/search` 返回 HTTP 200 且 body `status` 为 `"unavailable"`
- **THEN** 检索结果状态为 `RetrievalStatus::RetrievalUnavailable`

### Requirement: Remote search maps unauthorized body status to RetrievalUnauthorized
远端检索回包 `status` 为 `unauthorized` 时，`parse_reply` SHALL 把结果状态映射为 `RetrievalStatus::RetrievalUnauthorized`。

#### Scenario: unauthorized body status maps to RetrievalUnauthorized
- **WHEN** 远端 `/search` 返回 HTTP 200 且 body `status` 为 `"unauthorized"`
- **THEN** 检索结果状态为 `RetrievalStatus::RetrievalUnauthorized`
