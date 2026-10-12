## 0 准备

- [x] 核实 §47.1／§47.4／§12.2／INV-11／INV-32 的原文口径
- [x] 核实 `crates/` 内无既有 harness 观测或降级判定面
- [x] 决定落在 `evaluate/` 并记下与 `bridge::Degradation` 的分工

## 1 建立 OpenSpec 变更

- [x] `openspec new change w66-harness-observability-and-degradation --schema spec-driven`
- [x] 写 proposal / design / 两条 ADDED 需求 / tasks
- [x] `openspec validate w66-harness-observability-and-degradation --strict --no-interactive`

## 2 实现观测判定（H15）

- [x] `crates/nai-kernel/src/evaluate/harness.rs`：`ObservationLayer` 五类与 `required_fields()`
- [x] `ObservationRejection` 扁平枚举 + `ALL` 常量 + `label()`
- [x] `CREDENTIAL_MARKERS` 值扫描与观众原文脱敏判定
- [x] `evaluate_observation` 输出 `loggable` / `rejections` / `redacted`

## 3 实现降级判定（H17）

- [x] `FailureLayer` 六类与 `DegradeLevel`（§12.2 四级）
- [x] 首选级 + 可用性回退链；装配层置 `refuses_call`
- [x] `assert_no_free_form`：降级 + 失去策略／人设约束 ⇒ 具名违规
- [x] `evaluate/mod.rs` 挂 `pub mod harness;`

## 4 测试与非空洞守卫

- [x] `crates/nai-kernel/tests/harness.rs` 五类完整性与 `label()` 互异
- [x] 守卫②：值含凭据 ⇒ 拒；字段名含 `key` 但值干净 ⇒ 放行
- [x] 守卫③：不同失败层落到不同级；首选级不可用时沿序回退
- [x] 守卫④：降级且无策略约束 ⇒ 违规；仅给错误码仍生成 ⇒ 违规
- [x] 补守卫①的缺口：`required_fields_match_the_baseline_table` 逐项钉住 §47.1
- [x] 四组守卫各做一次注入，确认确实会失败

## 5 登记与文档

- [x] `tests/registry.yaml` 的 H15／H17 追加 `notes`（机制已落地，验收仍 `pending`）
- [x] `docs/kernel.md` 模块清单补 harness 判定面与具名拒绝数
- [x] `docs/development-plan.md` 加 W66 条目
- [x] `AGENTS.md` 加 W66 轮次段
- [x] `docs/reports/2026-10-12-w66-harness-observability-and-degradation.md`

## 6 闭环

- [x] `cargo xtask check --profile full` 通过（EXIT=0；88 目标 / 785 passed / 0 failed）
- [x] 勾完 tasks 后 `openspec archive -y`
- [x] `openspec validate --all --strict` 与 `--all --strict --archived` 两条都跑
- [x] 一次 commit + push；单次查询核对 CI 结论（避开匿名限流）
