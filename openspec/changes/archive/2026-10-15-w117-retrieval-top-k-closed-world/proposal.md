# Proposal: W117 — 检索结果 top_k 截断闭世界守卫

## Why
W116 闭合了 `RemoteError::status()` 的公开契约映射，并明确把 `parse_reply` 的 `top_k` 截断列为**独立缝、留待后续**。
本轮兑现该缝：`parse_reply`（`remote.rs:395-411`）把远端证据集转成上层 `Vec<Evidence>`，其截断语义
**整段零判据**——既有测试无一条断言 `top_k`（grep 零命中）。它有两处**非显然语义**，回归时静默吞掉：

- **filter 先于 take**：`reply.evidence.into_iter().filter_map(to_evidence).take(top_k)`——`top_k` 统计的是
  「丢弃空 `evidence_id` 后的**有效项**」数量，而非原始回包项数。若回归成 `take` 先于 `filter`，空 id 会
  先占满 `top_k` 名额，导致有效证据被少取而不自知。
- **take 保前 N、不补齐**：`take(top_k)` 保留序列**前 N 项且顺序不变**；`top_k == 0` ⇒ 空集合，
  `top_k >` 有效项数 ⇒ 保留全部、不补齐占位、不报错。三者中任意一条被改（例如把 `0` 当作"不限"、
  或把超出当作错误）都会改变上层拿到的证据集，而现有测试零信号。

`parse_reply` 是 `private fn`，经 `client.search`（remote.rs:329）与 `gateway.answer`（624）到达；
`gateway.search` 亦经 `client.search` 到同一路径。此缝与 W113–W116 同形：判定面闭合、零生产改动、确定性离线。

## What Changes
只补可测判据，把 `top_k` 截断的三条边界语义钉成显式闭世界守卫；**零生产代码改动**。

- `crates/nai-knowledge-core/tests/remote.rs` 新增 4 项（W117，已本地跑绿），经公共 `gateway.search` + Mock 覆盖：
  - `search_truncates_evidence_to_top_k_in_order`（5 有效项 + `top_k=2` ⇒ `[e0,e1]`）；
  - `search_applies_top_k_after_dropping_empty_ids`（`[e0,"",e1,e2]` + `top_k=2` ⇒ `[e0,e1]`，证 filter 先于 take）；
  - `search_top_k_zero_yields_empty_evidence`（`top_k=0` ⇒ 空集合）；
  - `search_top_k_exceeding_reply_keeps_all_evidence`（`top_k=10` > 5 项 ⇒ 全保留、不补齐）。
- `knowledge-pipeline` spec 新增 4 条 Requirement（各 1 Scenario），把 `top_k` 截断闭世界纳入基线。

## Modified Capabilities
- `knowledge-pipeline`（远端知识契约与撤销）：追加检索结果 `top_k` 截断（顺序 / filter-then-take / 零值 / 超限）闭世界守卫。

## Impact
- 无运行时行为变化（判定面闭合，不引入新执行体、不改 `remote.rs`）。
- 无契约字段改动、无迁移、无 schema 改动。
- registry：守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。
- 测试计数：`tests/remote.rs` 的测试由 24 增至 28（W117 +4）。
- spec：新增 4 条 Requirement（各 1 Scenario）。
