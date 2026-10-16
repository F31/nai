# Tasks: W121 — Operation 有界预算的运行期终止语义闭世界守卫

- [x] 1. 判据（先测）：`crates/nai-security/src/lib.rs` 的 tests 模块新增 4（`check_reports_cancellation_before_the_deadline`、`remaining_never_grows_as_time_passes`、`stopped_is_exactly_check_failure`、`cancellation_is_shared_across_cloned_handles`），覆盖运行期终止语义；本地跑绿（9 passed; 0 failed）
- [x] 2. spec：`runtime-composition` 追加 4 条 Requirement（各 1 Scenario）
- [x] 3. 文档：ADR 0099 + W121 报告 + `docs/development-plan.md` W121 段 + `docs/knowledge.md` W121 注记
- [x] 4. 全量门控 `cargo xtask check --profile full`（`RUST_TEST_THREADS=1` 缓解 link 瞬态）

归档 → `openspec validate --archived` → 一次 commit + push（SSH 443）→ CI 双平台观测 → 回填报告：
按项目闭环纪律执行（流程项，非 checkbox）。
