# Tasks: W123 — 装配根接上真实会话（R10 接线）

- [x] 1. `Composition` 实现 `ActiveSessionGate`（读真实会话；活跃 = Running/Draining；busy ⇒ fail closed）
- [x] 2. 判据：`crates/nai-runtime/tests/composition.rs` 新增 2 条（真实 lifecycle 双向；busy 锁 ⇒ Conflict）；本地跑绿（14 passed; 0 failed）
- [x] 3. spec：`runtime-composition` 追加 2 条 Requirement（各 1 Scenario）
- [x] 4. registry：`R10` 记进展并**维持 `implemented`**（运行时仍无升级激活调用点）
- [x] 5. 文档：ADR 0101 + W123 报告 + development-plan W123 段 + knowledge 注记
- [x] 6. 全量门控 `cargo xtask check --profile full`

归档 → `validate --archived` → 一次 commit + push（SSH 443）→ CI 双平台观测 → 回填报告。
