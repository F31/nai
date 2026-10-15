# Proposal: W113 — §24.4 远端留存清除：`RemoteGateway::delete` 传输契约判定面闭合

## Why
W111 报告 §7.2 把「远端留存清除（§24.4 知识链那套 `RemoteRetention`，与记忆记录不共用落点）」列为
memory 子系统边界工作的**唯一尚存真实候选**。§24.4 的删除不变量已在
`crates/nai-knowledge-core/src/remote.rs:658-659` 以注释钉死：「Delete must follow a local tombstone；
the receipt is remote acknowledgment, **never proof that third-party copies were erased**」。
但该不变量**零判据**：

- 既有测试 `sync_and_delete_are_gated_single_attempt_and_return_receipts` 只钉了
  `require_remote_delete == false` 与 `allow_third_party_retention == false` 两条**门控开关**，
  delete 的**输入边界**（空集 / >16 / 含空 id，remote.rs:670-672）完全未被单测覆盖；
- 「回执透传、绝不声称第三方已擦除」无测试守卫，契约若新增 `erased`/`purged` 之类的
  `RemoteReceiptStatus` 变体也不会被任何测试拦下。

回归即可悄悄把「远端删除成功」误读成「第三方副本已擦除」——正是 §24.4 明令禁止的虚假陈述。

## What Changes
只补可测判据，把既有的两条 §24.4 机制（delete 输入边界、回执透传枚举闭世界）变成显式守卫；
**零生产代码改动**。

- `crates/nai-knowledge-core/tests/remote.rs` 新增 2 项：
  - `remote_delete_rejects_malformed_document_ids`（空集 / >16 / 含空 id ⇒ `Unauthorized`，不触网）；
  - `remote_delete_receipt_is_passed_through_and_never_claims_erasure`（回执原样透传 + 对
    `RemoteReceiptStatus` 四个变体穷尽 match，防止新增「擦除证明」变体）。
- 既有 `sync_and_delete_are_gated_single_attempt_and_return_receipts` 归属 W113（钉
  `require_remote_delete` / `allow_third_party_retention` 门控 + 不触网），不重复造。
- `knowledge-pipeline` spec 新增一条 Requirement + 2 个 Scenario，把 §24.4 delete 契约纳入基线。

## Modified Capabilities
- `knowledge-pipeline`（远端知识契约与撤销）：追加 delete 传输契约判定面。

## Impact
- 无运行时行为变化（判定面闭合，不引入新执行体）。
- 无契约字段改动、无迁移、无 schema 改动（`RemoteReceiptStatus` 枚举形状被闭世界守卫钉住，
  但成员数不变）。
- registry：§24.4 远端留存不在 H03/H04（记忆）登记范围内，无既有 H 条目需改；如实披露
  `max_retention_seconds` 声明未强制（见 design.md 决策 5）。
