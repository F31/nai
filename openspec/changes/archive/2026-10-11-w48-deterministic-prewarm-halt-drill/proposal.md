## Why

`apps/engine/tests/engine.rs` 的 `halt_during_media_drill_prewarm_never_commits_a_frame` 是 W42 登记、
W44 定位到**断言行**的 ubuntu 间歇失败：

```
thread '…' panicked at apps/engine/tests/engine.rs:446:5:
a halt before the worker commits must not log a committed frame: [DecisionEntryView { seq: 1, …, "event":"armed" }, …]
```

W44 已把根因类别定死（**不是 hang**，而是「halt 之后决策日志里仍出现了 `media_drill_committed`」），
并如实登记「修法未做」。本轮修它。

**机制**：`Engine::start` 在 engine 打印出控制地址后即返回，**测试随后才写 halt 文件**；而 supervision 线程
此时**可能已经把整个 media drill 跑完并提交了帧**。也就是说，断言
「halt 先于提交 ⇒ 不得记录提交」所依赖的那个**先后顺序，是靠墙钟赌出来的**，而不是被构造出来的。
CI 上的调度差异让这个时序时快时慢 ⇒ 间歇性。同一份 Rust 代码在 `ca47168` 红、只改文档的 `064d754` 绿
（W44 §8.2），是这条非确定性最干净的证据。

**断言本身是对的**（halt 之后不得再出现新提交），**错的是测试建立这个前提的方式**。
故本轮**不放宽断言**，而是让前提成立：给被测的 render-worker 一个**确定性的 prewarm 阻塞点**，
使「halt 到达时 worker 仍在 prewarm」成为构造性事实。

## What Changes

- `apps/engine/tests/common/mod.rs`：新增 Linux 专用夹具 `park_asset_on_fifo`——把某个已声明的 avatar 素材
  换成**没有写端的命名管道**。render-worker 在 prewarm 中按序读取素材，第一次读就**阻塞**在管道上，
  于是它**不可能**在引擎把它拆掉之前走到提交。
- `apps/engine/tests/engine.rs`：
  - 把 `wait_for_keeper_pid` 里的 `/proc` 扫描抽成 `wait_for_supervised_pid(parent, timeout, exe, marker)`，
    保留 `wait_for_keeper_pid` 原语义，并新增 `wait_for_worker_pid`；
  - `halt_during_media_drill_prewarm_never_commits_a_frame`：先 `park_asset_on_fifo(asset)`，
    再**等到 render-worker 进程真的被派生出来**（`wait_for_worker_pid`）才写 halt 文件。
    此时 worker 已进入 prewarm 且被阻塞，halt 必然先于（不可能的）提交被观察到。
  - 断言与决策链校验**一字不改**。
- 不做：不改 `apps/engine/src/`、不改 `nai-runtime` 监督语义、不加依赖、不改既有验收 `status`。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `engineering-foundation`：新增一条需求——**断言因果顺序的演练 SHALL 由构造保证该顺序，MUST NOT 依赖墙钟时序**；
  且该演练在被测的控制路径失效时 **MUST** 仍失败（阻塞点不得把演练变成恒过）。

## Impact

- 修改：`apps/engine/tests/common/mod.rs`、`apps/engine/tests/engine.rs`、
  `openspec/specs/engineering-foundation/spec.md`（经归档吸收）、报告与 OpenSpec 变更文档。
- **零 Rust 源码改动（`src/` 与 `crates/` 均不动）、零新依赖、零 workspace 包增减**；CI workflow 不改。
- **平台边界**：被修的演练与新增夹具都是 `#[cfg(target_os = "linux")]`，**Windows 上不编译**；
  故本机门控只能证明「未误伤可移植目标」，**不能**证明修法在 Linux 上成立——这一点如实披露。
- **验证方式**：本项目门控（`check-deps` / `check --registry` / `check --profile portable` /
  `validate --all --strict --archived`）在本机跑；**被修测试的成立与否只能经 CI 的 ubuntu job 观测**
  （`F31/nai` public，`GET /actions/runs` 与 `GET /check-runs/{id}/annotations` 匿名可读）。
- **不做**：不改其余 Linux 演练；不动 `AVATAR_TIMEOUT` 等生产超时；不引入 `act`/Docker/
  `libc` 依赖（`mkfifo` 走既有 `Command` 先例，与测试中已有的 `kill` 同构）。
