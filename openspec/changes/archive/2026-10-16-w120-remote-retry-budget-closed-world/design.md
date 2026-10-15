# Design: W120 — post_json 重试预算与 read_body 读取上限闭世界守卫

## 缝（现测，不是推断）
基于代码实测（非摘要，`crates/nai-knowledge-core/src/remote.rs`）：

| 分支 | 位置 | 判据缺口 | 若被移除的后果 |
|---|---|---|---|
| `backoff >= op.remaining()` ⇒ `Unavailable` | `remote.rs:232-234` | 既有 429 用例只用 `Retry-After: 0`（退避 0s）⇒ **零判据** | 硬等到 `op` 到期，结果从 `Unavailable` 变 **`Timeout`** |
| `attempt >= max_retries` ⇒ `Unavailable` | `remote.rs:220-223` | 既有重试用例在耗尽前已成功 ⇒ **零判据** | 退避 0 时**无限重试**直至 `op` 到期 |
| 5xx 走指数退避、**忽略** `Retry-After` | `remote.rs:225-231` | 既有测试无法区分两者 ⇒ **零判据** | 5xx 用 `Retry-After: 100` 会立刻撞上上一行 ⇒ **不再重试** |
| 响应体 > 8 MiB ⇒ `Invalid` | `remote.rs:133-135` | 从未被触碰 ⇒ **零判据** | 响应体**无上限缓冲** |

## 决策
1. **判定面闭合，零生产代码改动**：只补测试，不引入新执行体、不改 `remote.rs`。
2. **每条分支都用「判别性构造」**，使断言在分支被移除时**必然变红**（而非只是记录现状）：
   - (a) 429 + `Retry-After: 100` + `op = 1s` ⇒ 正确实现立即 `Unavailable` 且耗时 <900ms；
     若分支被移除则 `sleep_until` 等到 `op` 到期 ⇒ `RetrievalTimeout` 且耗时 ~1s ⇒ **断言变红**。
   - (b) 持续 429 + `max_retries = 1`（退避 0）⇒ 正确实现 2 次尝试后 `Unavailable`；
     若分支被移除则无限重试（退避 0）直至 `op` 到期 ⇒ **`Timeout` 而非 `Unavailable`** ⇒ **断言变红**。
   - (c) 503 **故意带** `Retry-After: 100` + `op = 30s` ⇒ 正确实现（200ms 指数退避）**会重试并成功**
     （2 次尝试、`Complete`）；若误用 `Retry-After` 则 100s 退避撞上 (a) ⇒ `Unavailable` 且仅 1 次尝试
     ⇒ **断言变红**。这是本轮最能体现「判别性」的一条。
   - (d) 回包 8 MiB + 1（经合法字段 `evidence[0].text` 撑大，保证**是合法回包**而非畸形 JSON）⇒ 正确实现
     `Err(RemoteError::Invalid)`；若上限被移除则正常解析 ⇒ 得到 `Complete`/`Ok` ⇒ **断言变红**。
     ⚠️ 用合法回包很关键：畸形 JSON 也会返回 `Invalid`，那样就无法区分「读到大包被拒」与「解析失败」。
3. **尝试次数要显式清点**：(b)(c) 都断言 `attempts` 精确值，把「重试**有界**」与「确实重试了」两个方向都钉住
   （只断言最终状态会漏掉次数维度）。
4. **确定性离线**：Mock 为本地 `127.0.0.1:0`；退避最大 200ms，8 MiB 回包实测 1.33s 内完成；
   本地复跑 `running 40 tests ... 40 passed; 0 failed`。

## 明确不做
- 不改 `remote.rs` 任何判定规则（不改退避公式、不改上限、不新增重试）。
- 不断言真实网络下的退避时序（只钉「是否重试 / 何时放弃 / 放弃成什么」这类可判事实）。
- 不断言 `Retry-After` 的语义正确性（远端语义属远端责任；本轮只钉**本客户端选用哪个退避来源**）。
- 不碰介质级擦除、不建跨 memory↔remote 传播（与 W113–W119 一致）。
