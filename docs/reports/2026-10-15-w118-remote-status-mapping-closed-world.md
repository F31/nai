# W118 报告：parse_reply 的 RemoteStatus → RetrievalStatus 闭世界映射守卫

日期：2026-10-15｜归属：W117 之后「新开 nai 守卫缺口」勘定候选（`parse_reply` body status 映射判定面）｜零生产代码改动

## 一、裁定与缝（现测，不是推断）

W117 闭合了 `parse_reply`（`remote.rs:395-411`）的 `top_k` 截断。该函数还有第二处判定面：其
`RemoteStatus → RetrievalStatus` 的 6 臂映射（`remote.rs:396-403`）**大部分零判据**。基于代码实测（非摘要），
既有测试经 `parse_reply` 只覆盖 `Complete` / `Empty` 两臂（200 body status 只出现过 `"complete"` / `"empty"`）：

| `RemoteStatus`（body status） | `parse_reply` 映射 | 经 `parse_reply` 的判据 |
|---|---|---|
| `Complete` | `RetrievalComplete` | ✓ 既有（200 body `"complete"`） |
| `Empty` | `RetrievalEmpty` | ✓ 既有（200 body `"empty"`） |
| `Partial` | `RetrievalPartial` | **零** → **新增** |
| `Timeout` | `RetrievalTimeout` | **零** → **新增** |
| `Unavailable` | `RetrievalUnavailable` | **零** → **新增** |
| `Unauthorized` | `RetrievalUnauthorized` | **零** → **新增** |

**关键陷阱（缝的成因）**：既有断言里确有 `RetrievalTimeout` / `RetrievalUnavailable` / `RetrievalUnauthorized`，
但它们**全部来自 `search` 的 HTTP 内联错误分支**（`RemoteError → (status, [])`，`remote.rs:317-327`），
**不是** `parse_reply` 的 body status 映射。两条路径彼此独立 ⇒ 把 `parse_reply` 的映射换错**完全不可见**。

**裁定**：判定面闭合，零生产代码改动——只把该映射逐臂变成显式守卫，与 W113–W117 同形。

## 二、判据落点（`crates/nai-knowledge-core/tests/remote.rs`）

四条均经公共 `gateway.search` + Mock：Mock 返回 **HTTP 200** + `{"status": "<x>", "evidence": []}`，走
`client.search`（200 ⇒ **不走**内联错误分支）→ `parse_reply`，从而**隔离** body status 映射。测试名显式含
`remote_status` 固化「body status，非 HTTP 状态」语义。新增测试内辅助 `status_mock(status)` / `gateway_search_status(remote_status)`：

- **新增 1**：`search_maps_partial_remote_status_to_retrieval_partial`（body `"partial"` ⇒ `RetrievalPartial`）；
- **新增 2**：`search_maps_timeout_remote_status_to_retrieval_timeout`（body `"timeout"` ⇒ `RetrievalTimeout`）；
- **新增 3**：`search_maps_unavailable_remote_status_to_retrieval_unavailable`（body `"unavailable"` ⇒ `RetrievalUnavailable`）；
- **新增 4**：`search_maps_unauthorized_remote_status_to_retrieval_unauthorized`（body `"unauthorized"` ⇒ `RetrievalUnauthorized`）。

## 三、计数（现测，非推断）

`crates/nai-knowledge-core/tests/remote.rs`：测试 **28 → 32**（+4 = 全为新增，W118 四条均未曾被任何测试覆盖；
W113–W117 的归属项已在各自报告标为既有）。本地复跑：`running 32 tests ... test result: ok. 32 passed; 0 failed`
（含全部 4 条 W118 新增）。门控权威总数见 §六。

## 四、如实披露（判定面闭合 ≠ 验收达标）

- `parse_reply` 保持 `private`，仅经公共 `gateway.search` 覆盖；不提升为 `pub`。
- `gateway.answer` 亦调用 `parse_reply`（remote.rs:624），与 `search` 共用同一函数、语义一致；本轮经 `search`
  路径覆盖，不为 `answer` 另写独立断言（避免重复）。
- 本轮不改 `remote.rs`：不合并 `search` 内联分支与 `parse_reply` 映射——二者刻意分离（前者 HTTP 层错误、
  后者 body 层状态）；本轮的价值恰在显式区分两条路径。
