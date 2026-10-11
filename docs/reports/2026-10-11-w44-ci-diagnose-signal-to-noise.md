# W44：诊断通道的第一次真实使用——从 11 条注解里能读出什么

- 变更：`openspec/changes/w44-ci-diagnose-signal-to-noise`（归档后为 `openspec/changes/archive/2026-10-11-w44-ci-diagnose-signal-to-noise/`）
- 前置：W43（`docs/reports/2026-10-11-w43-ci-failure-annotation.md`）
- 性质：**观测质量**修复。零 Rust 源码改动、零新依赖、不新增 workspace 包、**不改任何验收 `status`**、**workflow 不改**。

## 1. 通道的第一次真实使用

W43 建好通道后，它在 **W43 自己的勘误提交**上第一次真实触发——不是 probe，是真失败：

| 事实 | 值 |
| --- | --- |
| run | `38110014189`（`81ce88b`，`docs(w43): 勘误…`） |
| 失败 job | `foundation (ubuntu-24.04)`，第 10 步 `Complete engineering foundation` |
| 诊断步 | 第 11 步 `Publish the gate failure summary as annotations` → **success** |
| 起止 | `03:57:07Z` → `03:59:57Z`（**2 分 50 秒**） |
| check-run | `114383450000`，`annotations_count: 12` |

**通道可用，且首次给出了测试级结论**（匿名读取，无任何凭据）：

```
test halt_during_media_drill_prewarm_never_commits_a_frame ... FAILED
```

这就是 W42 登记、历轮只能定位到**步级**的那个 ubuntu 间歇失败。**从「有个 job 挂了」到「哪个测试挂了」，这一步是 W43 通道兑现的全部价值。**

## 2. 一条必须撤回的推理：「本机 Windows 从未复现」

历轮在交接记录里沿用了一条旁证——**「本机 Windows 跑 portable/full 从未复现」**。**这条旁证不成立。**

先说清范围，避免给历史记录安上没有的错误：**W42/W43 的报告正文并没有用它**。报告里「判定为非确定性」的依据是
「同一份代码在 `ba1c33b` 失败、其上仅改文档的 `6f3941e`/`21163d0` 成功 ⇒ 时红时绿」，**那一依据是充分的，不受本节影响**。
需要撤回的只是这条**辅证**。

失败的测试 `halt_during_media_drill_prewarm_never_commits_a_frame` 带 `#[cfg(target_os = "linux")]`：

```
apps/engine/tests/engine.rs:395:#[cfg(target_os = "linux")]
apps/engine/tests/engine.rs:397:fn halt_during_media_drill_prewarm_never_commits_a_frame() {
```

`apps/engine/tests/engine.rs` 的 6 个 `#[test]` 里，**4 个是 Linux-only**（`killed_keeper…`、`media_drill…`、`halt_during…`、`http_revoke…`），
只有 `control_surface…` 与 `incomplete_or_unsafe…` 跨平台。CI 日志里的 `running 6 tests` 是 **Linux** 的数字；
**Windows 上这个 target 只跑 2 个测试**——失败的那个**根本不编译**。

⇒ **「本机不复现」是结构性必然，不是证据。** 文件头的既有注释其实早已写明「The supervision drills are Linux-only on purpose」，
本轮之前没有人把它和「本机不复现」对上。**推论**：今后不能用「本机 Windows 门控绿」去论证任何 Linux 侧行为。

## 3. 摘要在这次真实失败里质量很差：11 条里只有 1 条是信号

那次运行产出的注解（除 GitHub 自动加的 `Process completed with exit code 1.` 外）：

| # | 内容 | 性质 |
| --- | --- | --- |
| 1 | `test halt_during_media_drill_prewarm_never_commits_a_frame ... FAILED` | ✅ **唯一真信号** |
| 2 | `Checking thiserror v2.0.21` | ❌ 编译噪音 |
| 3 | `Compiling thiserror-impl v2.0.21` | ❌ 编译噪音 |
| 4–7 | `test … ... ok` ×4 | ❌ 无关 |
| 8 | `running 6 tests` | ❌ 无关 |
| 9 | `Running tests/engine.rs (target/debug/deps/engine-f2152189a41cd270)` | ❌ 无关 |
| 10–11 | （另有若干） | ❌ 无关 |

三类缺陷、三个确定成因：

