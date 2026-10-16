# W121 报告：Operation 有界预算的运行期终止语义闭世界守卫

日期：2026-10-16｜归属：换 crate 勘定候选（安全层 `Operation` 的**运行期**终止语义）｜零生产代码改动

## 一、裁定与缝（现测，不是推断）

W113–W120 已在 `nai-knowledge-core` 的 `remote.rs` 内完成一轮勘定。本轮**换 crate**，勘定安全层的
`Operation`（`nai-security/src/lib.rs:30-73`）——**所有带界操作共用的预算载体**。

既有测试只钉了**构造期**（`ceiling_is_the_adr_0004_bound_and_the_rejection_threshold` 已覆盖
`CEILING == 30s`、`new(CEILING).is_ok()`、`new(CEILING + 1ms)` 被拒）。而**运行期终止语义整段零判据**：

| 不变量 | 位置 | 判据缺口 | 若被改动的后果 |
|---|---|---|---|
| `check()` 先 cancel 后 deadline | `lib.rs:58-66` | **零判据** | 主动取消被误读成超时 |
| `remaining()` 单调不增、超期为 0 | `lib.rs:70-72` | **零判据** | 重试预算失控 |
| `stopped()` ≡ `check().is_err()` | `lib.rs:67-69` | **零判据** | 判据与读取面分家 |
| `cancel()` 跨克隆共享 | `lib.rs:32` / `55-57` | **零判据** | 克隆调用链漏掉取消 |

**裁定**：判定面闭合，零生产代码改动——只把上述运行期语义变成显式守卫，与 W113–W120 同形。
**不重复钉已钉住的**：构造期边界由既有 in-src 测试覆盖，本轮只补运行期。

## 二、判据落点（`crates/nai-security/src/lib.rs` 的 tests 模块）

- **新增 1**：`check_reports_cancellation_before_the_deadline` —— 用 `Operation::new(Duration::ZERO)` 后
  `sleep(20ms)` 构造「既取消又超期」⇒ 断言 `Cancelled`。**判别性**：两条件同时为真，若优先序被交换则
  返回 `DeadlineExceeded` ⇒ 变红。
- **新增 2**：`remaining_never_grows_as_time_passes` —— 两次取值间 `sleep(120ms)` ⇒ 断言第二次
  **严格小于**第一次，且首次值 ≤ 初始预算。
- **新增 3**：`stopped_is_exactly_check_failure` —— 在 live / cancelled / expired 三种情形下逐项核对
  `stopped()` 与 `check().is_err()` 相等。
- **新增 4**：`cancellation_is_shared_across_cloned_handles` —— `op.cancel()` ⇒ 断言克隆侧也 `stopped()`。

## 三、计数（现测，非推断）

`nai-security` in-src 单测：**5 → 9**（+4 = 全为新增，W121 四条均未曾被任何测试覆盖）。
本地复跑：`running 9 tests ... test result: ok. 9 passed; 0 failed`。全仓权威总数见 §六。

## 四、如实披露（判定面闭合 ≠ 验收达标）

- 只钉可判事实（优先序 / 单调 / 一致 / 共享），**不断言真实墙钟精度**（宿主机计时抖动属宿主面）。
- 新增 2 含 `sleep(120ms)` 等时间量，属「单调性」而非「精确时长」断言 ⇒ 不受正常调度抖动影响。
- 不断言跨进程 / 重启后的取消语义（`Arc<AtomicBool>` 仅进程内共享）。
- 介质级擦除、跨 memory↔remote 传播仍明确不做（与 W113–W120 一致）。
- registry：守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。

## 五、spec 增量（`runtime-composition`）

新增 4 条 Requirement（各 1 Scenario），把有界预算的终止语义纳入基线：
- **Operation check reports cancellation before the deadline**（A cancelled and expired operation reports cancellation）；
- **The remaining budget never grows as time passes**（Waiting consumes the remaining budget）；
- **stopped is exactly check failure**（Agreement holds in live, cancelled and expired states）；
- **Cancellation is shared across cloned operation handles**（Cancelling through one handle stops every clone）。

## 六、门控（全量 `cargo xtask check --profile full`）

- `GATE_EXIT`：**0**（后台 `target-check/gate-w121.sh`，`PREBUILD_OK at attempt 14` + `GATE_OK at attempt 2`）。`rustc 1.96.0 (ac68faa20 2026-05-25)`。
  - ⚠️ **本次门控耗时 56 分钟**（远超常态 ~7 分钟）：预构建阶段连续 13 次 LNK1104（Defender 持锁），门控阶段第 1 次再中 1 次，第 2 次即绿。全过程**无** `test result: FAILED`（grep 计数 = 0）；`error: linking` 14 条与 `error: process didn't exit` 1 条**全部来自被重试掉的那次链接瞬态**，非代码缺陷 ⇒ 按纪律**不修代码**，仅靠重试消化。
- **LNK1104 瞬态计数**：**14**（grep `LNK1104` = 28 行，因每次链接失败同时打印 `error: linking ... exit code: 1104` 与 `LINK : fatal error LNK1104 ...` 两行 ⇒ 14 次事件）。
- 全 workspace `cargo xtask check --profile full`：**95** 个测试二进制全 `test result: ok. ... 0 failed`，**真实测试失败 0**；clippy `--workspace --all-targets --locked -- -D warnings` 通过（否则 `GATE_OK` 不成立）；`cargo fmt --all --check` 通过；前端门控（`[frontend] 全部步骤通过`）通过；`tests/registry.test.mjs` 通过。其中 `nai-security` in-src 单测 **9 passed; 0 failed**（含 W121 四条新增，见 §三）。

## 七、提交 / 推送（归档 + 一次 commit + push，SSH 443）

- 提交 SHA：TBD（主提交，归档后回填）。
- push 区间：TBD，分支 `main`，SSH 443（`ssh.github.com:443` + `StrictHostKeyChecking=accept-new`）。
- 改动文件：`crates/nai-security/src/lib.rs`（+4 测试）、`docs/adr/0099-operation-budget-closed-world.md`（新建）、`docs/reports/2026-10-16-w121-operation-budget-closed-world.md`（新建）、`docs/development-plan.md`（W121 段）、`docs/knowledge.md`（订正注记）、`openspec/specs/runtime-composition/spec.md`（合入 4 条 Requirement）、`openspec/changes/archive/2026-10-16-w121-operation-budget-closed-world/`（归档目录）。
- openspec 三连：`validate w121-operation-budget-closed-world --strict` → valid；`archive --yes` → 4 条 Requirement 合入基线（`+ 4, ~ 0, - 0`）；`validate --archived --strict` → 全绿（**119 项，含 w121**，0 failed）。归档目录按**系统日期戳**记为 `2026-10-16-...`。`target-check/` 按纪律排除暂存。

## 八、CI 观测（主提交 + 回填提交各一次双平台观测）

- **主提交** run id：TBD（workflow `Engineering foundation`）。回读 `head_sha` 与推送 SHA **完整 40 位一致**；双平台 `foundation` job 均 `completed / success`。若失败，按 W117/W120 的五条瞬态判据诊断（失败集合漂移 / 相关包 git status 空 / 本机全绿 / 另一平台 success），并经 check-run 注解取真实错误。
- **回填提交**（docs-only）自身 CI：按纪律做 **1 次**双平台观测（先例 run 109 failure ⇒ 不观测就是赌）。**本报告不为回填提交写 §九**；其自身结论按「**回填提交自身不回填**」记入 memory，不在本报告占位（W119 起的写法）。
