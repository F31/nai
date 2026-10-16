# W130 报告：S07 诊断面扫描（日志 / 诊断包 / 崩溃转储）

- 轮次：W130｜日期：2026-10-17｜归属：`tests/registry.yaml` **S07**（无明文密钥）
- 关联：[ADR 0108](../adr/0108-diagnostic-artifact-secret-scan.md)、ADR 0038（W36 仓库密钥面）、
  ADR 0078（W100 执行配置面）、ADR 0100（`tested` 门槛）、
  `openspec/changes/archive/2026-10-17-w130-diagnostic-artifact-secret-scan/`、
  审计 `docs/reports/2026-10-16-state-of-implementation-audit.md`（缺口③：仓库内可闭环）

## 一、缺口（现测）

**缺口逐字来自 registry，不来自散文。** `S07` 的 notes 末句原话：
「日志/诊断包/崩溃转储的扫描仍未建，保持 implemented 不升级」。口径四面（仓库扫描／日志／诊断包／崩溃转储）
逐面现测：

| 面 | 仓库内落点 | 现测 |
|---|---|---|
| 仓库扫描 | 已建（W36） | `check --secrets` 仓库域 + 4 条 `--lib` 用例（两条各钉一个方向） |
| 日志 | **本轮建** | CI `tee` → `.nai-diagnostics/nai-gate.log`；消费者 `scripts/ci-diagnose.mjs` 把它发成匿名可读的 check-run annotation |
| 诊断包 | **无产生者** | 全仓搜诊断包目录/`.zip` 产物：0 命中；基线未为本仓指定诊断包目录 |
| 崩溃转储 | **无产生者** | 全仓搜 `core.dump`/crash 落点：0 命中 |

另实测（开工时的「零落点」核对）：全仓**无** `tracing`/`log` 依赖；`REDACTED` 唯一落点是 `Secret` 的
手工 `Debug`（**已有**测试覆盖，不是缺口）；`diagnostic_refs` 无消费者；仓库内**无任何代码写 `.log`**。

**本轮前 `cargo test -p nai-xtask --lib`：52 passed / 0 failed。**

## 二、改动与判据（真实证据）

### 2.1 生产代码（`xtask/src/secrets.rs` 新增产物域；仓库域一字未改）

| 项 | 内容 |
|---|---|
| `Artifact {dir, producer, source, marker}` + `ARTIFACTS` | 登记表；`source` 由 `include_str!` 编译期抓取 ⇒「登记了」与「核验了」是同一份字节。当前**只 1 项**：CI 门控日志 |
| `ArtifactHit {path, line, label}` | 命中结构，**不含匹配文本** |
| `ArtifactLedger {present, hits, scanned, non_text, non_file, directories, visited}` | 闭世界台账，四桶**各自独立累加** |
| `scan_tree(dir)` / `walk` | 递归遍历；缺目录 = 值；非 UTF-8 有损解码后**照样扫**；`DirEntry::file_type()` ⇒ **不跟随**符号链接 |
| `check_artifacts(dir)` | 产物域的独立入口 |
| `check(root)` | 追加产物域循环：按 `ARTIFACTS` 的 `dir` 逐个扫，累加 `artifacts_read`，把 `ledger.report()` 并入 findings |
| 具名登记的弱化 | 仓库域的 `else { continue; }`（非 UTF-8/读失败静默跳过）**保持原样**，只加注释（W130） |

### 2.2 接线（CI + 门控登记）

| 文件 | 改动 |
|---|---|
| `.github/workflows/foundation.yml` | 门控步 `tee` 目标 `$RUNNER_TEMP/nai-gate.log` → 仓库内约定目录 `.nai-diagnostics/nai-gate.log`（前置 `mkdir -p`）；诊断步读方同步；新增 **`if: always()`** 的 `cargo xtask check --secrets` 步 |
| `.gitignore` | 增 `/.nai-diagnostics/`（产物住在仓库内，扫描器才有明确的扫描域；但它不进 git 会发布的内容） |
| `scripts/contracts/workflow-steps.mjs` | `CI_ONLY_REASONS` +`ci-produced-artifact`；`CI_ONLY_REASON_PAYMENTS` 为该因配两条凭据（指向**消费者** + 扫描器的 `producer:` 字段）；`WORKFLOW_STEPS` +1 条在册步 |
| `scripts/contracts/step-config.mjs` | `CONDITION_SHAPES` +`always`；`conditionShapeOf` 认 `always()`；`platformLegsOf` 的 `always` 覆盖整个矩阵；**新增 R6** |
| `schemas/platform-matrix/platforms.json` | `toolchain-target-assumptions.source_files` +`xtask/src/secrets.rs`（它是平台条件编译文件） |

