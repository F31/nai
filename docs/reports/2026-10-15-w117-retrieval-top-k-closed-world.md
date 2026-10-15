# W117 报告：检索结果 top_k 截断闭世界守卫

日期：2026-10-15｜归属：兑现 W116 明确留待后续的 `parse_reply` `top_k` 独立缝｜零生产代码改动

## 一、裁定与缝（现测，不是推断）

W116 闭合了 `RemoteError::status()` 的公开契约映射，并明确把 `parse_reply` 的 `top_k` 截断列为**独立缝、留待后续**。
本轮兑现。基于代码实测（非摘要），`parse_reply`（`remote.rs:395-411`）的截断语义**整段零判据**——既有 24 个
测试无一条断言 `top_k`（grep 零命中）：

| 位置 | 现状 | 判据缺口 |
|---|---|---|
| `remote.rs:404-409` `parse_reply` 证据截断 | `reply.evidence.into_iter().filter_map(to_evidence).take(top_k).collect()` | 既有 24 测试**零 `top_k` 断言** → **新增** 4 条守卫 |

三条边界语义（本轮闭世界维度）：

| 维度 | 语义 | 覆盖测试 |
|---|---|---|
| 顺序 | `take(top_k)` 保留序列**前 N 项**、顺序不变 | `search_truncates_evidence_to_top_k_in_order` |
| 顺序（filter 先于 take） | `top_k` 统计**有效项**（丢空 id 之后）——**非显然** | `search_applies_top_k_after_dropping_empty_ids` |
| 零值 | `top_k=0` ⇒ `take(0)` ⇒ 空集合 | `search_top_k_zero_yields_empty_evidence` |
| 超限 | `top_k >` 有效项数 ⇒ 保留全部、不补齐 | `search_top_k_exceeding_reply_keeps_all_evidence` |

**裁定**：判定面闭合，零生产代码改动——只把三条边界语义变成显式守卫，与 W113–W116 同形。

## 二、判据落点（`crates/nai-knowledge-core/tests/remote.rs`）

四条均经公共 `gateway.search` + Mock 到达私有 `parse_reply`（`gateway.search` → `client.search` → `parse_reply`），
`gateway` 不装配 rerank，返回即 `parse_reply` 原始输出：

- **新增 1**：`search_truncates_evidence_to_top_k_in_order` —— 5 有效项 + `top_k=2` ⇒ 恰为前 2 项、顺序不变
  （`["e0","e1"]`）。
- **新增 2**：`search_applies_top_k_after_dropping_empty_ids` —— 判别性构造 `[e0, "", e1, e2]` + `top_k=2`
  ⇒ `[e0,e1]`（`len=2`）；若回归成 take-then-filter，`take(2)=[e0,""]` 过滤后仅 `[e0]`（`len=1`）⇒ 断言变红。
  把「`top_k` 统计有效项（filter 先于 take）」从字符串层面钉死，不靠读代码推断。
- **新增 3**：`search_top_k_zero_yields_empty_evidence` —— `top_k=0` ⇒ `take(0)` ⇒ 空集合（MUST 被显式清点，
  绝不静默变「未截断」）。
- **新增 4**：`search_top_k_exceeding_reply_keeps_all_evidence` —— `top_k=10` > 5 项 ⇒ 全保留、顺序不变，
  不补齐占位、不报错。

新增测试本地复用私有 `search_mock(evidence)` 与 `gateway_search_ids(top_k)` 两个测试内辅助，避免重复。

## 三、计数（现测，非推断）

`crates/nai-knowledge-core/tests/remote.rs`：测试 **24 → 28**（+4 = 全为新增，W117 四条均未曾被任何测试覆盖；
W113–W116 的归属项已在各自报告标为既有）。本地复跑：`running 28 tests ... test result: ok. 28 passed; 0 failed`
（含全部 4 条 W117 新增）。门控权威总数见 §六。

## 四、如实披露（判定面闭合 ≠ 验收达标）

