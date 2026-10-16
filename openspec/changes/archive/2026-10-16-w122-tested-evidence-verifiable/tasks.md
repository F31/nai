# Tasks: W122 — `tested` 的证据可核验（registry 状态回填第一批）

- [x] 1. registry 回填：`A13` `implemented → tested`（升级依据写入 notes）；`A03` 追加本轮复核留痕（状态维持 `implemented`，说明外部真值未闭合）
- [x] 2. 判据：`tests/registry.test.mjs` 新增 4 项（三方向一起判 / 闭世界覆盖 + 非空洞下限 / 两条注入反例）；本地跑绿（56 passed; 0 failed）
- [x] 3. spec：`admission-registry` 追加 3 条 Requirement（各 1 Scenario）
- [x] 4. 文档：ADR 0100 + W122 报告（含 15 条 `implemented` 缺口判定）+ `docs/development-plan.md` W122 段 + `docs/knowledge.md` W122 注记
- [x] 5. 全量门控 `cargo xtask check --profile full`（含 `check --registry`）

归档 → `openspec validate --archived` → 一次 commit + push（SSH 443）→ CI 双平台观测 → 回填报告：
按项目闭环纪律执行（流程项，非 checkbox）。
