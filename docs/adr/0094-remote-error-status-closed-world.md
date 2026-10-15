# ADR 0094：RemoteError::status() 闭世界映射守卫

- 状态：Accepted
- 日期：2026-10-15
- 归属：W116（`RemoteError::status()` 公开契约映射判定面缺口勘定，延续 W113/W114/W115 「先测后设计」判定面闭合节奏）
- 关联：ADR 0093（§检索证据闭世界）、ADR 0092（§远端网关构造期与输入边界）、ADR 0091（§24.4 delete 契约）、ADR 0089（memory purge surface）

## 背景
W115 闭合了检索链路进入「证据」后的闭世界守卫（rerank + to_evidence）。但 `RemoteError` 的公开契约
`RemoteError::status()`（`remote.rs:37-46`）把 5 个错误变体钉成 `RetrievalStatus` 的映射**整段零判据**——
既有 20 个 `#[tokio::test]` / `#[test]` 无一条断言该映射。其中两条**非显然坍缩**尤其危险（回归时静默吞掉）：

- `Invalid → RetrievalUnavailable`（非独立 `RetrievalInvalid` 分支）；
- `Timeout | Cancelled → RetrievalTimeout`（两个不同语义的错误并入同一状态）。

若回归成「Invalid 误映到 `RetrievalUnauthorized`」或「`Cancelled` 误映到 `RetrievalUnavailable`」，
上层 `RetrievalStatus` 消费者（如 gateway `search` 的 `parse_reply`）会据此做出错误的可重试 / 不可重试
决策，而该错误**完全无法被现有测试观测到**。

此缝与 W113/W114/W115 同形：判定面闭合、零生产改动、纯函数确定性离线。

## 决策
**判定面闭合，零生产代码改动**。只补可测判据，把 `RemoteError::status()` 的全映射钉成显式闭世界守卫：

1. 闭世界映射逐变体钉死：4 个 `#[test]` 覆盖全部 5 个变体（`Timeout`+`Cancelled` 共享一条测试内两个
   `assert_eq!`）；任何变体的映射回归都会让对应测试红，把「已声明 ≠ 已执行」的缝封闭；
2. 两条非显然坍缩须显式命名：`Invalid → RetrievalUnavailable` 与 `Timeout|Cancelled →
   RetrievalTimeout` 不是「直觉映射」（例如 `Invalid` 不像 `Unauthorized` 那样映射到独立态），故单独成测试
   并在注释中标注「非显然坍缩」，防止后人误以为映射是 1:1 而漏改；
3. 纯函数、确定性离线：`status()` 是无 IO、无随机性的纯函数，4 测试不启服务器、不触网络、不依赖
   `tokio`（用同步 `#[test]` 而非 `#[tokio::test]`），与既有 async 测试隔离；
4. 零生产改动 ⇒ 不引入新失败模式：测试只读取 `RemoteError::status()` 的返回值并 `assert_eq!` 到
   `RetrievalStatus` 常量；若 `remote.rs` 日后改了映射，唯一后果是测试红（主动信号），而非静默降级。

## 后果
- 正向：全映射被钉死，回归会红；两条非显然坍缩显式命名，防止后人漏改。
- 零运行时变化、零契约字段改动、零迁移、零 schema 改动。
- 如实披露（非验收达标）：
  - `RemoteError::status()` 是 `RemoteError` 公开枚举上的方法，与 `RetrievalStatus` 消费侧（gateway
    `parse_reply`）解耦；W116 只钉「声明侧的映射正确性」，不钉「消费侧如何据此决策」（属既有网关逻辑）。
  - 两条坍缩（Invalid→Unavailable、Timeout|Cancelled→Timeout）是既有语义，W116 仅把既有行为钉成守卫，
    不引入新的状态分支（如 `RetrievalInvalid`）。
  - registry：守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。
  - `search` 的其它边界（`parse_reply` 的 top_k 截断）属独立缝，留待后续；本次仅闭合 `status()` 映射。
