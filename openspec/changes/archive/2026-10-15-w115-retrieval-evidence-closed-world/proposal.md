# Proposal: W115 — 检索证据闭世界守卫

## Why
W114 闭合了远端网关的**构造期**与**输入边界**判定面（policy 校验、client args、sync 文档形状、
capabilities 首操作）。但检索链路进入「证据」后的两处**已声明、零判据**不变量仍可悄悄回归：

- `rerank_evidence`（`remote.rs:455-500`）对 base 与 reranker 输出的处理有三条「已声明」不变量，
  既有测试（`rerank_preserves_base_on_budget_expiry_and_drops_unknown_ids`）只钉了 reorder /
  unknown-id drop / budget-expiry partial，**以下三条零判据**：
  - `remote.rs:463-468`：空 base 直接短路返回 `RetrievalEmpty`，**不调用 reranker**；若回归成
    "对空 base 仍 spawn reranker"，会被静默吞掉（空输入下 reranker 行为不可观测）。
  - `remote.rs:490-495`：reranker 返回重复 `evidence_id` 时只保留首个（闭世界去重）；若回归成
    "允许重复 id 透传"，上层会拿到重复证据而不自知。
  - `remote.rs:476-478`：reranker 闭包 panic ⇒ `JoinError` ⇒ `RemoteError::Invalid`；若回归成
    "panic 被静默降级为 partial / empty"，故障被伪装成正常结果（假阴性）。
- `to_evidence`（`remote.rs:413-416`）：`evidence_id` 为空的项直接 `return None`（从 `search`
  结果 `filter_map` 掉）；既有测试只走合法非空 id，零判据——若回归成 "空 id 也保留"，会污染
  证据链路的身份键（空 id 在别的对象上命中，见 §6 的闭世界教训）。

这四处与 W113/W114 同形：判定面闭合、零生产改动、确定性离线。

## What Changes
只补可测判据，把四处已声明不变量钉成显式闭世界守卫；**零生产代码改动**。

- `crates/nai-knowledge-core/tests/remote.rs` 新增 4 项（W115，已本地跑绿）：
  - `rerank_empty_base_returns_empty_without_calling_reranker`（空 base 短路，用 panic reranker 证明未被调用）；
  - `rerank_deduplicates_repeated_ids`（base=[a,a,b]，identity reranker ⇒ 结果长度 2、顺序 a,b）；
  - `rerank_reranker_failure_maps_to_invalid`（panic reranker ⇒ `RemoteError::Invalid`，不降级）；
  - `search_drops_evidence_with_empty_id`（经公共 `search` + Mock，`evidence_id:""` 项被丢弃）。
- `knowledge-pipeline` spec 新增 4 条 Requirement（各 1 Scenario），把检索证据闭世界纳入基线。

## Modified Capabilities
- `knowledge-pipeline`（远端知识契约与撤销）：追加检索证据（rerank + to_evidence）闭世界守卫。

## Impact
- 无运行时行为变化（判定面闭合，不引入新执行体、不改 `remote.rs`）。
- 无契约字段改动、无迁移、无 schema 改动。
- registry：检索证据守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。
- 测试计数：`tests/remote.rs` 的 `#[tokio::test]` 由 16 增至 20（W115 +4）。
- spec：新增 4 条 Requirement（各 1 Scenario）。
