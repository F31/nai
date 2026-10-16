# ADR 0099：Operation 有界预算的运行期终止语义闭世界守卫

- 状态：Accepted
- 日期：2026-10-16
- 归属：W121（**换 crate** 勘定：安全层 `Operation` 的运行期终止语义判定面缺口）
- 关联：ADR 0098（§post_json 重试预算与 read_body 上限）、ADR 0097（§出站请求构造）、ADR 0096（§parse_reply 的 RemoteStatus 映射）、ADR 0004（上界外部事实）

## 背景
W113–W120 已完成 `nai-knowledge-core` 中 `remote.rs` 的一轮勘定。本轮换 crate，勘定安全层的
`Operation`（`nai-security/src/lib.rs:30-73`）——**所有带界操作共用的预算载体**。

既有测试只钉了**构造期**（`ceiling_is_the_adr_0004_bound_and_the_rejection_threshold` 已覆盖
`CEILING == 30s`、`new(CEILING).is_ok()`、`new(CEILING + 1ms)` 被拒）。而**运行期终止语义整段零判据**：

| 不变量 | 位置 | 缺口 | 若被改动的后果 |
|---|---|---|---|
| `check()` 先 cancel 后 deadline | `lib.rs:58-66` | **零判据** | 主动取消被误读成超时 |
| `remaining()` 单调不增、超期为 0 | `lib.rs:70-72` | **零判据** | 重试预算失控 |
| `stopped()` ≡ `check().is_err()` | `lib.rs:67-69` | **零判据** | 判据与读取面分家 |
| `cancel()` 跨克隆共享 | `lib.rs:32` / `55-57` | **零判据** | 克隆调用链漏掉取消 |

## 决策
**判定面闭合，零生产代码改动**。只补可测判据，把上述运行期终止语义钉成显式闭世界守卫：

1. **优先序用「同时成立」的构造钉**：`Operation::new(Duration::ZERO)` 后 `sleep(20ms)` ⇒ deadline 已过，再
   `cancel()` ⇒ 断言 `Cancelled`（而非 `DeadlineExceeded`）。两个条件**同时为真**，故断言真正钉住优先序，
   而非只钉「cancel 生效」。
2. **单调性断言严格变小**：先取 `remaining()`，再 `sleep(120ms)`，断言第二次**严格小于**第一次，
   并断言首次值 ≤ 初始预算（钉住「不超发」）。
3. **等价性逐情形核对**：`stopped()` 与 `check().is_err()` 在 live / cancelled / expired 三种情形下
   逐项相等（守卫不得自我满足）。
4. **共享性用克隆句柄钉**：在 `op.clone()` 上取 `stopped()`，由 `op.cancel()` 触发 ⇒ 断言克隆侧也 stopped。
5. **确定性离线**：无网络、无异步等待，纯 `Instant` 断言；本地复跑 `nai-security` 单测 9 passed。
6. **不重复钉已钉住的**：构造期边界由既有 in-src 测试覆盖，本轮只补运行期。

## 后果
- 正向：`Operation` 的运行期终止语义（优先序 / 单调性 / 判读一致 / 克隆共享）从零判据变为被判别性断言钉住；
  任一被改都会红，且不会与既有构造期守卫重叠。
- 零运行时变化、零契约字段改动、零迁移、零 schema 改动。
- 如实披露（非验收达标）：
  - 只钉可判事实（优先序 / 单调 / 一致 / 共享），**不断言真实墙钟精度**（宿主机计时抖动属宿主面）。
  - 断言含 `sleep(120ms)` 等时间量，属「单调性」而非「精确时长」断言，故不受正常调度抖动影响。
  - 不断言跨进程 / 重启后的取消语义（`Arc<AtomicBool>` 仅进程内共享）。
  - registry：守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。
