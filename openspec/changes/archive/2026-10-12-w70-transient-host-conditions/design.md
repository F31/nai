## 1. 判据：宿主的瞬态条件不得升级为安全结论

两族症状（`engine.rs` 撤权、`pipeline.rs` 预置）看起来无关，但判据是同一条：

> **宿主的瞬态条件（锁竞争、慢盘）MUST NOT 改变被测系统的结论**——
> 既不能夺走一个已经该发生的安全动作（撤权），也不能把一次正常但较慢的 bring-up 变成不可解释的失败。

这条判据在本仓有先例：W48 报告 §4 写着「一条会随宿主负载变色的断言，其结论**不可用于判定被测实现**——
它把『调度抖动』伪装成『被测的不确定性』」。W48 处理的是**测试前提**；本轮处理的是**代码对瞬态条件的处理**，
因为本轮两族的落点都在被测代码里（`nai-runtime` 的取锁、`Operation` 的预算），不在夹具里。

## 2. 缺口一：为什么是「有界等待 + 区分两类失败」而不是别的

### 2.1 事实链（读码可得，无需 CI 日志）

| 环节 | 位置 | 性质 |
| --- | --- | --- |
| 会话实例共享 | `apps/engine/src/lib.rs:314`、`crates/nai-control-api/src/lib.rs:204` | `Arc<Mutex<Session>>` 两侧共用 |
| 两侧都 fail-fast | `composition.rs:196`、`control-api/src/lib.rs:434` | `try_lock()` → `Conflict("session state busy")` |
| 活性检查每 5 ms 一次 | `apps/engine/src/lib.rs:67`（`DRILL_POLL`）、`:582`（`live_epoch()`） | 高频、**低频价值**的读 |
| 失败被当致命 | `apps/engine/src/lib.rs:582` 的 `?` | 经闭包传出 ⇒ 循环结束 |
| 撤权只在错误出口重试一次 | `apps/engine/src/lib.rs:547–549` | 同一条 fail-fast 取锁 ⇒ 仍是运气 |

`Session::revoke` 自身**不可能**失败：`crates/nai-kernel/src/session.rs:296` 只有一个 `actor_allowed`
前置（此处恒为 `true`），其余是两个 `checked_add`。⇒ **撤权被跳过的唯一机械通道就是取锁失败。**

### 2.2 三个被否掉的做法

| 做法 | 为什么不 |
| --- | --- |
| 把 `try_lock()` 一律换成 `lock()`（阻塞取锁） | 让**状态读取**可以无限期阻塞**监督线程**，并把规格里已有的「锁竞争 MUST 以显式冲突错误处理」这条契约抹掉。冲突本身不是缺陷；**把「瞬态」当「终局」**才是。 |
| 把 `wait_for_keeper_pid` 之后再加等待/重试 | 治的是夹具；缺口在代码（`tick` 的致命上抛）。加了也只是把概率再挪一挪。 |
| 降低 `apps/engine` 测试的并发（`--test-threads=1`） | 把宿主的调度假设搬进夹具，等于承认「结论取决于负载」；且会让 CI 更慢。W48 §4 已否决这一类。 |

### 2.3 采用的语义：两个预算、两类失败

关键是**两类读锁的失败后果不同**，所以预算与返回类型都必须不同：

| 用途 | 预算 | 预算耗尽 | 依据 |
| --- | --- | --- | --- |
| **撤权/故障处置**（`observe`、`control`、`enforce_duty`） | `SESSION_REVOKE_BUDGET` = 250 ms | **显式 `Conflict`**（如实上报，不静默跳过） | 这是一次性动作，**丢一次就没了**；250 ms 远小于 ADR 0004 的 30 s 上界，且相对微秒级持锁时间有 5 个数量级余量 |
| **epoch 活性检查**（`live_epoch`） | `SESSION_POLL_BUDGET` = 5 ms（= 一个 tick） | `Ok(None)`「本 tick 未观测到」 | 这是**周期性**读；5 ms 后就是下一个 tick。**不能**给长预算——长等待会拖慢 `renew`（`lease_renew_ms` = 100 ms、`lease_expire_ms` = 600 ms），反而制造新的时序脆弱 |

