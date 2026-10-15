# Tasks: W113 — §24.4 远端留存清除：`RemoteGateway::delete` 传输契约判定面闭合

- [x] 1. 判据（先测）：`crates/nai-knowledge-core/tests/remote.rs` 新增 2（`remote_delete_rejects_malformed_document_ids`、`remote_delete_receipt_is_passed_through_and_never_claims_erasure`）+ 归属 1（`sync_and_delete_are_gated_single_attempt_and_return_receipts`）；本地跑绿
- [x] 2. spec：`knowledge-pipeline` 追加 §24.4 delete 传输契约 Requirement + 2 Scenario
- [x] 3. 文档：ADR 0091 + W113 报告 + `docs/development-plan.md` W113 段 + `docs/knowledge.md` §24.4 就地订正
- [x] 4. 全量门控 `cargo xtask check --profile full`（`RUST_TEST_THREADS=1` 缓解 composition 瞬态）

归档 → `openspec validate --archived` → 一次 commit + push（SSH 443）→ CI 双平台观测 → 回填报告：
按项目闭环纪律执行（流程项，非 checkbox）。
