# w95 任务清单

- [x] 核实并裁定：CI 工作流的步集合是否被判据守住。两个探针：①往 `foundation.yml` 插一个 step ⇒ 本机 3 套件全绿零信号；②删掉 `schema --check` 判定步 ⇒ 同样全绿
- [x] 核实无替代守卫：`grep -rn foundation.yml crates/ xtask/` 为空；`test-reachability` 只从 workflow 读一个 marker（`descriptor.test.mjs`），其余 20 个 step 不在受判范围
- [x] 盘点 workflow：21 个 step（7 infra / 1 local / 1 diagnostic / 12 ci-only 或 partial），逐条记下「本机为什么跑不了」的成因
- [x] 建立 OpenSpec 变更 `w95-workflow-step-closure`（proposal / design / spec 差量 / tasks）
- [x] 实现 `scripts/contracts/workflow-steps.mjs`：`WORKFLOW_FILES` / `STEP_KINDS` / `CI_ONLY_REASONS` / `WORKFLOW_STEPS`，以及文件层与步层两层判据（身份唯一、闭世界两方向、假契约、双向字段、diagnostic 必须带 `if`）
- [x] 新建 `tests/contracts/workflow.test.mjs`，并在 `xtask/src/lib.rs` 的 `engineering_checks` 登记该套件
- [x] 在 `test-reachability.mjs` 的 `TEST_REGISTRATION_SITES` 登记新套件的站点（否则 W84 的闭世界立刻红——它的应有行为）
- [x] 非空洞注入 P1（加法探针）/ P2（减法探针）+ F1–F8，改前/改后对照并 sha256 逐字还原校验
- [x] 逐条核对与 `test-reachability` 的边界：两者的独有覆盖，MUST NOT 互相顶替
- [x] 写 ADR 0073 与活文档同步（`AGENTS.md` / `docs/development-plan.md`）
- [x] 写轮次报告并跑全量门控（`check --profile full`）与两条 `openspec validate`
- [x] 归档、一次提交推送、单次 CI 观测并回填报告
