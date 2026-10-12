## 0. 前置核实

- [x] 读 V3.9 §5.9（模式爬坡四段）、§47.2（六类回归）、§47.3（晋升两条条件）
- [x] 核实契约 `SessionMode` 只有三个取值、无 `Shadow`；§1.4／§17.3 把「影子模式」列在 R2
- [x] 确认代码里无既有 ramp/爬坡实现（`grep ramp|爬坡` 仅命中不相关的测试辅助函数）
- [x] 确认 `session::change_mode` 是会话维度（值守租约），与本轮的机制维度互补不重叠

## 1. 判定面

- [x] 新建 `crates/nai-kernel/src/evaluate/ramp.rs`
- [x] `RampStage` 四段 + `RAMP_ORDER` + `index()` / `label()` / `session_mode()` / `enabled_by_default()`
- [x] `RegressionCategory` 六类 + `REGRESSION_CATEGORIES` + `label()`
- [x] `EnablementRecord` / `PromotionRequest` / `RampVerdict`
- [x] `RampRejection` 八个具名拒绝 + `ALL` + `label()`
- [x] `evaluate_promotion`：非自动阶段只要求「确实是前进」；自动阶段要求回归 + 记录
- [x] 在 `evaluate/mod.rs` 注册模块

## 2. 测试

- [x] `crates/nai-kernel/tests/ramp.rs`（13 项）
- [x] 集合守卫：构造出的拒绝集合恰为 `RampRejection::ALL`
- [x] 内容守卫：六类标签、四段顺序与默认启用集合、`session_mode()` 映射逐项写死
- [x] INV-11：扫值不扫字段名（名字含凭据标记而值干净的记录 MUST 放行）
- [x] 缺失类别须列出**全部**，不得只报第一个
- [x] 非空洞自检：五组注入各自被抓住（忽略回归 / 忽略记录 / 忽略修订 / 扫字段名 / 只报首个）

## 3. 登记与文档

- [x] `tests/registry.yaml`：H18 的 `notes` 改为「判定机制已交付，验收保持 `pending`」（`status` 不动）
- [x] `docs/kernel.md`：新增 W67 模块段
- [x] `docs/development-plan.md`：W67 轮次条目
- [x] `AGENTS.md`：W67 轮次段
- [x] `docs/reports/2026-10-12-w67-harness-ramp-gating.md`

## 4. 闭环

- [x] `cargo xtask check --profile full` 通过（EXIT=0；89 目标 / 798 passed / 0 failed）
- [x] `openspec validate w67-… --strict` 通过；需求正文 300 字符（< 500）
- [x] `openspec archive w67-… --yes`（核对归档目录日期戳）
- [x] 两条 validate 口径都跑（`--all --strict` / `--all --strict --archived`）
- [x] 一次 commit + push
- [x] 单次查询核对 CI 结论（避开匿名限流）
