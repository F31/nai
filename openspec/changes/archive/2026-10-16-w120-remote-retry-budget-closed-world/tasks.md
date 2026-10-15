# Tasks: W120 — post_json 重试预算与 read_body 读取上限闭世界守卫

- [x] 1. 判据（先测）：`crates/nai-knowledge-core/tests/remote.rs` 新增 4（`remote_search_returns_unavailable_when_backoff_exceeds_remaining_budget`、`remote_search_returns_unavailable_when_retry_budget_is_exhausted`、`remote_search_retries_server_error_with_exponential_backoff_not_retry_after`、`remote_search_rejects_response_body_beyond_the_read_cap`），每条都用判别性构造；本地跑绿（40 passed; 0 failed）
- [x] 2. spec：`knowledge-pipeline` 追加 4 条 Requirement（各 1 Scenario）
- [x] 3. 文档：ADR 0098 + W120 报告 + `docs/development-plan.md` W120 段 + `docs/knowledge.md` W120 注记
- [x] 4. 全量门控 `cargo xtask check --profile full`（`RUST_TEST_THREADS=1` 缓解 link 瞬态）

归档 → `openspec validate --archived` → 一次 commit + push（SSH 443）→ CI 双平台观测 → 回填报告：
按项目闭环纪律执行（流程项，非 checkbox）。
