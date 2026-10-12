# W70：瞬态宿主条件不得改变结论（V3.9 §5.9、§12.2、§20.6、§46.3；runtime 装配根与会话共享）

- 日期：2026-10-12
- 变更：`openspec/changes/archive/2026-10-12-w70-transient-host-conditions/`
- 规格：`openspec/specs/runtime-composition/spec.md`（**MODIFIED** 两条既有需求 + **ADDED** 一条，**非**新建能力）
- 代码：`crates/nai-runtime/src/composition.rs`、`crates/nai-runtime/src/lib.rs`、`crates/nai-runtime/tests/composition.rs`、`apps/engine/src/lib.rs`
- 登记：**未新增 registry 条目**（不改任何 `status`、不改基线文本）

## 一、判断

本轮**不是**一次「清 flaky CI」的运维动作。触发是 W69 收尾时登记的两族 CI 红，核实后它们
**不是同一类模糊的「不稳」，而是两个可机械证明的缺陷**，其中一个**是安全缺陷**。

| push | 平台 | 落在 | 表面说法 | 核实后的性质 |
|---|---|---|---|---|
| `a9f9aea`（run `38200252090`） | ubuntu | `apps/engine/tests/engine.rs::killed_keeper_is_halted_by_the_engine_and_the_chain_survives` | 「时序竞态」 | **安全缺陷**：一次微秒级、会自清除的锁竞争，可**永久终结监督循环**，此后 keeper 被杀**再也不被检测与撤权** |
| `87dce31`（run `38201171920`，**只改一个 `.md`**） | windows | `apps/engine/tests/pipeline.rs` 的 4 个用例，`provisioning store unavailable: storage: DEADLINE_EXCEEDED` | 「慢主机」 | **预算与工作量不匹配**：全新数据根上跑**全链迁移**被挤进 5 s 的逐次调用预算 |

两族都**不属于任何功能线**，但每一轮 push 都在削弱「推送可信」这件事本身——这正是它排在任何新功能线之前的原因。
W48 报告 §5 第 2 条早已把这件事**明确留给单独一轮**（原句：**「Windows 侧 `nai-runtime` 的时序脆弱未修…需单独一轮。」**），
本轮就是那一轮。

## 二、事实链（全部由源码得出，不依赖 CI 日志）

### 2.1 缺口一：瞬态锁竞争可夺走一个安全决定

因果链逐段可核实，无一处需要假设：

1. `ControlApi` 与装配根**共享同一个 `Arc<Mutex<Session>>`**（`apps/engine/src/lib.rs`、`crates/nai-control-api/src/lib.rs`）。
2. 两侧取锁都是 **fail-fast 的 `try_lock`**：`Composition::session_lock`（`composition.rs:236`）与
   控制面 `status` 处理器（`crates/nai-control-api/src/lib.rs`）都返回即走，不等。
3. `DrillGuard::tick` **每 5 ms** 调一次 `live_epoch()`（`DRILL_POLL`，`apps/engine/src/lib.rs:69`；`tick` @ `:593`）。
4. 旧的 `live_epoch()` 以 **`?`** 上抛取锁错误 ⇒ 一次竞争**被当成故障**。
5. `tick` 把该错误当**致命** ⇒ 闭包返回 `Err` ⇒ **`supervise` 结束监督循环**。
6. 循环一结束，**keeper 被杀也不再被检测、不再被撤权**。

`Session::revoke` 自身**只剩 `actor_allowed` 一个前置**（`crates/nai-kernel/src/session.rs`，本场景恒 `true`）⇒
「撤权被跳过」**只有取锁一条机械通道**，所以上述链条是**唯一可能的原因**，可以照它修。

还有一处**巧合式的掩盖**：既有规格的「续租失败不得静默退出」只有**一次性补偿 `halt`**（在错误出口）来满足，
而那个出口**用的是同一条 fail-fast 取锁**⇒「到底撤没撤权」取决于**第二次**取锁运气。

### 2.2 缺口二：bring-up 期限没有按工作量标定

- `Store::open`（`crates/nai-storage/src/store.rs`）在**全新数据根**上会跑**全链迁移**（`migration.rs` 的 `VERSION: i64 = 6`，
  6 个脚本、每段 `BEGIN IMMEDIATE` + `COMMIT`），连接以 **`synchronous=FULL`** 打开。
- 而它当时的预算来自 `Composition::prepare` 的 `short()` = **`SHORT_OPERATION` = 5 s**——一个**逐次调用**的预算。
- 对照：ADR 0004 明确 **30 s 是上界**（`Operation::new` 对 `timeout > 30s` 直接返回 `ErrorCode::InvalidArgument`），
  ADR 0038 称其为**安全层不变量**；同仓做**同类迁移工作**的既有取值一律是 **30 s**
  （`crates/nai-storage/src/tests.rs`、`crates/nai-upgrade-core/tests/*`、`crates/nai-provider-core/examples/openai_smoke.rs`）。
