# Design: W115 — 检索证据闭世界守卫

## 缝（现测，不是推断）
基于代码实测（非摘要，`crates/nai-knowledge-core/src/remote.rs`）：

| 位置 | 现状 | 判据缺口 |
|---|---|---|
| `remote.rs:463-468` `rerank_evidence` 空 base 短路 | `base.is_empty()` ⇒ `Ok(RerankOutcome{ status: RetrievalEmpty, evidence: Vec::new() })`，不进 `spawn_blocking` | 既有 `rerank_*` 只钉 reorder/unknown/budget，**零判据** → **新增** `rerank_empty_base_returns_empty_without_calling_reranker` |
| `remote.rs:490-495` rerank 输出去重 | `seen: BTreeSet` + `filter(seen.insert)` 仅保留首个重复 `evidence_id` | 同上，**零判据** → **新增** `rerank_deduplicates_repeated_ids` |
| `remote.rs:476-478` reranker 故障映射 | `spawn_blocking(rerank)` 的 `JoinError`（panic）⇒ `result.map_err(\|_\| RemoteError::Invalid)?` | 同上，**零判据** → **新增** `rerank_reranker_failure_maps_to_invalid` |
| `remote.rs:413-416` `to_evidence` 空 id 丢弃 | `if value.evidence_id.is_empty() { return None; }`，被 `search` 的 `filter_map` 掉 | 既有 `search` 只走合法非空 id，**零判据** → **新增** `search_drops_evidence_with_empty_id` |

## 决策
1. **判定面闭合，零生产代码改动**：只补测试，不引入新执行体、不改 `remote.rs`。
2. **空 base 短路须可证「未调用 reranker」**：用 `panic!` reranker 作为探针——若 `rerank_evidence`
   对空 base 仍进入 `spawn_blocking`，panic 会跨 `JoinError` 冒泡；实际短路路径在 `spawn_blocking`
   之前返回，故 panic reranker 不会触发，从而**证明 reranker 未被调用**。这是把「短路 ⇒ 不调用」
   变成可观测断言，而非仅靠返回 `RetrievalEmpty` 推断。
3. **重复 id 去重闭世界**：`base=[a,a,b]` + identity reranker ⇒ 预期 `len()==2` 且 `[a,b]`。
   断言顺序与去重语义（保留首个）逐字对应 `remote.rs:490-495`。identity reranker 不经 `known`
   过滤（所有 id 都在 base 中），故只测去重维度，不与 unknown-id drop（W114 既有测试）重叠。
4. **reranker 故障映射 `Invalid` 不降级**：用 `panic!` reranker，`spawn_blocking` 返回 `JoinError`
   ⇒ `RemoteError::Invalid`。断言 `.err() == Some(RemoteError::Invalid)`（非 `partial` / `empty`
   假阴性）。`.err()` 而非 `.unwrap_err()`——`RerankOutcome` 的 Ok 值未实现 `Debug`（与 W114 两处
   `::new` 同因），`.unwrap_err()` 会因要求 Ok 值可 `Debug` 而编译失败。
5. **`to_evidence` 空 id 丢弃经公共 API**：`to_evidence` 是 `private fn`，测试必须经公共 `search`
   API + Mock server。Mock `/search` 返回 `[{evidence_id:"good",...},{evidence_id:"",...}]`，
   断言结果 `len()==1` 且 `evidence_id=="good"`。把「空 id 在 `filter_map` 被丢弃」钉成端到端守卫。
6. **如实披露**：`rerank_evidence` 的 unknown-id drop（W114 既有 `rerank_preserves_base_*` 已钉）
   与本案重复 id 去重是两个独立闭世界维度——unknown drop 由 `known: BTreeSet` 负责（454-472），
   重复 drop 由 `seen: BTreeSet` 负责（490-495）；W115 只补后者，不重复前者。空 base 短路路径
   直接返回 `RetrievalEmpty`（不沿用 `base_status` 入参），该语义由 W115 显式钉住。

## 明确不做
- 不改 `remote.rs` 任何判定规则（不新增 unknown-id 校验、不改去重实现、不改故障映射）。
- 不把 `to_evidence` 提升为 `pub`（保持私有，仅经公共 `search` 覆盖）。
- 不碰介质级擦除、不建跨 memory↔remote 传播（与 W113/W114 一致）。
- 不升级 `search` 的其它边界（分页/top_k 校验属独立变更，不在本轮）。