### 2.3 判据（`nai-xtask --lib` 52 → **60**）

新增 9 条（其中 1 条 `cfg(unix)` 门控）：①`an_artifact_carrying_credential_material_is_reported_without_echoing_it`
（命中报出但**不回显**匹配文本）；②`a_clean_artifact_tree_passes_and_still_counts_what_it_read`；
③`a_non_text_artifact_is_named_and_still_scanned_instead_of_being_dropped`（**二进制不丢**）；
④`an_empty_artifact_tree_reports_zero_rather_than_being_skipped`（空集报 0）；
⑤`a_missing_artifact_directory_is_reported_as_absent_not_as_an_empty_scan`（缺目录是值）；
⑥`every_visited_entry_lands_in_exactly_one_bucket`（闭世界可失败）；
⑦`subdirectories_are_walked_recursively`；⑧`#[cfg(unix)] a_symbolic_link_is_named_and_its_target_is_not_read`；
⑨`every_registered_artifact_names_the_repository_text_that_produces_it`（登记凭据双向核验，4 条断言）。
node 侧：`step-config.test.mjs` 新增 **F10**（产物扫描须覆盖失败路径），合成世界扩为 8 步 / 4 种 kind / 3 种条件形状。

## 三、注入验证（每条判据都要证明它会响）

**基线**：注入前先跑 `cargo fmt --all`（写）⇒ `--check` EXIT=0 ⇒ 基线是 **fmt-clean 的最终字节**。
基线哈希：`secrets.rs=4bfe9c56` / `foundation.yml=09fb0fe9` / `step-config.mjs=4e507471` / `workflow-steps.mjs=639f2518`。

| 注入 | 归属 | 实测结果（逐条现测） |
|---|---|---|
| A1 `marker` → 不存在的串 | ⑨ | 红：`foundation.yml does not contain "…"`（59/1） |
| A2 `dir` → `.nai-INJECTED-A2` | ⑨ | 红：`does not name the directory the scan walks (…)`（59/1） |
| A3 `scanned += 1` → `= ledger.visited` | ⑥ | 红：`an entry was visited but never accounted for`（58/2，连带 ②） |
| A4 非 UTF-8 分支 → `continue` | ③ | 红：`a binary artifact must be named, not passed over in silence`（58/2，⑥ 亦捕获未落桶） |
| A5 `producer` → `xtask/src/secrets.rs` | ⑨ | 红：`the criterion must not be its own credential`（59/1） |
| A6 `producer` → 未跟踪文件 | ⑨ | 红：`… is not a tracked file`（59/1） |
| B1 删 workflow 的 `if: always()` | R6 | 红：`carries the condition "" instead of always()`（node 28/2）——**只有 R6**，R1/R2 未误报 |
| B1b `always()` → `runner.os == 'Linux'` | R6 + R3 | 红：**R6** 与 **R3**（`silently skip the other platforms`）同时报（node 28/2） |
| B2 `CONDITION_SHAPES` 去 `always` | F8 | 红：形状表逐字断言（15/1） |
| B3 删 `ci-produced-artifact` 凭据格 | W108 | 红：`a cause nobody pays for is a label, not a reason`（11/3） |
| B4 从 `CI_ONLY_REASONS` 逐出该取值 | W95 | 红：**双向**——`not in the cause table` + 反向 `a payment with no cause is dead weight`（11/3） |

**覆盖说明（如实）**：⑨ 共 **4** 条断言，A1/A2/A5/A6 逐条覆盖。首轮只做了 A1/A2，
**A5/A6 是本轮补齐**——纪律要求「每个限定 MUST 用反例验证它在拦什么」。

**还原**：四文件 `sha256sum` 全部等于基线；`cargo fmt --all --check` EXIT=0；`nai-xtask --lib` 60/0；
node 六文件 **110/110**；`git status` 仍为 9 个修改项、无未跟踪残留。

**宿主瞬态（如实披露）**：`cargo test` 期间两次 `LNK1104 cannot open … nai_xtask-<hash>.exe`（输出 exe 被占用）
⇒ 清 `target/debug/deps/nai_xtask-*.exe` + `CARGO_BUILD_JOBS=1` 后即过；按纪律「逐目标能过 ⇒ 非代码缺陷」，未改代码。

## 四、registry 判定（**保持 `implemented`，不升 `tested`**）

现测：S07 口径四面中**两面**（仓库、日志）已有**真实产生者**与判据；**两面**（诊断包、崩溃转储）
在仓库内**没有产生者** ⇒ 在本仓库**建不了**（登记空目录不是覆盖，是发通行证，见 design.md）。
按 ADR 0100 §6 与 `admission-registry` 的 Honest backfill 要求，**剩下的缺口仍在 ⇒ MUST NOT 升 `tested`**。
registry `S07` 的 notes 就地订正为：仓库面（W36）+ 日志面（W130，含失败路径覆盖）已建；
诊断包/崩溃转储**无产生者**、扫描未建 ⇒ 保持 `implemented`，并具名这两面。

