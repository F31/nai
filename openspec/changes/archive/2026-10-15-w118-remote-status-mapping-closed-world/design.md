# Design: W118 — parse_reply 的 RemoteStatus → RetrievalStatus 闭世界映射守卫

## 缝（现测，不是推断）
基于代码实测（非摘要，`crates/nai-knowledge-core/src/remote.rs:395-411`）：

| `RemoteStatus`（body status） | `parse_reply` 映射 | 经 `parse_reply` 的判据 |
|---|---|---|
| `Complete` | `RetrievalComplete` | ✓ 既有（200 body `"complete"`） |
| `Empty` | `RetrievalEmpty` | ✓ 既有（200 body `"empty"`） |
| `Partial` | `RetrievalPartial` | **零** → **新增** `search_maps_partial_remote_status_to_retrieval_partial` |
| `Timeout` | `RetrievalTimeout` | **零** → **新增** `search_maps_timeout_remote_status_to_retrieval_timeout` |
| `Unavailable` | `RetrievalUnavailable` | **零** → **新增** `search_maps_unavailable_remote_status_to_retrieval_unavailable` |
| `Unauthorized` | `RetrievalUnauthorized` | **零** → **新增** `search_maps_unauthorized_remote_status_to_retrieval_unauthorized` |

**关键**：既有断言里的 `RetrievalTimeout` / `RetrievalUnavailable` / `RetrievalUnauthorized` 来自
`search`（`remote.rs:317-327`）的 **HTTP 内联错误分支**（`RemoteError → (status, [])`），与 `parse_reply`
的 **body status 映射**是两条独立路径。故 `parse_reply` 后四臂在此之前零判据。

## 决策
1. **判定面闭合，零生产代码改动**：只补测试，不引入新执行体、不改 `remote.rs`。
2. **逐臂经 body status 钉死**：Mock 返回 **HTTP 200** + `{"status": "<x>", "evidence": []}`，走
   `gateway.search` → `client.search`（200 ⇒ 不走内联错误分支）→ `parse_reply`。这样能**隔离** body status
   映射，不与 HTTP 内联错误路径混淆。
3. **区分两条路径是核心价值**：若不隔离，用 HTTP 错误码（如 500/401）测，只会覆盖内联分支、永远测不到
   `parse_reply` 的映射（本轮的缝正来自此混淆）。测试名显式写 `remote_status`（body status）以固化语义。
4. **闭世界四臂 + 既有两臂 = 全 6 臂**：`Complete`/`Empty` 已由既有测试（经 body status）覆盖；本轮补
   `Partial`/`Timeout`/`Unavailable`/`Unauthorized` 四臂，合计 6/6。
5. **确定性离线**：Mock 为本地 `127.0.0.1:0`，`gateway` 不装配 rerank，返回即 `parse_reply` 原始输出；
   本地复跑 `running 32 tests ... 32 passed; 0 failed`。

## 明确不做
- 不改 `remote.rs` 任何判定规则（不改映射、不合并 `search` 内联分支与 `parse_reply` 映射）。
- 不把 `parse_reply` 提升为 `pub`（保持私有，仅经公共 `gateway.search` 覆盖）。
- 不测 `gateway.answer` 路径的独立断言（其与 `search` 共用 `parse_reply`，语义一致；本轮经 `search` 覆盖）。
- 不碰介质级擦除、不建跨 memory↔remote 传播（与 W113–W117 一致）。
