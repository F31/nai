# Design: W121 — Operation 有界预算的运行期终止语义闭世界守卫

## 缝（现测，不是推断）
基于代码实测（非摘要，`crates/nai-security/src/lib.rs:30-73`）：

```rust
pub struct Operation { deadline: Instant, cancelled: Arc<AtomicBool> }
impl Operation {
    pub const CEILING: Duration = Duration::from_secs(30);
    pub fn new(timeout: Duration) -> Result<Self, SecurityError> { if timeout > Self::CEILING { return Err(...); } ... }
    pub fn cancel(&self) { self.cancelled.store(true, Ordering::Release); }
    pub fn check(&self) -> Result<(), SecurityError> {
        if self.cancelled.load(Ordering::Acquire) { return Err(err(ErrorCode::Cancelled)); }
        if Instant::now() >= self.deadline { return Err(err(ErrorCode::DeadlineExceeded)); }
        Ok(())
    }
    pub fn stopped(&self) -> bool { self.check().is_err() }
    pub fn remaining(&self) -> Duration { self.deadline.saturating_duration_since(Instant::now()) }
}
```

| 不变量 | 位置 | 判据缺口 | 若被改动的后果 |
|---|---|---|---|
| `check()` 先 cancel 后 deadline | `lib.rs:58-66` | **零判据** | 主动取消被误读成超时 |
| `remaining()` 单调不增、超期为 0 | `lib.rs:70-72` | **零判据** | 重试预算失控 |
| `stopped()` ≡ `check().is_err()` | `lib.rs:67-69` | **零判据** | 判据与读取面分家，守卫自我满足 |
| `cancel()` 跨克隆共享 | `lib.rs:32` / `55-57` | **零判据** | 克隆调用链漏掉取消 |

**已钉住的不重复钉**：`ceiling_is_the_adr_0004_bound_and_the_rejection_threshold`（既有 in-src 测试）已覆盖
构造期（CEILING == 30s、new(CEILING) Ok、new(CEILING+1ms) Err），本轮**不重复**。

## 决策
1. **判定面闭合，零生产代码改动**：只补测试，不引入新执行体、不改 `lib.rs` 生产代码。
2. **优先序用「同时成立」的构造钉**：`Operation::new(Duration::ZERO)` 后立即 `sleep(20ms)` ⇒ deadline 已过，
   再 `cancel()` ⇒ 断言 `Cancelled`（而非 `DeadlineExceeded`）。两个条件**同时为真**，故断言真正钉住优先序，
   而非只钉「cancel 生效」。
3. **单调性断言严格变小**：先取 `remaining()`，再 `sleep(120ms)`，断言第二次**严格小于**第一次，
   并断言首次值 ≤ 初始预算（钉住「不超发」）。
4. **等价性断言逐情形核对**：`stopped()` 与 `check().is_err()` 在 live / cancelled / expired 三种情形下
   逐项相等（守卫不得自我满足）。
5. **共享性用克隆句柄钉**：在 `op.clone()` 上取 `stopped()`，由 `op.cancel()` 触发 ⇒ 断言克隆侧也 stopped。
6. **确定性离线**：无网络、无异步等待，纯 `Instant` 断言；本地复跑 `nai-security` 单测 9 passed。

## 明确不做
- 不改 `Operation` 的判定规则（不改优先序、不改预算取值、不新增取消入口）。
- 不断言真实墙钟精度（只钉「单调 / 优先序 / 一致 / 共享」这些可判事实）。
- 不碰介质级擦除、不建跨 memory↔remote 传播（与 W113–W120 一致）。
