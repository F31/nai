# Tasks: W118 — parse_reply 的 RemoteStatus → RetrievalStatus 闭世界映射守卫

- [x] 1. 判据（先测）：`crates/nai-knowledge-core/tests/remote.rs` 新增 4（`search_maps_partial_remote_status_to_retrieval_partial`、`search_maps_timeout_remote_status_to_retrieval_timeout`、`search_maps_unavailable_remote_status_to_retrieval_unavailable`、`search_maps_unauthorized_remote_status_to_retrieval_unauthorized`）；本地跑绿（32 passed; 0 failed）
- [x] 2. spec：`knowledge-pipeline` 追加 4 条 Requirement（各 1 Scenario）
- [x] 3. 文档：ADR 0096 + W118 报告 + `docs/development-plan.md` W118 段 + `docs/knowledge.md` W118 注记
- [x] 4. 全量门控 `cargo xtask check --profile full`（`RUST_TEST_THREADS=1` 缓解 link 瞬态）

归档 → `openspec validate --archived` → 一次 commit + push（SSH 443）→ CI 双平台观测 → 回填报告：
按项目闭环纪律执行（流程项，非 checkbox）。
