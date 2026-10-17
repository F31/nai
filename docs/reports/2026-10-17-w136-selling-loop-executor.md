# W136：B07 的执行体——「讲品循环」从一次判定变成会跑的东西

- 轮次：W136（2026-10-17）
- 基线：`docs/直播数字人 + 智能体系统技术方案 V3.11.md`（§53、§5.7、§5.3、§48.5、§57.3）
- 变更：`openspec/changes/archive/2026-10-17-w136-pitch-executor/`
- ADR：`docs/adr/0111-selling-loop-executor.md`
- 验收：**B07 基础讲品循环**（`r1`）`pending → implemented`

## 一、勘定

### 1.1 四环现测（开工前）

| 环 | 现测 | 证据 |
| --- | --- | --- |
| ① 判定面 | **已有** | `evaluate_pitch`，四条 row 全判，`tests/selling.rs` 19 项 |
| ② 执行体 | **没有** | `grep -rln "selling\|PitchPlan\|PitchPolicy" --include=*.rs crates/ apps/` ⇒ 只命中 `nai-kernel`（自身、`evaluate/mod.rs`、`evaluate/assembly.rs` 的注释、测试） |
| ③ 调用方 | **没有** | `Composition` 的公开面里没有任何讲品方法 |
| ④ 真实产生者 | **不在仓库内** | 段落内容与话术、TTS 时长、播放采样、§5.7 五面重校验 |

判定面的**形状**是缺口的成因：它判一整份 plan 的快照（`entrances` 是整条时间线），而循环是
一次进一段的，每一步要问「现在能进这一段吗」——快照判定没有「上一次进入是什么时候」这个状态。

### 1.2 一条既有判据比规格弱（本轮顺带修）

| | 内容 |
| --- | --- |
| 规格（`basic-selling-loop`） | 「同一段落的**重复进入** MUST 不早于段落级冷却」 |
| W68 实现 | `entrances.windows(2)` 且 `pair[1].segment == pair[0].segment` |
| 反例 | `selling@0 → spec@600 → selling@900`，冷却 1000ms ⇒ **判为合规**（没有任何一对相邻同段） |
| 语义 | 卖点在 300ms 后就回来了。规格要拒，实现放行 |
| 处置 | 冷却读数改成「该段自己的上次进入」；判别性样本按这个形状取（旧实现放行、新实现拒绝），边界另一侧（`gap == cooldown`）仍放行 |

这属于「判据增强会让既有用例前提失效 ⇒ 修前提、不关判据」的一类：改的是**读数来源**，
不是把某条断言调松。

### 1.3 计数（逐条现测，不引用上轮）

- `acceptance` **88** 条；本轮前 tested 12 / implemented 26 / pending **42** / blocked 5 / n/a 3。
- 本轮后：tested 12 / implemented **27** / pending **41** / blocked 5 / n/a 3（B07 单条迁移）。
- 未归档 OpenSpec 变更：开工前 **0**，本轮建 1 个并在收口时归档。

## 二、交付

### 2.1 kernel：把 row 3 抽成单一定义

| 新增/改动 | 内容 |
| --- | --- |
| `PitchHistory` | 最后一次进入 + **每段**最后一次进入 + `count()`；`record` **无条件**记录 |
| `evaluate_transition` | 单调前提先判（递减 ⇒ **只**报 `EntranceTimesDecrease`，区间规则不评估）；冷却读该段上次进入；切换读上一次次进入 |
| `evaluate_pitch` row 3 | 改为逐条重放 `evaluate_transition`，不再自己写一遍窗口比较 |

`record` 无条件记录是刻意的：历史记录的是**发生了什么**。让一次违规擦掉自己的证据，冷却就能被绕过。

### 2.2 runtime：执行体与调用方

| 项 | 位置 | 要点 |
| --- | --- | --- |
| `PitchRunner::start` | `crates/nai-runtime/src/pitch.rs` | 先整份判定；`!allowed` ⇒ 具名拒绝，**不产生半个循环** |
| `enter` | 同上 | **fail closed**：拒绝时不记录、不移动游标 |
| `pause` / `resume` | 同上 | 检查点不存未播出文本；**归位不是新进入**（不增长时间线、不进冷却），但要过 §5.7 重校验 |
| `ResumeRecheck` | 同上 | 五面 × 三态；`fresh()` / `unavailable()` / `with()` |
| `PitchRefusal` | 同上 | 四族四码：`INVALID_ARGUMENT` / `CONFLICT` / `EVIDENCE_EXPIRED` / `CAPABILITY_NOT_READY` |
| 调用方 | `Composition` | `begin_pitch` / `enter_segment` / `pause_pitch` / `resume_pitch` / `pitch` |

