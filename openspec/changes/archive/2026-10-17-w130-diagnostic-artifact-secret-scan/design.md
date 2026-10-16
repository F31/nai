# Design: W130 — S07 诊断面扫描

## 缺口（逐项现测，MUST NOT 由「同族」推断）
基线 `docs/直播数字人 + 智能体系统技术方案 V3.11.md`：
- 行 **83**（INV-11）：「密钥只经凭据引用访问；界面、**日志、诊断包、崩溃转储**不含明文密钥 | S07」
- 行 **415**（T-04）：「密钥泄露 | 系统凭据库；最小权限与分用途密钥；日志、诊断包、转储脱敏；密钥扫描 | S07」
- 行 **634**：「S07 | 密钥：日志、诊断包、崩溃转储、仓库扫描均无明文密钥 | R1」

口径含**四面**。开工前现测：

| 面 | 仓库内落点 | 证据 |
|---|---|---|
| 仓库扫描 | 已建（W36） | `check --secrets` 的仓库域 + 4 条 `--lib` 测试 |
| 日志 | **本轮建**（唯一有真实产生者的一面） | CI `tee` → `.nai-diagnostics/nai-gate.log`；消费者 `scripts/ci-diagnose.mjs` |
| 诊断包 | **仓库内无产生者** | 全仓搜 `.zip`/诊断包目录：0 命中；基线未为本仓指定诊断包目录 |
| 崩溃转储 | **仓库内无产生者** | 全仓搜 `core.dump`/`crash`：0 命中；`CRASH_DUMP_*`/`RUST_BACKTRACE` 无落点 |

另实测（本轮开工时的「零落点」核对）：全仓**无** `tracing`/`log` 依赖；`REDACTED` 的唯一落点是
`Secret` 的手工 `Debug`——**已有**测试覆盖，不是本轮缺口；`diagnostic_refs` 无消费者；
仓库内**没有任何代码写 `.log` 文件**。⇒ 唯一真实存在的诊断产物是 CI 门控日志。

## 登记表的构造决定：为什么不登记「诊断包/崩溃转储」两个目录
`ARTIFACTS` 是**产生者的名册**，不是「我们希望扫的目录」的清单。登记一个**没有产生者**的目录，
等于给「扫了个空目录」发一张通行证：它会让 `present = true`、`scanned = 0`，报告上看与「干净」
一模一样，而实际上**没有任何东西被扫**。因此本轮**只**登记 CI 门控日志，把「诊断包与崩溃转储
在仓库内没有产生者」这件事写进报告、registry notes 与 ADR 具名，而**不是**用两个空目录把它
敷衍过去。这条判据的可观测后果由 `every_registered_artifact_names_the_repository_text_that_produces_it`
双向钉住：登记的 `marker` MUST 逐字存在于 `producer` 的文本里，且那个 `marker` MUST 点名 `dir`。

## 两个域，一套判据，两种缺省语义
| | 仓库域（既有） | 产物域（本轮） |
|---|---|---|
| 域 | `git ls-files --cached --others --exclude-standard` | `ARTIFACTS` 登记的目录，递归 |
| 缺目录 | 不适用（git 必在） | `present = false` =「没东西可扫」（**是值，不是错误**） |
| 非 UTF-8 | `else { continue; }`——**静默跳过**（具名登记的弱点，本轮不修） | 有损解码后**照样扫**，并具名进 `non_text` |
| 符号链接 | 不适用 | `DirEntry::file_type()`（`lstat` 语义）⇒ **不跟随**（构造性） |
| 命中输出 | `file:line (label)`，不回显匹配文本 | 同左（`ArtifactHit` 同样**不含**匹配文本） |

**为什么产物域 MUST 收下非 UTF-8**：崩溃转储就是二进制。若照抄仓库域的 `continue`，整条记录
会被静默丢掉，报告看起来与干净一模一样。有损解码把无效字节变成替换字符、**ASCII 段逐字保留**
——而高熵凭据（`sk-` 前缀那一类）恰在 ASCII 段。这条差别由
`a_non_text_artifact_is_named_and_still_scanned_instead_of_being_dropped` 钉住。

