## 0. 前置核实

- [x] 读 V3.9 §41.1–§41.5（九段顺序、不可裁段、分段预算、片段标注、可判定性）、§47.1（L1 装配的记录项）、§47.4（装配失败落点）
- [x] 确认 `crates/nai-kernel/src/evaluate/` **没有**装配判定（`harness.rs` 里的 `Assembly` 全是层名与字段名）
- [x] 确认 `agent-harness` 已含 H01/H02 两条需求 ⇒ 本轮是 `MODIFIED`，**不是** `ADDED`、**不是**新建能力
- [x] 确认 `H01`/`H02` 的 registry `notes` 仍写「未开始（W56）」；确认 §47.6 的 W56 行目标正是上下文装配器
- [x] 确认 `evaluate::show` / `evaluate::selling` / `evaluate::harness` 判的维度均不同，无重复落点

## 1. 判定面（H01）

- [x] 新建 `crates/nai-kernel/src/evaluate/assembly.rs`
- [x] `Segment` 九段 + `SEGMENT_ORDER` + `label()` + `index()` + `uncroppable()`
- [x] `SourceKind` 七类 + `SOURCE_KINDS` + `must_be_data()`（恰为基线点名的四类）
- [x] `Trust` / `Fragment` / `Crop` / `SegmentUsage` / `SegmentBudget` / `SegmentSlice` / `AssemblyPlan` / `AssemblyPolicy`
- [x] `AssemblyRejection` 十四条具名拒绝 + `ALL` + `label()`
- [x] `evaluate_assembly`：六步顺序判定（段名 → 顺序 → 不可裁齐备 → 不可裁未被裁 → 裁剪记录 → 裁剪落点）+ 预算 + 标注，各自报出不短路
- [x] `callable` 仅在零拒绝时为真；**不**复用 `harness::FailureLayer` 的降级计算

## 2. 判定面（H02）

- [x] `AssemblyRecord` / `LoggedFragment` / `RebuildVerdict` / `RebuildRejection`（六条）
- [x] `rebuild_assembly`：只消费日志，按**日志自身顺序**排出被引用片段并与记录比较
- [x] `FragmentNotLogged` 与 `FragmentContentNotInLog` **分开**
- [x] INV-11 复用 `harness::CREDENTIAL_MARKERS`（不建第二张标记表）
- [x] `AssemblyRecord::logged_field_names()` 与 `ObservationLayer::Assembly::required_fields()` 对齐
- [x] 在 `evaluate/mod.rs` 注册模块

## 3. 测试

- [x] `crates/nai-kernel/tests/assembly.rs`
- [x] 两条「构造出的拒绝集合恰为 `ALL`」总账断言
- [x] 内容守卫：九段顺序、七类来源、不可裁序号、四类「一律为数据」**逐项写死**（不是只数条数）
- [x] 顺序守卫：同时乱序与缺段时两类别都报出
- [x] 正向前置：合规装配零拒绝且 `callable` 为真（否则「全都拒」也能让测试变绿）
- [x] 跨模块守卫：装配记录字段与 `harness` 的 L1 必需字段表逐项相等
- [x] 非空洞自检：注入样本各自被抓住（集合比较 / 只报首个缺失段 / 忽略未知段名 / 只判「有裁剪记录」/ 把四条拒绝合成一条 / 按记录顺序排序再比较）

## 4. 登记与文档

- [x] `tests/registry.yaml`：改 `H01`/`H02` 的 `notes`（判定机制已交付、`status` 保持 `pending` 并给理由）
- [x] `docs/kernel.md`：D10 模块清单加 `assembly`，并新增 W69 段
- [x] `docs/development-plan.md`：W69 轮次条目 + 下一主线 + 更新日期
- [x] `AGENTS.md`：W69 轮次段
- [x] `docs/reports/2026-10-12-w69-context-assembly.md`
- [x] 订正工作记忆里「R1 七条已全部有判定面」的不准确陈述

## 5. 闭环

- [x] `cargo xtask check --profile full` 通过（记录目标数与通过项数，与上一轮对照）
- [x] `openspec validate w69-context-assembly --strict` 通过（需求正文 < 500 字符）
- [x] 勾选本文件全部任务
- [x] `openspec archive w69-context-assembly --yes`（核对归档目录日期戳）
- [x] 两条 validate 口径都跑（`--all --strict` / `--all --strict --archived`）
- [x] 一次 commit + push
- [x] 单次低频查询核对 CI 结论（避开匿名限流）并回填报告
