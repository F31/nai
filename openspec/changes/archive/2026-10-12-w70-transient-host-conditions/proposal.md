## Why

W69 收口时留下一条**指向本轮的待办**（`docs/reports/2026-10-12-w69-context-assembly.md` §4.3、§七）：
两次 push 的 `foundation` 工作流**都红**，但**红在不同平台、不同测试上**，而两次的提交**只差一个 `.md` 文件**。

| run | 提交 | ubuntu-24.04 | windows-latest | 失败内容 |
| --- | --- | --- | --- | --- |
| `38200252090` | `a9f9aea`（内容提交） | **failure** | success | `apps/engine/tests/engine.rs` 的 `killed_keeper_is_halted_by_the_engine_and_the_chain_survives`（`engine.rs:289`） |
| `38201171920` | `87dce31`（**仅改 `.md`**） | success | **failure** | `apps/engine/tests/pipeline.rs` 4 项：`provisioning store unavailable: storage: DEADLINE_EXCEEDED` |

**这两族不是新问题，而是 W48 明确留给单独一轮的遗留项**。W48 报告 §5 第 2 条原文：
「**Windows 侧 `nai-runtime` 的时序脆弱未修**（W44 §8.2：`DeadlineExceeded "operation stopped"` /
`PermissionDenied "scope permission missing"`）。那是**另一处、另一类**，需单独一轮。」
本轮就是那一轮。

### 实测缺口一：瞬态锁竞争会**永久终止监督循环**（可机械证实）

读码可证的链条，全部落在 `crates/nai-runtime/src/composition.rs` 与 `apps/engine/src/lib.rs`：

1. `ControlApi` 与装配根**共享同一个 `Arc<Mutex<Session>>`**
   （`apps/engine/src/lib.rs:314` `composition.session()` → `ControlApi::with_shared_session`）。
2. 两侧都用 **fail-fast** 取锁：`Composition::session_lock()`（`composition.rs:196`）与状态端点
   （`crates/nai-control-api/src/lib.rs:434`）都是 `try_lock()`，撞车即 `Conflict("session state busy")`。
3. 监督循环**每 tick（`DRILL_POLL` = 5 ms）**就调用 `live_epoch()`，而 `live_epoch`
   （`apps/engine/src/lib.rs:561`）用 `session_lock()` 取锁后**以 `?` 上抛**。
4. ⇒ 一次微秒级、**自我消除**的锁竞争，被 `DrillGuard::tick` 当作**致命错误**，经 `?` 传出闭包，
   监督循环**不再重试、直接结束**（`supervise` 在 `Err` 分支后即 `supervisor.shutdown()`）。

后果是安全性的：**循环一旦结束，此后 keeper 被强杀再也没有故障检测与撤权**。既有规格
`runtime-composition`「Engine supervision loop」的 Scenario *Renewal failure does not silently exit*
写的是「MUST NOT **在未撤权的情况下**退出监督循环」——实现只在**错误出口**里补一次 `halt`
（`lib.rs:547–549`）来满足它，而那次 `halt` 走的是**同一条 fail-fast 取锁**，因此
**「是否撤权」取决于又一轮锁竞争的运气**，而不是取决于设计。

### 实测缺口二：bring-up 期限没有按工作量标定（可机械证实）

`apps/engine/src/lib.rs:393` 与 `crates/nai-runtime/src/composition.rs:21`（`SHORT_OPERATION`）都用
**5 秒**的 `Operation` 期限，而这条操作**包含 `Store::open` 的全链 schema 迁移**
（`store.rs:142` → `migration::migrate`：6 个迁移脚本、每个一次 `BEGIN IMMEDIATE` + `COMMIT`，
且连接以 `synchronous=FULL` + WAL 打开），**首次打开全新数据根时全部 6 步都要跑**。

- 计数：`generate` 侧无关；迁移脚本数 = `migration.rs:6` 的 `SQL: [&str; 6]`。
- 同仓**同类工作**的取值：`crates/nai-storage/src/tests.rs:10`、`crates/nai-upgrade-core/tests/*.rs`
  全部用 **30 秒**；`crates/nai-provider-core/examples/openai_smoke.rs` 用 30 秒。
- 界：**ADR 0004 第 13 行**「Operation 用本地单调期限和取消标记，**最长 30s**」；
  **ADR 0038 第 76 行**「`Operation::new` 拒绝超过 30 秒的操作（**安全层不变量**）」。

⇒ 5 秒是一个**没有任何文档依据、且小于同仓同类工作取值**的局部常量；它的前提
（「迁移 5 秒内一定跑完」）从未被标定，且在加载中的 runner 上被证伪。

## What Changes

只做**两处**改动，都落在这条判据上：**宿主的瞬态条件（锁竞争、慢盘）不得升级为安全结论或不可判定的失败。**

### 一、瞬态锁竞争不得夺走安全决定