**§5.7 重校验为什么是输入**：五面的真实产生者是守门与知识层，仓库内没有。在这里写「假设它是新的」，
等于把**没测过的事实**写成**授权**。`unavailable()` 的存在是为了让「我们没有这份重校验」有
**可表示的取值**——否则调用方只能在「撒谎」与「不调用」之间选，而前者更可能发生。

## 三、判据

### 3.1 kernel（`tests/selling.rs`）：19 → **25**

| 新增用例 | 钉住什么 | 旧实现会怎样 |
| --- | --- | --- |
| `a_repeat_with_another_segment_in_between_still_hits_the_cooldown` | 冷却的读数是该段自己的上次进入 | **放行**（判别性） |
| `a_repeat_exactly_at_the_cooldown_boundary_is_allowed` | 「不早于」是 `<`，不是 `<=` | 两侧都成立 |
| `the_incremental_transition_agrees_with_the_batch_verdict` | 增量与批次对同一时间线答案相同 | 成立 |
| `the_first_entrance_has_no_interval_rule_to_break` | 空历史不是违规 | 成立 |
| `a_decreasing_transition_reports_only_the_monotonic_refusal` | 递减序列只报单调，不产生下溢伪违规 | 成立 |
| `the_history_keeps_the_latest_entry_of_each_segment` | 每段保留最新；未进入的段是 `None` | 成立 |

### 3.2 runtime（`tests/pitch.rs`，新增 **12** 条）

| 组 | 用例 |
| --- | --- |
| 启动前判定 | `a_plan_that_could_never_run_never_starts`、`a_plan_whose_claim_stands_on_a_stale_revision_never_starts` |
| 推进 | `the_loop_advances_in_the_fixed_order`、`a_refused_entry_leaves_the_timeline_untouched`、`a_segment_repeat_is_judged_against_its_own_last_entry` |
| §5.7 | `every_face_is_named_in_the_baseline_order`、`a_resume_without_a_checkpoint_is_refused`、`an_unmeasured_recheck_refuses_the_resume`、`a_stale_face_is_named_individually`、`a_fresh_recheck_resumes_without_entering_again` |
| 调用方 | `no_loop_started_is_not_the_same_as_progress_zero`、`the_composition_returns_named_refusals_with_distinct_codes` |

### 3.3 注入验证（I1–I7，逐组真红）

| 注入 | 改动 | 结果 |
| --- | --- | --- |
| I1 | 冷却退回「相邻同段」 | kernel **1 红**（判别性样本）+ runtime **1 红**（孪生样本）；`the_incremental_transition_agrees…` **仍绿**——两侧同时退化时「一致性」本身不是充分判据 |
| I2 | 拒绝后仍记录并推进 | runtime **3 红**（fail-closed 三条） |
| I3 | `Unavailable` 当作通过 | runtime **2 红** |
| I4 | 归位当成一次新进入 | runtime **2 红** |
| I5 | 计划被拒仍启动 | runtime **2 红** |
| I6 | 五个面退化成同一个名字 | runtime **3 红** |
| I7 | 两类重校验拒绝合并成同一错误码 | runtime **1 红** |

**逐字节还原**：注入基线取 fmt-clean 的最终字节，五个文件 `sha256sum -c` 全部 `OK`
（`selling.rs`、`pitch.rs`、`composition.rs`、`tests/selling.rs`、`tests/pitch.rs`）。

## 四、门控

### 4.1 首次门控红：真缺陷（clippy `collapsible_if` 5 处）

`cargo xtask check --profile full` 首跑 **88s** 红在 clippy（`-D warnings`）：5 处 `collapsible_if`
（`map_or(true, …)` → `is_none_or(…)`；嵌套 `if let Some + if` → let-chain）。改完先跑
`cargo fmt --all --check` 确认 fmt-clean，再重跑——注入基线与门控基线都是**最终字节**。

### 4.2 之后 **31 次**红在 `LNK1104`：不是代码缺陷

