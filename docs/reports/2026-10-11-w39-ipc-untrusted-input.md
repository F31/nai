# W39 实测报告：攻击者可控输入面的随机输入测试（S11「模糊」这一半）

- 日期：2026-10-11
- 变更：`openspec/changes/w39-ipc-untrusted-input/`
- 指南：[docs/hosts.md](../hosts.md)
- 判据：`tests/registry.yaml` S11（保持 `tested`，但**这次有据可依**）

## 结论

S11 的验收名是「IPC **模糊**与路径穿越」，状态 `tested`，而同一行的 notes 写着「**模糊测试待建**」——
全仓库 `grep -ri fuzz` **零命中**。**验收名里的一半没实现，却被标成 `tested`**。这是登记与事实不一致，
比缺功能更危险：读者会据此以为攻击者可控输入的健壮性已被验证。本轮补上这一半，并把口径写进规格。

**先写 harness，再跑**。结果：**未发现实现缺陷**。这一点必须说准——随机测试的**阴性结果不是安全证明**，
「这批语料没打出来」**不**等于「不存在缺陷」。本报告不声称发现任何漏洞，也不声称系统安全。

## 一、审计（为什么需要单独一轮）

IPC 面上有**三个攻击者完全可控的字节入口**外加一个字符串入口，其中两个此前**没有任何测试**：

| 入口 | 代码 | 此前测试 |
| --- | --- | --- |
| 帧长度前缀 | `crates/nai-plugin-sdk/src/frame.rs:41` `read` | 6 项定向单测 |
| `Envelope`/`CapabilityInvocation`/`CancelRequest` 解码 | `crates/nai-plugin-hosts/src/host.rs:154,181,195` | **零** |
| `Handshake` 解码与字段校验 | `crates/nai-plugin-sdk/src/handshake.rs:33` `accept` | 2 项定向单测（无畸形 body） |
| 路径包含 | `crates/nai-plugin-hosts/src/boundary.rs:32` `verify_within_root` | 1 项手写用例 |

定向测试证明的是**已想到的形状**被正确处理。`length` 字段的取值空间是 `0..2^32`，手写用例只碰了几个点；
插件声明的路径是**任意字符串**，手写三个用例说明不了「任意输入都出不去」。**这两件事的证明力不同，不能相互替代。**

第三个发现：`frame::read` 在**每次读之前**检查 `cancelled`/`deadline`（`frame.rs:88-96`），`parse_manifest` 同理
（`lib.rs:45-53`）——这个顺序是安全性质（**撤权/过期优先于内容解释**），但此前只用**一个特定输入**钉过，
没有钉「任意输入下都不例外」。

## 二、断言的是全域性与有界性，不是「找到了漏洞」

harness（`crates/nai-plugin-hosts/tests/untrusted_input.rs`，8 项）断言三条**可判定**的性质：

1. **全域性**：任意字节 ⇒ `Ok(Some)` / `Ok(None)` / `Err(具名 code)`；**不存在** panic，**不存在**第四种结局。
2. **有界性**：长度前缀超过 `MAX_FRAME_BYTES` 时**在分配前**拒绝（`RESOURCE_EXHAUSTED`），分配量不由输入无界决定。
3. **顺序不变式**：取消已置位或期限已过时，**任意**输入都返回 `CANCELLED` / `DEADLINE_EXCEEDED`。

外加两条结构性断言：随机 body 往返**逐字节相同**；**被拒的帧终止连接**，其后的合法帧**不会**被继续服务。

## 三、非空洞性守卫（本设计的关键）

harness 最容易的死法：生成器只产**合法**输入 ⇒ 全绿而什么都没证明。故语料**结构化**构造危险形状
（随机长度前缀含 `0xFFFFFFFF`、截断 body、合法帧后跟垃圾、**合法 `Envelope` 头 + 任意 `message_type`/payload**、
由 `{正常段, "..", ".", 空段, 长段, unicode}` 组合的路径），并对每类**实际命中计数**，计数为零即**测试失败**。

**实测命中计数**（固定种子，可复算）：

| 语料 | 命中 |
| --- | --- |
| 帧流（4096 次） | `bodies=683`、`clean_eof=179`、`oversized=1982`、`truncated=1935` |
| 结构化 Envelope（2048 次） | `invocation_decoded=1569`、`version_incompatible=601`、`invalid_argument=846`、`denied_or_absent=225`、`completed=185`、`cancel_acks=191` |
| 路径串（2048 次） | `rejected=1927`、`accepted=121`、`traversal_rejected=651` |

**守卫反向自检**（`tasks.md` §3.3）：把阈值临时调到不可能命中，帧流与路径两个测试**确实失败**并打印计数，随后恢复。
这证明守卫不是摆设。

## 四、过程中被 harness 逼出来的两处真实语义

### F-1 ACL 拒绝是 **result 级**错误，不是连接级错误

第一版 harness 断言 `serve` 的返回值，得到 `denied_or_absent=0`。根因不在测试语料，而在**我读错了语义**：
`CapabilityHost::dispatch` 在 ACL 拒绝时把 `CapabilityResult{end: StreamError, error: PermissionDenied}`
**写回对端**，然后返回 `Ok(())`（`host.rs:258-264`）。于是 `serve` 走完循环、读到 EOF、返回 `Ok(())`。

