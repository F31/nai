# ADR 0096：parse_reply 的 RemoteStatus → RetrievalStatus 闭世界映射守卫

- 状态：Accepted
- 日期：2026-10-15
- 归属：W118（`parse_reply` body status 映射判定面缺口勘定，延续 W113–W117 「先测后设计」判定面闭合节奏）
- 关联：ADR 0095（§检索结果 top_k 截断）、ADR 0094（§RemoteError::status() 闭世界）、ADR 0093（§检索证据闭世界）

## 背景
W117 闭合了 `parse_reply`（`remote.rs:395-411`）的 `top_k` 截断。该函数还有第二处判定面：其
`RemoteStatus → RetrievalStatus` 的 6 臂映射（`remote.rs:396-403`）**大部分零判据**——既有测试经
`parse_reply` 只覆盖 `Complete` / `Empty` 两臂（200 body status 只出现过 `"complete"` / `"empty"`），
其余四臂 `Partial` / `Timeout` / `Unavailable` / `Unauthorized` 从未被 body status 触发。

**关键陷阱**：既有断言里确有 `RetrievalTimeout` / `RetrievalUnavailable` / `RetrievalUnauthorized`，但它们
**全部来自 `search` 的 HTTP 内联错误分支**（`RemoteError → (status, [])`，`remote.rs:317-327`），**不是**
`parse_reply` 的 body status 映射。两条路径彼此独立。因此把 `parse_reply` 的映射换错（例如把 `Unavailable`
映到 `RetrievalUnauthorized`、把 `Partial` 映到 `RetrievalComplete`）**完全不可见**，上层会拿到错误的
`RetrievalStatus` 而不自知。

此缝与 W113–W117 同形：判定面闭合、零生产改动、确定性离线。

## 决策
**判定面闭合，零生产代码改动**。只补可测判据，把 `parse_reply` 的 `RemoteStatus → RetrievalStatus` 映射
逐臂钉成显式闭世界守卫：

1. `Partial → RetrievalPartial`：新增 `search_maps_partial_remote_status_to_retrieval_partial`；
2. `Timeout → RetrievalTimeout`：新增 `search_maps_timeout_remote_status_to_retrieval_timeout`；
3. `Unavailable → RetrievalUnavailable`：新增 `search_maps_unavailable_remote_status_to_retrieval_unavailable`；
4. `Unauthorized → RetrievalUnauthorized`：新增 `search_maps_unauthorized_remote_status_to_retrieval_unauthorized`。

四条均经公共 `gateway.search` + Mock：Mock 返回 **HTTP 200** + `{"status": "<x>", "evidence": []}`，走
`client.search`（200 ⇒ **不走**内联错误分支）→ `parse_reply`，从而**隔离** body status 映射。测试名显式含
`remote_status` 固化「这是 body status，不是 HTTP 状态」的语义。既有两臂（`Complete`/`Empty`）已由既有测试
（经 body status）覆盖，本轮补齐后合计 **6/6 臂**全钉。

## 后果
- 正向：`parse_reply` 的 6 臂映射被完整钉死，任一臂换错会红；并显式区分「body status 映射」与「HTTP 内联
  错误映射」两条路径，防止后人以为 HTTP 层的断言已经覆盖了 `parse_reply`。
- 零运行时变化、零契约字段改动、零迁移、零 schema 改动。
- 如实披露（非验收达标）：
  - `parse_reply` 保持 `private`，仅经公共 `gateway.search` 覆盖；不提升为 `pub`。
  - `gateway.answer` 亦调用 `parse_reply`（remote.rs:624），与 `search` 共用同一函数、语义一致；本轮经
    `search` 路径覆盖，不为 `answer` 另写独立断言（避免重复）。
  - 本轮**不改** `remote.rs`：不合并 `search` 内联分支与 `parse_reply` 映射（二者刻意分离：前者是 HTTP 层
    错误，后者是 body 层状态）。
  - registry：守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。
