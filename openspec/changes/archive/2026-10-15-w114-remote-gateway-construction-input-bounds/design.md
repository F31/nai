# Design: W114 — 远端网关构造期与输入边界闭世界守卫

## 缝（现测，不是推断）
基于代码实测（非摘要）：

| 位置 | 现状 | 判据缺口 |
|---|---|---|
| `remote.rs:67-74` `RemoteKnowledgeClient::new` 校验 | 空 endpoint / >2048 / timeout 零或 >600s / max_retries >10 ⇒ `Invalid` | 既有 `Mock::client()` 只传合法参数，**零判据** → **新增** `remote_knowledge_client_new_rejects_invalid_args` |
| `remote.rs:518-526` `RemoteGateway::new` 校验 | `source_id`/`policy_ref`/`permission_revision` 空、`allowed_kb_ids` 空、`unknown_revision_ttl_ms` 0 或 >5000 ⇒ `Invalid` | 既有 `gateway()` 只用合法 policy `.unwrap()`，**零判据** → **新增** `remote_gateway_new_rejects_invalid_policy` |
| `remote.rs:571-581` `capabilities` 首操作守卫 | 首个 `allowed_operations` 非 Search/Answer ⇒ `Unauthorized` | 既有测试首 op 都是 Search，**零判据** → **新增** `remote_capabilities_rejects_when_first_allowed_op_not_search_or_answer` |
| `remote.rs:639-647` `sync` 输入边界 | 文档集空 / >16 / 单文档 content>65536 ⇒ `Unauthorized` | 既有 `sync_and_delete_are_gated_*` 只钉门控开关，**零判据** → **新增** `remote_sync_rejects_malformed_documents_without_network` |

## 决策
1. **判定面闭合，零生产代码改动**：只补测试，不引入新执行体、不改 `remote.rs`。
2. **构造期闭世界（两处 `::new`）**：`RemoteGateway::new` 与 `RemoteKnowledgeClient::new` 的
   非法变体逐项钉死（remote.rs:518-526 / 67-74）。`.err()` 而非 `.unwrap_err()`——两个构造
   返回类型的 Ok 值（`RemoteGateway` / `RemoteKnowledgeClient`）未实现 `Debug`，`.unwrap_err()`
   会因要求 Ok 值可 `Debug` 而编译失败；改用 `.err()` 与 `Some(RemoteError::Invalid)` 比较，
   同样现测断言拒绝、且断言合法参数仍构造成功（守卫不误伤正常路径）。
3. **sync 输入边界闭世界**：空集 / >16 / content>65536 三条逐项钉死（remote.rs:639-647），
   与 W113 的 delete 输入边界对称。显式用 attempt 计数器证明**仅合法单文档调用触网**
   （其余被拒调用不得发起 `/sync` 请求），把「拒绝 ⇒ 不触网」变成可测断言，而非仅靠
   返回错误推断。
4. **capabilities 首操作守卫**：首 op 为 `Sync`（非 Search/Answer）⇒ `Unauthorized` 且不触网；
   首 op 为 `Answer` ⇒ 放行（合法正常路径断言远端 `supports_delete`）。把 `remote.rs:573-579`
   的「首个 allowed op 必须是检索/问答」钉成显式守卫。
5. **门控开关归属既有测试**：`sync_and_delete_are_gated_single_attempt_and_return_receipts`
   已钉 `allow_document_upload == false` / `allow_third_party_retention == false` ⇒ sync 拒、
   门控调用不触网；W114 显式归属，不重复造，只补**文档形状**边界。
6. **如实披露**：`RemoteGateway::new` 不校验 `allowed_operations`（只查身份字段与
   `unknown_revision_ttl_ms`）；`capabilities` 守卫依赖调用方把「检索/问答」放在
   `allowed_operations` 首位。这是「首操作」约定而非穷举枚举校验——W114 钉住该约定
   的当前语义，若未来改为「任意位置含 Search/Answer 即可」属新变更，不在本轮。

## 明确不做
- 不改 `remote.rs` 任何判定规则（不新增 `allowed_operations` 穷举校验、不强制 `max_retention_seconds`）。
- 不建跨 memory↔remote 的传播（两子系统不共用落点，结构性分离）。
- 不碰介质级擦除（W111 §7.2 第 2 项，已明确不做）。
- 不把「首操作」守卫升级为「集合含 Search/Answer 任一即可」——那是语义变更，需新变更评审。
