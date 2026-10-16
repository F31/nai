# Proposal: W124 — 有界存储维护的零覆盖上限补齐（R08 补覆盖）

## Why
R08「存储维护有界」此前只有一句「WAL/配额逻辑部分覆盖」，**无任何载体**。实测零覆盖的边界：
`grep MAX_PAYLOAD_BYTES src/tests.rs` = **0**；快照拒绝 `-wal/-shm/-journal` 的 mutable journal 规则 = **0**；
`MAX_BYTES` = **0**。（遥测缓冲 `decision_buffer_stats` 有 9 处 ⇒ 不是整项未测。）

⇒ 这些上限可以被抬高或删除而**没有一条测试会红**，「有界」于是只是一句声明。

## What Changes
1. `crates/nai-storage/src/decision.rs`：`MAX_PAYLOAD_BYTES` 改为 `pub(crate)`（让判据**引用**上限而非复制字面量；W77 对 `Operation::CEILING` 的同一条规矩）。
2. `crates/nai-storage/src/tests.rs`：新增 2 条判别性判据（载荷上限、快照 sidecar 拒绝，后者带对照）。

## Modified Capabilities
- `local-persistence`：决策载荷上限 MUST 具名拒绝超界；快照 MUST 拒绝携带可变 journal sidecar。

## Impact
- registry：`R08` **维持 `implemented`**（`snapshot::MAX_BYTES`、`publication::{MAX_CHUNKS,MAX_BYTES}`、`knowledge::MAX_CONTENT_BYTES` 仍零覆盖，覆盖仍属部分）。
- 生产代码仅一处**可见性**变化（`pub(crate)`），无行为改动。测试 55→57。
