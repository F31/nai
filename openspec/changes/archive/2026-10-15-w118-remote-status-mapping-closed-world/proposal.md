# Proposal: W118 — parse_reply 的 RemoteStatus → RetrievalStatus 闭世界映射守卫

## Why
W117 闭合了 `parse_reply` 的 `top_k` 截断。`parse_reply`（`remote.rs:395-411`）还有第二处判定面：
其 `RemoteStatus → RetrievalStatus` 的 6 臂映射（`remote.rs:396-403`）**大部分零判据**——既有测试经
`parse_reply` 只覆盖 `Complete` / `Empty` 两臂（200 body status 只出现过 `"complete"` / `"empty"`），
其余四臂 `Partial` / `Timeout` / `Unavailable` / `Unauthorized` **从未被 body status 触发**。

**关键陷阱**：既有断言里确有 `RetrievalTimeout` / `RetrievalUnavailable` / `RetrievalUnauthorized`，但它们
**全部来自 `search` 的 HTTP 内联错误分支**（`RemoteError → (status, [])`，`remote.rs:317-327`），**不是**
`parse_reply` 的 body status 映射。因此把 `parse_reply` 的映射换错（例如把 `Unavailable` 映到
`RetrievalUnauthorized`、或把 `Partial` 映到 `RetrievalComplete`）**完全不可见**——上层会拿到错误的
`RetrievalStatus` 而不自知。

此缝与 W113–W117 同形：判定面闭合、零生产改动、确定性离线。

## What Changes
只补可测判据，把 `parse_reply` 的 `RemoteStatus → RetrievalStatus` 映射逐臂钉成显式闭世界守卫；
**零生产代码改动**。

- `crates/nai-knowledge-core/tests/remote.rs` 新增 4 项（W118，已本地跑绿），经公共 `gateway.search` +
  Mock（HTTP 200 body status）覆盖此前零判据的四臂：
  - `search_maps_partial_remote_status_to_retrieval_partial`；
  - `search_maps_timeout_remote_status_to_retrieval_timeout`；
  - `search_maps_unavailable_remote_status_to_retrieval_unavailable`；
  - `search_maps_unauthorized_remote_status_to_retrieval_unauthorized`。
- `knowledge-pipeline` spec 新增 4 条 Requirement（各 1 Scenario），把该映射闭世界纳入基线。

## Modified Capabilities
- `knowledge-pipeline`（远端知识契约与撤销）：追加 `parse_reply` 的 `RemoteStatus → RetrievalStatus` 映射（partial/timeout/unavailable/unauthorized 四臂）闭世界守卫。

## Impact
- 无运行时行为变化（判定面闭合，不引入新执行体、不改 `remote.rs`）。
- 无契约字段改动、无迁移、无 schema 改动。
- registry：守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。
- 测试计数：`tests/remote.rs` 的测试由 28 增至 32（W118 +4）。
- spec：新增 4 条 Requirement（各 1 Scenario）。
