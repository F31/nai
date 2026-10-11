# W48：给那条 ubuntu 间歇竞态一个确定性的 prewarm 阻塞点

- 变更：`openspec/changes/w48-deterministic-prewarm-halt-drill`（归档后为 `openspec/changes/archive/2026-10-11-w48-deterministic-prewarm-halt-drill/`）
- 前置：W44（`docs/reports/2026-10-11-w44-ci-diagnose-signal-to-noise.md`）
- 性质：**测试确定性**修复。零 Rust 源码改动（`src/`、`crates/` 均不动）、零新依赖、零 workspace 包增减、
  **不放宽任何断言**、**不改任何验收 `status`**、CI workflow 不改。

## 1. 修的是「测试建立前提的方式」，不是断言

被修的断言**原样保留**：

```rust
assert!(
    !entries
        .iter()
        .any(|entry| entry.payload_json.contains("media_drill_committed")),
    "a halt before the worker commits must not log a committed frame: {entries:?}"
);
```

它测的是「halt 先于提交 ⇒ 不得记录提交」。**engine 的拆除路径本来就是完整的**：`DrillGuard::tick`
在每个轮询点检查 graceful halt（`stop`）与 epoch 撤销（`live != floor`），二者任一成立就返回
`HALT`/`REVOKED`，`run_media_drill` 随即中止；而它的所有 tick/wait 都在提交**之前**。
所以「**只要** halt 被观察到，就不会记录提交」成立。

**不成立的是「halt 一定先于提交被观察到」**：`Engine::start` 在 engine 打印出控制地址后即返回，
**测试随后才写 halt 文件**，而 supervision 线程此时**可能已把整个 drill 跑完并提交了帧**。
W44 §8.1 已由 CI 注解把机制定到这一条，并给出精确位置——`panicked at apps/engine/tests/engine.rs:446:5`。

⇒ **断言对、前提错。修前提，不动断言。**

## 2. 改动

| 文件 | 改动 | 为什么 |
| --- | --- | --- |
| `apps/engine/tests/common/mod.rs` | 新增 `park_asset_on_fifo`（Linux-only）：把某个已声明 avatar 素材换成**无写端命名管道** | render-worker 在 prewarm 按序读素材，`File::open`（`O_RDONLY`、非 `O_NONBLOCK`）**阻塞**，于是它**不可能**走到提交 |
| `apps/engine/tests/engine.rs` | 把 `wait_for_keeper_pid` 的 `/proc` 扫描抽成 `wait_for_supervised_pid(parent, timeout, exe, marker)`，保留原语义并新增 `wait_for_worker_pid` | 需要在发 halt 前确认 worker **确已进入 prewarm** |
| 同上 | `halt_during_media_drill_prewarm_never_commits_a_frame`：先 `park_asset_on_fifo(首个素材)`，再 `wait_for_worker_pid`，**才**写 halt | 让「halt 发生在 prewarm 期间」成为构造性事实；断言与决策链校验一字不改 |

渲染侧链条（`apps/render-worker/src/lib.rs::produce`）已核对：**manifest → 各 `--asset` 素材 → PCM → admit → 写 `--ready`**，
故阻塞点确定落在 **prewarm 之内、`--ready` 之前**；engine 的 `guard.wait(ready …)` 因此永不满足，
drill 停在 prewarm，直到 halt 把它拆掉。

### 2.1 为什么必须「先等 worker 出现」

只有「worker 被派生」这一事实能排除一种**更隐蔽的假通过**：若 halt 来得太早（例如落在 `supervise` 的
`await_grant` keeper 握手里），drill **根本到不了 prewarm**，worker 不会被派生，提交无从发生——测试仍会通过，
但**不再在测它名字里的那件事**。故必须先观察到该前置阶段确已进入，再发 halt。

### 2.2 为什么阻塞点不是「恒过」

- **同夹具（不带管道）确实会提交**：`media_drill_produces_a_committed_frame_through_the_real_worker`
  用的就是同一套素材/PCM/avatar 闭包，且实测提交成功。阻塞点移除的是**时序赌博**，不是提交能力。
- **可观测属性仍有牙齿**：若 engine 的 halt 路径失效（不在 halt 后中止 drill），drill 会卡在 prewarm 直到
  **自身的 15s prewarm 期限**，而 `halt_and_wait` 也是 15s ⇒ engine 无法在该上限内干净退出，
  `engine.halt_and_wait().success()` 就会失败。断言链并未因为阻塞点而免疫于回归。