⇒ **`live_epoch` 的 `Ok(None)` 不得被读作「epoch 变了」**（那会**误撤权**），也不得被读作「发生了故障」
（那是今天的缺陷）。它只表示「本轮没有观测」。`tick` 因此把该判定**推迟到下一个 tick**，
而**故障 `observe` 照常执行**——所以「keeper 死了」这件事**不再依赖 epoch 读取成功**，这比今天**更强**。

### 2.4 一处刻意的「一处定义」

`SESSION_*_BUDGET` 与 `BRINGUP_OPERATION` 定义在 `crates/nai-runtime/src/composition.rs`，
`apps/engine` 通过 `pub const` 复用（`--provision` 用 `BRINGUP_OPERATION`）。
理由与 ADR 0027 的立场一致：**同一条策略不得有第二处定义**（否则两处会漂移，且门控只能看到其中一处）。

## 3. 缺口二：bring-up 预算按工作量标定

| 项 | 取值 |
| --- | --- |
| bring-up 预算 | `BRINGUP_OPERATION` = **30 s** |
| 界 | ADR 0004 第 13 行「最长 30s」；ADR 0038 第 76 行「拒绝超过 30 秒的操作（安全层不变量）」 |
| 适用面 | **只**给包含 schema 迁移的操作：`Composition::prepare` 的 `Store::open`、`--provision` |
| 不适用面 | 逐次调用预算 `SHORT_OPERATION` **保持 5 s**（读取类不含迁移） |

判据是可判定的：**该操作是否包含 schema 迁移**。含 ⇒ bring-up 预算；不含 ⇒ 逐次调用预算。
`apps/engine/src/pipeline.rs` 的两处 5 s（`decision_entries` / `verify_decision_chain`）是**已打开库上的读取**，
不含迁移 ⇒ **本轮不动**，且这个「不动」正是上述判据的结果，不是遗忘。

### 3.1 为什么必须让失败可判定

family 2 拿到的 CI 注解只有 `provisioning store unavailable: storage: DEADLINE_EXCEEDED`——
**看不出是哪一步、花了多久**。本轮把 `store open` 失败的文案改为**指名步骤 + 已耗时**；
`create_workspace` / `set_permission` / `save_config` 三步的文案原本就已各自具名。

理由沿用 W44 的教训（信噪比）：**一次不可判定的红，观测成本等于一整轮**。

## 4. 规格为何要改（而不是只改代码）

`runtime-composition` 已有 *Shared session between control surface and composition* 与
*Engine supervision loop* 两条需求，本轮**只加强、不削弱**：

- 前者的「锁竞争 MUST 以显式冲突错误处理」被**收窄到查询/命令路径**，并补上「撤权路径有界重试」；
- 后者补上「监督循环 MUST NOT 因瞬态锁竞争终止；活性检查失败 MUST 视为本 tick 未观测到」——
  把今天的**隐含前提**（「取锁不会失败」）变成**明文判据**；
- 新增一条「bring-up 预算按工作量标定」，因为「短 `Operation` 约定」是被写进第一条需求的**明文规则**，
  而它对 bring-up 是错的；不改规格就等于让代码继续违反自己的明文规则。

## 5. 验证方式与边界

- 本机可做：单元测试钉住三个常量（含「bring-up ≤ 30 s」「poll ≤ revoke」的关系）、
  集成测试钉住两类失败语义、`cargo fmt`/`clippy`/`cargo test`、门控 full。
- **本机不能做**：`killed_keeper_*` 与 `pipeline.rs` 的夹具行为在 Linux 上成立与否。
  该测试是 `#[cfg(target_os = "linux")]`，Windows 上不编译（W48 §3.2 同款披露）。
  ⇒ **以 CI 的 ubuntu/windows job 结论为准**，用匿名 API 低频复核。
- **本轮不声称 CI 从此全绿**。它修的是两条已证的脆弱路径；若仍红，新文案会把下一次变成可判定的判据。
