## 0. 前置核实

- [x] 读 V3.9 §53（四项规则）、§5.7（段落与检查点）、§5.3（播报容量）、§5.4（守门）、§45.3（禁用域）
- [x] 核实 registry 中「讲品／段落编排／插问／节目／讲解／商品／卖点／售后」命中数为 0
- [x] 确认无既有 selling/pitch 实现；确认 `evaluate::show` 判的是单个 ShowPlan，维度不同
- [x] 确认 `scheduling-and-budgets` 的「冷却」是观众/任务维度，与段落维度不重叠

## 1. 判定面

- [x] 新建 `crates/nai-kernel/src/evaluate/selling.rs`
- [x] `SegmentKind` 四段 + `SEGMENT_ORDER` + `label()` + `parse()`
- [x] `Checkpoint`（含 `carried_text` 字段，使「带未播出文本」可被构造）
- [x] `Entrance` / `ActionCue` / `Claim` / `ClaimKind` / `FactRef` / `PitchPolicy` / `PitchPlan`
- [x] `PitchRejection` 十个具名拒绝 + `ALL` + `label()`
- [x] `evaluate_pitch`：四类判定各自报出，不短路
- [x] 在 `evaluate/mod.rs` 注册模块

## 2. 测试

- [x] `crates/nai-kernel/tests/selling.rs`（19 项）
- [x] 集合守卫：构造出的拒绝集合恰为 `PitchRejection::ALL`
- [x] 内容守卫：四段顺序与禁用域五类**逐项写死**（不是只数条数）
- [x] 顺序守卫：同时乱序与缺段时两类别都报出
- [x] 非空洞自检：五组注入各自被抓住（集合比较 / 只报首个缺失段 / 忽略未知段名 / 只问引用是否存在 / 区间规则无前置门控）

## 3. 登记与文档

- [x] `tests/registry.yaml`：新增 B07「基础讲品循环」（`r1`、`pending`）
- [x] `docs/kernel.md`：D10 模块清单加 `selling`，并新增 W68 段
- [x] `docs/development-plan.md`：W68 轮次条目 + 下一主线 + 更新日期订正
- [x] `AGENTS.md`：W68 轮次段
- [x] `docs/reports/2026-10-12-w68-basic-selling-loop.md`（含基线计数偏差登记）

## 4. 闭环

- [x] `cargo xtask check --profile full` 通过（EXIT=0；90 目标 / 817 passed / 0 failed）
- [x] `openspec validate w68-… --strict` 通过；需求正文最长 162 字符（< 500）
- [x] `openspec archive w68-… --yes`（核对归档目录日期戳）
- [x] 两条 validate 口径都跑（`--all --strict` / `--all --strict --archived`）
- [x] 一次 commit + push
- [x] 单次查询核对 CI 结论（避开匿名限流）