## 五、门控（`cargo xtask check --profile full`）

**门控共跑 4 次**（全部带 `CARGO_BUILD_JOBS=1`；r2/r3/r4 另带 `RUST_TEST_THREADS=1`）——
**两次红在宿主瞬态、两次绿**：

| 次 | 结果 | 失败点 |
|---|---|---|
| r1 | `GATE_EXIT=1`（2m08s） | `crates/nai-runtime/tests/composition.rs` 的 `prepare_without_profile_is_rejected`（行 124）：`StorageError { code: DeadlineExceeded, reason: "operation stopped" }` |
| r2 | `GATE_EXIT=0`（5m34s） | — |
| r3 | `GATE_EXIT=1`（1m31s） | `apps/engine/tests/pipeline.rs` 的 `a_playback_that_did_not_complete_never_logs_an_acknowledgement`（行 673）：`run.status` 为 `None`（子进程未正常退出、stderr 为空），该文件耗时 **39.95s** |
| r4 | `GATE_EXIT=0`（6m41s） | — |

**判定为宿主瞬态的四条替代判据（逐条现测，不是「跑一次就绿」的巧合）**：

| # | 判据 | 现测 |
|---|---|---|
| ① | 错误类同族、且全在 bring-up／子进程 | composition 侧：同一 `StorageError{DeadlineExceeded,"operation stopped"}`，成因是 `store_operation()` 复用**5s 期限的 `Operation`**；pipeline 侧：`provision_with_engine()` 起真实 engine 子进程，负载下被杀 ⇒ 报 `status: None` |
| ② | 失败集合**漂移**、不绑定用例 | ① 单独重跑 `-p nai-runtime --test composition` **3 次**：run1 `14/0` 绿、run2 `14/0` 绿、run3 换**另一个**用例 `invalid_runtime_config_is_rejected_at_provision`（行 168）；② 单独重跑 `-p nai-engine --test pipeline` **3 次**：**3/3 全绿**（21.97s / 24.99s / 23.26s，对比失败那次 39.95s）；③ 两次门控红的**目标不同**（composition ↔ pipeline）⇒ 同一份代码、漂移的失败集合 |
| ③ | 相关文件 `git status` 为空 | `crates/nai-runtime/`、`crates/nai-storage/`、`apps/engine/` 均不在本轮回改集（`git status --porcelain -- apps/` 为空） |
| ④ | 同一份代码重复跑全绿 | r2、r4 两次完整门控全绿，落地计数逐项相同（见下） |

⚠️ **composition 那条瞬态本仓早已登记**：`.workbuddy/memory/KNOWLEDGE.md` 的 W84 行、以及 W106 / W107 / W112
报告（W112 逐字记到同一对用例）⇒ **既有宿主坑再次现身，不是本轮引入的缺陷**。**处置＝限并发，不是代码修正**——
但 r3 说明**限并发只是减轻、不是消除**（r3 就是在 `RUST_TEST_THREADS=1` 下红的），故本轮**不以「限并发后全绿」
当结论**，结论只落在上表四条判据上。CI **不带** `RUST_TEST_THREADS=1` ⇒ CI 侧仍可能遇上同类瞬态（如实披露）。

**最终绿（r4；`GATE_EXIT=0`，6m41s）现测计数**（与 r2 逐项相同）：

- 步骤集合：`cargo fmt --all --check` → `cargo clippy --workspace --all-targets --locked -- -D warnings` →
  `cargo test --workspace --locked --exclude nai-xtask` → `cargo test --locked --package nai-xtask --lib` →
  `node scripts/frontend-gate.mjs`（7 步全过）
- 错误类扫描（`LNK1104` / `error[` / `^error` / `out of memory` / `FAILED` / `panicked`）：**0**
- `test result` 行 **97** = **79** 个真实测试目标 + **18** 个 `Doc-tests`
  （`^     Running ` 命中 **80** 条 = 79 个目标 + 1 条 cargo 自己回显的 ``Running `target\debug\nai-xtask.exe check …` ``）
- Rust 合计 **1045 passed / 0 failed**（逐目标 `0 failed`，`grep -cE "test result:.*[1-9][0-9]* failed"` = **0**）
- Node 侧 **13** 个套件、**324 passed / 0 failed**（逐套件 `# fail 0` = 13/13）
- **真实 rustc 告警 0**（排除增量编译 lock 文件的宿主噪音）