**只看 `serve` 的返回值，会把「被拒绝」误计为「被放行」**。修正：harness 改为解析宿主**写出的帧**
（顺带断言宿主只输出良构帧、只输出已登记的消息类型），这才看到那 225 次拒绝。
「连接没断」与「请求被批准」是两件事——这正是本仓库反复强调的那类误读。

### F-2 空 payload 是一个合法的 `CancelRequest`

`CancelRequest::decode(&[])` **成功**（全默认值），于是「看起来是垃圾」的帧会走到取消路径并收到 `CancelAck`。
harness 起初把它当「意外消息类型」直接 panic，暴露了这一行为，现按真实类型解析并计数（`cancel_acks=191`）。
这不是缺陷，是 protobuf 的默认值语义；但**不知道它就会把正常回复读成异常**。

## 五、改动

| 文件 | 改动 |
| --- | --- |
| `crates/nai-plugin-hosts/tests/untrusted_input.rs`（新） | 8 项随机输入测试 + splitmix64 固定种子 PRNG + 非空洞性守卫 |
| 规格 | `plugin-hosts-and-ipc` 新增 3 条需求 / 6 个场景 |
| `docs/hosts.md` | 新增「随机输入测试（S11『模糊』，W39）」一节 |
| `tests/registry.yaml` | S11 notes 记录证据、口径与边界；状态保持 `tested`；evidence 指向本报告 |
| `docs/development-plan.md`、`AGENTS.md` | 记录本轮 |
| 新增 | 本报告、`openspec/changes/w39-ipc-untrusted-input/` |

**实现零改动**：`frame.rs`/`handshake.rs`/`host.rs`/`boundary.rs` **一行未改**——harness 未发现需要修的东西。
**零新增依赖**，18 包与依赖方向不变（hosts → sdk → contracts）。

## 六、实测

```
cargo test -p nai-plugin-hosts --offline
lib 单测 6 passed；e2e 3 passed；untrusted_input 8 passed；0 failed（实测约 0.5s，不构成门控瓶颈）
```

门控：

```
cargo xtask check --secrets          credential scan passed: 695 files, no prefixed credential material
cargo xtask check --registry         admission registry check passed
cargo xtask check-deps               dependency boundaries passed: 18 workspace packages
cargo xtask check --profile portable EXIT=0；82 个测试目标（65 Running + 17 Doc-tests）；683 passed / 0 failed
tsc --noEmit（contracts-ts / desktop）  EXIT=0
vite build（desktop）                 ✓ 30 modules transformed；✓ built in 2.92s
node --test tests/*.test.mjs（desktop）  # pass 5；npm run test:ui  # pass 15
openspec validate --all --strict --archived  全通过
```

相对 W38：**目标 81 → 82（+1 = 本轮的 `untrusted_input` 集成测试目标）、通过 675 → 683（+8 = 本文件 8 项）**，
两处增量互相印证。全量门控端到端实测约 5 分钟（单个 knowledge_publication 测试就超过 60 秒），与本次改动无关。

**门控确实拦下了两次**（都记在这里，不藏）：

1. 首轮 `cargo fmt --all --check` **失败**（`xtask: cargo fmt --all --check failed`）——三处 rustfmt 换行差异。`cargo fmt --all` 后通过；
   已确认**只**动了本文件（其余 `git status` 无变化）。
2. 次轮 `cargo clippy --workspace --all-targets -- -D warnings` **失败**：`assert!(minor <= 0, ...)` 被判
   `clippy::absurd_extreme_comparisons`（`0` 是 `u32` 的最小值，该比较恒真）。**这不是噪声**——那条断言本来就是**
   恒真**的弱断言，clippy 说得对。改为 `assert_eq!(minor, 0, ...)`（宿主的协商范围在此恰为单点），断言才真正有内容。

这两次说明 portable 门控不是橡皮图章：**格式与 lint 都在门控内**，且这次真的起了作用。

## 七、未覆盖边界

- **不是覆盖率导向模糊测试**：无 `cargo-fuzz`/libFuzzer、不用 nightly、无独立语料目录；探索能力显著弱于真 fuzzer。
  一律称「随机输入测试」——名字骗人就是证据骗人。未来若引入 nightly 工具链可另立变更升级，本轮不做。
- **阴性结果不是安全证明**：本轮**未发现**缺陷；**不**据此宣称「不存在缺陷」或「系统安全」。
- **不新增仓库级证据目录**：§30.1 未为模糊测试指定目录，per-crate `tests/` 是既有惯例（`tests/e2e.rs`），
  **不**臆造 `tests/fuzz`。
- **不测传输层对端身份**：真实命名管道/Unix socket 的 `SO_PEERCRED` 在钉住的 rustc 1.96.0 上仍不稳定（E0658）；
  Windows Named Pipe peer PID、AppContainer/Job Object 仍未实测（Blocked）。本轮 fuzz 的是**解码与分发**，不是传输。
- **随机语料不是覆盖率证据**：计数只证明「语料确实命中了这些形状」，**不**证明分支覆盖完整。
