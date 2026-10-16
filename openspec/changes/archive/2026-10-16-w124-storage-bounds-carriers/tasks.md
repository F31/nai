# Tasks: W124 — 有界存储维护的零覆盖上限补齐

- [x] 1. `MAX_PAYLOAD_BYTES` 改 `pub(crate)`（判据引用上限而非复制字面量）
- [x] 2. 判据 2 条：载荷上限（具名拒绝）+ 快照 sidecar（三后缀 + 对照）；本地 55→57 全绿
- [x] 3. spec：`local-persistence` 追加 2 条 Requirement
- [x] 4. registry：`R08` 记进展并**维持 `implemented`**（上限仍未全覆盖）
- [x] 5. 文档：ADR 0102 + W124 报告 + development-plan/knowledge 注记
- [x] 6. 全量门控 + 一次 commit + push + CI 观测

归档 → `validate --archived` → commit/push → CI 双平台观测 → 回填报告。
