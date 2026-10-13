# w93 任务清单

- [x] 核实 `decided_in_repo` 的「已落地」是否被判据守住：探针①`landing` → `README.md` 全绿；探针②删规格落地句 全绿
- [x] 核实另两态（结论：无可订正数据）——4 条 `basis` 都真答了；4 条前置全部未达成（H05/06/07/08/13/14/16 与 W61 的 H11/H12 均 `pending`）
- [x] 建立 OpenSpec 变更 `w93-landing-content-anchor`（proposal / design / spec 差量 / tasks）
- [x] 实现 `landing_anchor` 判据：第五类 `ANCHOR_FIELDS` + `COUPLED_REQUIRED_FIELDS`（同生同灭且等长）+ 依赖 `readTrackedText`（读不到具名失败，不静默降级）
- [x] 改 `shapeErrors()`：类别划分四类 → 五类，闭世界 `LEDGER_FIELDS` 加 `landing_anchor`；`REFERENCE_FIELDS` 不纳入锚点
- [x] 给 4 条 `decided_in_repo` 配真实内容锚点（PD-09 三路径、PD-22、PD-24、PD-31），逐条实测存在
- [x] 改 `tests/registry.test.mjs`：注入 `readTrackedText`、缺输入清单加它、新增锚点用例（含反向不红）、修 W88 反向用例的前提
- [x] 非空洞注入 P1/P2 + F1–F8（含 F5/F6 按字段存在触发与 F7/F8 闭世界与类别并集），改前/改后对照并 sha256 还原校验
- [x] 写 ADR 0071 与活文档同步（`AGENTS.md` / `docs/development-plan.md`）
- [x] 写轮次报告并跑全量门控（`check --profile full`）与两条 `openspec validate`
- [x] 归档、一次提交推送、单次 CI 观测并回填报告
