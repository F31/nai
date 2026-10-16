# Proposal: W125 — 存储写入上限的名册与快照/发布族覆盖（R08 进展）

## Why
W124 把 R08 的缺口写成「`snapshot::MAX_BYTES`、`publication::{MAX_CHUNKS,MAX_BYTES}`、
`knowledge::MAX_CONTENT_BYTES` 仍零覆盖」。本轮先做**名册**再做覆盖，因为逐项点名会漏：
按「具名拒绝」普查 `nai-storage`，约束**单次写入或单份快照/备份可占用资源**的上限共 **19** 项，
而本轮之前**只有 2 项有载体**（决策载荷、遥测缓冲）——其余 17 项可以被抬高或删除而**没有一条测试会红**，
「有界」于是只是一句声明。

同一普查暴露两处**口径订正**：
1. W124 把 `publication`/`knowledge` 的上限记成 R08 的门槛，而 R08 由 D03 映射，D03 的原始存储边界是
   「登记的不可变文件**数量/大小/路径受限**」——三族都要收，不能只收一族的四分之一。
2. `publication.rs` 把「分块数」与「分块字节」**合并成一个拒绝**：一条计数夹具会替字节上限背书。

## What Changes
1. `knowledge.rs` / `publication.rs` / `snapshot.rs` 的 6 个上限常量改 `pub(crate)`——判据**引用**
   上限而不是复制字面量（W77 对 `Operation::CEILING` 的同一条规矩）。
2. `publication.rs`：计数与字节拆成两个具名拒绝（`snapshot chunk count bound` /
   `snapshot chunk byte bound`），使越界者**可归因**。
3. `tests.rs`：修掉唯一一处**复制字面量**（`"x".repeat(16 * 1024 * 1024 + 1)` → 引用常量）；
   新增 **6** 条判据（名册幅度 1 + 边界 5）。
4. 名册与口径写入 ADR 0103；`tests/registry.yaml` 的 R08 就地订正并记覆盖进展。

## Modified Capabilities
- `local-persistence`：快照**读回**先按声明上限拒绝，且「超限」与「哈希不符」是**不同**的具名拒绝；
  单份 artifact 的上限在**写入**与**读回**两侧同值同界。
- `knowledge-pipeline`：发布的快照上限是**所选文档的合计**，不是逐文档上限。

## Impact
- registry：`R08` **维持 `implemented`**——名册 19 项中已覆盖 8、剩余 11 **具名列出**，
  覆盖仍属部分，故不升 `tested`（第四次使用「守卫通过 ≠ 本项达标」）。
- 生产代码：仅可见性 + 一处拒绝拆名；**无行为语义改动**（同一输入仍以同一 `ErrorCode` 拒绝）。
