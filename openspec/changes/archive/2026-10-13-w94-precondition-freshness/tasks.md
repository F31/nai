# w94 任务清单

- [x] 核实 `needs_precondition` 的「还在等」是否被判据守住：探针 `PD-26.precondition` → `["S03"]`（`status: tested`）全绿
- [x] 核实另两个候选（结论：无可做的事）——21 条 `closure` 确实闭合在仓外；`question` 逐字对账过脆（34 条中 11 条未命中）
- [x] 实测判定面：status 取值域（implemented 29 / tested 22 / pending 43 / blocked 6 / not_applicable 3）；`S12` 是 `not_applicable` 却仍依赖外部 ⇒ 归**未达成**；工作项表无状态列、36 行中 11 行无关联验收
- [x] 建立 OpenSpec 变更 `w94-precondition-freshness`（proposal / design / spec 差量 / tasks）
- [x] 实现判据：`ACCEPTANCE_STATUSES`／`MET_STATUSES`／`UNMET_STATUSES` + `baselineWorkItemAcceptance()` + 两个必填依赖，触发条件按字段存在
- [x] `shapeErrors()` 加取值域闭世界断言（MET ∪ UNMET 恰等于取值域、两两不相交）
- [x] 改 `tests/registry.test.mjs`：注入两个新依赖、缺输入清单加它们、新增用例（含 F5 `S12` 不红、F6 反向不红）；修 W88 反向用例前提（`W00` 现不可判定）
- [x] 非空洞注入 P1 + F1–F8（含探针回归），改前/改后对照并 sha256 还原校验（ALL-OK）
- [x] 写 ADR 0072 与活文档同步（`AGENTS.md` / `docs/development-plan.md`）
- [x] 写轮次报告并跑全量门控（`check --profile full`）与两条 `openspec validate`
- [x] 归档、一次提交推送、单次 CI 观测并回填报告
