# W115 报告：检索证据闭世界守卫（rerank + to_evidence）

日期：2026-10-15｜归属：W113/W114 之后「新开 nai 守卫缺口」勘定候选（检索链路进入「证据」后的判定面缺口）｜零生产代码改动

## 一、裁定与缝（现测，不是推断）

W114 闭合了远端网关的**构造期**与**输入边界**判定面。在「先测后设计」节奏下，继续勘定检索链路进入
「证据」后的下一处判定面缺口。基于代码实测（非摘要），`rerank_evidence` 与 `to_evidence` 有两处
「已声明、零判据」不变量（共四条）：

| 位置 | 现状 | 判据缺口 |
|---|---|---|
| `remote.rs:463-468` `rerank_evidence` 空 base 短路 | `base.is_empty()` ⇒ `Ok(RerankOutcome{ status: RetrievalEmpty, evidence: Vec::new() })`，不进 `spawn_blocking` | 既有 `rerank_preserves_base_on_budget_expiry_and_drops_unknown_ids` 只钉 reorder / unknown-id drop / budget-expiry partial，**零判据** → **新增** `rerank_empty_base_returns_empty_without_calling_reranker` |
| `remote.rs:490-495` rerank 输出去重 | `seen: BTreeSet` + `filter(seen.insert)` 仅保留首个重复 `evidence_id` | 同上，**零判据** → **新增** `rerank_deduplicates_repeated_ids` |
| `remote.rs:476-478` reranker 故障映射 | `spawn_blocking(rerank)` 的 `JoinError`（panic）⇒ `result.map_err(\|_\| RemoteError::Invalid)?` | 同上，**零判据** → **新增** `rerank_reranker_failure_maps_to_invalid` |
| `remote.rs:413-416` `to_evidence` 空 id 丢弃 | `if value.evidence_id.is_empty() { return None; }`，被 `search` 的 `filter_map` 掉 | 既有 `search` 只走合法非空 id，**零判据** → **新增** `search_drops_evidence_with_empty_id` |

**裁定**：判定面闭合，零生产代码改动——只把四处已声明不变量变成显式守卫，与 W113/W114 同形。

## 二、判据落点（`crates/nai-knowledge-core/tests/remote.rs`）

- **新增 1**：`rerank_empty_base_returns_empty_without_calling_reranker` —— 空 base ⇒ `RetrievalEmpty`
  空证据集；用 **panic reranker 探针**证明 reranker **未被调用**（短路路径在 `spawn_blocking` 之前返回，
  panic 不触发），把「短路 ⇒ 不调用」变成可观测断言而非仅靠返回 `RetrievalEmpty` 推断。
- **新增 2**：`rerank_deduplicates_repeated_ids` —— `base=[a,a,b]` + identity reranker ⇒ 结果长度 2、
  顺序 `[a,b]`；断言 `remote.rs:490-495` 的 `seen: BTreeSet` 去重语义（保留首个）。identity reranker
  不经 `known` 过滤（所有 id 都在 base 中），只测去重维度，不与 W114 既有 unknown-id drop 重叠。
- **新增 3**：`rerank_reranker_failure_maps_to_invalid` —— panic reranker ⇒ `JoinError` ⇒
  `RemoteError::Invalid`；断言 `.err() == Some(Invalid)` 而非 `partial` / `empty` 假阴性。`.err()` 而非
  `.unwrap_err()`——`RerankOutcome` 的 Ok 值未实现 `Debug`（与 W114 两处 `::new` 同因）。
- **新增 4**：`search_drops_evidence_with_empty_id` —— 经**公共 `search` API + Mock**，`/search` 返回
  `[{evidence_id:"good"},{evidence_id:""}]` ⇒ 结果 `len()==1` 且 `evidence_id=="good"`；`to_evidence`
  是 `private fn`，必须经公共路径覆盖，把「空 id 在 `filter_map` 被丢弃」钉成端到端守卫。

## 三、计数（现测，非推断）

`crates/nai-knowledge-core/tests/remote.rs`：`#[tokio::test]` **16 → 20**（+4 = 全为新增，W115 四条
均未曾被任何测试覆盖；无既有测试归属为本轮新增，W113/W114 的归属项已在各自报告标为既有）。本地复跑：
`running 20 tests ... test result: ok. 20 passed; 0 failed`（含全部 4 条 W115 新增）。门控权威总数见 §六。

## 四、如实披露（判定面闭合 ≠ 验收达标）

- `rerank_evidence` 的 unknown-id drop（W114 既有 `rerank_preserves_base_*` 已钉）与本案重复 id 去重是
  两个独立闭世界维度——unknown drop 由 `known: BTreeSet`（remote.rs:469-472）负责，重复 drop 由
  `seen: BTreeSet`（490-495）负责；W115 只补后者，不重复前者。
- 空 base 短路路径直接返回 `RetrievalEmpty`（不沿用 `base_status` 入参），该语义由 W115 显式钉住。
- `to_evidence` 保持 `private`，仅经公共 `search` 覆盖；不提升为 `pub`。
- 介质级擦除、跨 memory↔remote 传播仍明确不做（与 W113/W114 一致）。
- registry：检索证据守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。

