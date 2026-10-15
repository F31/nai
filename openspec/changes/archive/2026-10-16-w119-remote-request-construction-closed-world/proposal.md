# Proposal: W119 — 远端出站请求构造闭世界守卫（凭证头 + endpoint 规范化）

## Why
W118 闭合了 `parse_reply` 的 body status 映射。把视线从「回包解析」移到**出站请求构造**：
`RemoteKnowledgeClient::request()`（`remote.rs:100-112`）在每次 POST 前组装 URL 与请求头，其两条不变量
**整段零判据**——既有测试经 `Mock::client()` 恒传 `api_key: None`、且 endpoint 形如 `http://127.0.0.1:{port}`
（无尾斜杠），故：

- **凭证头（安全相关）**：`api_key = Some(secret)` ⇒ 每个出站请求 MUST 带
  `Authorization: Bearer <key>`（`remote.rs:105-110`）。**从未被任何测试观测**——若该头被静默丢弃，
  请求会以「未认证」身份发出而**无人察觉**（远端可能以匿名/降级身份应答，或全部 401 而定位不到根因）。
  反向闭世界同样零判据：`api_key = None` ⇒ MUST 不带 `Authorization` 头（不凭空造凭证）。
- **endpoint 规范化**：`endpoint.trim_end_matches('/')`（`remote.rs:103`）⇒ 拼出的路径是 `/search` 而非
  `//search`。既有 endpoint 均无尾斜杠，该分支从未被执行 ⇒ 零判据。

此缝与 W113–W118 同形：判定面闭合、零生产改动、确定性离线。

## What Changes
只补可测判据，把出站请求构造的上述不变量钉成显式闭世界守卫；**零生产代码改动**。

- `crates/nai-knowledge-core/tests/remote.rs` 新增 4 项（W119，已本地跑绿），用**观测型 Mock**（记录
  实际出站请求的 URL 与 `Authorization` 头，而非只看响应）：
  - `remote_request_sends_bearer_authorization_header_when_api_key_present`（精确值 `Bearer <key>`）；
  - `remote_request_omits_authorization_header_when_api_key_absent`（闭世界：无头）；
  - `remote_request_normalizes_trailing_slash_in_endpoint`（单尾斜杠 ⇒ `/search`）；
  - `remote_request_normalizes_repeated_trailing_slashes_in_endpoint`（多尾斜杠 ⇒ `/search`）。
- `knowledge-pipeline` spec 新增 4 条 Requirement（各 1 Scenario），把出站请求构造闭世界纳入基线。

## Modified Capabilities
- `knowledge-pipeline`（远端知识契约与撤销）：追加远端出站请求构造（`Authorization: Bearer` 凭证头 + endpoint 尾斜杠规范化）闭世界守卫。

## Impact
- 无运行时行为变化（判定面闭合，不引入新执行体、不改 `remote.rs`）。
- 无契约字段改动、无迁移、无 schema 改动。
- registry：守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。
- 测试计数：`tests/remote.rs` 的测试由 32 增至 36（W119 +4）。
- spec：新增 4 条 Requirement（各 1 Scenario）。
