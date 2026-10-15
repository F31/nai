# Tasks: W114 — 远端网关构造期与输入边界闭世界守卫

- [x] 1. 判据（先测）：`crates/nai-knowledge-core/tests/remote.rs` 新增 4（`remote_gateway_new_rejects_invalid_policy`、`remote_knowledge_client_new_rejects_invalid_args`、`remote_sync_rejects_malformed_documents_without_network`、`remote_capabilities_rejects_when_first_allowed_op_not_search_or_answer`）；本地跑绿
- [x] 2. spec：`knowledge-pipeline` 追加 4 条 Requirement（各 1 Scenario）
- [x] 3. 文档：ADR 0092 + W114 报告 + `docs/development-plan.md` W114 段 + `docs/knowledge.md` §25.3 就地订正
- [x] 4. 全量门控 `cargo xtask check --profile full`（`RUST_TEST_THREADS=1` 缓解 composition 瞬态）

归档 → `openspec validate --archived` → 一次 commit + push（SSH 443）→ CI 双平台观测 → 回填报告：
按项目闭环纪律执行（流程项，非 checkbox）。
