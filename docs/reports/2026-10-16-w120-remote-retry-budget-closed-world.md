# W120 报告：post_json 重试预算与 read_body 读取上限闭世界守卫

日期：2026-10-16｜归属：W119 之后「新开 nai 守卫缺口」勘定候选（客户端内部**预算与边界**分支）｜零生产代码改动

## 一、裁定与缝（现测，不是推断）

W119 闭合了出站请求构造。继续勘定客户端内部预算与边界分支：`post_json`（`remote.rs:195-241`）与 `read_body`
（`remote.rs:133-135`）有三类分支**整段零判据**——既有 429 用例只用过 `Retry-After: 0`、且总在预算耗尽前就成功，
故这些分支一次都没被执行：

| 分支 | 位置 | 判据缺口 | 若被移除的后果 |
|---|---|---|---|
| `backoff >= op.remaining()` ⇒ `Unavailable` | `remote.rs:232-234` | `Retry-After: 0` ⇒ 退避 0s，永远不触发 ⇒ **零判据** | 硬等到 `op` 到期 ⇒ 结果从 `Unavailable` 变 **`Timeout`** |
| `attempt >= max_retries` ⇒ `Unavailable` | `remote.rs:220-223` | 既有重试用例耗尽前已成功 ⇒ **零判据** | 退避 0 时**无限重试**直至 `op` 到期 |
| 5xx 走指数退避、**忽略** `Retry-After` | `remote.rs:225-231` | 既有测试无法区分两者 ⇒ **零判据** | 5xx 用 `Retry-After: 100` 会立刻撞上一行 ⇒ **不再重试** |
| 响应体 > 8 MiB ⇒ `Invalid` | `remote.rs:133-135` | 从未被触碰 ⇒ **零判据** | 响应体**无上限缓冲** |

**裁定**：判定面闭合，零生产代码改动——只把上述分支变成显式守卫，与 W113–W119 同形。

## 二、判据落点（`crates/nai-knowledge-core/tests/remote.rs`）

**每条都用「判别性构造」**，使断言在分支被移除时**必然变红**（而非仅记录现状）。新增测试内辅助
`retrying_header(status, retry_after)`：

- **新增 1**：`remote_search_returns_unavailable_when_backoff_exceeds_remaining_budget` —— 429 +
  `Retry-After: 100` + `op = 1s` ⇒ `Unavailable` **且耗时 <900ms**。判别性：分支被移除则 `sleep_until` 等到
  `op` 到期 ⇒ `RetrievalTimeout` 且耗时 ~1s ⇒ 断言变红。
- **新增 2**：`remote_search_returns_unavailable_when_retry_budget_is_exhausted` —— 持续 429 +
  `max_retries = 1`（退避 0）⇒ `Unavailable`，**尝试次数恰为 2**。判别性：分支被移除则无限重试（退避 0）
  直至到期 ⇒ `Timeout` 而非 `Unavailable` ⇒ 断言变红。
- **新增 3**：`remote_search_retries_server_error_with_exponential_backoff_not_retry_after` —— 503 **故意带**
  `Retry-After: 100` + `op = 30s` ⇒ **仍重试并成功**（2 次尝试、`Complete`）。判别性：若 5xx 误用
  `Retry-After`，100s 退避撞上新增 1 ⇒ `Unavailable` 且仅 1 次尝试 ⇒ 断言变红。这是本轮最能体现判别性的一条。
- **新增 4**：`remote_search_rejects_response_body_beyond_the_read_cap` —— 回包 **8 MiB + 1**，经
  **合法字段** `evidence[0].text` 撑大 ⇒ `Err(RemoteError::Invalid)`。用合法回包很关键：畸形 JSON 也返回
  `Invalid`，那样就无法区分「读到大包被拒」与「解析失败」。判别性：上限被移除则正常解析 ⇒ 得 `Complete` ⇒ 变红。

另把**尝试次数**显式清点（新增 2、3 断言精确值）：只断言最终状态会漏掉次数维度。

## 三、计数（现测，非推断）

`crates/nai-knowledge-core/tests/remote.rs`：测试 **36 → 40**（+4 = 全为新增，W120 四条均未曾被任何测试覆盖；
W113–W119 的归属项已在各自报告标为既有）。本地复跑：`running 40 tests ... test result: ok. 40 passed; 0 failed`
（含全部 4 条 W120 新增；8 MiB 用例在内，全量耗时 1.33s）。门控权威总数见 §六。

## 四、如实披露（判定面闭合 ≠ 验收达标）

- 不断言真实网络下的退避时序（只钉「是否重试 / 何时放弃 / 放弃成什么」这类可判事实）。
- 不断言 `Retry-After` 的语义正确性（远端语义属远端责任；本轮只钉**本客户端选用哪个退避来源**）。
- 新增 3 依赖「5xx 与 429 退避来源不同」这一**既有设计**；若将来有意改为 5xx 也用 `Retry-After`，该判据会红，
  届时应**先改设计再改判据**，而不是反向。
- 介质级擦除、跨 memory↔remote 传播仍明确不做（与 W113–W119 一致）。
- registry：守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。