- ⇒ 全新数据根 + `synchronous=FULL` + 慢盘 ⇒ **`DEADLINE_EXCEEDED`**，与观察到的失败文案一致。

## 三、修法：把「瞬态」与「故障」分开，并按工作量给预算

治理判据（写进规格，可判定）：

> **宿主的瞬态条件（锁竞争、慢盘）MUST NOT 改变被测系统的结论。**

### 3.1 一个预算一份定义（ADR 0027）

| 路径 | 预算 | 耗尽时的语义 |
|---|---|---|
| 撤权与故障处置（`observe` / `control` / `enforce_duty`） | `SESSION_REVOKE_BUDGET` = **250 ms**，有界重试 | **显式 `Conflict`** —— 撤权**要么落地、要么被报告**，**绝不静默跳过** |
| epoch 存活轮询（`live_epoch`） | `SESSION_POLL_BUDGET` = **5 ms**（= 一个 tick） | **`Ok(None)`** = 「本 tick 未观测到」，**不是故障** |
| bring-up（`Composition::prepare` 的 `Store::open`） | `BRINGUP_OPERATION` = **30 s**（ADR 0004 上界） | 具名失败，**且文案带已耗时** |
| 逐次调用 | `SHORT_OPERATION` = **5 s**（**不变**，不含迁移） | 同上 |

`live_epoch()` 的签名变成 `Result<Option<u64>>`：`Some(e)` 正常、`None` 竞争、**`Err` 仅锁中毒**——
把「没看到」与「出错了」在类型上分开。`SessionLockMiss { Busy, Poisoned }` 就是为此存在。

### 3.2 被否决的三个替代（都不是本轮的修法）

| 替代 | 为什么不 |
|---|---|
| 把 `session_lock` 一律改成阻塞 `lock()` | 把一次宿主竞争**换成一次无限等待**，控制面 `status` 会挂住；而且它「修」的是运气，不是语义 |
| 只在测试夹具里等 | 被修的是**产品代码对瞬态条件的处理**，不是调度假设；夹具等待会让缺陷在生产里原样保留 |
| `--test-threads=1` | 同上：把「并发下必须成立」的断言降级成「串行下成立」，是**放宽**而不是**修** |

### 3.3 明确**没有**做的事

不改任何断言、不放宽任何窗口、**不用 sleep 掩盖**、不改 CI workflow、不降 `--test-threads`、
不改 `apps/engine/tests/*` 任何一行、**不新增契约名**、**不新增 registry 条目**、不改任何既有 `status`、不改基线文本。

**一处刻意的不改**：`apps/engine/src/pipeline.rs` 的两处 5 s 预算**保持原样**——它们是在**已打开的 store**上读，
**不含迁移**，因此不属于缺口二。这是判据的**结果**，不是遗漏。

## 四、测试与非空洞注入验证

新增断言全部落在**既有**目标内（`crates/nai-runtime` 的 lib 单测模块与 `tests/composition.rs`）
⇒ **不新增测试目标**（这是刻意的：目标数不动，增量只能来自项数）。

三个行为测试（`crates/nai-runtime/tests/composition.rs`）：

| 测试 | 断言 |
|---|---|
| `a_transient_session_lock_race_does_not_steal_the_revoke` | 另起线程持锁 20 ms 期间，`control(ControlAction::Mute)` **仍返回 `1`** |
| `a_busy_session_lock_reads_as_not_observed_rather_than_a_fault` | 无竞争 `Some(0)`；持锁 30 ms 期间为 **`None`（不是 `Err`）**；释放后回到 `Some(0)` |
| `a_revoked_session_is_observed_as_a_changed_epoch` | **正向前置**：撤权后 epoch **确实变化**——否则「恒报未观测到」也能让测试变绿 |

两条策略钉死测试（`crates/nai-runtime/src/composition.rs`）：

| 测试 | 断言 |
|---|---|
| `the_lock_and_operation_budgets_match_the_policy_table` | 逐项钉死四个常量；关系 `BRINGUP > SHORT`、`POLL ≤ REVOKE`、`REVOKE ≤ BRINGUP`；`Operation::new(31s)` 必错、`Operation::new(BRINGUP)` 必成 |
| `the_bring_up_open_is_bounded_by_the_bring_up_budget` | 在**新建临时目录**上真的调用 `open_bringup_store(...)`，断言其 `remaining() > SHORT_OPERATION` |

**持锁时序用 mpsc 握手**（`hold_session`，`tests/composition.rs:17`）确保**持锁先于**被测调用——**不用 sleep 竞猜**。

### 4.1 五组非空洞注入（每组都被抓住，源文件按内容哈希复原为 `IDENTICAL`）

