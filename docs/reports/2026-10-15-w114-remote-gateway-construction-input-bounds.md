# W114 报告：远端网关构造期与输入边界闭世界守卫

日期：2026-10-15｜归属：W113 之后「新开 nai 守卫缺口」勘定候选（远端网关构造期与输入边界）｜零生产代码改动

## 一、裁定与缝（现测，不是推断）

W113 闭合了 `RemoteGateway::delete` 传输契约判定面。用户经 AskUserQuestion 选定「新开 nai 守卫缺口」，
对 nai 当前守卫/判据体系重新勘定，挑出下一个「先测后设计」的判定面闭合候选。基于代码实测
（非摘要），远端网关的**构造期**与**输入边界**仍有四处「已声明、零判据」不变量：

| 位置 | 现状 | 判据缺口 |
|---|---|---|
| `remote.rs:67-74` `RemoteKnowledgeClient::new` 校验 | 空 endpoint / >2048 / timeout 零或 >600s / max_retries >10 ⇒ `Invalid` | 既有 `Mock::client()` 只传合法参数，**零判据** → **新增** `remote_knowledge_client_new_rejects_invalid_args` |
| `remote.rs:518-526` `RemoteGateway::new` 校验 | `source_id`/`policy_ref`/`permission_revision` 空、`allowed_kb_ids` 空、`unknown_revision_ttl_ms` 0 或 >5000 ⇒ `Invalid` | 既有 `gateway()` 只用合法 policy `.unwrap()`，**零判据** → **新增** `remote_gateway_new_rejects_invalid_policy` |
| `remote.rs:571-581` `capabilities` 首操作守卫 | 首个 `allowed_operations` 非 Search/Answer ⇒ `Unauthorized` | 既有测试首 op 都是 Search，**零判据** → **新增** `remote_capabilities_rejects_when_first_allowed_op_not_search_or_answer` |
| `remote.rs:639-647` `sync` 输入边界 | 文档集空 / >16 / 单文档 `content > 65536` ⇒ `Unauthorized` | 既有 `sync_and_delete_are_gated_*` 只钉门控开关，**零判据** → **新增** `remote_sync_rejects_malformed_documents_without_network` |

**裁定**：判定面闭合，零生产代码改动——只把四处已声明不变量变成显式守卫，与 W113 同形。

## 二、判据落点（`crates/nai-knowledge-core/tests/remote.rs`）

- **新增 1**：`remote_gateway_new_rejects_invalid_policy` —— policy 六类非法变体（source_id /
  policy_ref / permission_revision 空、allowed_kb_ids 空、unknown_revision_ttl_ms 0 或 >5000）⇒
  `RemoteError::Invalid` 且不触网；另断言合法 policy 仍构造成功（守卫不误伤正常路径）。
- **新增 2**：`remote_knowledge_client_new_rejects_invalid_args` —— endpoint 空 / >2048、timeout 零 /
  >600s、max_retries >10 ⇒ `RemoteError::Invalid` 且不建 http client（纯构造、不触网）；另断言合法
  参数仍构造成功。`.err()` 而非 `.unwrap_err()`——两个构造的 Ok 值（`RemoteGateway` /
  `RemoteKnowledgeClient`）未实现 `Debug`，`.unwrap_err()` 会编译失败。
- **新增 3**：`remote_sync_rejects_malformed_documents_without_network` —— 文档集空 / >16 / 单文档
  content>65536 ⇒ `RemoteError::Unauthorized`；用 attempt 计数器显式证明**仅合法单文档调用触网**，
  把「拒绝 ⇒ 不触网」升级为可测断言（而非仅靠返回错误推断）；另断言合法单文档仍成功。
- **新增 4**：`remote_capabilities_rejects_when_first_allowed_op_not_search_or_answer` —— 首个
  allowed op 为 Sync（非 Search/Answer）⇒ `Unauthorized` 且不触网；首 op 为 Answer ⇒ 放行（断言远端
  `supports_delete`，合法正常路径）。
- **归属（不重复造）**：既有 `sync_and_delete_are_gated_single_attempt_and_return_receipts` 已钉
  `allow_document_upload == false` / `allow_third_party_retention == false` ⇒ sync 拒、门控调用不触网；
  W114 显式归属，只补**文档形状**边界。

## 三、计数（现测，非推断）

`crates/nai-knowledge-core/tests/remote.rs`：`#[tokio::test]` **12 → 16**（+4 = 全为新增，W114 四条
均未曾被任何测试覆盖；无既有测试归属为本轮新增，W113 的归属 1 已在 §二标为既有）。门控权威总数见 §六。

## 四、如实披露（判定面闭合 ≠ 验收达标）

- `RemoteGateway::new` **不校验** `allowed_operations`（只查身份字段与 `unknown_revision_ttl_ms`）；
  `capabilities` 守卫依赖调用方把「检索/问答」放在 `allowed_operations` **首位**。这是「首操作」约定
  而非穷举枚举校验——W114 钉住该约定的当前语义，若未来改为「集合含 Search/Answer 任一即可」属新变更，
  不在本轮。
- 介质级擦除仍明确不做（W111 §7.2 第 2 项）。
- registry：远端网关构造/边界守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。

## 五、spec 增量（`knowledge-pipeline`）

新增 4 条 Requirement（各 1 Scenario），把构造期与输入边界闭世界纳入基线：
- **Remote gateway construction rejects invalid policy**（Malformed policy is rejected at gateway construction）；
- **Remote knowledge client construction rejects invalid args**（Malformed client args are rejected at client construction）；
- **Remote sync rejects malformed documents without network**（Malformed sync documents are rejected without network）；
- **Remote capabilities requires a search or answer first operation**（Capabilities requires a search or answer first operation）。

## 六、门控（全量 `cargo xtask check --profile full`，待 #443 跑后回填）

- 待全量门控后填 `GATE_EXIT`、各测试 bin `fail` 计数、本机 LNK1104 瞬态处置与最终结论。

## 七、提交 / 推送（待 #444 归档 + 提交后回填）

- 待归档 + 一次 commit + push（SSH 443）后填提交 SHA 与 push 区间、改动文件清单。

## 八、CI 观测（待 #445 推送后回填）

- 待主提交与回填提交各一次 CI 双平台观测后填 run id / `head_sha` / `conclusion` / 双平台结论。
