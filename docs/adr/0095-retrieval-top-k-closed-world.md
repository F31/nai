# ADR 0095：检索结果 top_k 截断闭世界守卫

- 状态：Accepted
- 日期：2026-10-15
- 归属：W117（兑现 W116 明确留待后续的 `parse_reply` `top_k` 独立缝，延续 W113–W116 「先测后设计」判定面闭合节奏）
- 关联：ADR 0094（§RemoteError::status() 闭世界）、ADR 0093（§检索证据闭世界）、ADR 0092（§远端网关构造期与输入边界）

## 背景
W116 闭合了 `RemoteError::status()` 的公开契约映射，并在报告/ADR 中明确把 `parse_reply` 的 `top_k` 截断列为
**独立缝、留待后续**。本轮兑现该缝。`parse_reply`（`remote.rs:395-411`）把远端证据集转成上层
`Vec<Evidence>`，其截断语义**整段零判据**——既有 24 个测试无一条断言 `top_k`（grep 零命中）。两处**非显然语义**：

- **filter 先于 take**：`reply.evidence.into_iter().filter_map(to_evidence).take(top_k)`——`top_k` 统计的是
  「丢弃空 `evidence_id` 后的**有效项**」数量，而非原始回包项数。若回归成 `take` 先于 `filter`，空 id 会先
  占满 `top_k` 名额，导致有效证据被少取而不自知。
- **take 保前 N、不补齐**：`take(top_k)` 保留序列**前 N 项且顺序不变**；`top_k == 0` ⇒ 空集合，`top_k >`
  有效项数 ⇒ 保留全部、不补齐占位、不报错。

`parse_reply` 是 `private fn`，经 `client.search`（remote.rs:329）与 `gateway.answer`（624）到达；
`gateway.search` 亦经 `client.search` 到同一路径。此缝与 W113–W116 同形：判定面闭合、零生产改动、确定性离线。

## 决策
**判定面闭合，零生产代码改动**。只补可测判据，把 `top_k` 截断的三条边界语义钉成显式闭世界守卫：

1. 顺序：新增 `search_truncates_evidence_to_top_k_in_order`（5 有效项 + `top_k=2` ⇒ `[e0,e1]`），钉 `take`
   保前 N 且顺序不变；
2. filter-then-take（非显然）：新增 `search_applies_top_k_after_dropping_empty_ids`，用判别性构造
   `[e0, "", e1, e2]` + `top_k=2` ⇒ `[e0,e1]`（`len=2`）；若回归成 take-then-filter 则 `len=1` ⇒ 断言变红，
   把「`top_k` 统计有效项」从字符串层面钉死；
3. 零值：新增 `search_top_k_zero_yields_empty_evidence`（`top_k=0` ⇒ 空集合，MUST 被显式清点，绝不静默变
   「未截断」）；
4. 超限：新增 `search_top_k_exceeding_reply_keeps_all_evidence`（`top_k=10` > 5 项 ⇒ 全保留，不补齐占位、不报错）。

四条均经公共 `gateway.search` + Mock 覆盖（私有 `parse_reply` 不可直调）；Mock 为本地 `127.0.0.1:0`，确定性离线。

## 后果
- 正向：`top_k` 截断的顺序 / filter-then-take / 零值 / 超限四语义被钉死，回归会红；filter-then-take 的
  判别性构造使「`top_k` 统计有效项」不靠读代码推断。
- 零运行时变化、零契约字段改动、零迁移、零 schema 改动。
- 如实披露（非验收达标）：
  - `parse_reply` 保持 `private`，仅经公共 `gateway.search` 覆盖；不提升为 `pub`。
  - 本轮只钉「既有截断语义」，不改 `search`/`answer` 的入参校验——`top_k` 是否应设上界属独立缝，明确不做。
  - `gateway.answer` 亦调用 `parse_reply(reply.clone(), top_k)`（624），本轮经 `gateway.search` 路径覆盖同一
    函数；`answer` 路径的 `top_k` 截断与 `search` 共用 `parse_reply`，语义一致。
  - registry：守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。
