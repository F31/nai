## ADDED Requirements

### Requirement: Remote gateway construction rejects invalid policy
`RemoteGateway::new` SHALL 拒绝 `source_id` / `policy_ref` / `permission_revision` 任一为空、`allowed_kb_ids` 为空、或 `unknown_revision_ttl_ms` 为 0 或 >5000 的 policy，返回 `Invalid` 且不触网。

#### Scenario: Malformed policy is rejected at gateway construction
- **WHEN** policy 的 `source_id` / `policy_ref` / `permission_revision` 为空、`allowed_kb_ids` 为空、或 `unknown_revision_ttl_ms` 为 0 或 >5000
- **THEN** `RemoteGateway::new` 返回 `Invalid` 且不发起任何网络请求

### Requirement: Remote knowledge client construction rejects invalid args
`RemoteKnowledgeClient::new` SHALL 拒绝空 endpoint、endpoint 长度 >2048、`timeout` 为零或 >600 秒、`max_retries` >10，返回 `Invalid` 且不建立 http client（不触网）。

#### Scenario: Malformed client args are rejected at client construction
- **WHEN** `endpoint` 为空 / 长度 >2048、`timeout` 为零 / >600 秒、或 `max_retries` >10
- **THEN** `RemoteKnowledgeClient::new` 返回 `Invalid` 且不建立 http client

### Requirement: Remote sync rejects malformed documents without network
`RemoteGateway::sync` SHALL 仅接受非空、≤16 个、且每个文档 `content` 长度 ≤65536 字节的文档集；否则 SHALL 以 `Unauthorized` 拒绝且不向远端 `/sync` 端点发起请求。

#### Scenario: Malformed sync documents are rejected without network
- **WHEN** `sync` 文档集为空、超过 16 个、或任一文档 `content` 长度 >65536 字节
- **THEN** `RemoteGateway::sync` 返回 `Unauthorized` 且不向远端 `/sync` 端点发起请求

### Requirement: Remote capabilities requires a search or answer first operation
`RemoteGateway::capabilities` SHALL 仅当 policy 首个 `allowed_operations` 为 `Search` 或 `Answer` 时才查询远端 capabilities，否则 SHALL 以 `Unauthorized` 拒绝且不触网。

#### Scenario: Capabilities requires a search or answer first operation
- **WHEN** policy 首个 `allowed_operations` 不是 `Search` 或 `Answer`（如 `Sync` / `Delete`）
- **THEN** `RemoteGateway::capabilities` 返回 `Unauthorized` 且不查询远端 capabilities
