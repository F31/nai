## Why

W110 把遗忘接上了控制面，但**清除侧仍然是调用方在填空题**。现测（`crates/nai-runtime/tests/memory.rs`
新增判据 `claiming_fully_erased_must_leave_no_payload_in_the_store`，本轮**先跑红**）：

```
test claiming_fully_erased_must_leave_no_payload_in_the_store ... FAILED
宣称已完全擦除 ⇒ 载荷 MUST NOT 仍在库里；实测仍读到
  "the record body that MUST be persisted verbatim, not a pointer"
```

请求体给 `purge_state: "complete"` + `claims_fully_erased: true`，系统**接受**，
而 `memory_records.content` 里那句原文**一字未动**。这不是「少一个功能」，是 §42.4 的两条要求同时落空：

- 「删除 MUST 可验证：清理侧给出完成状态」——状态是**调用方给的**，不是执行得出的；
- 「MUST NOT 宣称已完全擦除」——系统接受了一个**零执行支撑**的宣称。

而 W110 的报告如实写着「副本清除未执行」，**端点却允许调用方宣称已清完**——交付面自相矛盾。

**副本集合现测为空集**：`memory_records` 是唯一载荷落点（读取面直扫表），**无**记忆索引、
**无**缓存、**无**远端同步、控制面与 runtime **零** `decision_log` / `audit_records` 写入。
所以本轮不是「给已有的副本加清除」，而是**先让「副本」成为一件可清点、可核对的事**。

## What Changes

- **「清除」从字段变成动作**：`purge_state` 与 `claims_fully_erased` 从遗忘请求体**删除**——
  它们是**结论**，结论只能由执行得出。`third_party_retention` **保留**：它是**事实**，
  且只有调用方知道（本仓不接触第三方留存）。
- **新增清除执行体**：`Composition::purge_memory` —— **先阻断读取、再清载荷**，顺序不可交换（§42.4）。
  载荷清除 = `content` 置空 + 记 `purged_at_ms`；行**保留为 tombstone**（§24.4「先发布 tombstone」）。
- **新增承载物**：迁移 `0009-memory-purge.sql` 给 `memory_records` 加 `purged_at_ms`（可空）。
- **新增副本清单**（`COPY_SITES`，声明式闭世界，当前**空集**）：每个在册点 MUST 有清除体；
  记忆载荷落点由**代码派生**（扫存储层写记忆的 SQL 目标表）与在册集合比对。
- **擦除口径写进形状**：`erased` 恒为 `{ scope: "logical", physical_bytes_erased: false }`——
  本仓**没有**做介质级擦除（SQLite 页与 WAL 残留不在承诺内），故**永远**不宣称介质级完全擦除。

## Modified Capabilities

- `local-control-surface`：遗忘端点的**结论来源**（请求体不再承载结论），并新增副本清单的闭世界要求。
- `local-persistence`：新增清除动作的承载物与「只置一次、不覆盖」。
- `runtime-composition`：新增清除执行体（顺序、清单清点、结论由事实得出）。

## Impact

- `crates/nai-control-api/src/lib.rs`：`MemoryForgetRequest` 收窄；响应带系统结论；副本清单与其校验。
- `crates/nai-runtime/src/memory.rs`：`purge_memory` 执行体；`forget_memory` 参数收窄。
- `crates/nai-kernel/src/evaluate/memory.rs`：`ForgetOutcome` 的 `read_side_blocked` 兄弟字段
  `body_purged`（由「真的清过了」得出）。
- `crates/nai-storage/`：迁移 0009 + `purge_memory_body` 访问器（**版本号成套五处**）。
- `apps/engine/src/lib.rs`：装配处改调清除入口。