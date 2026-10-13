# 任务 · W82 台账信封的闭世界

- [x] 1. `pending-decisions.mjs` 新增并导出 `TOP_LEVEL_FIELDS`（逐字写死的三个键）
- [x] 2. `validatePendingDecisions` 新增**信封闭世界**断言：顶层出现的每个键 MUST ∈
  `TOP_LEVEL_FIELDS`，否则报「键名」（**不看值**）
- [x] 3. `validatePendingDecisions` 新增**信封必需**断言：声明的每个顶层字段 MUST 出现在台账上
- [x] 4. `shapeErrors()` 新增：`TOP_LEVEL_FIELDS` ∩ `LEDGER_FIELDS` MUST 为空（具名）
- [x] 5. `tests/registry.test.mjs` 新增用例：顶层未知键注入（含**值干净**的一次）、
  「条目字段名放到顶层」注入、逐字钉 `TOP_LEVEL_FIELDS`、信封字段逐个删除（30 → 32）
- [x] 6. 非空洞注入 F0 + F1–F4（数据级）+ F5–F9（表级，含 F8／F9 两条**反向注入**），逐项开火并还原
- [x] 7. ADR 0060 + `specs/spec-driven-delivery` 差量（一条 MODIFIED Requirement）
- [x] 8. 轮次报告 + `AGENTS.md` + `docs/development-plan.md` 同步
- [x] 9. 全量门控（EXIT=0）、两条 validate、归档、提交与 CI 观测
