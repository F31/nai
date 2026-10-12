# 任务 · W80 引用完整性

- [x] 1. `pending-decisions.mjs` 新增并导出 `CITATION_FIELDS`（六字段）与 `ID_LIST_FIELDS`（`precondition`）
- [x] 2. `pending-decisions.mjs` 新增并导出 `shapeErrors()`：处置枚举 ↔ 必填字段表双向一致、引用域与 id 域不相交、每个必填字段必有覆盖；并在 `validatePendingDecisions` 里无条件并入返回值
- [x] 3. 引用扫描改为遍历 `CITATION_FIELDS`；错误措辞改为「条目 + 字段 + 章节号」三件齐
- [x] 4. `tests/registry.test.mjs` 扩展「引用必须可解析」用例：对 `recommendation`／`closure`／`note` **各注入一次并要求指名字段**；新增一条断言 `CITATION_FIELDS` 逐字含三字段、`REQUIRED_FIELD` 键集 == `DISPOSITIONS`（27 → 28）
- [x] 5. 非空洞注入：字段级 F1–F3（三个此前未扫的字段）、表级 F4–F5（把清单改坏）逐项开火并还原（V0 基线先 PASS）
- [x] 6. ADR 0058 + `specs/spec-driven-delivery` 差量（一条 ADDED Requirement）
- [x] 7. 轮次报告 + `AGENTS.md` + `docs/development-plan.md` 同步
- [x] 8. 全量门控（EXIT=0）、两条 validate、归档、提交与 CI 观测
