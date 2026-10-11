## Context

W44 把这条 ubuntu 间歇失败定位到断言行，并给出机制：

> `Engine::start` 在 engine 打印出控制地址后即返回，**测试随后才写 halt 文件**；而 supervision 线程
> 此时**可能已经把整个 media drill 跑完并提交了帧**。

engine 侧的拆除路径本身是**有**的：`DrillGuard::tick` 在每个轮询点检查 `stop`（graceful halt）与
`live != floor`（epoch 被撤销），二者任一成立就返回 `HALT`/`REVOKED`，`run_media_drill` 随即中止。
问题**不在** engine 缺少中止能力，而在**测试没能保证 halt 真的先到**：

- `run_media_drill` 里所有 `guard.tick`/`guard.wait` 都在**提交之前**，所以「只要 halt 被观察到，
  就不会记录提交」成立；
- 但「halt **一定**先于提交被观察到」并不成立——它取决于测试线程与 supervision 线程的调度先后。

因此断言「halt 先于提交」时，**必须先让这个前提为真**。可选做法有两类：

1. 给被测工作进程一个**确定性的 prewarm 阻塞点**；
2. 注入一个人为变慢的 render-worker。

本轮取 (1)：改动完全落在测试夹具内，不触碰生产二进制与监督语义。

## Goals / Non-Goals

- Goal：让 `halt_during_media_drill_prewarm_never_commits_a_frame` 的结论**由构造决定**，不再随调度变色。
- Goal：**不放宽任何断言**——「halt 之后不得记录提交」一字不改；决策链校验也保留。
- Goal：改动可被后人读懂：为什么这个素材是管道、为什么在写 halt 前要等 worker 出现。
- Non-Goal：不修 W44 §8.2 观测到的 **Windows 侧** `nai-runtime` 时序脆弱（另一处、另一类）。
- Non-Goal：不引入非确定性之外的任何行为变更；不改 `AVATAR_TIMEOUT`、不改监督线程的拆除顺序。

## Decisions

### 用「无写端的命名管道」作阻塞点，而不是注入慢 worker

render-worker 的 `run` 在 prewarm 阶段按序读三类输入：manifest、**各 `--asset` 素材**、PCM。
把**第一个素材**换成一个**没有写端**的命名管道后：

- worker 的 `fs::read(asset)` 在 `File::open` 上阻塞（`O_RDONLY`、非 `O_NONBLOCK`），
  于是它**停在 prewarm 里**，既不会写 `--ready` 标记，也**不可能**走到提交；
- engine 仍在 `guard.wait(ready …)` 中按 5ms 轮询并 `tick`，一旦 halt 到达即以 `HALT` 中止；
- engine 拆除时 `Supervisor::shutdown` 对 worker 发 `kill` 并 reap，阻塞中的 `open` 随进程终止而结束，
  **不留残留进程**。

选它而非「注入慢 worker」的理由：慢 worker 是把不确定性换成了**另一种不确定性**（慢多少才够？在更快的
机器上会不会又不够？），而管道把 worker **钉死在 prewarm**，与机器速度无关。

### 为什么 `--asset` 可以是管道、而 `--pcm` 不能

engine 侧**不读**素材内容（素材只作为 `--asset` 传给 worker）；engine 的 `frame_metadata` 只读 manifest 的
`output_width`/`output_height`。故素材换成管道不影响 engine 的既有步骤。

`--pcm` 则**不能**：engine 的 `pcm_samples` 对 PCM 取 `fs::metadata(..).len()`，而管道大小为 0，
会被判成 `PCM must hold whole f32 samples`，在派生 worker **之前**就失败——那是另一条路径，不是 prewarm。
（如实记下这个边界，免得后人误试。）

### 写 halt 之前必须等到 worker 真的出现

只有「worker 被派生」这一事实能排除一种**更隐蔽的假通过**：若 halt 来得太早（例如落在
`await_grant` 的 keeper 握手里），drill **根本到不了 prewarm**，worker 不会被派生，提交无从发生——
测试仍会通过，但**不再在测它名字里的那件事**。故先 `wait_for_worker_pid`（复用已有的 `/proc` 扫描手法，
与 `wait_for_keeper_pid` 同源），确认 worker 已进入 prewarm，再发 halt。

### 阻塞点不得把演练变成恒过

阻塞点只移除**时序赌博**，不改变「drill 本来会提交」这一事实——同一夹具（不带管道）在
`media_drill_produces_a_committed_frame_through_the_real_worker` 里**确实提交了帧**。
此外，被断言的可观测属性（`halt_and_wait().success()` + 决策日志无 `media_drill_committed` +
决策链完整）仍会在 engine 的 halt 路径失效时失败——`halt_and_wait` 有 15s 上限，
若 supervision 不因 halt 中止，drill 会卡在 prewarm 直到自身 15s 期限，engine 便无法在该上限内干净退出。

### 为什么不走 `libc` 依赖

`mkfifo` 用 `Command::new("mkfifo")` 调用，与同文件里已有的 `Command::new("kill")` 同构——
`kill` 与 `mkfifo` 同属 coreutils，在 CI 的 `ubuntu-24.04` 映像中必然存在。这样避免为一次测试
新增 `libc` 依赖并改动 `Cargo.lock`（离线缓存与 `--locked` 门控都要照顾）。

## Risks

- **本机无法验证**：被修测试与新增夹具都是 Linux-only，本机（Windows）**不编译**它们。
  本轮只能跑可移植门控证明「未误伤」，**成立与否必须由 CI 的 ubuntu job 观测**——这一点写进报告，
  不用「本机绿」冒充 Linux 侧结论（W44 §2 的教训）。
- **`mkfifo` 缺失**：若映像无 coreutils（不会有，但如实登记），测试会以具名 panic 失败，
  不会静默跳过。
- **管道阻塞形态变化**：若将来 worker 改了素材读取方式（例如加 `O_NONBLOCK` 或改读 PCM 在前），
  阻塞点会失效——届时该测试会退回为原来的时序敏感形态（可能又变间歇），
  故在夹具注释里写明它成立所依赖的前提。
- **`/proc` 扫描的固有竞争**：读 `/proc/<pid>/status` 时进程可能已消失，既有辅助函数已用
  `let Ok(..) else continue` 容忍；新增函数沿用同一写法。
