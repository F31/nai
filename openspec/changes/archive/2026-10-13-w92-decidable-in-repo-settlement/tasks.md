# w92 任务清单

- [x] 核实三条 `decidable_in_repo` 的落地面：PD-22/PD-24 同一段活规格；PD-34 闭合在仓外（登记不实）
- [x] 核实不越权边界：基线 §46.3「初值 1」是一般描述、规格可作 R1 具体化；§48.7 本就登记该问题为待决
- [x] 建立 OpenSpec 变更 `w92-decidable-in-repo-settlement`（proposal / design / spec 差量 / tasks）
- [x] 改 `tests/pending-decisions.yaml`（PD-22/PD-24 → `decided_in_repo` + `landing`；PD-34 → `needs_external` + `closure`）并同步 `DISPOSITION_COUNTS`
- [x] 改 `tests/registry.test.mjs`：两处注入的样本定位改为「按字段存在」，并加 `decidable_in_repo == 0` 的成果断言（`xtask` 零改动）
- [x] 非空洞注入 F1–F8（含 F5 索引/工作区判据分离与 F6/F7 反向不红），改前/改后对照并还原校验
- [x] 写 ADR 0070 与活文档同步（`AGENTS.md` / `docs/development-plan.md`；`docs/kernel.md` 无 Rust 改动 ⇒ 无需同步）
- [x] 写轮次报告并跑全量门控（`check --profile full`）与两条 `openspec validate`
- [x] 归档、一次提交推送、单次 CI 观测并回填报告
