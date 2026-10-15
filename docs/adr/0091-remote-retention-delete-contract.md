# ADR 0091：§24.4 远端留存清除 —— `RemoteGateway::delete` 传输契约判定面闭合

- 状态：Accepted
- 日期：2026-10-15
- 归属：W113（memory 子系统边界工作的延续；W111 §7.2 遗留清单唯一尚存真实候选）
- 关联：ADR 0089（memory purge surface）、ADR 0090（§42.5 跨会话边界）

## 背景
W111 报告 §7.2 把「远端留存清除（§24.4 知识链那套 `RemoteRetention`，与记忆记录不共用落点）」
列为 memory 边界工作的唯一尚存真实候选。§24.4 的删除不变量已在
`crates/nai-knowledge-core/src/remote.rs:658-659` 以注释钉死，但无判据守卫：

- delete 的**输入边界**（空集 / >16 / 含空 id，remote.rs:670-672）未被任何单测覆盖；
- 「回执透传、绝不声称第三方已擦除」无守卫，契约若新增 `erased`/`purged` 变体无人拦。

回归即可把「远端删除成功」误读成「第三方副本已擦除」——§24.4 明令禁止的虚假陈述。

## 决策
**判定面闭合，零生产代码改动**。只补可测判据，把既有的两条 §24.4 机制变成显式守卫：

1. delete 输入边界闭世界（空集 / >16 / 含空 id ⇒ `Unauthorized`，不触网）；
2. 回执原样透传 + 对 `RemoteReceiptStatus` 穷尽 match 四态，防止新增「擦除证明」变体；
3. 既有 `sync_and_delete_are_gated_single_attempt_and_return_receipts` 归属本 ADR（门控开关），不重复造。

`tombstone → 远端删除` 的本地前置由存储层 `Store::remote_delete_targets` 负责（`docs/knowledge.md:65`）；
`RemoteGateway::delete` 是传输层，假定 tombstone 已存在，本 ADR 不重复存储层判定。

## 后果
- 正向：§24.4 delete 契约的两条不变量被钉死，回归会红；`RemoteReceiptStatus` 枚举闭世界守卫
  使「新增擦除证明变体」会编译失败。
- 零运行时变化、零契约字段改动、零迁移、零 schema 改动。
- 如实披露（非验收达标）：`max_retention_seconds`（`RemotePolicy` 字段）声明但未被 `gate` 强制，
  本地无保留期上限执行机制；介质级擦除仍明确不做（W111 §7.2 第 2 项）。
- registry：§24.4 远端留存不在 H03/H04（记忆）范围内，无既有 H 条目需改。
