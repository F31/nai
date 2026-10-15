# Design: W113 — §24.4 远端留存清除：`RemoteGateway::delete` 传输契约判定面闭合

## 缝（现测，不是推断）
基于代码实测（非摘要）：

| 位置 | 现状 | 判据缺口 |
|---|---|---|
| `remote.rs:669` `if !self.policy.require_remote_delete` | 已门控，delete 在 `false` 时返回 `Unauthorized` | 既有测试已钉（归属 W113） |
| `remote.rs:670-672` delete 输入边界（空集 / >16 / 含空 id） | 已门控返回 `Unauthorized`，但**无单测** | **新增** `remote_delete_rejects_malformed_document_ids` |
| `remote.rs:658-659` 注释「receipt ≠ 第三方擦除证明」 | 行为成立（`delete` 透传 `RemoteReceipt`），但**无判据**防止契约新增 `erased` 变体 | **新增** 枚举闭世界守卫 |
| `remote.rs:682` `self.client.delete(...)` 透传 | 回执原样返回 | 新增透传断言 |

## 决策
1. **判定面闭合，零生产代码改动**：只补测试，不引入新执行体、不改 `remote.rs`。
2. **delete 输入边界闭世界**：空集 / >16 / 含空 id 三条逐项钉死（remote.rs:670-672）。
   与既有 `sync_and_delete_are_gated...` 互补——那条只钉门控开关，本测试钉输入形状。
3. **回执透传 ≠ 擦除证明**：`remote_delete_receipt_is_passed_through_and_never_claims_erasure`
   断言回执原样返回（`request_id` / `policy_ref` / `status` 透传）；并对 `RemoteReceiptStatus`
   穷尽 match 四个变体（accepted / pending / deleted / retained），**若契约新增 `erased` / `purged`
   之类变体，match 编译失败**，强制就 §24.4 不变量评审一次。这是把注释 658-659 变成可测守卫。
4. **门控开关归属既有测试**：`sync_and_delete_are_gated_single_attempt_and_return_receipts`
   已钉 `require_remote_delete == false` ⇒ 拒、`allow_third_party_retention == false` ⇒ sync 拒、
   门控调用不触网；W113 显式归属，不重复造。
5. **如实披露**：`max_retention_seconds`（`RemotePolicy` 字段）声明但**未被 `gate` 强制**
   （remote.rs:538-569 只查 `expires_at_utc_ms` 等，不查保留期上限）；`RemoteRetention.retention_seconds`
   仅是远端能力声明元数据，本地无强制机制。W113 不建该机制（属「远端留存保留期执行」能力，
   非传输契约判定面），如实记为遗留。
6. **tombstone 在存储层已落地**：`docs/knowledge.md:65` 的 `Store::remote_delete_targets`
   返回本地 tombstone 后的 kb-scoped 删除目标；`RemoteGateway::delete` 是**传输层**，假定
   tombstone 已存在。W113 不重复存储层判定，只闭合传输契约。

## 明确不做
- 不改 `remote.rs` 任何判定规则（不新增加密擦除、不强制 `max_retention_seconds`）。
- 不引入「已完全擦除」结论字段（§24.4 明令禁止把回执当擦除证明）。
- 不建跨 memory↔remote 的传播（两子系统不共用落点，结构性分离；如未来有人「帮忙」传播
  memory purge 到 remote，属新变更，不在本轮）。
- 不碰介质级擦除（W111 §7.2 第 2 项，已明确不做）。