- `parse_reply` 保持 `private`，仅经公共 `gateway.search` 覆盖；不提升为 `pub`。
- 本轮只钉「既有截断语义」，不改 `search`/`answer` 的入参校验——`top_k` 是否应设上界属独立缝，明确不做。
- `gateway.answer` 亦调用 `parse_reply(reply.clone(), top_k)`（remote.rs:624），本轮经 `gateway.search` 路径
  覆盖同一函数；`answer` 路径的 `top_k` 截断与 `search` 共用 `parse_reply`，语义一致。
- 介质级擦除、跨 memory↔remote 传播仍明确不做（与 W113–W116 一致）。
- registry：守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。

## 五、spec 增量（`knowledge-pipeline`）

新增 4 条 Requirement（各 1 Scenario），把 `top_k` 截断闭世界纳入基线：
- **Remote search truncates evidence to top_k preserving order**（Evidence is truncated to the first top_k in order）；
- **Remote search applies top_k after dropping empty-ID evidence**（top_k counts valid evidence after dropping empty IDs）；
- **Remote search with zero top_k yields empty evidence**（top_k zero truncates to empty）；
- **Remote search with top_k exceeding reply keeps all evidence**（top_k greater than evidence count keeps all）。

## 六、门控（全量 `cargo xtask check --profile full`）

- `GATE_EXIT`：**0**（后台 `target-check/gate-w117.sh`，`PREBUILD_OK at attempt 1` + `GATE_OK at attempt 1`，重跑首跑即绿）。`rustc 1.96.0 (ac68faa20 2026-05-25)`。
- **首次门控 `GATE_EXIT=1` 的真实根因 = `cargo fmt --all --check` 失败**（非 LNK1104、非测试失败、非逻辑回归）：新增的两处多参 `.search(...)` 调用（`tests/remote.rs:1175` 等）未按 rustfmt 折行。最小化修复（`cargo fmt --all`，仅重排 `remote.rs` 新增行，`git diff` 确认无其他文件被改）后重跑 `GATE_EXIT=0`。失败日志存 `target-check/w117-gate-final.log`（已被重跑覆盖，见下条）。
- **门控脚本自省（教训，记入 memory）**：首跑时 `gate-w117.sh` 的重试循环把**预构建阶段**的瞬态 `LNK1104` 行（prebuild attempt 1 命中、attempt 2 已恢复）误判为门控阶段的失败成因——因为它 `grep -q LNK1104 "$LOG"` 扫的是**累积日志**，于是每次真实失败（fmt）都被误标为「LNK1104 retrying」，空转到 8 次上限。重跑时日志开头 `: > "$LOG"` 清空、且 fmt 已修，故 `GATE_OK at attempt 1`。**判据**：若重试循环连续报「LNK1104」却每次都删除**同一个**并不存在于 deps 的 exe 路径，应怀疑是陈旧日志行污染，而非真·瞬时锁。
- **LNK1104 瞬态计数**：重跑 **0**（grep `LNK1104` = 0；`PREBUILD_OK attempt 1`）。首跑预构建曾命中一次瞬态 LNK1104（attempt 2 恢复，按既定纪律认定为宿主瞬态、非回归、不修代码）。
- 全 workspace `cargo xtask check --profile full`：**95** 个测试二进制全 `test result: ok. ... 0 failed`，**0 FAILED / 0 error**；clippy `--workspace --all-targets --locked -- -D warnings` 通过（否则 `GATE_OK` 不成立）；`cargo fmt --all --check` 通过（修复后）；前端门控（`[frontend] 全部步骤通过`）通过。权威测试 target 中 `crates/nai-knowledge-core/tests/remote.rs` 运行 `running 28 tests ... 28 passed; 0 failed`（含全部 4 条 W117 新增，见 §三）。

## 七、提交 / 推送（归档 + 一次 commit + push，SSH 443）

