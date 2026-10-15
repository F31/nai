# Design: W116 — RemoteError::status() 闭世界映射守卫

## 缝（现测，不是推断）
基于代码实测（非摘要，`crates/nai-knowledge-core/src/remote.rs:37-46`）：

| 变体 | `status()` 映射 | 判据缺口 |
|---|---|---|
| `RemoteError::Unauthorized` | `RetrievalStatus::RetrievalUnauthorized` | 既有 20 测试**零判据** → **新增** `remote_error_status_unauthorized_maps_to_retrieval_unauthorized` |
| `RemoteError::Unavailable` | `RetrievalStatus::RetrievalUnavailable` | 同上 → **新增** `remote_error_status_unavailable_maps_to_retrieval_unavailable` |
| `RemoteError::Invalid` | `RetrievalStatus::RetrievalUnavailable`（**非显然坍缩**：无独立 `RetrievalInvalid`） | 同上 → **新增** `remote_error_status_invalid_maps_to_retrieval_unavailable` |
| `RemoteError::Timeout` / `RemoteError::Cancelled` | `RetrievalStatus::RetrievalTimeout`（**坍缩**：两语义并入同态） | 同上 → **新增** `remote_error_status_timeout_and_cancelled_map_to_retrieval_timeout` |

## 决策
1. **判定面闭合，零生产代码改动**：只补测试，不引入新执行体、不改 `remote.rs`。
2. **闭世界映射逐变体钉死**：4 个 `#[test]` 覆盖全部 5 个变体（Timeout+Cancelled 共享一条测试
   内两个 `assert_eq!`）。如此「已声明 ≠ 已执行」的缝被封闭——任何变体的映射回归都会让对应测试红。
3. **两条非显然坍缩须显式命名**：`Invalid → RetrievalUnavailable` 与 `Timeout|Cancelled →
   RetrievalTimeout` 不是「直觉映射」（例如 `Invalid` 不像 `Unauthorized` 那样映射到独立态），
   故单独成测试并在注释中标注「非显然坍缩」，防止后人误以为映射是 1:1 而漏改。
4. **纯函数、确定性离线**：`status()` 是无 IO、无随机性的纯函数，4 测试不启服务器、不触网络、
   不依赖 `tokio`（用同步 `#[test]` 而非 `#[tokio::test]`），与既有 async 测试隔离，运行 0.84s。
5. **零生产改动 ⇒ 不引入新失败模式**：测试只读取 `RemoteError::status()` 的返回值并 `assert_eq!`
   到 `RetrievalStatus` 常量；若 `remote.rs` 日后改了映射，唯一后果是测试红（主动信号），而非
   静默降级。

## 明确不做
- 不改 `remote.rs` 任何判定规则（不改映射、不新增 `RetrievalInvalid` 分支）。
- 不把 `RemoteError` 变体或 `RetrievalStatus` 枚举升版（H03/H04 契约边界不在本轮）。
- 不碰介质级擦除、不建跨 memory↔remote 传播（与 W113/W114/W115 一致）。
- 不升级 `search` 的其它边界（`parse_reply` 的 top_k 截断属独立缝，留待后续；本次仅闭合 `status()`）。
