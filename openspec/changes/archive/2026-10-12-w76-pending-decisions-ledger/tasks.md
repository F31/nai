# 任务（w76 待裁定台账）

## 1. 拆解与登记

- [x] 1.1 拆出 §18.2 / §39.7 / §48.7 / §57.5 的累积链并去重（唯一 **34** 条）
- [x] 1.2 逐条给出处置与依据（`introduced_in` / `also_in` / 五态之一 + 对应必填字段）

## 2. 实现

- [x] 2.1 新建 `tests/pending-decisions.yaml`（34 条 + `CHAPTER_COUNTS` 对照的常量表）
- [x] 2.2 新建 `scripts/contracts/pending-decisions.mjs`（`DISPOSITIONS` / `CHAPTER_COUNTS` /
      `validatePendingDecisions(ledger, deps)`，deps 显式注入且缺失即失败）
- [x] 2.3 断言并入 `tests/registry.test.mjs`（已由 `check --registry` 执行，`engineering_checks` 零改动）
- [x] 2.4 **附带修复（阻塞本轮门控的既有缺陷）**：`crates/nai-runtime/tests/common/mod.rs` 的
      `open_store`／`provision` 复用产品侧唯一的 `nai_runtime::composition::BRINGUP_OPERATION`，
      不再自造 5 s 副本——夹具做的正是「全新数据根 + 全链迁移」，那是 bring-up 的工作量，
      被挤进逐次调用预算时，在默认并行跑下表现为随机的 `DeadlineExceeded`

## 3. 验证

- [x] 3.1 非空洞注入验证七组（basis 指向不存在章节 / basis 为空 / 无 closure / 前置不存在 /
      删一条 / 处置越枚举 / id 重复），每组确认断言真开火后再还原
- [x] 3.2 并发连跑 3 轮 `cargo test -p nai-runtime --test composition`（默认 test-threads），确认修复稳定
- [x] 3.3 门控 `check --profile full`（本机）

## 4. 文档与闭环

按「一次 commit + push」的闭环顺序执行，本节为**执行记录**（无 checkbox）：

门控 → 勾 openspec tasks → `openspec archive -y` → 两条 validate（`--all --strict` 与
`--all --strict --archived`）→ 一次 commit + push → 单次低频 CI 查询 → 回填（回填提交自身不再回填）。

ADR 0054、`docs/reports/2026-10-12-w76-pending-decisions-ledger.md`、`docs/development-plan.md`
与 `AGENTS.md` 的轮次记录随本轮提交。
