# Tasks: W117 — 检索结果 top_k 截断闭世界守卫

- [x] 1. 判据（先测）：`crates/nai-knowledge-core/tests/remote.rs` 新增 4（`search_truncates_evidence_to_top_k_in_order`、`search_applies_top_k_after_dropping_empty_ids`、`search_top_k_zero_yields_empty_evidence`、`search_top_k_exceeding_reply_keeps_all_evidence`）；本地跑绿（28 passed; 0 failed）
- [x] 2. spec：`knowledge-pipeline` 追加 4 条 Requirement（各 1 Scenario）
- [x] 3. 文档：ADR 0095 + W117 报告 + `docs/development-plan.md` W117 段 + `docs/knowledge.md` W117 注记
- [x] 4. 全量门控 `cargo xtask check --profile full`（`RUST_TEST_THREADS=1` 缓解 link 瞬态）

归档 → `openspec validate --archived` → 一次 commit + push（SSH 443）→ CI 双平台观测 → 回填报告：
按项目闭环纪律执行（流程项，非 checkbox）。
