# 任务 · W79 节界

- [x] 1. `baseline.mjs` 新增 `headingLevel(line)`（ATX 标题层级；非标题返回 0）
- [x] 2. `baseline.mjs` 重写 `sectionTable`：搜索域封闭在 `[标题行, 下一个层级 ≤ 该标题的行)`，返回三态（`found` / `hasTable` / `rows` + `after` + `level` + `start` + `end` + `tableStart`），并 **export**
- [x] 3. 三个解析器（`parseFamilyIndex` / `parseReleaseLines` / `parseCapabilityPacks`）各加 `hasTable` 分支，报具名错误 `§X has no table inside its section`
- [x] 4. `tests/registry.test.mjs` +3 用例（24 → 27）：B-A 三张表本节内确有表、B-B 邻节的表不得被借用、B-C 更深标题不截断、B-D §51.2／§57.5 是散文节的前提
- [x] 5. 非空洞注入：文件级 F1–F7 逐项开火并还原（先确认 V0 基线为 PASS）
- [x] 6. ADR 0057 + `specs/admission-registry` 差量（一条 ADDED Requirement）
- [x] 7. 轮次报告 + `AGENTS.md` + `docs/development-plan.md` 同步
- [x] 8. 全量门控（EXIT=0）、两条 validate、归档、提交与 CI 观测
