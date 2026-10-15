# Design: W117 — 检索结果 top_k 截断闭世界守卫

## 缝（现测，不是推断）
基于代码实测（非摘要，`crates/nai-knowledge-core/src/remote.rs:395-411`）：

| 位置 | 现状 | 判据缺口 |
|---|---|---|
| `remote.rs:404-409` `parse_reply` 证据截断 | `reply.evidence.into_iter().filter_map(to_evidence).take(top_k).collect()` | 既有 24 测试**零 `top_k` 断言** → **新增** 4 条守卫 |

三条边界语义（本轮的闭世界维度）：

| 维度 | 语义 | 覆盖测试 |
|---|---|---|
| 顺序 | `take(top_k)` 保留序列**前 N 项**、顺序不变 | `search_truncates_evidence_to_top_k_in_order` |
| 顺序（filter 先于 take） | `top_k` 统计**有效项**（丢空 id 之后）——**非显然** | `search_applies_top_k_after_dropping_empty_ids` |
| 零值 | `top_k=0` ⇒ `take(0)` ⇒ 空集合 | `search_top_k_zero_yields_empty_evidence` |
| 超限 | `top_k >` 有效项数 ⇒ 保留全部、不补齐 | `search_top_k_exceeding_reply_keeps_all_evidence` |

## 决策
1. **判定面闭合，零生产代码改动**：只补测试，不引入新执行体、不改 `remote.rs`。
2. **私有 `parse_reply` 经公共路径覆盖**：`parse_reply` 是 `private fn`，测试经公共 `gateway.search` + Mock 到达
   （`gateway.search` → `client.search` → `parse_reply`）。Mock 回 `/search` 返回可控证据集，`gateway` 不装配
   rerank，故返回即 `parse_reply` 的原始输出，无干扰。
3. **filter-then-take 用判别性构造证明**：`[e0, "", e1, e2]` + `top_k=2` ⇒ 正确实现 `[e0, e1]`（`len=2`）；
   若回归成 take-then-filter，`take(2)=[e0, ""]` 过滤后仅 `[e0]`（`len=1`）⇒ 断言变红。该构造使**顺序**从
   字符串层面可判：`len=2` 与 `len=1` 二分，不靠读代码推断。
4. **零值与超限显式清点**：`top_k=0` 断言空集合（MUST 被显式清点，绝不静默变「未截断」）；
   `top_k=10` > 5 项断言全保留（MUST NOT 补齐占位 / MUST NOT 报错）。二者把 `take` 的边界语义钉死。
5. **确定性离线、零网络**：Mock 为 `tiny_http` 本地 `127.0.0.1:0`，返回值由测试构造，无外部依赖；
   本地复跑 `running 28 tests ... 28 passed; 0 failed`。

## 明确不做
- 不改 `remote.rs` 任何判定规则（不改截断实现、不改 filter/take 顺序）。
- 不把 `parse_reply` 提升为 `pub`（保持私有，仅经公共路径覆盖）。
- 不改 `search`/`answer` 的入参校验（`top_k` 是否应设上界属独立缝，本轮只钉「既有截断语义」）。
- 不碰介质级擦除、不建跨 memory↔remote 传播（与 W113–W116 一致）。
