# Proposal: W120 — post_json 重试预算与 read_body 读取上限闭世界守卫

## Why
W119 闭合了出站请求构造。继续勘定客户端内部的**预算与边界**分支：`post_json`
（`remote.rs:195-241`）与 `read_body`（`remote.rs:133-135`）有三类分支**整段零判据**——既有 429 用例只用过
`Retry-After: 0`、且总在预算耗尽前就成功，故下列分支一次都没被执行：

- **(a) 退避装不进剩余预算**：`if backoff >= op.remaining() { return Err(Unavailable) }`（`remote.rs:232-234`）。
  既有 `Retry-After: 0` ⇒ 退避 0s，永远不触发。若该分支被移除，会 `sleep_until` 硬等到 `op` 到期 ⇒
  结果从 `Unavailable` 变成 **`Timeout`**，调用方拿到错误的「超时」语义（实际是预算不足）。
- **(b) 重试预算耗尽**：`attempt >= self.max_retries` ⇒ `Unavailable`（`remote.rs:220-223`）。既有重试用例在
  耗尽前已成功，故从未命中。若被移除 ⇒ 退避为 0 时**无限重试**直至 `op` 到期。
- **(c) 5xx 忽略 `Retry-After`**：退避分支 `if status == 429 { Duration::from_secs(retry_after) } else { 200ms × 2^attempt }`
  （`remote.rs:225-231`）——即 5xx 走**指数退避**、429 才用 `Retry-After`。既有测试无法区分两者。若 5xx 误用
  `Retry-After`，远端返回的 `Retry-After: 100` 会立刻撞上 (a) ⇒ 直接 `Unavailable` 而**不再重试**。
- **(d) 有界读取**：`read_body` 的 8 MiB 上限（`remote.rs:133-135`）⇒ 超限 `Invalid`。从未被触碰；若被移除
  ⇒ 响应体无上限缓冲。

此缝与 W113–W119 同形：判定面闭合、零生产改动、确定性离线。

## What Changes
只补可测判据，把上述分支钉成显式闭世界守卫；**零生产代码改动**。

- `crates/nai-knowledge-core/tests/remote.rs` 新增 4 项（W120，已本地跑绿）：
  - `remote_search_returns_unavailable_when_backoff_exceeds_remaining_budget`（429 + `Retry-After: 100`
    + 1s `op` ⇒ `Unavailable` 且**立即返回**，不等到超时）；
  - `remote_search_returns_unavailable_when_retry_budget_is_exhausted`（持续 429 + `max_retries=1`
    ⇒ `Unavailable`，尝试次数恰为 **2**）；
  - `remote_search_retries_server_error_with_exponential_backoff_not_retry_after`（503 带 `Retry-After: 100`
    ⇒ 仍重试并成功，尝试次数 2 ⇒ 证明 5xx 用指数退避、忽略 `Retry-After`）；
  - `remote_search_rejects_response_body_beyond_the_read_cap`（>8 MiB 回包 ⇒ `Err(RemoteError::Invalid)`）。
- `knowledge-pipeline` spec 新增 4 条 Requirement（各 1 Scenario），将这些预算/边界纳入基线。

## Modified Capabilities
- `knowledge-pipeline`（远端知识契约与撤销）：追加 `post_json` 重试预算（退避预算、重试次数上界、5xx 退避来源）与 `read_body` 8 MiB 读取上限闭世界守卫。

## Impact
- 无运行时行为变化（判定面闭合，不引入新执行体、不改 `remote.rs`）。
- 无契约字段改动、无迁移、无 schema 改动。
- registry：守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。
- 测试计数：`tests/remote.rs` 的测试由 36 增至 40（W120 +4）。
- spec：新增 4 条 Requirement（各 1 Scenario）。
