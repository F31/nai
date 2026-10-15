# Tasks: W119 — 远端出站请求构造闭世界守卫（凭证头 + endpoint 规范化）

- [x] 1. 判据（先测）：`crates/nai-knowledge-core/tests/remote.rs` 新增 4（`remote_request_sends_bearer_authorization_header_when_api_key_present`、`remote_request_omits_authorization_header_when_api_key_absent`、`remote_request_normalizes_trailing_slash_in_endpoint`、`remote_request_normalizes_repeated_trailing_slashes_in_endpoint`），用观测型 Mock 记录 url 与 Authorization 头；本地跑绿（36 passed; 0 failed）
- [x] 2. spec：`knowledge-pipeline` 追加 4 条 Requirement（各 1 Scenario）
- [x] 3. 文档：ADR 0097 + W119 报告 + `docs/development-plan.md` W119 段 + `docs/knowledge.md` W119 注记
- [x] 4. 全量门控 `cargo xtask check --profile full`（`RUST_TEST_THREADS=1` 缓解 link 瞬态）

归档 → `openspec validate --archived` → 一次 commit + push（SSH 443）→ CI 双平台观测 → 回填报告：
按项目闭环纪律执行（流程项，非 checkbox）。