| 注入 | 被抓住的断言 |
|---|---|
| 去掉重试截止（把 deadline 改成恒不满足） | `a_transient_session_lock_race_does_not_steal_the_revoke` |
| 把 `Ok(None)` 改回注入的 `Err(Conflict)` | `a_busy_session_lock_reads_as_not_observed_rather_than_a_fault` |
| 把 `BRINGUP_OPERATION` 退回 5 s | `the_lock_and_operation_budgets_match_the_policy_table` |
| 把 `bringup()` 的实现换成 `Operation::new(SHORT_OPERATION)` | 同上（`assertion failed: bringup_left > short_left`） |
| 把 `open_bringup_store` 内的 `bringup()` 换成 `short()` | `the_bring_up_open_is_bounded_by_the_bring_up_budget` |

第五组是**在验证过程中发现测试设计不足后补的**：只钉常量对「**常量正确、但调用点接错**」是瞎的
⇒ 把 `open_bringup_store` 提成 `prepare` 真正调用的那个函数，再让它进测试。

## 五、边界（如实）

- **本机不能证明 Linux 侧成立**：`apps/engine/tests/engine.rs` 是 `#[cfg(target_os = "linux")]`，
  本机 Windows 门控**跑不到**那一族；缺口一在 ubuntu 上的修复只能**以 CI 结论为准**。
- **family 1 在 CI 上的触发时点未能还原**：链是从源码读出来的，不是从那份失败日志读出来的
  ⇒ 报告**不主张**「已复现当时的失败」，只主张「这条链存在且被消除」。
- **H 系与验收状态一律不动**：本轮不涉及 harness 轨道，不改任何 `status`。
- **基线仍有两处待收录**：W68 登记的 B07 偏差，以及 W67 登记的「爬坡四段含影子而 R1 内无落点」。
  两者都属**升基线轮次**（MUST 1:1 行替换），本轮一并**不改基线文本**。

## 六、门控与计数

`cargo xtask check --profile full` **通过**（含前端门控：两包 typecheck/build、桌面包单测、`tests/ui`）。

| 口径 | W69 | W70 | 增量 |
|---|---|---|---|
| 测试目标数 | 91（73 + 18） | **91（73 + 18）** | **+0**（刻意：断言加在既有目标内） |
| 通过项数 | 853 | **858** | **+5** |

**两个增量互相对照**：`+5` 恰为本轮新增的 5 条断言（lib `2` + `tests/composition.rs` `12`，此前为 `0` + `9`）
⇒ 同时证明「目标数未误变」与「未误改别处」。

本机门控本轮先红了**两次**，均为环境而非代码：第一次是 `cargo fmt --all --check`（手写编辑破坏 100 列换行，
已 `cargo fmt --all` 修）；第二次是 `nai-engine` 四个目标同时 `LNK1104`（批量链接争用，
逐包可过 ⇒ 非代码缺陷）。按 W69 先例用 `--no-run` 预构建一轮即收敛，随后门控一次通过。**期间未改任何代码**。

## 七、CI 观测

对本次推送 `38387d4` **做一次低频查询**（判结论三件齐：唯一 `cb` 查询参数、`Cache-Control: no-cache`、回读 `head_sha` 等于本次推送的 SHA）：

| run | sha | 状态 | 结论 |
|---|---|---|---|
| `38207263895` | `38387d4` | completed | **success** |

步级：`foundation (ubuntu-24.04)` **completed / success**、`foundation (windows-latest)` **completed / success**，无失败步骤。

**这次绿说明什么、不说明什么（如实）**：

- 它说明**本次推送的完整门控在两个平台上都通过**，即缺口二（bring-up 预算）在 windows 上**未再复现**，
  且缺口一的改动**没有破坏** ubuntu 侧任何既有断言（含那条目标测试）。
- 它**不能**证明缺口一已被消除——它本来是**间歇性**的：W69 已实测到「同一棵树在两个平台上结论互换」
  （`a9f9aea` ubuntu 红 / windows 绿 → `87dce31` 恰好相反 → `190a0b7` 双绿）。**一次绿是必要条件，不是充分证据。**
- 因此本轮的立论**不依赖这次绿**，而是依赖**第二条事实链在源码上被消除**（§2.1：那条链是唯一可能的机械通道）。
  后续若该测试再红，届时应视为**新证据**并重开诊断，而不是视为本轮无效。

## 八、遗留与下一轮

- 本轮**不新增条目** ⇒ **不产生新的基线偏差**。
- 待办（轮次外，已登记）：**升基线轮次**需一并收录 B07 偏差、W67 的「影子在 R1 无落点」待确认项；
  **A12** 仍 `pending`（外部投入，ADR 0046 D3 明确拒绝为它引入 `tauri` crate）；
  **B01/O03/O04** 需外部输入；**O01/O02/O05** 需真实候选来源与生产接线。