1. **裸 `error` 子串**：`STRONG` 用 `error` 做关键词 ⇒ `Checking thiserror` / `Compiling thiserror-impl`（`thiserror` 含 `error`）被判成强信号。
2. **通过测试被当成信号**：`…cors_queries_unimplemented_routes_and_expired_access_are_explicit ... ok` 里的 `unimplemented` 命中关键词 ⇒ 一个**通过**的测试被当成强信号，**并把它的后置上下文（正是那两行 `thiserror`）拖进摘要**。
3. **无关行填满配额**：信号稀少时用普通行补足到上限 ⇒ 1 条真信号被 10 条噪音淹没。

成因 2 是本地复演时**第一次跑就暴露**的——它说明关键词匹配这个判据**从维度上就是错的**。

## 4. 第四件事：这份日志本身可能不完整

那次采集到的日志**只有 11 行非空内容**，而 job 跑了 **2 分 50 秒**。判断依据是摘要里**没有**「（另有 N 行未展示）」这条提示，
而该提示只在 `omitted > 0`（即 `lines.length > picked.length`）时输出 ⇒ `lines.length` 只能是 11。

一次完整的 `cargo xtask check --profile full` 必然产出数千行（编译 + 8 个测试套件）。**11 行对不上 2 分 50 秒。**

采到的内容也没有 `test result:` 汇总行——libtest 跑完全部测试后**必然**打印它。

**根因未定**（本轮不猜）：可能是写入端被提前终止、输出缓冲丢失、或采集被切断。**可以确定的是它的危害**：
一份被截断的日志正在被当成全部证据，而摘要**没有任何提示**。⇒ 本轮让摘要在这种形态下**自己点破**。

## 5. 改动（全部在 `scripts/ci-diagnose.mjs`）

| 缺陷 | 改法 |
| --- | --- |
| 1. 裸 `error` | `\berror(\[|:)` —— 要求紧跟错误码方括号或冒号；`thiserror` / `thiserror-impl` 不再命中 |
| 2. 通过测试被当信号 | 新增噪音类：libtest 的 `test <name> ... ok\|ignored` 永不入选。**判据从「含不含某个词」改为「有没有报告失败」** |
| 3. 填充行淹没信号 | 选行改为「强信号 → 后置上下文与所属目标头 → 弱信号」；**一条信号都没有时**才退化尾部窗口，绝不填充 |
| — | 新增**目标头前查**（`Running tests/xxx.rs …`）：测试名不足以说明失败落在哪个二进制 |
| 4. 截断无提示 | 新增**截断自证**：有逐测试行却无 `test result:` 汇总行 ⇒ 输出一条 `::warning::` 并给出总行数 |

W43 的三条契约**不变**：日志缺失/为空静默退出 0；永不非零退出（参数错误除外）；逐行转义输出。
`engineering-foundation` 新增两条需求（见 §7）。**workflow 一行未改**——接线沿用 W43。

## 6. 验证（`scripts/*.mjs` 不在工程门控内，故以本机端到端样本为准）

**6.1 `STRONG` 正则可判性，12/12：**

```
ok  : quiet  "    Checking thiserror v2.0.21"
ok  : quiet  "   Compiling thiserror-impl v2.0.21"
ok  : SIGNAL "error[E0308]: mismatched types"
ok  : SIGNAL "error: could not compile `nai-kernel` (lib test) due to 1 previous error"
ok  : SIGNAL "test halt_during_x ... FAILED"
ok  : quiet  "test result: ok. 6 passed; 0 failed; …"
ok  : SIGNAL "FRONTEND_STEP_FAILED: tsc exited 1"
ALL 12 OK
```

**6.2 截断样本**（复刻本次真实 10 行）——**输出 3 条，`thiserror` 两行消失**：

```
::warning …::日志在测试汇总行（test result:）之前结束（共 10 行）——采集到的输出可能被截断，失败详情未必完整
::error …::     Running tests/engine.rs (target/debug/deps/engine-f2152189a41cd270)
::error …::test halt_during_media_drill_prewarm_never_commits_a_frame ... FAILED
```

对比 W43 的 11 条（1 真 + 10 噪）⇒ 现在 3 条**全部**有信息量。

**6.3 完整失败样本**——断言消息被完整带出：

```
::error …::     Running tests/engine.rs (…)
::error …::test halt_during_media_drill_prewarm_never_commits_a_frame ... FAILED
::error …::failures:
::error …::---- halt_during_media_drill_prewarm_never_commits_a_frame stdout ----
::error …::thread '…' panicked at apps/engine/tests/engine.rs:436:5:
::error …::halt must be bounded and clean
::error …::test result: FAILED. 5 passed; 1 failed; …
::error …::error: test failed, to rerun pass `--test engine`
```

