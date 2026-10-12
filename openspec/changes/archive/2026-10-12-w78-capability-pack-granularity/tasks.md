# 任务 · W78 打包粒度

- [x] 1. `registry.mjs` 导出 `CAPABILITY_PACK_IDS`（补注释：它是 §51.3 的**镜像**，故必须与正文对账）
- [x] 2. `baseline.mjs` 新增 `CAPABILITY_PACK_ROWS`（8 行的标签+编号，逐项写死）
- [x] 3. `baseline.mjs` 新增 `parseCapabilityPacks(text)` → `{found, rows, union, overlaps, retained, errors}`
- [x] 4. `parseBaseline` 的返回值加入 `capabilityPacks`
- [x] 5. `reconcile` 增加第 ④ 区块：G-A 表形状 / G-B 依据非空 / G-C 划分 / G-D 边界互斥 / G-E 三向集合一致
- [x] 6. `tests/registry.test.mjs` 增加用例：§51.3 是划分、边界互斥、常量接在正文上（21 → 24）
- [x] 7. 非空洞注入：文件级 F1–F9 逐项开火并还原；测试内三组用合成存根覆盖解析器自身
- [x] 8. ADR 0056 + `specs/admission-registry` 差量（一条 ADDED Requirement）
- [x] 9. 轮次报告 + `AGENTS.md` + `docs/development-plan.md` 同步
- [x] 10. 全量门控（EXIT=0 / 92 目标 / 860 passed）、两条 validate、归档、提交与 CI 观测
