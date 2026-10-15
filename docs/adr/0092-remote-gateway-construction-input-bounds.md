# ADR 0092：远端网关构造期与输入边界闭世界守卫

- 状态：Accepted
- 日期：2026-10-15
- 归属：W114（远端网关守卫缺口勘定，延续 W113 「先测后设计」判定面闭合节奏）
- 关联：ADR 0091（§24.4 delete 契约）、ADR 0089（memory purge surface）、ADR 0090（§42.5 跨会话边界）

## 背景
W113 闭合了 `RemoteGateway::delete` 的传输契约判定面。但远端网关的**构造期**与**输入边界**仍有
四处「已声明、零判据」不变量（实测 remote.rs）：

- `RemoteKnowledgeClient::new`（remote.rs:67-74）对 endpoint / timeout / max_retries 的校验，
  既有的 `Mock::client()` 只传合法参数，从未被单测覆盖；
- `RemoteGateway::new`（remote.rs:518-526）对 policy 身份字段与 `unknown_revision_ttl_ms` 的
  校验，既有的 `gateway()` 辅助函数只用合法 policy `.unwrap()`，零判据；
- `RemoteGateway::sync` 输入边界（remote.rs:639-647）：文档集空 / >16 / 单文档 content>65536，
  既有的 `sync_and_delete_are_gated_single_attempt_and_return_receipts` 只钉门控开关，零判据；
- `RemoteGateway::capabilities` 首操作守卫（remote.rs:571-581）：首个 `allowed_operations` 非
  Search/Answer 即拒，既有测试首 op 都是 Search，零判据。

回归即可把非法 policy / 参数放行进构造，或把非检索能力的 policy 暴露给上层——与 W113 同形的
判定面缺口。

## 决策
**判定面闭合，零生产代码改动**。只补可测判据，把四处已声明不变量钉成显式闭世界守卫：

1. `RemoteGateway::new` 构造期闭世界：policy 六类非法变体（source_id / policy_ref /
   permission_revision 空、allowed_kb_ids 空、unknown_revision_ttl_ms 为 0 或 >5000）⇒
   `Invalid`，不触网；
2. `RemoteKnowledgeClient::new` 构造期闭世界：endpoint 空 / >2048、timeout 零 / >600s、
   max_retries >10 ⇒ `Invalid`，不建 http client（不触网）；
3. `sync` 输入边界闭世界：文档集空 / >16 / 单文档 content>65536 ⇒ `Unauthorized`，且显式证明
   **仅合法调用触网**（其余被拒调用不得发起 `/sync`）；
4. `capabilities` 首操作守卫：首个 allowed op 非 Search/Answer（如 Sync/Delete）⇒ `Unauthorized`，
   不触网。

`.err()` 而非 `.unwrap_err()`——两个构造返回类型的 Ok 值（`RemoteGateway` /
`RemoteKnowledgeClient`）未实现 `Debug`，`.unwrap_err()` 会因要求 Ok 值可 `Debug` 而编译失败；
改用 `.err()` 与 `Some(RemoteError::Invalid)` 比较，同样现测断言拒绝、且断言合法参数仍构造成功
（守卫不误伤正常路径）。

## 后果
- 正向：四处不变量被钉死，回归会红；`sync` 边界另用 attempt 计数器证明「拒绝 ⇒ 不触网」，
  把「不触网」从「靠返回错误推断」升级为可测断言。
- 零运行时变化、零契约字段改动、零迁移、零 schema 改动。
- 如实披露（非验收达标）：
  - `RemoteGateway::new` 不校验 `allowed_operations`（只查身份字段与 `unknown_revision_ttl_ms`）；
    `capabilities` 守卫依赖调用方把「检索/问答」放在 `allowed_operations` **首位**。这是「首操作」
    约定而非穷举校验——W114 钉住当前语义，若未来改为「集合含 Search/Answer 任一即可」属新变更。
  - 介质级擦除仍明确不做（W111 §7.2 第 2 项）。
  - registry：远端网关构造/边界守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。
