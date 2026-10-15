# ADR 0093：检索证据闭世界守卫（rerank + to_evidence）

- 状态：Accepted
- 日期：2026-10-15
- 归属：W115（检索链路进入「证据」后的判定面缺口勘定，延续 W113/W114 「先测后设计」判定面闭合节奏）
- 关联：ADR 0092（§远端网关构造期与输入边界）、ADR 0091（§24.4 delete 契约）、ADR 0089（memory purge surface）

## 背景
W114 闭合了远端网关的**构造期**与**输入边界**判定面。但检索链路进入「证据」后的两处仍有四处
「已声明、零判据」不变量（实测 `crates/nai-knowledge-core/src/remote.rs`）：

- `rerank_evidence`（remote.rs:455-500）对 base 与 reranker 输出的处理有三条「已声明」不变量，
  既有 `rerank_preserves_base_on_budget_expiry_and_drops_unknown_ids` 只钉了 reorder / unknown-id
  drop / budget-expiry partial，**以下三条零判据**：
  - `remote.rs:463-468` 空 base 短路返回 `RetrievalEmpty`，不进入 `spawn_blocking`；若回归成
    "对空 base 仍 spawn reranker"，会被静默吞掉（空输入下 reranker 行为不可观测）；
  - `remote.rs:490-495` reranker 返回重复 `evidence_id` 时只保留首个（闭世界去重）；若回归成
    "允许重复 id 透传"，上层会拿到重复证据而不自知；
  - `remote.rs:476-478` reranker 闭包 panic ⇒ `JoinError` ⇒ `RemoteError::Invalid`；若回归成
    "panic 被静默降级为 partial / empty"，故障被伪装成正常结果（假阴性）。
- `to_evidence`（remote.rs:413-416）：`evidence_id` 为空的项直接 `return None`（被 `search` 的
  `filter_map` 掉）；既有测试只走合法非空 id，零判据——若回归成 "空 id 也保留"，会污染证据链路的
  身份键（空 id 在别的对象上命中，见 §6 闭世界教训）。

回归即可把非法证据形状放行进检索结果，或把重排故障伪装成正常——与 W113/W114 同形的判定面缺口。

## 决策
**判定面闭合，零生产代码改动**。只补可测判据，把四处已声明不变量钉成显式闭世界守卫：

1. `rerank_evidence` 空 base 短路：新增 `rerank_empty_base_returns_empty_without_calling_reranker`，
   用 **panic reranker 探针**证明 reranker **未被调用**（短路路径在 `spawn_blocking` 之前返回，panic
   不触发），把「短路 ⇒ 不调用」变成可观测断言，而非仅靠返回 `RetrievalEmpty` 推断；
2. `rerank_evidence` 重复 id 去重：新增 `rerank_deduplicates_repeated_ids`（`base=[a,a,b]` + identity
   reranker ⇒ 结果长度 2、顺序 `[a,b]`），断言 `remote.rs:490-495` 的 `seen: BTreeSet` 去重语义；
3. `rerank_evidence` 故障映射：新增 `rerank_reranker_failure_maps_to_invalid`（panic reranker ⇒
   `RemoteError::Invalid`），断言 `.err() == Some(Invalid)` 而非 `partial` / `empty` 假阴性；
4. `to_evidence` 空 id 丢弃：新增 `search_drops_evidence_with_empty_id`，经**公共 `search` API + Mock**
   证明 `evidence_id:""` 项被 `filter_map` 掉（该 fn 为 `private`，必须走公共路径覆盖）。

`.err()` 而非 `.unwrap_err()`——`RerankOutcome` 的 Ok 值未实现 `Debug`（与 W114 两处 `::new` 同因），
`.unwrap_err()` 会因要求 Ok 值可 `Debug` 而编译失败；改用 `.err()` 与 `Some(RemoteError::Invalid)`
比较，同样现测断言拒绝。

## 后果
- 正向：四处不变量被钉死，回归会红；空 base 短路与 reranker 故障分别用 panic 探针把「不调用」与
  「不降级」升级为可测断言，避免假阴性。
- 零运行时变化、零契约字段改动、零迁移、零 schema 改动。
- 如实披露（非验收达标）：
  - `rerank_evidence` 的 unknown-id drop（W114 既有 `rerank_preserves_base_*` 已钉）与本案重复 id
    去重是两个独立闭世界维度——unknown drop 由 `known: BTreeSet`（remote.rs:469-472）负责，重复 drop
    由 `seen: BTreeSet`（490-495）负责；W115 只补后者，不重复前者。
  - 空 base 短路路径直接返回 `RetrievalEmpty`（不沿用 `base_status` 入参），该语义由 W115 显式钉住。
  - `to_evidence` 保持 `private`，仅经公共 `search` 覆盖；不提升为 `pub`。
  - registry：检索证据守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。