## 五、spec 增量（`knowledge-pipeline`）

新增 4 条 Requirement（各 1 Scenario），把这些预算/边界纳入基线：
- **Remote search returns unavailable when retry backoff exceeds remaining budget**（Backoff that does not fit the remaining budget yields unavailable at once）；
- **Remote search stops retrying after the retry budget is exhausted**（Retry budget exhaustion yields unavailable with a bounded attempt count）；
- **Remote search retries server errors with exponential backoff and ignores Retry-After**（A 5xx with a large Retry-After is still retried）；
- **Remote search rejects a response body beyond the read cap**（A response body over 8 MiB is rejected as invalid）。

## 六、门控（全量 `cargo xtask check --profile full`）

- `GATE_EXIT`：**0**（后台 `target-check/gate-w120.sh`，`PREBUILD_OK at attempt 1` + `GATE_OK at attempt 1`，修复后首跑即绿）。`rustc 1.96.0 (ac68faa20 2026-05-25)`。
- **首次门控 `GATE_EXIT=1`（真实根因 = registry 守卫 `no Rust source hides a second copy of the operation ceiling`，由本轮新增代码触发，非 LNK1104、非测试失败）**：新增 3、4 为「给足剩余预算」各写了
  `Operation::new(Duration::from_secs(30))`，而 30s 正是 `Operation::CEILING`（`nai-security/src/lib.rs:44`）；
  `tests/registry.test.mjs:1688` 的守卫禁止在 Rust 源码里出现 `Operation::new(Duration::from_secs(30))` 字面量——
  要取上界 MUST 引用 `Operation::CEILING`，不得留下「第二份、会漂移的副本」。改为
  `Operation::new(Operation::CEILING)`（语义完全相同）后：`node --test tests/registry.test.mjs` 由 51/52 转 **52/52**，门控重跑 `GATE_EXIT=0`。
  - **教训**：给 `Operation` 一个「很大的剩余预算」时 MUST 用 `Operation::CEILING`，**不要写 30s 字面量**；
    这条守卫是 W77「CEILING 公开 + 消费者改引」的下游，写测试同样受约束。
- **LNK1104 瞬态计数**：**0**（grep `LNK1104` = 0；`PREBUILD_OK attempt 1`）。本机单跑 `remote` 目标首跑曾遇一次
  LNK1104（`remote-3e1c9df997db1a40.exe` 被占用）⇒ 删除该 exe 后重跑即绿，**按纪律不改代码**。
- 全 workspace `cargo xtask check --profile full`：**95** 个测试二进制全 `test result: ok. ... 0 failed`，
  **0 FAILED / 0 error**；clippy `--workspace --all-targets --locked -- -D warnings` 通过；`cargo fmt --all --check`
  通过；前端门控（`[frontend] 全部步骤通过`）通过；`tests/registry.test.mjs` **52/52** 通过。权威测试 target 中
  `crates/nai-knowledge-core/tests/remote.rs` 运行 `running 40 tests ... 40 passed; 0 failed`（含全部 4 条 W120 新增，见 §三）。

## 七、提交 / 推送（归档 + 一次 commit + push，SSH 443）

- 提交 SHA：TBD（主提交，归档后回填）。
- push 区间：TBD，分支 `main`，SSH 443（`ssh.github.com:443` + `StrictHostKeyChecking=accept-new`）。
- 改动文件：`crates/nai-knowledge-core/tests/remote.rs`（+4 测试）、`docs/adr/0098-remote-retry-budget-closed-world.md`（新建）、`docs/reports/2026-10-16-w120-remote-retry-budget-closed-world.md`（新建）、`docs/development-plan.md`（W120 段）、`docs/knowledge.md`（订正注记）、`openspec/specs/knowledge-pipeline/spec.md`（合入 4 条 Requirement）、`openspec/changes/archive/2026-10-16-w120-remote-retry-budget-closed-world/`（归档目录）。
- openspec 三连：`validate w120-remote-retry-budget-closed-world --strict` → valid；`archive --yes` → 4 条 Requirement 合入基线；`validate --archived --strict` → 全绿（**118 项，含 w120**，0 failed）。归档目录按**系统日期戳**记为 `2026-10-16-...`。`target-check/` 按纪律排除暂存。

## 八、CI 观测（主提交 + 回填提交各一次双平台观测）

- **主提交** run id：TBD（workflow `Engineering foundation`）。回读 `head_sha` 与推送 SHA **完整 40 位一致**；双平台 `foundation` job 均 `completed / success`。若失败，按 W117/W119 的瞬态判据诊断（失败集合漂移 / 相关包 git status 空 / 本机全绿 / 另一平台 success），并经 check-run 注解取真实错误。
- **回填提交**（docs-only）自身 CI：按纪律做 **1 次**双平台观测（先例 run 109 failure ⇒ 不观测就是赌）。**本报告不为回填提交写 §九**；其自身结论按「**回填提交自身不回填**」记入 memory 日志，**不在本报告占位**（W119 起的写法，避免为填 run id 再叠加提交）。