- 介质级擦除、跨 memory↔remote 传播仍明确不做（与 W113–W117 一致）。
- registry：守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。

## 五、spec 增量（`knowledge-pipeline`）

新增 4 条 Requirement（各 1 Scenario），把 `parse_reply` 的 body status 映射闭世界纳入基线：
- **Remote search maps partial body status to RetrievalPartial**（partial body status maps to RetrievalPartial）；
- **Remote search maps timeout body status to RetrievalTimeout**（timeout body status maps to RetrievalTimeout）；
- **Remote search maps unavailable body status to RetrievalUnavailable**（unavailable body status maps to RetrievalUnavailable）；
- **Remote search maps unauthorized body status to RetrievalUnauthorized**（unauthorized body status maps to RetrievalUnauthorized）。

## 六、门控（全量 `cargo xtask check --profile full`）

- `GATE_EXIT`：**0**（后台 `target-check/gate-w118.sh`，`PREBUILD_OK at attempt 1` + `GATE_OK at attempt 1`，首跑即绿）。`rustc 1.96.0 (ac68faa20 2026-05-25)`。本轮**先跑 `cargo fmt --all`**（吸取 W117 教训），门控首跑即绿、无 fmt 失败。
- **LNK1104 瞬态计数**：**0**（grep `LNK1104` = 0；`PREBUILD_OK attempt 1`，无 Defender 持锁的测试 exe 链接失败；仅偶发增量编译锁删除告警 `拒绝访问。 (os error 5)` 按既定纪律认定为非回归、不修代码）。
- **门控脚本改进**（吸取 W117 教训）：`gate-w118.sh` 把门控阶段的 LNK1104 检测/清理**限定到本次尝试的独立日志**（`w118-gate-phase.log`），使**预构建阶段的陈旧 LNK1104 行不会把真实失败误标成「LNK1104 retrying」**（W117 曾因此空转到 8 次上限）。
- 全 workspace `cargo xtask check --profile full`：**95** 个测试二进制全 `test result: ok. ... 0 failed`，**0 FAILED / 0 error**；clippy `--workspace --all-targets --locked -- -D warnings` 通过（否则 `GATE_OK` 不成立）；`cargo fmt --all --check` 通过；前端门控（`[frontend] 全部步骤通过`）通过。权威测试 target 中 `crates/nai-knowledge-core/tests/remote.rs` 运行 `running 32 tests ... 32 passed; 0 failed`（含全部 4 条 W118 新增，见 §三）。

## 七、提交 / 推送（归档 + 一次 commit + push，SSH 443）

- 提交 SHA：TBD（主提交，归档后回填）。
- push 区间：TBD，分支 `main`，SSH 443（`ssh.github.com:443` + `StrictHostKeyChecking=accept-new`）。
- 改动文件：`crates/nai-knowledge-core/tests/remote.rs`（+4 测试）、`docs/adr/0096-remote-status-mapping-closed-world.md`（新建）、`docs/reports/2026-10-15-w118-remote-status-mapping-closed-world.md`（新建）、`docs/development-plan.md`（W118 段）、`docs/knowledge.md`（订正注记）、`openspec/specs/knowledge-pipeline/spec.md`（合入 4 条 Requirement）、`openspec/changes/archive/2026-10-15-w118-remote-status-mapping-closed-world/`（归档目录）。
- openspec 三连：`validate w118-remote-status-mapping-closed-world --strict` → valid；`archive --yes` → 4 条 Requirement 合入基线；`validate --archived --strict` → 全绿（**116 项，含 w118**，0 failed）。`target-check/` 按纪律排除暂存。

## 八、CI 观测（主提交 + 回填提交各一次双平台观测）

- **主提交** run id：TBD（workflow `Engineering foundation`）。回读 `head_sha` 与推送 SHA **完整 40 位一致**；双平台 `foundation` job 均 `completed / success`。若 windows 失败，按 W117 的四条瞬态判据诊断（失败集合漂移 / 相关包 git status 空 / 本机全绿 / 另一平台 success），并经 check-run 注解取真实错误。
- **回填提交**（docs-only）自身 CI 双平台观测：run TBD，回读 `head_sha` 与推送 SHA **完整 40 位一致**；双平台 `foundation` job 均 `completed / success`。回填提交 CI 自身绿（先例 run 109 failure ⇒ 本次实测 success，不赌）；不为回填提交写 §九，仅做 1 次观测。
