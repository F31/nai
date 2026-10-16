# Design: W126 — 升级下载阶段的续传、重试预算与哈希闸门

## 接缝与判定分工

| 世界事实 | 接缝 | 本轮替身 / 真实验证 |
|---|---|---|
| 一个窗口的字节到不到 | `PackageFetcher::fetch` | `ScriptedFetcher`（脚本）+ 真实 loopback `HttpFetcher` |
| 本地已持久化多少 | `ResumeLedger::{durable_bytes, append, assembled, discard}` | `MemoryLedger`（含「吞掉 append」的违约变体） |
| 允许重问几次 | `FetchPolicy::max_attempts_per_window` | 默认 3，边界判据显式给 1/2/3 |

**记录 ≠ 决策**：core 只决定「向哪一段要、要不要重问、何时按名放弃」；它不持有传输。

## 关键决策与备选

### 1. 短读是值，`Err` 是终局
`Fetched.bytes` 允许短于窗口——断流是可存活常态，不该被表示为错误。于是重问的**归属**留在 core。
反面：把断流表示为 `Err`，就得按错误码分「可重问 / 不可重问」，那等于让传输回答「你会不会成功」——传输不知道。
**终局错误不重问**也是刻意的：`PermissionDenied` 再问一次只是烧预算。

### 2. 偏移每次从台账读回，而不是由编排器记
判据 `a_ledger_that_does_not_persist_is_a_named_failure` 直接钉这一点：`append` 报告成功但不落盘时，
下载必须**具名失败**。若偏移来自本地变量，这个注入会让「进度」变成幻觉而一切照旧绿。
同理，哈希闸门的输入是 `ledger.assembled(path)`：判据 `an_artifact_already_durable_is_verified_without_moving_a_byte`
在**一个窗口都没发**的情况下仍断言 4096 字节通过校验——那些字节只能来自台账。

### 3. 完整但哈希不符 ⇒ 拒绝并**先丢弃**
备选是「重取一次」。否决理由：损坏可能来自**源**而不是链路，重取会无限重复同一件坏事；
更要紧的是**续传点会被毒化**——坏前缀若留在介质上，下一次运行会从它的末尾续传，而那个位置永久不会被重取
（重启也带不走）。拒绝 + 丢弃是安全方向；判据 `a_discarded_corrupt_artifact_is_refetched_from_zero`
保证下一次从 0 开始。判定词沿用校验链的 `package content hash mismatch`：**一条校验链，不是两条**。

### 4. 声明的上界在**动一个字节之前**应用
`bound_declared` 用与 `verify_package` 相同的两个判定词（`package file count limit` /
`package content byte limit`），但**不是同一条规则**：那条约束「实际到达」，这条约束「即将索要」。
共享函数等于假装两个不同输入是一件事，故分开；判据 `the_declared_bounds_are_applied_before_any_window`
钉的是「零请求」这一半。

### 5. 停止信号：入口一条 `checkpoint` + 循环内 `op.check()`
本轮把「bound 之后」那条重复的 `checkpoint` 删掉了——它与入口那条在同一条直线路径上、
可观测差异为空，属于「同一件事两处定义」。留下的两条各有判据：
入口那条由 `the_callers_stop_signals_are_honoured_before_the_first_window`（调用方 cancel 标志 / 过期 deadline），
循环内那条由 `a_cancellation_arriving_mid_transfer_stops_the_loop`——后者用**取到窗口就 cancel 的传输**构造，
所以停止信号到达的时机是构造事实，不依赖墙钟。

## 真实 socket 的那两条判据
`tests/download.rs::loopback` 里是测试自写的最小 HTTP/1.1 peer 与 client：

- peer 按 `Range: bytes=N-` 返回 `206`，并**可声明完整的 `Content-Length` 却只送一部分**（断流）；
  它把收到的每个 `Range` 偏移**记账**下来。
- client 每个窗口发一次真实 `GET`，返回**线上实际到达**的字节（可能短）。
- `an_interrupted_transfer_over_a_real_socket_resumes_and_verifies` 断言记账 == `[0, 1024]`
  ⇒ 「偏移来自台账」这件事是在**线上**被证实的，不是在脚本里被声明的。
- `a_source_that_keeps_dropping_the_socket_is_refused_by_name` 断言记账 == `[0, 1, 2]`
  ⇒ 每次尝试都只多持久化 1 字节，重问预算同时管住次数与进度。

**为什么不引 HTTP 客户端依赖**：`loopback.rs` 的注释与 `offline.rs` 的论证是同一件事——
「不会偷偷下载」在本 crate 是**编译期性质**（无 HTTP 客户端、否决 `allow_fetch` 开关）。
连 dev-dependency 都不加，这句话就仍然逐字成立。代价如实披露：这个 peer 是测试自写的，
它证明的是**本仓库的下载编排对真实 socket 的行为**，不是与第三方服务器实现的互通。

## 如实披露（非验收达标）
- 不断言真实分发端点、真实 CDN、真实磁盘/GPU 度量：那是 `R09` 的剩余缺口，属外部注入点。
- 不断言吞吐或时序；判据只钉「向哪一段要、要了几次、什么时候放弃、放弃成什么」这类可判事实。
- 抓取阶段的 `assembled()` 把整个工件取回内存与既有 `verify_package` 同形（后者本就收 `&[u8]`）；
  大工件下的流式校验不是本轮范围。