三次批量重试共 31 次，**失败目标两两不同**（`pipeline` → `engine` → `nai_engine` → `memory` / `ramp` /
`observation` → `supervisor` … `shared_ram` → `gate` … `assembly`），全部停在**链接期**，`test result`
计数 **0**。四条判据排除代码：

1. **逐目标能过**：`cargo test -p nai-runtime --test ramp --no-run` **10.55s 绿**、
   `--test observation --no-run` **6.70s 绿**——正是刚刚失败过的两个目标；
2. **目录可写**：`touch target/debug/deps/zzz-probe.exe` 成功；最小 `rustc probe.rs -o
   target/debug/deps/zzzprobe.exe` 成功（输出到仓库根同样成功）⇒ 不是路径权限、不是链接器坏；
3. **无残留进程**（`tasklist` 无 `cargo` / `rustc` / `link.exe` / 测试二进制）；磁盘 **58G** 空闲、
   `target` **28G**（不是打满）；
4. **伴随症状指向宿主**：同一次构建里反复出现
   `error deleting lock file for incremental compilation session directory … 拒绝访问 (os error 5)`
   ——rustc 连自己的**增量会话锁**都删不掉 ⇒ 有宿主侧持有 `target\debug\incremental`。本机
   `McAfee` 在装（`SecurityCenter2` 报 `productState 266240`）、Windows Defender 实时防护 `False`、
   `Get-MpPreference` 报 `0x800106ba` ⇒ **无法加排除项**。

### 4.3 收口那次（环境适配后一次绿）

处置 = **环境适配**，不改代码、不放宽判据：`CARGO_INCREMENTAL=0` + `CARGO_BUILD_JOBS=1`，且
**预链接与门控都带**（两边增量状态不同会互判脏，只带一边等于换一个循环）。
预链接 `cargo test --workspace --locked --exclude nai-xtask --no-run` **4m32s** `EXIT=0`、
`LNK1104` **0**、`拒绝访问` **0**；随后门控 **6m05s**、**`EXIT=0`**、`LNK1104` **0**。
**归档与文档回填之后对最终工作树又跑了一次**（20:17，**3m25s**，`EXIT=0`、`LNK1104` **0**，
计数与下表逐项相同）——**这一次才是提交时的工作树**，前一次是代码冻结时的同一批用例。计数：

- Rust **1103 / 0**（W135 是 1085 / 0 ⇒ **+18**：`tests/pitch.rs` 新增 12 + `tests/selling.rs` 19 → 25 的 6）；
- `Running` **84** 行（83 + 新增测试目标 `pitch` 一条）/ `Doc-tests` **18** 行 ⇒ `test result: ok` **101** 行；
- Node **348 / 0**、**15** 个套件（与 W135 逐项相同——本轮没有新增 Node 判据）；
- 错误类（`LNK1104` / `error[E…]` / `error:`）**0**、`warning:` **0**；前端 6 个 build/test 步骤全通过
  （`packages/contracts-ts` typecheck + build、`apps/desktop` typecheck + test + build、`<root>` test:ui）。

⚠️ **本机门控与 CI 的差别（如实登记）**：CI 用默认并行度与默认增量设置，本机这次是 `JOBS=1` +
增量关闭。这只说明「用例与断言在**本机**通过」，**MUST NOT** 记作 CI 结论；并行度差异是否在 CI 侧
复现同样的宿主症状，由本轮 CI 观测回答（若 CI 红，先按「本机绿 ≠ CI 绿」处理，不回头改判据）。

## 五、具名登记（本轮**发现但不做**）

1. **`apps/engine` 未接**：engine 的 `Script` 只有 `ManualEvent` 的 comment/gift，没有讲品概念。
   接它要先设计脚本层编排格式 ⇒ 另一个形状的设计，本轮不做，如实记 0 命中。
2. **§5.7 五面重校验无真实产生者**：本轮只建接缝并让它 fail closed。
3. **段落内容与话术、TTS 时长、播放采样**：§53 明确属内容创作与运行时，本轮无任何主张。
4. **`PitchHistory` 没有「被拒的进入也算历史」的选项**：刻意不提供，否则冷却可被绕过。
5. **`evaluate_pitch` 其余三条 row 的语义未动**：只改 row 3 的读数来源。
