# 任务（w77）

## 1 裁定与立项

- [x] 1.1 核实依赖方向与现存副本，裁定 bring-up 预算的定义点归属
- [x] 1.2 建立 OpenSpec 变更 w77-bringup-budget-single-home（proposal / design / spec delta / tasks）

## 2 实现

- [x] 2.1 `nai-security`：公开 `Operation::CEILING`，`new()` 改用它做拒绝阈值
- [x] 2.2 `nai-storage`：新增 `pub const BRINGUP_OPERATION: Duration = Operation::CEILING;`
- [x] 2.3 `nai-runtime`：`composition.rs` 改 re-export，并订正「Sized here once」的注释
- [x] 2.4 消费者改引：`nai-storage/{tests,upgrade}.rs`、`nai-control-api/src/tests.rs`、
      `nai-upgrade-core/tests/{plan,migration,lifecycle,gates}.rs`、`nai-provider-core/examples/*`、
      `xtask/examples/golden_eval.rs`

## 3 验证

- [x] 3.1 Rust 断言：`nai-security` 的 `CEILING` 三断言、`nai-storage` 的等式断言
- [x] 3.2 Node 守卫（并入 `tests/registry.test.mjs`）：定义点唯一 / 值引用 `CEILING` / 无 30 s 字面量
- [x] 3.3 非空洞注入验证：七组（Node 五 + Rust 二）逐项开火并还原

## 4 文档与闭环

- [x] 4.1 写 `docs/adr/0055-bringup-budget-single-home.md`
- [x] 4.2 写规格差量（`scheduling-and-budgets`）并确认已归档吸收
- [x] 4.3 写 `docs/reports/2026-10-12-w77-bringup-budget-single-home.md`
- [x] 4.4 同步 `AGENTS.md` 与 `docs/development-plan.md` 的轮次记录
- [x] 4.5 门控 `full` → 勾 tasks → `archive -y` → 两条 validate → 一次 commit + push → CI 观测