**门控在「全部改动定稿后」重跑**：r4 之前所有代码/契约/规格/文档改动均已落盘（r2 之后的改动只有
`openspec/specs/*`、`docs/*`、归档目录 —— 门控的输入集合不含它们；门控读到的 `docs/` 文件只有
`docs/control.md` / `docs/engine.md` / `docs/engineering.md` / `docs/adr/0043-*.md` 与 registry 的 evidence 集，
本轮回改均未触及）。

**对照 W129**（**1037 / 0**、**79** 目标、Node **323**）：Rust **+8** == 本轮 `nai-xtask --lib` **52 → 60**
（ADR 0108 记：新增 9 条，其中 1 条 `cfg(unix)` 在本机不编入）；目标数 **79 不变**（新用例挂在既有
lib 目标上）；Node **+1** == `tests/contracts/step-config.test.mjs` 的 **F10**。

**另跑 `cargo xtask check --secrets`**（它**不在** `full` 的步骤集合里，故单独取证）⇒ `EXIT=0`，打印两段结算：

```
diagnostic scan: no directory at E:/projects/nai/.nai-diagnostics (nothing to scan)
credential scan passed: 1319 files git would publish and 0 diagnostic artifact file(s) carry no prefixed credential material
```

「缺目录是**值**」在真机上得到实证：报的是**没有这个目录**、不是「跳过」，且整条命令仍**通过**。

### 5.1 归档后 `validate --all --strict` 首跑红（**真实缺陷，已就地订正**）

归档后立刻跑两条 validate：`validate --all --strict --archived` **128/0 绿**，而 `validate --all --strict`
**41 passed / 2 failed**：

```
✗ spec/engineering-foundation   ⚠ requirements[22]: Requirement text is very long (>500 characters)
✗ spec/live-provider-evidence  ⚠ requirements[1]:  Requirement text is very long (>500 characters)
```

**根因**：W130 把新增条款**写进了既有 Requirement 的正文**——`engineering-foundation` @383 从 **442**（HEAD
实测）涨到 **604** 字符、`live-provider-evidence` @20 从 **386**（HEAD 实测）涨到 **838** 字符
（上限判定是 `> 500`；全仓活规格 448 条既有正文里最大正好是 **500**（`runtime-composition@326`），
另有 498 / 497 / 497 三条贴着线）。这正是本仓纪律「**一条 Requirement 只声明一条行为**」所指的缺陷：
`always()` 条件与「诊断产物域」各自都是**另一条行为**；而且 `always()` 规则被**两个规格各声明一遍**，
另违反「同一件事 MUST NOT 两处定义」。

**订正（就地重做差量并重新归档；不压字、不动判据）**：撤销归档 → 改 change 差量 → 重归档
（`+2 added / ~1 modified`）：

| 规格 | MODIFIED（正文长度） | ADDED（正文长度） |
|---|---|---|
| `live-provider-evidence` | `Credentials come from the process environment…`（**386**，回到 W36 原样） | `The registered diagnostic artifact domain is scanned by the same command`（**404**） |
| `engineering-foundation` | `Every registered workflow step's execution configuration…`（**440**，去掉 `always` 正文） | `A step that judges an artifact produced by an earlier step declares always()`（**160**） |

`always` 规则**只留一处**（`engineering-foundation`）。订正后两条 validate：**43/0** 与 **128/0** 全绿。

⚠️ **判据可达性缺口（具名登记，本轮不修）**：**归档前没有任何信号**。
①`validate <change> --strict` 对 **MODIFIED 的正文长度豁免**——
`node_modules/@fission-ai/openspec/dist/core/validation/validator.js` 第 255–258 行逐字写着
`MODIFIED is left alone: its text is the existing requirement`，而 500 字符上限只在**主规格**上查；
②本机门控 `cargo xtask check --profile full` 的步骤集合里**没有**规格校验
（`xtask/src/lib.rs` 的 `engineering_checks` 只登记 `tests/contracts/*.test.mjs`）。
③CI **有**两步规格校验——`.github/workflows/foundation.yml:93` 的 `pnpm spec:validate`
（= `openspec validate --all --strict --no-interactive`）与第 94 行的 `validate --archived --no-interactive`
（实测覆盖面：`--all --strict` **43** 项活规格、`--archived` **128** 项归档变更）⇒ CI **会**抓到，
但抓到的时机是**推送之后**（本机门控绿 + CI 红）。两条合起来 ⇒ 超长的 MODIFIED 正文**只在归档之后**
才被看见（本轮即这样被抓到：本地手工 `--all --strict`）。修它属于另一件工作（把规格校验接进本机门控，
或给 MODIFIED 也加长度判据），本轮**不动**。

## 六、提交与推送

_（回填）_

## 七、CI 观测（触发确认 1 次 + 完成态 ≥2 次）

_（回填）_
