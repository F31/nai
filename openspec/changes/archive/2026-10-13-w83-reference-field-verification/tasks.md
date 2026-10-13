# 任务 · W83 引用字段的验证范围

- [x] 1. `pending-decisions.mjs` 新增并导出 `REFERENCE_FIELDS`（**逐字写死**的八个：七个引用字段 ＋ `precondition`）
- [x] 2. `validatePendingDecisions` 把「§ 扫描」与「`precondition` id 校验」合并为**一个**按
  `REFERENCE_FIELDS` 遍历、**只由字段存在触发**的循环（触发条件里**不出现 `disposition`**）
- [x] 3. `precondition` 的错误措辞改由 `${field}` 生成（条目 + 字段 + 值三件齐）
- [x] 4. `shapeErrors()` 新增：`REFERENCE_FIELDS` ↔ `CITATION_FIELDS ∪ ID_LIST_FIELDS` **双向**具名相等
- [x] 5. `tests/registry.test.mjs` 新增用例（32 → 34）：泛化到 `ID_LIST_FIELDS` 的行为用例
  （错处置下坏 id MUST 失败）＋「合法 id 在非专属处置上 MUST NOT 红」的**反向注入**
  ＋ 逐字钉 `REFERENCE_FIELDS` 与非空洞
- [x] 6. 非空洞注入 F0 + F1–F4（数据级）＋ F5–F8（表级，含反向注入），逐项开火并还原
- [x] 7. ADR 0061 + `specs/spec-driven-delivery` 差量（一条 MODIFIED Requirement）
- [x] 8. 轮次报告 + `AGENTS.md` + `docs/development-plan.md` 同步
- [x] 9. 全量门控（EXIT=0）、两条 validate、归档、提交与 CI 观测
