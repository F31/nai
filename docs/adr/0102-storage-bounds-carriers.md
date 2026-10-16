# ADR 0102：有界存储维护的上限必须有载体

- 状态：Accepted｜日期：2026-10-16｜归属：W124（R08 补覆盖）
- 关联：ADR 0100/0101、`local-persistence` spec、R08（存储维护有界）、W77（上限常量公开让消费者引用）

## 背景
R08 此前只有一句「WAL/配额逻辑部分覆盖」，**无载体**。实测：`grep MAX_PAYLOAD_BYTES src/tests.rs` = 0；
快照拒绝 `-wal/-shm/-journal` 的 mutable journal 规则 = 0；`MAX_BYTES` = 0。
⇒ 上限可被抬高或删除而**没有一条测试会红**，「有界」于是只是声明。

## 决策
1. **上限要能被判据引用，而不是被复制**：`MAX_PAYLOAD_BYTES` 改 `pub(crate)`，测试直接用常量。
   写第二个 `65536` 会让上限与检查它的代码各自漂移（W77 对 `Operation::CEILING` 立过的同一条规矩）。
2. **判别性构造 + 具名拒绝**：载荷分 `cap/2`（通过）与 `cap`（序列化后越界）两档，越界断言**具名**
   `RESOURCE_EXHAUSTED` ⇒ 「静默截断」同样变红。
3. **对照是断言的一部分**：快照注入 sidecar ⇒ `CONFLICT`；**删掉 sidecar 后同一快照必须能恢复**。
   没有这条对照，「拒绝」可能只是备份本身坏了，判据会假通过。
4. **不越级宣称**：`snapshot::MAX_BYTES`(256MiB)、`publication::{MAX_CHUNKS,MAX_BYTES}`、
   `knowledge::MAX_CONTENT_BYTES` 仍零覆盖 ⇒ R08 **维持 `implemented`**。

## 后果
- 正向：两条上限从「散文」变成「可判定的拒绝」。
- 生产代码仅一处**可见性**变化，无行为改动；测试 55→57。
- 如实披露：本轮不宣称 R08 完整覆盖；256MiB 级夹具代价过高，暂不列入。
