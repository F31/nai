# Tasks: W133 — H01/H02 的缺失生产者

## 1. 勘定

- [x] 1.1 读 `tests/registry.yaml` 的 H01–H04 原文与 notes，逐项核实而非照信
- [x] 1.2 现测「`AssemblyPlan` 在生产代码里零构造者」：`grep -rn AssemblyPlan` 排除 tests ⇒ 只有定义与消费两处
- [x] 1.3 现测 H03/H04 的控制面链已由 `apps/engine/src/lib.rs:327` 注入 `MemoryAdapter` 接通，缺的是模型侧候选产生者
- [x] 1.4 裁定分组：H01/H02 与本轮可交付的「纯计算构造者」同型，H03/H04 不同型 ⇒ 本轮不纳入并具名登记

## 2. 先写会红的判据

- [x] 2.1 `crates/nai-kernel/tests/assemble.rs`：10 条（顺序 / 缺席 / 闭世界九段 / 尾部裁剪 / 无上限不裁 / 不可裁段 / 存活占用 / 未定义段名 / 预算不代编 / 正向不变量）
- [x] 2.2 `crates/nai-runtime/tests/assembly.rs`：+2 条（候选→装配→落链→读回→重建；被拒的计划零落盘）
- [x] 2.3 首跑确认真红：`error[E0432]: unresolved import nai_kernel::assemble`

## 3. 实现

- [x] 3.1 `crates/nai-kernel/src/assemble.rs`：`plan_assembly` + `PlannedAssembly::with_plan`（借用视图，避免自引用）
- [x] 3.2 复用 `SEGMENT_ORDER` / `Segment::uncroppable` / `Segment::parse`，零业务常量
- [x] 3.3 `lib.rs` 加 `pub mod assemble;`
- [x] 3.4 判定面与执行体**零改动**（`evaluate/assembly.rs`、`runtime/src/assembly.rs` 未动）

## 4. 正向半与登记

- [x] 4.1 kernel `--test assemble` 10/10、`--test assembly` 既有 40/40
- [x] 4.2 runtime `--test assembly` 29/29（27 + 2）
- [x] 4.3 `cargo fmt --all --check` 干净；`cargo clippy -p nai-kernel -p nai-runtime --all-targets -- -D warnings` 无告警
- [x] 4.4 接线登记核对：`engineering_checks` 只登记 Node 测试；新增 Rust 测试目标由 `cargo test --workspace` 自动覆盖 ⇒ 无需登记

## 5. 非空洞注入

- [x] 5.1 基线取自 fmt-clean 的最终字节（3 个文件 sha256）
- [x] 5.2 九组注入全部真红：K1 去排序（3 红）/ K2 补空段（2）/ K3 头部裁剪（1）/ K4 裁不可裁段（1）/ K5 占用报全量（1）/ K6 静默跳过（1）/ K7 代编预算（1）/ K8 不记原因（1）/ K9 按名字排序（4）
- [x] 5.3 三文件逐字节还原，复跑 10 + 29 全绿
- [x] 5.4 如实登记：K3 只有 1 红，`usage_counts_only_what_survived` 对裁剪位置不敏感（它算数量与总和，不比身份）

## 6. 收口

- [x] 6.1 openspec：proposal + spec 差量（ADDED 六条）+ tasks（例行轮次不写 design.md）
- [x] 6.2 registry `H01`/`H02` notes 就地订正，`status` 由证据决定（**仍 `pending`**）
- [x] 6.3 文档：轮次报告（五节）+ `docs/development-plan.md` 三行 + `docs/knowledge.md` 一行
- [x] 6.4 全量门控 `cargo xtask check --profile full` ⇒ 绿
- [x] 6.5 归档 change + 两条 validate
- [x] 6.6 一次 commit + push + CI 观测（触发确认 1 次 + 完成态 ≥2 次，不轮询）