- 提交 SHA：**`a48e177861456e2f89be5ea5d8ebc3fab93469b4`**（主提交）。
- push 区间：**`ae9ab9c..a48e177`**，分支 `main`，SSH 443（`ssh.github.com:443` + `StrictHostKeyChecking=accept-new`）。
- 改动文件：`crates/nai-knowledge-core/tests/remote.rs`（+4 测试）、`docs/adr/0095-retrieval-top-k-closed-world.md`（新建）、`docs/reports/2026-10-15-w117-retrieval-top-k-closed-world.md`（新建）、`docs/development-plan.md`（W117 段）、`docs/knowledge.md`（订正注记）、`openspec/specs/knowledge-pipeline/spec.md`（合入 4 条 Requirement）、`openspec/changes/archive/2026-10-15-w117-retrieval-top-k-closed-world/`（归档目录）。共 10 文件、+396 行。
- openspec 三连：`validate w117-retrieval-top-k-closed-world --strict` → valid；`archive --yes` → 4 条 Requirement 合入基线（`+ 4, ~ 0, - 0`）；`validate --archived --strict` → 全绿（**115 项，含 w117**，0 failed）。`target-check/` 按纪律排除暂存（显式 `git add -- <path>` 逐个暂存）。

## 八、CI 观测（主提交 + 回填提交各一次双平台观测）

- **主提交** run id：**38745838707**（workflow `Engineering foundation`，event `push`）。回读 `head_sha` = `a48e177861456e2f89be5ea5d8ebc3fab93469b4`，与推送 SHA **完整 40 位一致**：
  - `foundation (ubuntu-24.04)` — completed / **success**
  - `foundation (windows-latest)` — completed / **failure**（step 12 `Storage and credential boundary (Windows native vault only on Windows)` = `cargo xtask test --suite storage`；后续 step 13–22 全部 `skipped`）
  - **判定 windows step 12 为 CI runner 瞬态、非本变更回归**，依据（四条）：① `--suite storage` 只含 `nai-storage` + `nai-security`（`xtask/src/lib.rs:212-222`），本变更仅动 `crates/nai-knowledge-core/tests/remote.rs` + 文档 + openspec，`git status` **未触碰这两个包**；② 同代码 `a48e177` ubuntu step 12（同 suite）**success**；③ 本机 Windows `cargo xtask test --suite storage` → **55 passed / 0 failed**（host 侧同 suite 全绿）；④ 该 step 名明标「Windows native vault only on Windows」，天然依赖 runner 的凭据保管库状态。铁律二：宿主瞬态 MUST NOT 改变被测系统结论。
- **回填提交**（SHA `dff75b40cf42c801a32538f388c8a6287fac27cf`，docs-only）自身 CI 双平台观测：run **38748652853**，回读 `head_sha` 与推送 SHA **完整 40 位一致**：
  - `foundation (ubuntu-24.04)` — completed / **success**
  - `foundation (windows-latest)` — completed / **failure**，但**失败步与主提交不同**——本次 = step 10 `Complete engineering foundation (portable steps + frontend workspace)`（= `cargo xtask check --profile full`），主提交 = step 12（storage suite）。check-run 注解给出真实错误：`crates/nai-runtime/tests/assembly.rs` 的 `every_rebuild_rejection_is_reachable_through_the_runtime` 在 `tests/common/mod.rs:139` `provision()` 的 `save_config(..).unwrap()` 处 panic：`StorageError { code: DeadlineExceeded, reason: "operation stopped" }` —— **有界操作的截止期超时**（测试装配期），非断言失败、非本变更所在包。
  - **两次 windows 失败集合漂移**（step 12 `nai-storage`/`nai-security` ↔ step 10 `nai-runtime` assembly）、**错误类同族**（宿主机时限 / DeadlineExceeded）、**相关包本轮 `git status` 全空**、**同代码 ubuntu success + 本机 Windows 全绿** ⇒ 判定为 **windows-latest runner 瞬态**（时限 / 负载敏感），非 W117 回归（铁律二：宿主瞬态 MUST NOT 改变被测系统结论）。已追加复核提交重跑确认（见下）。
- **复核提交**（docs-only）自身 CI 双平台观测：run TBD，回读 `head_sha` 与推送 SHA **完整 40 位一致**；预期双平台 `foundation` 均 `completed / success`（含 windows）。**不为复核提交写 §九**，其自身结论按纪律记入 memory 日志。
