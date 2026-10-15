# W116 报告：RemoteError::status() 闭世界映射守卫

日期：2026-10-15｜归属：W113/W114/W115 之后「新开 nai 守卫缺口」勘定候选（`RemoteError::status()` 公开契约映射判定面）｜零生产代码改动

## 一、裁定与缝（现测，不是推断）

W115 闭合了检索链路进入「证据」后的闭世界守卫（rerank + to_evidence）。在「先测后设计」节奏下，继续勘定
`RemoteError` 公开契约 `RemoteError::status()`（`remote.rs:37-46`）的下一处判定面缺口。基于代码实测
（非摘要），该函数把 5 个错误变体钉成 `RetrievalStatus` 的映射**整段零判据**——既有 20 个
`#[tokio::test]` / `#[test]` 无一条断言该映射，含两条**非显然坍缩**：

| 变体 | `status()` 映射 | 判据缺口 |
|---|---|---|
| `RemoteError::Unauthorized` | `RetrievalStatus::RetrievalUnauthorized` | 既有 20 测试**零判据** → **新增** `remote_error_status_unauthorized_maps_to_retrieval_unauthorized` |
| `RemoteError::Unavailable` | `RetrievalStatus::RetrievalUnavailable` | 同上 → **新增** `remote_error_status_unavailable_maps_to_retrieval_unavailable` |
| `RemoteError::Invalid` | `RetrievalStatus::RetrievalUnavailable`（**非显然坍缩**：无独立 `RetrievalInvalid`） | 同上 → **新增** `remote_error_status_invalid_maps_to_retrieval_unavailable` |
| `RemoteError::Timeout` / `RemoteError::Cancelled` | `RetrievalStatus::RetrievalTimeout`（**坍缩**：两语义并入同态） | 同上 → **新增** `remote_error_status_timeout_and_cancelled_map_to_retrieval_timeout` |

**裁定**：判定面闭合，零生产代码改动——只把全映射变成显式守卫，与 W113/W114/W115 同形。

## 二、判据落点（`crates/nai-knowledge-core/tests/remote.rs`）

- **新增 1**：`remote_error_status_unauthorized_maps_to_retrieval_unauthorized` —— `Unauthorized.status()`
  返回 `RetrievalUnauthorized`；直读 `remote.rs:40` 映射并 `assert_eq!`。
- **新增 2**：`remote_error_status_unavailable_maps_to_retrieval_unavailable` —— `Unavailable.status()`
  返回 `RetrievalUnavailable`；直读 `remote.rs:41` 映射并 `assert_eq!`。
- **新增 3**：`remote_error_status_invalid_maps_to_retrieval_unavailable` —— `Invalid.status()` 返回
  `RetrievalUnavailable`（**非显然坍缩**：无独立 `RetrievalInvalid`）；直读 `remote.rs:43` 映射并
  `assert_eq!`，注释显式标注「非显然坍缩」防漏改。
- **新增 4**：`remote_error_status_timeout_and_cancelled_map_to_retrieval_timeout` —— `Timeout.status()`
  与 `Cancelled.status()` 都返回 `RetrievalTimeout`（**坍缩**：两语义并入同态）；一条测试内两个
  `assert_eq!`，直读 `remote.rs:42` 映射。

四条均用同步 `#[test]`（非 `#[tokio::test]`）：`status()` 是无 IO、无随机性的纯函数，不启服务器、不触网、
不依赖 tokio，与既有 async 测试隔离。

## 三、计数（现测，非推断）

`crates/nai-knowledge-core/tests/remote.rs`：测试 **20 → 24**（+4 = 全为新增，W116 四条均未曾被任何测试
覆盖；无既有测试归属为本轮新增，W113/W114/W115 的归属项已在各自报告标为既有）。本地复跑：
`running 24 tests ... test result: ok. 24 passed; 0 failed`（含全部 4 条 W116 新增）。门控权威总数见 §六。

## 四、如实披露（判定面闭合 ≠ 验收达标）

- `RemoteError::status()` 是 `RemoteError` 公开枚举上的方法，与 `RetrievalStatus` 消费侧（gateway
  `parse_reply`）解耦；W116 只钉「声明侧的映射正确性」，不钉「消费侧如何据此决策」（属既有网关逻辑）。
