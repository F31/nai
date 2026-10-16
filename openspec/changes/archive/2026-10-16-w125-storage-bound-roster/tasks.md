# Tasks: W125 — 存储写入上限的名册与快照/发布族覆盖

- [x] 1. 6 个上限常量改 `pub(crate)`（判据引用上限而非复制字面量）
- [x] 2. `publication` 的合并拒绝拆成计数/字节两条具名拒绝
- [x] 3. `tests.rs`：修掉唯一一处复制字面量；+6 条判据（名册幅度 1、边界 5）；本地 57→63 全绿
- [x] 4. 注入验证：三条独立注入（读回尺寸闸门、发布合并理由、内容界含不含端点）各自必须变红
- [x] 5. spec：`local-persistence` +2、`knowledge-pipeline` +1 Requirement
- [x] 6. registry：`R08` 就地订正名册口径并记覆盖进展，**维持 `implemented`**
- [x] 7. 文档：ADR 0103 + W125 报告 + development-plan/knowledge 注记
- [x] 8. 全量门控 + 一次 commit + push + CI 观测

归档 → `validate --archived` → commit/push → CI 双平台观测 → 回填报告。
