# Proposal: W114 — 远端网关构造期与输入边界闭世界守卫

## Why
W113 闭合了 `RemoteGateway::delete` 的传输契约判定面（删除目标边界 + 回执透传枚举闭世界）。但
远端网关的**构造期**与**输入边界**仍有四处「已声明、零判据」的不变量，回归即可悄悄把非法
policy / 参数放行进构造，或把非检索能力的 policy 暴露给上层：

- `RemoteGateway::new`（`remote.rs:518-526`）对 policy 身份字段与 `unknown_revision_ttl_ms`
  的校验，既有的 `gateway()` 辅助函数只用合法 policy `.unwrap()`，无任何非法变体测试。
- `RemoteKnowledgeClient::new`（`remote.rs:67-74`）对 endpoint / timeout / max_retries 的
  校验，既有的 `Mock::client()` 只传合法参数，零判据。
- `RemoteGateway::sync` 输入边界（`remote.rs:639-647`）：documents 空集 / >16 / 单文档
  `content > 65536`。既有的 `sync_and_delete_are_gated_single_attempt_and_return_receipts`
  只钉了 `allow_document_upload` / `allow_third_party_retention` 两条**门控开关**，文档形状
  边界零判据。
- `RemoteGateway::capabilities` 首操作守卫（`remote.rs:571-581`）：要求 policy 首个
  `allowed_operations` 为 `Search` 或 `Answer`，否则拒绝。既有测试的首 op 都是 `Search`，
  零判据。

这四处都是「已声明的不变量」但缺显式守卫——与 W113 同形的判定面缺口。

## What Changes
只补可测判据，把四处已声明不变量钉成显式闭世界守卫；**零生产代码改动**。

- `crates/nai-knowledge-core/tests/remote.rs` 新增 4 项（W114，已本地跑绿）：
  - `remote_gateway_new_rejects_invalid_policy`（policy 六类非法变体 ⇒ `Invalid`，不触网）；
  - `remote_knowledge_client_new_rejects_invalid_args`（endpoint/timeout/max_retries 五类非法变体 ⇒ `Invalid`，不触网）；
  - `remote_sync_rejects_malformed_documents_without_network`（空集 / >16 / content>65536 ⇒ `Unauthorized`，并断言仅合法调用触网）；
  - `remote_capabilities_rejects_when_first_allowed_op_not_search_or_answer`（首 op 非 Search/Answer ⇒ `Unauthorized`，不触网）。
- `knowledge-pipeline` spec 新增 4 条 Requirement（各 1 Scenario），把构造期与输入边界闭世界纳入基线。

## Modified Capabilities
- `knowledge-pipeline`（远端知识契约与撤销）：追加远端网关构造期与输入边界闭世界守卫。

## Impact
- 无运行时行为变化（判定面闭合，不引入新执行体、不改 `remote.rs`）。
- 无契约字段改动、无迁移、无 schema 改动。
- registry：远端网关构造/边界守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。
- 测试计数：`tests/remote.rs` 的 `#[tokio::test]` 由 12 增至 16（W114 +4）。
- spec：新增 4 条 Requirement（各 1 Scenario）。
