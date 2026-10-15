## ADDED Requirements

### Requirement: Remote request sends Bearer authorization header when API key present
远端客户端在配置了 `api_key` 时 SHALL 为每个出站请求携带 `Authorization: Bearer <key>` 请求头，其值为 `Bearer ` 后接凭证原文。

#### Scenario: API key is sent as an exact Bearer header
- **WHEN** 以 `api_key = Some("test-api-key")` 构造远端客户端并发出检索请求
- **THEN** 观测到的出站请求头含 `Authorization`，其值逐字等于 `Bearer test-api-key`

### Requirement: Remote request omits authorization header when no API key
远端客户端在未配置 `api_key` 时 SHALL 不携带 `Authorization` 请求头，不得凭空构造凭证头。

#### Scenario: No API key means no Authorization header
- **WHEN** 以 `api_key = None` 构造远端客户端并发出检索请求
- **THEN** 观测到的出站请求**不含** `Authorization` 头

### Requirement: Remote request normalizes trailing slash in endpoint
远端客户端 SHALL 在拼接请求 URL 前去掉 endpoint 结尾的斜杠，使最终路径不含重复斜杠。

#### Scenario: Single trailing slash in endpoint is trimmed
- **WHEN** endpoint 以单个 `/` 结尾（如 `http://127.0.0.1:{port}/`）并发出检索请求
- **THEN** 观测到的请求路径为 `/search`（而非 `//search`）

### Requirement: Remote request normalizes repeated trailing slashes in endpoint
远端客户端 SHALL 去掉 endpoint 结尾的**全部**斜杠，而非仅去掉一个。

#### Scenario: Repeated trailing slashes in endpoint are trimmed
- **WHEN** endpoint 以多个 `/` 结尾（如 `http://127.0.0.1:{port}///`）并发出检索请求
- **THEN** 观测到的请求路径为 `/search`（而非 `///search`）