- `crates/nai-runtime/src/composition.rs`：新增**有界等待**取锁，并区分两个预算——
  - `SESSION_REVOKE_BUDGET`（250 ms）：用于**撤权与故障处置**路径（`observe`/`control`/`enforce_duty`）。
    微秒级竞争在本预算内被等待化解；**预算耗尽仍返回显式 `Conflict`**（不是静默跳过）。
  - `SESSION_POLL_BUDGET`（5 ms，= 一个 tick）：用于 **epoch 活性检查**。预算耗尽返回
    `Ok(None)`（「**本 tick 未观测到**」），**MUST NOT** 被当成故障——5 ms 后就是下一个 tick。
- 新增 `Composition::live_epoch() -> Result<Option<u64>>`：`Ok(Some(e))` / `Ok(None)`（竞争）/
  `Err`（仅锁中毒）。
- `apps/engine/src/lib.rs`：`tick` 的 epoch 检查改为「**未观测到变化则跳过本 tick，且故障 `observe` 仍执行**」；
  监督循环的终止只允许来自 `HALT`/`REVOKED`/真实故障。
  `supervise` 的错误出口改用 `.ok().flatten().unwrap_or(floor)`。

**这一改动的语义是收紧而不是放松**：今天「读不到 epoch」会**结束监督**；改后「读不到 epoch」只是**本 tick 不做该判定**，
而**故障处置照跑**。

### 二、一次性 bring-up 按其工作量标定预算

- `crates/nai-runtime/src/composition.rs`：新增 `pub const BRINGUP_OPERATION: Duration = 30 s`
  （= ADR 0004 的上界），**只**用于包含 schema 迁移的 bring-up：`Composition::prepare` 的 `Store::open`；
  **逐次调用的 `SHORT_OPERATION` 保持 5 s 不变**（读取类操作不含迁移，与本条判据一致）。
- `apps/engine/src/lib.rs`：`--provision` 改用同一常量（**一处定义**，不新建第二份策略），
  并让 `store open` 失败的文案**指名步骤与已耗时**，使下一次红可判定，而不是只给一个 `DEADLINE_EXCEEDED`。

### 明确不做

- **不改任何断言、不放宽任何测试窗口、不加睡眠掩盖、不改 CI workflow。**
- **不通过降低测试并发（`--test-threads`）来回避**：那是把宿主的调度假设搬进夹具，与 W48 §4 的立场相反；
  本轮修的是**代码对瞬态条件的处理**。
- **不修 `apps/engine/tests/*`**：两处夹具的断言均**未动**。

### Modified Capabilities

- `runtime-composition`（**修改 2 条既有需求 + 新增 1 条**）：
  ①*Shared session between control surface and composition* 补上「撤权路径有界重试、轮询读取不得把竞争当故障」
  与两个新场景；②*Engine supervision loop* 补上「MUST NOT 因瞬态锁竞争终止」与新场景；
  ③新增 *Bring-up operations are budgeted for the work they include*。

## Impact

- `crates/nai-runtime/src/composition.rs`（实现 + `#[cfg(test)]` 单元测试钉住三个常量）。
- `crates/nai-runtime/tests/composition.rs`（**在既有测试目标内**加两条：持锁时 `control` 仍落地、
  持锁时 `live_epoch` 报「未观测到」而非冲突——**不新增测试目标**）。
- `apps/engine/src/lib.rs`（`live_epoch` 语义、`tick` 分支、`supervise` 错误出口、`--provision` 预算与文案）。
- `openspec/specs/runtime-composition/spec.md`（归档时写入）。
- 文档：`docs/kernel.md`、`docs/development-plan.md`、`AGENTS.md`、`docs/reports/2026-10-12-w70-*.md`。
- `tests/registry.yaml`：**不新增条目**（本轮是既有需求的实现加固，不新增验收家族成员）
  ⇒ §48.5 家族索引与 §57.4 计数行**均不需订正**，**不产生新的基线偏差**。
- **零新增依赖、零新增契约名（§46.5）、零前端改动、workspace 仍 19 包、不改基线文本。**

## 边界与如实披露

1. **family 1（`engine.rs`）在 CI 上观察到的那次抖动，其触发时点未能从现有证据还原。**
   本轮修的是「**这条路径存在且可证**」，不是「已复现并定位了那一次」。
   `apps/engine/tests/engine.rs` 是 `#[cfg(target_os = "linux")]`，**本机（Windows）不编译**，
   本机门控**不能**证明修法在 Linux 上成立——成立与否**以 CI 的 ubuntu job 结论为准**（照 W48 §3.2 的纪律）。
2. **family 2 的机制是机械可证的**（5 s 覆盖 6 步迁移），修法是把预算按工作量标定到 ADR 0004 的上界。
3. 本轮**提高的是这两条路径对宿主瞬态的容忍度**；它**不**声称 CI 从此不再有间歇失败——
   若仍有失败，新文案（步骤 + 耗时）会把它变成可判定的下一组判据。
