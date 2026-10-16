# Proposal: W121 — Operation 有界预算的运行期终止语义闭世界守卫

## Why
W113–W120 已在 `nai-knowledge-core` 的 `remote.rs` 内完成一轮勘定。本轮**换 crate**，勘定安全层的
`Operation`（`nai-security/src/lib.rs:30-73`）——**所有带界操作共用的预算载体**。

既有测试只钉了**构造期**（`new(CEILING).is_ok()` 与越界 `+1ms` 被拒）。而**运行期终止语义整段零判据**：

- **(a) 终止优先序**：`check()` 先判 `cancelled`、再判 `deadline`（`lib.rs:58-66`）。于是「既取消又超期」
  的 Operation MUST 报 **`Cancelled`**，而非 `DeadlineExceeded`。两者的上层处置不同（主动取消 vs
  超时降级）；若优先序被交换，取消会被误读成超时，且**没有任何测试会发现**。
- **(b) 剩余预算单调不增**：`remaining()` = `deadline.saturating_duration_since(now)`（`lib.rs:70-72`）
  MUST 随时间流逝而**不增**，超期后为 0。若被改成「按等待补齐」之类，重试预算会失控。
- **(c) 判据与读取面一致**：`stopped()` = `check().is_err()`（`lib.rs:67-69`）MUST **严格一致**（守卫不得自我满足、
  读取面不得与判定面分家）。
- **(d) 取消跨句柄共享**：`cancelled: Arc<AtomicBool>` ⇒ 任一**克隆**句柄上的 `cancel()` MUST 让**全部**句柄都
  `stopped()`。若退化成每句柄独立 bool，克隆出来的调用链会漏掉取消。

此缝与 W113–W120 同形：判定面闭合、零生产改动、确定性离线。

## What Changes
只补可测判据，把上述运行期终止语义钉成显式闭世界守卫；**零生产代码改动**。

- `crates/nai-security/src/lib.rs` 的 `tests` 模块新增 4 项（W121，已本地跑绿）：
  - `check_reports_cancellation_before_the_deadline`（先 cancel 后 expiry ⇒ `Cancelled`）；
  - `remaining_never_grows_as_time_passes`（单调不增、超期为 0）；
  - `stopped_is_exactly_check_failure`（`stopped()` == `check().is_err()`）；
  - `cancellation_is_shared_across_cloned_handles`（克隆句柄共享取消）。
- `runtime-composition` spec 新增 4 条 Requirement（各 1 Scenario），把有界预算的终止语义纳入基线。

## Modified Capabilities
- `runtime-composition`（运行时装配与预算）：追加 `Operation` 有界预算的运行期终止语义（优先序 / 单调性 / 判定与读取一致 / 克隆共享）。

## Impact
- 无运行时行为变化（判定面闭合，不引入新执行体、不改 `lib.rs` 生产代码）。
- 无契约字段改动、无迁移、无 schema 改动。
- registry：守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。
- 测试计数：`nai-security` 单测由 5 增至 9（W121 +4）。
- spec：新增 4 条 Requirement（各 1 Scenario）。
