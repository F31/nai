# 任务 · W81 字段集封闭性

- [x] 1. `pending-decisions.mjs` 新增并导出 `STRUCTURAL_FIELDS`（`id`／`disposition`）与
  `LEDGER_FIELDS`（逐字写死的十个字段）
- [x] 2. `CITATION_FIELDS` 增加 `question`（复述字段 MUST 被扫）
- [x] 3. `shapeErrors()` 新增：三类两两不相交；三类之并 MUST 恰等于 `LEDGER_FIELDS`
- [x] 4. `validatePendingDecisions` 新增**闭世界**断言：条目里出现的每个键 MUST ∈ `LEDGER_FIELDS`，
  否则报「条目 + 字段名」（**不看值**）
- [x] 5. `tests/registry.test.mjs` 新增用例：未知字段注入（含**值干净**的一次）、`question` 引用注入、
  逐字钉 `LEDGER_FIELDS` 与三类划分（28 → 30）
- [x] 6. 非空洞注入 F0 + F1–F7（含两条「不该红时不红」与两条表级），逐项开火并还原
- [x] 7. ADR 0059 + `specs/spec-driven-delivery` 差量（一条 ADDED Requirement）
- [x] 8. 轮次报告 + `AGENTS.md` + `docs/development-plan.md` 同步
- [x] 9. 全量门控（EXIT=0）、两条 validate、归档、提交与 CI 观测