## 闭世界台账：四个桶各自独立累加
`ArtifactLedger { present, hits, scanned, non_text, non_file, directories, visited }`。
`visited` 数每一个 `read_dir` 条目（含目录），四个桶**各自**数自己那一类、**不是派生**。
于是 `visited == scanned + non_text.len() + non_file.len() + directories` 是一条**可失败**的判据：
新增一条分支却忘了落桶，它会红（注入 A3 证明：把 `scanned += 1` 改成 `= visited`，判据报
「an entry was visited but never accounted for」）。空集 MUST 被清点成 `0` 并通过；
缺目录 MUST 说「no directory at … (nothing to scan)」，**MUST NOT** 用「跳过」措辞。

## `always()` 是必须的，不是保险
GitHub 的默认语义：**前序步失败 ⇒ 后续步被跳过**。产物扫描判的是**前序步写的产物**，
而失败那一次的日志恰恰最该扫（libtest 会转储失败测试的 stdout）。于是「没写 `if:`」等于
「门控一红，扫描就消失」——一个**在需要它的那一刻才消失**的判定。R6 因此要求该步的 `if:`
MUST 是 `always()`。`always` 与「没写条件」在**平台集合**上等价（都覆盖整个矩阵），
差别**只在失败语义**；这也是 `CONDITION_SHAPES` 必须把 `always` 收成独立形状的原因。

## 边界：规则改动与数据改动分开
- **规则**（写进规格）：`always` 形状、R6、「产物扫描覆盖失败路径」。
- **数据**（不写规格）：成因闭集新增取值 `ci-produced-artifact`、兑付表新增一格、`ARTIFACTS`
  新增一行。W95 与 W108 的 Requirement 是**生成的**（「成因取自一个闭集」「每个成因 SHALL 付代价」），
  新增取值是对这两条**通用规则**的实例化，MUST NOT 在规格里再写一遍——那是**同一件事两处定义**。

## 注入验证矩阵（真跑，每条注入都确认「对应判据变红且可归因」，随后逐字节还原）
| 注入 | 目标判据 | 实测（#527 现测） |
|---|---|---|
| A1 `marker` → 不存在的串 | `every_registered_artifact_names_…` | 红：`foundation.yml does not contain "…"` |
| A2 `dir` → 别处 | 同上 | 红：`does not name the directory the scan walks (…)` |
| A3 `scanned += 1` → `= visited` | `every_visited_entry_lands_in_exactly_one_bucket` | 红：`an entry was visited but never accounted for` |
| A4 非 UTF-8 分支 → `continue` | `a_non_text_artifact_is_named_and_still_scanned_…` | 红：`a binary artifact must be named, not passed over in silence` |
| A5 `producer` → `xtask/src/secrets.rs` | `every_registered_artifact_names_…` | 红：`the criterion must not be its own credential` |
| A6 `producer` → 未跟踪文件 | 同上 | 红：`… is not a tracked file` |
| B1 删 `if: always()` | W100 真实 workflow 判据 | 红：**R6**（只有 R6，R1/R2 未误报） |
| B1b `always()` → `runner.os == 'Linux'` | 同上 | 红：**R6 + R3**（`silently skip the other platforms`）同时报 |
| B2 `CONDITION_SHAPES` 去 `always` | F8 | 红：形状表逐字断言 |
| B3 删 `ci-produced-artifact` 凭据格 | W108 | 红：`a cause nobody pays for is a label, not a reason` |
| B4 从 `CI_ONLY_REASONS` 逐出该取值 | W95 | 红：**双向**——`not in the cause table` + 反向 `a payment with no cause is dead weight` |

覆盖说明（如实）：`every_registered_artifact_names_…` 共 **4** 条断言，A1/A2/A5/A6 逐条覆盖；
首轮只做了 A1/A2，A5/A6 是本轮补齐——纪律要求「每个限定 MUST 用反例验证它在拦什么」。

## 明确不做
- **不修**仓库域 `check` 的 `else { continue; }`（非 UTF-8/读失败静默跳过）：这是**具名登记的弱化**，
  只加注释，不改行为。顺手修会让本轮的范围失控，也会让「哪一处是本轮的改动」变模糊。
- **不登记**没有产生者的目录（诊断包、崩溃转储）：见上。登记它们不是覆盖，是**发通行证**。
- **不**把 CI 日志改成别的落点（如 `$RUNNER_TEMP`）：扫描器需要明确的扫描域，产物必须住在
  它看得见的仓库内约定目录里（已被 `.gitignore` 排除，不进 git 会发布的内容）。
- **不动** `xtask/src/lib.rs` 的 `check --secrets` 分发签名：沿用已注册签名 ⇒ 不触碰 W96 的
  usage/逐字清单。
