# w88 任务

- [x] 核实 PD-09 的推荐与仓库真实状态（`nai-security` / 平台矩阵 / §11.5 原文 / 台账校验器）
- [x] 裁清：第六态 `decided_in_repo` + 必填 `landing`（仓库成员路径）；排除 `answered_by_baseline`
- [x] 建 change 目录与 OpenSpec 规格差量（`spec-driven-delivery`：2 MODIFIED + 1 ADDED）
- [x] 实现校验器改动（第 6 态、第 4 类别、`repositoryFiles` 依赖、清单型判定改为按类别表）
- [x] 台账数据：`PD-09` 迁到 `decided_in_repo` 并给 `landing`；`DISPOSITION_COUNTS` 5→4 / 0→1
- [x] 扩展 `tests/registry.test.mjs` 判据（47 → 50），正反双向注入
- [x] 非空洞注入：F0–F7 探针 + 改前/改后对照（`field === 'precondition'` vs 类别表）
- [x] 写 ADR 0066、ADR 0054 追加勘误段、同步活文档（AGENTS.md / development-plan.md）
- [x] 报告 + 全量门控 + 归档 + 两条 validate + 提交推送 + CI 观测 + 回填
