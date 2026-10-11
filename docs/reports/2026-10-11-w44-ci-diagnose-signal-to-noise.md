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

1. **采集日志为何只有 11 行**——未定。新的截断自证会在**下一次**失败时自动点破它。
2. **`halt_during_media_drill_prewarm_never_commits_a_frame` 的根因**——本轮只把结论推进到**测试级**。已知线索：
   - 该测试断言「halt 必须**有界**且干净」，而 `Engine::halt_and_wait` 的上限是 **15 s**；
   - `AVATAR_TIMEOUT`（prewarm 等待上限）**同样是 15 s** —— 两者相等，任何一侧的偏差都直接吃到另一个的余量；
   - `DrillGuard::wait/tick` **确实**响应 halt（`stop` ⇒ `Err("HALT")`），故「halt 被忽略」这一解释不成立；
   - 疑似挂点在 `supervisor.shutdown()` 或 `server.await`，**未证实**——本机无法复现（测试是 Linux-only），
     需要一次完整日志或一台 Linux 主机。
3. 按 §2 的订正，**不要再**用「本机 Windows 门控绿」去论证任何 Linux 侧行为。
