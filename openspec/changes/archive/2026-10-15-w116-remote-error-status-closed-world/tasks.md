# Tasks: W116 — RemoteError::status() 闭世界映射守卫

- [x] 1. 判据（先测）：`crates/nai-knowledge-core/tests/remote.rs` 新增 4（`remote_error_status_unauthorized_maps_to_retrieval_unauthorized`、`remote_error_status_unavailable_maps_to_retrieval_unavailable`、`remote_error_status_invalid_maps_to_retrieval_unavailable`、`remote_error_status_timeout_and_cancelled_map_to_retrieval_timeout`）；本地跑绿（24 passed; 0 failed）
- [x] 2. spec：`knowledge-pipeline` 追加 4 条 Requirement（各 1 Scenario）
- [x] 3. 文档：ADR 0094 + W116 报告 + `docs/development-plan.md` W116 段 + `docs/knowledge.md` §25.x 就地订正
- [x] 4. 全量门控 `cargo xtask check --profile full`（`RUST_TEST_THREADS=1` 缓解 link 瞬态）

归档 → `openspec validate --archived` → 一次 commit + push（SSH 443）→ CI 双平台观测 → 回填报告：
按项目闭环纪律执行（流程项，非 checkbox）。
