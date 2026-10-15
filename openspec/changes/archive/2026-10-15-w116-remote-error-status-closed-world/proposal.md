# Proposal: W116 — RemoteError::status() 闭世界映射守卫

## Why
W115 闭合了检索链路进入「证据」后的闭世界守卫（rerank + to_evidence）。但 `RemoteError` 的公开
契约 `RemoteError::status()`（`remote.rs:37-46`）把 5 个错误变体钉成 `RetrievalStatus` 的映射**整段
零判据**——既有 20 个 `#[tokio::test]` / `#[test]` 无一条断言该映射。其中两条**非显然坍缩**尤其
危险（回归时静默吞掉）：

- `Invalid → RetrievalUnavailable`（非独立 `RetrievalInvalid` 分支）；
- `Timeout | Cancelled → RetrievalTimeout`（两个不同语义的错误并入同一状态）。

若回归成「Invalid 误映到 `RetrievalUnauthorized`」或「`Cancelled` 误映到 `RetrievalUnavailable`」，
上层 `RetrievalStatus` 消费者（如 gateway `search` 的 `parse_reply`）会据此做出错误的可重试 /
不可重试决策，而该错误**完全无法被现有测试观测到**。

此缝与 W113/W114/W115 同形：判定面闭合、零生产改动、纯函数确定性离线。

## What Changes
只补可测判据，把 `RemoteError::status()` 的全映射钉成显式闭世界守卫；**零生产代码改动**。

- `crates/nai-knowledge-core/tests/remote.rs` 新增 4 项（W116，已本地跑绿）：
  - `remote_error_status_unauthorized_maps_to_retrieval_unauthorized`；
  - `remote_error_status_unavailable_maps_to_retrieval_unavailable`；
  - `remote_error_status_invalid_maps_to_retrieval_unavailable`（非显然坍缩）；
  - `remote_error_status_timeout_and_cancelled_map_to_retrieval_timeout`（坍缩）。
- `knowledge-pipeline` spec 新增 4 条 Requirement（各 1 Scenario），把 `RemoteError::status()`
  映射纳入基线。

## Modified Capabilities
- `knowledge-pipeline`（远端知识契约与撤销）：追加 `RemoteError::status()` 闭世界映射守卫。

## Impact
- 无运行时行为变化（判定面闭合，不引入新执行体、不改 `remote.rs`）。
- 无契约字段改动、无迁移、无 schema 改动。
- registry：守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。
- 测试计数：`tests/remote.rs` 的测试由 20 增至 24（W116 +4）。
- spec：新增 4 条 Requirement（各 1 Scenario）。