**6.4 回归 W43 的 2307 行噪声样本**——三条真错误仍全部命中，且输出从「3 真 + 若干噪」收敛为 6 条全信号：

```
::error …::error[E0599]: no method named 'retract' found for enum 'Transition'
::error …::test result: FAILED. 6 passed; 1 failed; …
::error …::error: could not compile `nai-kernel` (lib test) due to 1 previous error
[ci-diagnose] 共 2307 行，输出 6 条 annotation（其余 2301 行未展示）
```

**6.5 契约保持**：缺文件 ⇒ 静默、exit 0；空文件 ⇒ 静默、exit 0；无参数 ⇒ exit 2（参数错误是脚本自身缺陷，应当被发现）。
**6.6** `node --check scripts/ci-diagnose.mjs` 通过。

## 7. 规格

`engineering-foundation` 新增两条需求（归档时吸收）：

- **A gate-failure summary surfaces signals, not filler**：摘要以指向失败的行及其紧邻详情与**测试目标头**为中心；
  判定依「是否报告失败」而非「是否含关键词」；信号稀少时保持少而准，MUST NOT 用无关行填充。
- **An incomplete capture is declared as such**：出现逐测试行却缺汇总行时 MUST 明示不完整并给出总行数。

把这两条写进规格的理由与 W43 相同：**它们就是这份摘要的全部价值**。一份被填充行淹没、又不说明自己不完整的摘要，
形式上「有输出」，实质上是**误导**——比没有更糟。

## 8. 未决与下一步

### 8.1 追加观测：新脚本在真实失败上的第一次运行（run `38111087579` / `ca47168`）

修复推送后 ubuntu job **再次失败**（第 10 步 failure、第 11 步诊断步 success）——这是**新脚本的第一次真实运行**。
**11 条注解全部是信号**（对比 §3 修复前的「1 真 + 10 噪」），且**没有出现截断警示**：

| 注解内容 | 价值 |
| --- | --- |
| `     Running tests/engine.rs (target/debug/deps/engine-f2152189a41cd270)` | 目标头（新增） |
| `test halt_during_media_drill_prewarm_never_commits_a_frame ... FAILED` | 失败测试名 |
| `failures:` / `---- … stdout ----` / 失败名 | 汇总段 |
| `thread '…' (19037) panicked at apps/engine/tests/engine.rs:446:5:` | **精确到行** |
| `a halt before the worker commits must not log a committed frame: [DecisionEntryView { seq: 1, …, "event":"armed" }, …]` | **断言原文** |
| `test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.03s` | 汇总行 |
| `error: test failed, to rerun pass …` | 收束 |

**两条独立结论：**

1. **截断自证的分辨力成立**。这次**没有**警示，因为本次日志确实含 `test result:` 汇总行；而 §4 那次（只有 11 行、无汇总行）**会**被警示。
   同一个判据在两种真实形态上给出了相反且**各自正确**的结论——这正是它该有的行为。
2. **根因类别确定，且与本节原先的猜测不同**。失败点是 `apps/engine/tests/engine.rs:446`，
   即**第二个**断言（`!entries.any(… "media_drill_committed" …)`），**不是** `halt_and_wait` 的 15 s 上限
   ⇒ **不是 hang**，而是「halt 之后决策日志里仍出现了 `media_drill_committed`」。
   机制：`Engine::start` 在 engine 打印出控制地址后即返回，**测试随后才写 halt 文件**；而 supervision 线程
   此时**可能已经把整个 media drill 跑完并提交了帧**。CI 上的调度差异让这个时序时快时慢 ⇒ 间歇性。
   这也再次说明它与「本机」无关（§2）——该测试在 Windows 上根本不执行。

**剩余未决：**

1. **采集日志为何只有 11 行**（§4）——未定。截断自证会在下一次该形态出现时自动点破它。
2. **上述竞态的修法**——未做。方向是让断言不再依赖「halt 一定先于提交到达」这一时序假设
   （例如给测试一个确定性的 prewarm 阻塞点，或注入人为慢的 render-worker），而不是放宽断言——
   断言本身（halt 之后不得再出现新提交）是对的。
3. 按 §2 的订正，**不要再**用「本机 Windows 门控绿」去论证任何 Linux 侧行为。