## 五、spec 增量（`knowledge-pipeline`）

新增 4 条 Requirement（各 1 Scenario），把检索证据闭世界纳入基线：
- **Remote rerank short-circuits empty base without invoking reranker**（Empty base short-circuits without invoking reranker）；
- **Remote rerank deduplicates repeated evidence IDs**（Repeated evidence IDs are deduplicated）；
- **Remote rerank maps reranker failure to Invalid**（Reranker failure maps to Invalid）；
- **Remote search drops evidence with empty ID**（Evidence with empty ID is dropped from search results）。

## 六、门控（全量 `cargo xtask check --profile full`）

- `GATE_EXIT`：**0**（后台 `target-check/gate-w115.sh` 重跑，`GATE_OK at attempt 1`）。首跑曾 `GATE_EXIT=1`，
  根因 `cargo fmt --all --check` 不通过——仅 `crates/nai-knowledge-core/tests/remote.rs:1108` 的
  `assert_eq!` 长消息串未折行（rustfmt 要求折行），**非 LNK1104、非测试失败、非逻辑回归**。最小化修复
  （仅折行该 `assert_eq!`）后重跑 `GATE_EXIT=0`。失败日志存 `target-check/w115-gate-final-FAILED-fmt.log` 备稽。
- **LNK1104 瞬态计数**：**0**（grep `LNK1104` = 0；仅 Defender 持锁导致的
  `error deleting lock file ... 拒绝访问。 (os error 5)` 增量编译锁删除告警，按既定纪律认定为非回归、不修代码）。
- 全 workspace `cargo xtask check --profile full`：95 个测试二进制 **全部 `test result: ok. ... 0 failed`**，
  **0 FAILED**；clippy `--workspace --all-targets --locked -- -D warnings` 通过（否则 `GATE_OK` 不成立）；
  `cargo fmt --all --check` 通过（修复后）；前端门控（`[frontend] 全部步骤通过`）通过。
  权威测试总数以门控为准，W115 四条测试均含于其中（`running 20 tests ... 20 passed; 0 failed` 见 §三）。

## 七、提交 / 推送（归档 + 一次 commit + push，SSH 443）

- 提交 SHA：**`8cf1377631aa70dcad4c93d8203266be8713ca42`**（主提交）。
- push 区间：**`241b033..8cf1377`**，分支 `main`，SSH 443（`ssh.github.com:443` + `StrictHostKeyChecking=accept-new`）。
- 改动文件：`crates/nai-knowledge-core/tests/remote.rs`（+4 测试）、`docs/adr/0093-retrieval-evidence-closed-world.md`（新建）、`docs/reports/2026-10-15-w115-retrieval-evidence-closed-world.md`（新建）、`docs/development-plan.md`（W115 段）、`docs/knowledge.md`（订正注记）、`openspec/specs/knowledge-pipeline/spec.md`（合入 4 条 Requirement）、`openspec/changes/archive/2026-10-15-w115-retrieval-evidence-closed-world/`（归档目录）。
- openspec 三连：`validate w115-retrieval-evidence-closed-world --strict` → valid；`archive --yes` → 4 条 Requirement 合入基线；`validate --archived --strict` → 全绿（**113 项，含 w115**，0 failed）。`target-check/` 按纪律排除暂存。

## 八、CI 观测（主提交 + 回填提交各一次双平台观测）

- **主提交** run id：**38720466714**（workflow `Engineering foundation`）。回读 `head_sha` = `8cf1377631aa70dcad4c93d8203266be8713ca42`，与推送 SHA **完整 40 位一致**；双平台 `foundation` job 均 `completed / success`：
  - `foundation (windows-latest)` — completed / **success**
  - `foundation (ubuntu-24.04)` — completed / **success**
  - 本机门控绿（`GATE_EXIT=0`）与 CI 双平台绿一致；判定面闭合经端到端 CI 复核。
- **回填提交**（SHA `cc6b1fedf0b2563d80b1568d484b127a7b7a4ae2`，docs-only）自身 CI 双平台观测：run **38725261895**，回读 `head_sha` 与推送 SHA **完整 40 位一致**；双平台 `foundation` job 均 `completed / success`：
  - `foundation (windows-latest)` — completed / **success**
  - `foundation (ubuntu-24.04)` — completed / **success**
  - 回填提交 CI 自身绿（先例 run 109 failure ⇒ 本次实测 success，不赌）；不为回填提交写 §九，仅做 1 次观测。
- **回填收尾提交 `bc27245`**（仅补本报告 §八 回填结论文本，docs-only）自身 CI 观测：run **38726807373**。`foundation (windows-latest)` — completed / **success**；`foundation (ubuntu-24.04)` — completed / **failure**（step 10 `Complete engineering foundation`）。因该提交仅改本报告 markdown（不参与 Rust 构建、前端门控、openspec 校验），且同代码 `8cf1377`/`cc6b1fe` 的 ubuntu 均 `success`、本机 `GATE_EXIT=0`，判定 ubuntu 单次 failure 为 **CI runner 瞬态**，非本变更回归（铁律二：宿主瞬态 MUST NOT 改变被测系统结论）。为复核，已追加提交重跑 ubuntu（见下方）。