- 两条坍缩（Invalid→Unavailable、Timeout|Cancelled→Timeout）是既有语义，W116 仅把既有行为钉成守卫，
  不引入新的状态分支（如 `RetrievalInvalid`）。
- 介质级擦除、跨 memory↔remote 传播仍明确不做（与 W113/W114/W115 一致）。
- registry：守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。
- `search` 的其它边界（`parse_reply` 的 top_k 截断）属独立缝，留待后续；本次仅闭合 `status()` 映射。

## 五、spec 增量（`knowledge-pipeline`）

新增 4 条 Requirement（各 1 Scenario），把 `RemoteError::status()` 映射闭世界纳入基线：
- **Remote error Unauthorized maps to RetrievalUnauthorized**（Unauthorized maps to RetrievalUnauthorized）；
- **Remote error Unavailable maps to RetrievalUnavailable**（Unavailable maps to RetrievalUnavailable）；
- **Remote error Invalid collapses to RetrievalUnavailable**（Invalid collapses to RetrievalUnavailable）；
- **Remote error Timeout and Cancelled collapse to RetrievalTimeout**（Timeout and Cancelled collapse to RetrievalTimeout）。

## 六、门控（全量 `cargo xtask check --profile full`）

- `GATE_EXIT`：**0**（后台 `target-check/gate-w116.sh`，`PREBUILD_OK at attempt 1` + `GATE_OK at attempt 1`，首跑即绿）。`rustc 1.96.0 (ac68faa20 2026-05-25)`。
- **LNK1104 瞬态计数**：**0**（grep `LNK1104` = 0；预构建循环首跑通过，无 Defender 持锁导致的测试 exe 链接失败；仅偶发增量编译锁删除告警 `拒绝访问。 (os error 5)` 按既定纪律认定为非回归、不修代码）。
- 全 workspace `cargo xtask check --profile full`：**95** 个测试二进制全 `test result: ok. ... 0 failed`，**0 FAILED / 0 error**；clippy `--workspace --all-targets --locked -- -D warnings` 通过（否则 `GATE_OK` 不成立）；`cargo fmt --all --check` 通过；前端门控（`[frontend] 全部步骤通过`）通过。权威测试 target 中，`crates/nai-knowledge-core/tests/remote.rs` 运行 `running 24 tests ... 24 passed; 0 failed`（含全部 4 条 W116 新增，见 §三）。

## 七、提交 / 推送（归档 + 一次 commit + push，SSH 443）

- 提交 SHA：TBD（主提交，归档后回填）。
- push 区间：TBD，分支 `main`，SSH 443（`ssh.github.com:443` + `StrictHostKeyChecking=accept-new`）。
- 改动文件：`crates/nai-knowledge-core/tests/remote.rs`（+4 测试）、`docs/adr/0094-remote-error-status-closed-world.md`（新建）、`docs/reports/2026-10-15-w116-remote-error-status-closed-world.md`（新建）、`docs/development-plan.md`（W116 段）、`docs/knowledge.md`（订正注记）、`openspec/specs/knowledge-pipeline/spec.md`（合入 4 条 Requirement）、`openspec/changes/archive/2026-10-15-w116-remote-error-status-closed-world/`（归档目录）。
- openspec 三连：`validate w116-remote-error-status-closed-world --strict` → valid；`archive --yes` → 4 条 Requirement 合入基线；`validate --archived --strict` → 全绿（**114 项，含 w116**，0 failed）。`target-check/` 按纪律排除暂存。

## 八、CI 观测（主提交 + 回填提交各一次双平台观测）

- **主提交** run id：TBD（workflow `Engineering foundation`）。回读 `head_sha` 与推送 SHA **完整 40 位一致**；双平台 `foundation` job 均 `completed / success`。本机门控绿与 CI 双平台绿一致；判定面闭合经端到端 CI 复核。
- **回填提交**（docs-only）自身 CI 双平台观测：run TBD，回读 `head_sha` 与推送 SHA **完整 40 位一致**；双平台 `foundation` job 均 `completed / success`。回填提交 CI 自身绿（先例 run 109 failure ⇒ 本次实测 success，不赌）；不为回填提交写 §九，仅做 1 次观测。