### 2.3 记下一个边界：`--pcm` 不能当阻塞点

engine 的 `pcm_samples` 对 PCM 取 `fs::metadata(..).len()`，而**管道大小为 0**，会被判成
`PCM must hold whole f32 samples`——在派生 worker **之前**就失败。那是另一条路径，**不是** prewarm。
故阻塞点只能落在 **manifest 之外的素材**上（engine 侧不读素材内容：素材只作为 `--asset` 传给 worker，
`frame_metadata` 只读 manifest 的 `output_width`/`output_height`）。

## 3. 验证

### 3.1 本机能做的（全部实测）

| 检查 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | 通过（两文件语法与格式均经 rustfmt 解析确认） |
| `cargo xtask check-deps` | **18** workspace 包，依赖边界通过 |
| `cargo xtask check --registry` | 通过（V3.7 acceptance + D01–D14 映射） |
| `cargo xtask check --profile portable` | **82 个测试目标**（65 `Running` + 17 `Doc-tests`）、**706 passed / 0 failed** |
| `validate --all --strict` | **38 passed / 0 failed**（37 主规格 + 本变更；**归档前**测得，归档后为 37） |
| `validate --all --strict --archived` | **53 passed / 0 failed**（**归档后**；W47 时为 52） |

706 与 W42 记录的 695 相差 **恰 +11**，等于 W47 新增的 `xtask/src/sbom.rs` **11 项单测**；
**本轮零新增测试**（改动全部落在 `#[cfg(target_os = "linux")]` 内，Windows 侧一个测试都不增）。
测试目标数 82 与 W41/W42 记录一致（计数时已排除 `Running \`…nai-xtask.exe check --profile …\`` 这条
cargo 包装行，它不是测试目标）。

### 3.2 本机**不能**做的（如实披露）

被修测试与新增夹具都是 `#[cfg(target_os = "linux")]`，**Windows 上不编译**；本机 portable 里
`tests\engine.rs` 目标只跑 2 个跨平台测试（Linux-only 的 4 个整体缺席）。

⇒ **本机门控不能证明修法在 Linux 上成立**，只能证明「未误伤可移植目标」。
成立与否**以 CI 的 ubuntu job 结论为准**：`F31/nai` 为 public，`api.github.com` 匿名可达，
用 `GET /actions/runs?per_page=N` 与 `GET /check-runs/{id}/annotations` 复核。
这也是 W44 §2 的教训——**不得**用「本机 Windows 门控绿」论证任何 Linux 侧行为。

## 4. 规格

`engineering-foundation` 新增一条需求（归档时吸收进主规格）：

- **A drill that asserts causal order establishes it by construction**：断言先后关系的演练 SHALL 通过构造
  使该顺序成立（在被测工作进程上放置确定性阻塞点），MUST NOT 依赖「请求发得足够早」这一调度假设；
  断言的前提（如「已进入 prewarm」）SHALL 先被观察到；阻塞点 MUST NOT 使演练变成恒过；
  夹具所需的宿主能力缺失时 MUST 以具名失败呈现。

把这条写进规格的理由与 W43/W44 相同：**一条会随宿主负载变色的断言，其结论不可用于判定被测实现**——
它把「调度抖动」伪装成「被测的不确定性」，正是 W42–W44 反复付出观测成本才查清的那件事。

## 5. 未决与边界

1. **截断成因仍未定**（W44 §4：采集到的日志曾只有 11 行而 job 跑了 2 分 50 秒）。本轮不涉及。
2. **Windows 侧 `nai-runtime` 的时序脆弱未修**（W44 §8.2：`DeadlineExceeded "operation stopped"` /
   `PermissionDenied "scope permission missing"`）。那是**另一处、另一类**，需单独一轮。
3. **阻塞点依赖 worker 的读取顺序**：若将来 worker 改成先读 PCM、或对素材用非阻塞读，阻塞点会失效，
   该测试会退回原时序敏感形态。此事已写进夹具的文档注释，是已知前提而非隐藏假设。
4. **不引入 `libc`**：`mkfifo` 走 `Command::new("mkfifo")`，与同文件既有的 `Command::new("kill")` 同构
   （同属 coreutils，CI 的 `ubuntu-24.04` 必然具备），从而避免为一次测试新增依赖并改动 `Cargo.lock`
   （离线缓存与 `--locked` 门控都要照顾）。
