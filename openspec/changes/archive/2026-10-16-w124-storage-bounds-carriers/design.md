# Design: W124 — 有界存储维护的零覆盖上限

## 缝（现测）
| 上限 | 位置 | 判据缺口 | 若被改动的后果 |
|---|---|---|---|
| `MAX_PAYLOAD_BYTES` = 65536 | `decision.rs:11` | **零引用** | 决策载荷无界增长，静默吃盘 |
| mutable journal 规则 | `snapshot.rs:293` | **零引用** | 快照可通过 WAL sidecar 夹带批准哈希之外的数据 |

## 决策
1. **判据引用上限，不复制字面量**：`MAX_PAYLOAD_BYTES` 改 `pub(crate)`（否则测试只能写第二个 65536，
   两处会漂移——这正是 W77 立过的规矩）。
2. **判别性构造**：载荷用 `cap/2`（通过）与 `cap`（序列化后 `cap+overhead`，越界）两档；
   越界断言**具名** `RESOURCE_EXHAUSTED`，故「静默截断」也会红。
3. **快照 sidecar 带对照**：三个后缀各注入一次 ⇒ `CONFLICT`；**随后删除 sidecar，同一快照必须能恢复** ——
   这条对照把「拒绝是因为 sidecar」与「拒绝是因为备份本身坏了」分开（否则断言会假通过）。
4. **不改行为**：只改可见性 + 加测试。

## 明确不做
- 不补 `snapshot::MAX_BYTES`(256MiB) / `publication` / `knowledge` 的上限判据：256MiB 级夹具代价过高，
  且本轮不宣称 R08 已完整覆盖。
