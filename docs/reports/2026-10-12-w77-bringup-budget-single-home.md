# W77 轮次报告：一个预算一份定义（定义归属执行者）

- 日期：2026-10-12
- 变更：`openspec/changes/w77-bringup-budget-single-home`
  （归档后为 `openspec/changes/archive/2026-10-12-w77-bringup-budget-single-home`）
- ADR：`docs/adr/0055-bringup-budget-single-home.md`
- 基线：V3.10（**未改**）
- 产品行为：**零变化** —— 没有任何一处预算数值被改动

## 一、判断与范围

本轮做**上一轮登记的第 1 条**：`nai-control-api` 的同族预算缺陷。选它而不是「打包粒度门控」
或 harness 轨道的理由有三条，且都是**可核实的**：

1. 它不是「潜在问题」——**W75 在 CI 上的失败正落在它**（`crates/nai-control-api/src/tests.rs:364`）；
2. 它是 W76 报告 §八第 8 条**具名登记**的遗留；
3. 它是「一个预算一份定义」（ADR 0027）在**跨 crate 场景**下的结构性版本，而这条纪律已被
   W70（产品侧）与 W76（夹具）各证伪一次——**这次的问题不在数值，在定义的位置**。

范围：预算的**定义位置**与**消费者引用**。**不调参、不改行为、不新增依赖边、不改基线正文**。

## 二、核实

### 2.1 依赖方向否掉了最直觉的答案

修 `nai-control-api` 的夹具，最直觉的写法是 `Operation::new(nai_runtime::BRINGUP_OPERATION)`。
**核实后不可行**：

| 事实 | 证据 |
| --- | --- |
| `nai-runtime` **依赖** `nai-control-api` | `crates/nai-runtime/Cargo.toml:17` |
| ⇒ 反向引用构成**依赖环** | 同上 |
| `nai-control-api` 与 `nai-runtime` **都**依赖 `nai-storage` | 两者 `Cargo.toml` |
| `BRINGUP_OPERATION` 只定义在 `nai-runtime::composition` | `composition.rs:31`（改动前） |
| `Operation` 的 30 s 上界是一个**字面量** | `nai-security/src/lib.rs:36`（改动前） |

⇒ **W70 立的规则没错，落点错了**：它被放在**首个遇到问题的消费者**上，
而 bring-up 的执行者是 `Store::open`，在 `nai-storage`。

### 2.2 现存副本清单（**11 个位点 / 9 个文件**）

| # | 位置 | 原写法 | 工作量 | 语义 |
| --- | --- | --- | --- | --- |
| 1 | `nai-storage/src/tests.rs:10` | `from_secs(30)` 字面量 | bring-up | `BRINGUP_OPERATION` |
| 2 | `nai-storage/src/upgrade.rs:183` | `from_secs(30)` 字面量 | 迁移 | `BRINGUP_OPERATION` |
| 3 | `nai-control-api/src/tests.rs:351` | **`from_secs(10)`** 字面量 | bring-up | `BRINGUP_OPERATION` |
| 4 | `nai-upgrade-core/tests/plan.rs:82` | `from_secs(30)` 字面量 | 演练 | `Operation::CEILING` |
| 5 | `nai-upgrade-core/tests/migration.rs:31` | `from_secs(30)` 字面量 | 迁移演练 | `Operation::CEILING` |
| 6 | `nai-upgrade-core/tests/lifecycle.rs:11` | `from_secs(30)` 字面量 | 演练 | `Operation::CEILING` |
| 7 | `nai-upgrade-core/tests/gates.rs:94` | `from_secs(30)` 字面量 | 演练 | `Operation::CEILING` |
| 8 | `nai-provider-core/examples/openai_smoke.rs:74` | `from_secs(30)` 字面量 | 冒烟 | `Operation::CEILING` |
| 9 | `nai-provider-core/examples/openai_smoke.rs:108` | 同上 | 冒烟 | `Operation::CEILING` |
| 10 | `live_provider_smoke.rs:111` | **`const OPERATION_CEILING` 私有影子** | 冒烟 | `Operation::CEILING` |
| 11 | `xtask/examples/golden_eval.rs:52` | **`const OPERATION_CEILING` 私有影子** | 评测 | `Operation::CEILING` |

**第 10、11 条是本轮最好的证据**：它们各自**重新定义了一个常量**，名字就叫 `OPERATION_CEILING`
—— 正是 `composition.rs` 那句注释所禁止的 *"a second, drifting copy"* 的**活标本**。
⇒ 「数值重复」与「定义重复」是两种不同的复制，而后者才是**会漂移**的那种。

**只第 3 条是错的数值**（10 s），其余是**正确的数值、错误的位置**。
这个区分很重要：它说明本轮**不是**在修 bug 数值，而是在修**结构**。

### 2.3 「取上界」原本不可判定

`BRINGUP_OPERATION = Duration::from_secs(30)`，而 `Operation::new` 拒绝 `> from_secs(30)`。
「bring-up 取 ADR 0004 的上界」这件事**只写在注释里** ⇒ 两个 30 恰好相等**是设计还是巧合，
机器看不见**。这使「改一个忘一个」在类型与断言上**都无感**。

## 三、裁定

1. **定义点归属「工作类别的执行者」**（ADR 0055 决策 1）：
   `BRINGUP_OPERATION` 下移到 `nai-storage`；`nai-runtime` 改 **re-export**（API 不变，`apps/engine` 零改动）。
2. **提取 `Operation::CEILING`**（决策 2）：它既是公开常量也是 `new()` 的拒绝阈值
   ⇒「取上界」变成**可判定的等式**。
3. **`SHORT_OPERATION` 不上移**（决策 3）：它**没有第二个消费者**；为不存在的问题造公共定义
   就是 W73 的「假契约」。
4. **同值不同名**（决策 4）：bring-up 用 `BRINGUP_OPERATION`，演练/冒烟「用满上界」用
   `Operation::CEILING`。把「数值相同」与「语义相同」分开——否则 upgrade 的演练夹具会被迫
   自称 bring-up，那是**语义错位**，比重复更坏。

## 四、实现（14 个文件）

| 类别 | 文件 | 改动 |
| --- | --- | --- |
| 新定义 | `crates/nai-security/src/lib.rs` | 新增 `pub const Operation::CEILING`；`new()` 改用它；新增 1 条单测 |
| 新定义 | `crates/nai-storage/src/lib.rs` | 新增 `pub const BRINGUP_OPERATION: Duration = Operation::CEILING;`（+ 文档） |
| 新断言 | `crates/nai-storage/src/tests.rs` | `op()` 改用常量；新增 `bringup_budget_is_the_operation_ceiling` |
| 改 re-export | `crates/nai-runtime/src/composition.rs` | `pub const … = from_secs(30);` → `pub use nai_storage::BRINGUP_OPERATION;`，订正注释 |
| 消费者 | `crates/nai-storage/src/upgrade.rs` | `op()` 改用常量；删失效 `use std::time::Duration` |
| 消费者 | `crates/nai-control-api/src/tests.rs` | **10 s → `nai_storage::BRINGUP_OPERATION`**（本轮主角） |
| 消费者 | `nai-upgrade-core/tests/{plan,migration,lifecycle,gates}.rs` | `Operation::new(Operation::CEILING)`（lifecycle 删失效 import） |
| 消费者 | `nai-provider-core/examples/openai_smoke.rs` | 两处 → `Operation::CEILING` |
| 消费者 | `nai-provider-core/examples/live_provider_smoke.rs` | **删私有 `const OPERATION_CEILING`**，4 处引用改指 `Operation::CEILING`，删失效 import |
| 消费者 | `xtask/examples/golden_eval.rs` | 同上（删私有常量 + 2 处引用 + 删失效 import） |
| 判据 | `tests/registry.test.mjs` | 新增 2 个测试（含 4 条断言）、`rustSources()` 遍历与 `rustHits()` |

**共同点**：每一处都是「引用一个已存在的定义」，没有一处是「写下一个新的数」。
`cargo fmt --all` 已跑；**无行为差异**。

## 五、非空洞注入验证（**7 组**）

每组都做「注入 → 跑判据 → 期望失败 → 还原」。**七组全部按预期开火**：

| 注入 | 期望 | 实测 |
| --- | --- | --- |
| V0 无注入（对照） | PASS | **PASS** |
| V1a 在别处新增第二处 `pub const BRINGUP_OPERATION` | 失败 | **FAIL** |
| V1b 删掉唯一的定义 | 失败 | **FAIL** |
| V2 定义的值不再引用 `CEILING`（改回字面量） | 失败 | **FAIL** |
| V3c `nai-control-api` 改回 `Duration::from_secs(30)` | 失败 | **FAIL** |
| V3d 新增 `const OPERATION_CEILING` 私有影子 | 失败 | **FAIL** |
| V4 把 `Operation::CEILING` 从 30 s 改成 20 s | 失败 | **FAIL** |
| V5 把 `BRINGUP_OPERATION` 改成字面量 20 s | 失败 | **FAIL** |

**V4/V5 是 Rust 侧**（证明两条等式断言不是自指的）：
- V4：`nai-security` → `ceiling_is_the_adr_0004_bound_and_the_rejection_threshold` **FAILED**
  （`4 passed; 1 failed`）—— 因为那条断言把 **30 s 写死**为外部锚点（ADR 0004）。
  **若只断言 `new(CEILING).is_ok()`，改小 CEILING 后它仍会通过** ⇒ 这一条是必须的。
- V5：`nai-storage` → `bringup_budget_is_the_operation_ceiling` **FAILED**。

**还原已核实**：注入后 `git status --short` 只余本轮的 14 个改动文件。

## 六、验证

### 6.1 定向 clippy（两次，均 EXIT=0）

```
cargo clippy -p nai-security -p nai-storage -p nai-runtime -p nai-control-api -p nai-upgrade-core --all-targets -- -D warnings
cargo clippy -p nai-provider-core -p nai-xtask --all-targets -- -D warnings
```

第二次是**必需的**：`xtask check --profile portable` 只跑 `cargo test --workspace` 与
`cargo test -p nai-xtask --lib`，**examples 不在其中** ⇒ 三个 example 的改动不会被门控编译到，
必须自己点名。

### 6.2 Node 测试

`node --test tests/registry.test.mjs` → **21/21 pass**（W76 = 19，本轮 +2）。

### 6.3 门控

```
cargo xtask check --profile full   →  EXIT=0
```

**实测**：**92 个测试目标 = 74 `Running` + 18 `Doc-tests`**，**860 passed / 0 failed**，
日志内**无任何** `test result: FAILED` / `^error` / `LNK1104` / `panicked` / `not ok`。
两条新守卫在门控里**确实执行**（`ok 20 - the bring-up budget is defined exactly once…`、
`ok 21 - no Rust source hides a second copy of the operation ceiling`）。
前端门控 `[frontend] 全部步骤通过`，收尾行 `full: engineering foundation checks … passed`。

**与 W76 的口径对照（如实登记一处差）**：W76 报告记 91 = 73 `Running` + 18 `Doc-tests`、
858 passed。本轮是 92 = **74** + 18、**860** passed。

- **`passed` 的 +2 与新增测试逐条对应**：`ceiling_is_the_adr_0004_bound_and_the_rejection_threshold`
  （`nai-security`）与 `bringup_budget_is_the_operation_ceiling`（`nai-storage`）各 +1。**这条对得上**。
- **目标数的 +1 对不上任何新增测试目标**（本轮没有新增测试文件）。核实后归因于**计数口径**：
  `portable` 是**两条** cargo 命令（`cargo test --workspace --exclude nai-xtask` 与
  `cargo test -p nai-xtask --lib`）；本轮**第一次跑到的那份日志恰好就是 73**——它止于
  **第二条命令的链接失败**，因此**少了 `nai-xtask --lib` 这一条 `Running`**。完整两命令的现测值是
  **74** ⇒ **W76 记录里的 73 属同一形状的「缺一条」**。**这正是记忆里那条纪律的又一次适用**：
  *计数类范围断言引用前必须现跑，MUST NOT 从旧记录抄。*

### 6.5 为什么门控跑了七次（环境事实，值得记下）

门控第一次就红，且**错误形态一路在变**（`LNK1104` → `E0462`/`E0463` → `rlib format`），
最后在日志里抓到了**真正的第一因**：

```
memory allocation of 2097152 bytes failed
error: only metadata stub found for `dylib` dependency `std` …
process didn't exit successfully: clippy-driver.exe … (exit code: 0xc0000409, STATUS_STACK_BUFFER_OVERRUN)
```

**本机 `nproc = 24`** ⇒ cargo 默认开 **24 路**并发；而我为了让产物一致先做了 `cargo clean -p`
（删 1.4 GiB）⇒ 变成**全量冷重建**，多个 `clippy-driver` 同时跑直接把内存打爆。
**编译器进程 OOM 中途死亡会留下截断产物**（`.rmeta` 在、`.rlib` 缺），下游于是报出
`can't find crate` / `only metadata stub found` / `required to be available in rlib format`
——**这些都是果，不是因**。只看表面错误会去怀疑代码或依赖，方向全错。

第二因（独立）：**间歇性 `LNK1104`**。实测**逐目标随机**（`nai_engine-*.exe` → `pipeline-*.exe`
→ `engine-*.exe` → `nai_xtask-*.exe` 轮着来），**串行（`CARGO_BUILD_JOBS=1`）也能复现**；
已排除：**磁盘未满**（E: 余 57 GB）、**目录可写**（同目录内新建/删除 `.exe` 均成功）、
**无残留进程**（cargo/rustc/link/mspdbsrv 全无）、**目标文件不存在**（不是被占用）
⇒ 环境级，与记忆里已登记的「间歇性链接争用」同族。

**所以处置不是改代码，而是改变编排**：先 `cargo test --workspace --exclude nai-xtask --no-run`
**与** `cargo test -p nai-xtask --lib --no-run` **把全部测试二进制预链接好**（各一次成功），
再跑门控 —— 此时 `cargo test` 只剩**执行**、不再链接 ⇒ 命中 LNK1104 的机会消失。
⚠ **两条命令都要**：`--exclude nai-xtask` 会漏掉后者，本轮正因如此多跑了一次。
**判据**：`git diff` 只动 16 个文件、逐包定向 clippy 两次全绿、单目标独立链接成功
⇒ **不是代码缺陷**（与「逐包能过 ⇒ 不是代码缺陷」同一条判据）。

### 6.6 门控里那处 `not ok 14` 的定性（非阻塞）

第一次门控日志里 `tests/contracts/schema.test.mjs` 有一条 `not ok 14`，错误来自沙箱的
**safe-delete shim**（把 `dist/.w75-scan-probe.rs` 移入回收站时报「系统找不到指定的文件」）。
定性依据两条：① **单独跑该文件 17/17 通过**；② **xtask 侧并未报 node 失败**——日志里
**唯一的 `xtask:` 失败行只有 clippy**，说明跑到 clippy 时 node 各步都已通过。
成因是**并发重复调用同一文件**：该测试把探针写到**写死的** `dist/.w75-scan-probe.rs`，
两个进程互相把对方的文件先删掉，而 shim **不认 `force: true`** ⇒ 把良性的清理竞态变成失败。
⇒ **判为非阻塞**，并**如实登记**（登记，不改：它不是本轮引入的，且改它属于扩大范围）。

## 七、CI 复核

本轮 run **`38261845020`** —— `head_sha = d32a7fc`，**与本轮推送 SHA 一致**（`created_at = 2026-10-12T14:17:41Z`），
结论 **`success`**（两次查询：先 `in_progress`，后 `completed / success`）。

**三件齐**（唯一查询参数 `cb=$(date +%s%N)` ＋ `Cache-Control: no-cache` ＋ 回读 `head_sha` == 推送 SHA）
⇒ 结论可信。**同一次查询顺带证实**：本轮之前的三轮 run 均为 `success`——
W76 回填 `935f304` → `38253557675`、W76 主提交 `5f1e12c` → `38251222058`、
W75 回填 `d89da43` → `38238332385`。

**观测通道本轮新踩的一个坑（已写进记忆）**：`curl` 走代理报
`schannel: failed to receive handshake, SSL/TLS connection failed`（`EXIT=35`）。
排除出网问题的依据：同一代理端口**在听**、经它访问 **http** 得 **301**、**直连 https** 得 **200**
⇒ **TLS 1.3 与 schannel 握手失败**。加 **`--tlsv1.2 --tls-max 1.2`** 后恢复正常（200）
⇒ 以后凡 https 查询都带上这两个参数。

**按观测轮终止约定**：本条的**回填提交**会触发一个新的 run，**该 run 不再回填**——否则无限递推。

## 八、遗留

1. **判据的边界如实登记**：G-C 只抓**两个具名模式**（`pub const BRINGUP_OPERATION` 的位置与数量、
   `Operation::new(Duration::from_secs(30))` 字面量、`const OPERATION_CEILING` 私有影子）。
   若有人新起一个**别的名字**的 30 s 常量（如 `const SMOKE_TIMEOUT`），扫描**抓不到**。
   它挡的是「已知的那两种复制方式」，不是「一切复制」——**这是边界，不是缺陷**。
2. `nai-upgrade-core/tests/{plan,migration,upgrade,gates}.rs` 的
   `Instant::now() + Duration::from_secs(30)`（**自造 deadline**）**未改**：不是 `Operation::new`
   的实参 ⇒ 不属本命题（改了是扩大范围，不是修问题）。
3. `apps/engine/src/pipeline.rs` 与 `nai-control-api/src/tests.rs:617/736` 的 5 s 是
   **在已打开的 store 上读**（逐次调用）⇒ 原样保留（W70 已裁定）。
4. **`SHORT_OPERATION` 仍是 `nai-runtime` 的私有常量**：它只有一个消费者。若将来出现第二个，
   **MUST** 走本 ADR 的同一条流程（先问「这份预算描述的是谁的工作」）。
5. **`Operation::CEILING` 与 `BRINGUP_OPERATION` 的关系**由一条 Rust 等式钉住；
   但「为什么 bring-up 就该取上界」仍是**设计判断**，脚本证不了它（ADR 0055 决策 2）。
6. **本轮环境两坑已登记**（细则见 §6.5 / §6.6，长期记录见 `.workbuddy/memory/KNOWLEDGE.md` §11）：
   ① `nproc = 24` ⇒ 冷缓存下 `clippy --workspace --all-targets` **OOM**（截断产物会伪装成
   「代码/依赖有错」）；② **间歇性 `LNK1104`** 的**预链接绕法**。
   **它们不是本轮的产物，也不会因本轮结束而消失** ⇒ 下一轮若再见同类形态，**先按这两条判据走**。
7. **`cargo xtask schema --check` 本机仍跑不了**（宿主对「node 自旋 node」的 `EBUSY`）⇒ 零漂移由
   「**未改任何 schema 与生成器输入**」（结构性理由）＋ **CI 的独立步骤**兜底，
   **MUST NOT 写成「已通过」**（本轮的 `openspec/specs/scheduling-and-budgets/spec.md` 改的是**规格正文**，
   与 `schemas/` 下的机器可读契约无关）。
8. **CI 回填的终止约定**：按 W57 先例，**回填本条 CI 结论的那次提交，其自身的 run 不再回填**。

## 九、方法论教训

1. **「复制数值」与「复制定义」是两种不同的复制，而后者才会漂移。**
   本轮 11 个位点里只有 1 个是**错的数值**（10 s），其余是**正确的数值、错误的位置**。
   把问题读成「有人在乱写数字」会修错东西；读成「定义没有归属」才对。
2. **修订一条规则时，要先问它为什么原本落在那里。**
   `Sized here once so the engine cannot hold a second, drifting copy` —— 这句注释里的
   **承诺是对的**，但写下它的位置服务的是**写下它的人**。定位规则时，问「这份东西描述的是
   谁的工作」，而不是「谁先遇到了这个问题」。
3. **依赖方向要先核实，它会否掉整个方案。**
   「引 `nai_runtime::BRINGUP_OPERATION`」看起来是最自然的修法，但
   `nai-runtime → nai-control-api` 让它是**环**。核实只花了三条命令，省掉一次返工。
4. **自指的断言必须配一个外部锚点。**
   `assert!(new(CEILING).is_ok())` 在 `CEILING` 被改小后**照样通过**——因为拒绝阈值跟着变小。
   必须有一条把 **30 s 写死**的断言（W66 教训的又一次适用）。
5. **「不在门控步骤里的目标」要自己点名。**
   `portable` 只跑 `cargo test --workspace` 与 `cargo test -p nai-xtask --lib` ⇒ **examples 不被编译**。
   本轮改的三个 example 若不做一次 `--all-targets` 的定向 clippy，**门控绿而它们是坏的**。
6. **错误形态一直在变，不等于原因一直在变。**
   本轮门控的红一路上从 `LNK1104` 换到 `E0462`/`E0463`、`metadata stub`、`rlib format`，
   看起来像「产物损坏在乱跑」；**真正的第一因只有一个**——`memory allocation … failed`（OOM）。
   后面那一串**都是它留下的截断产物的回声**。⇒ **判据方向要往日志更前面找**：
   先问「有没有进程死过」，再问「代码错在哪」。
   我为此绕了四轮，而第一因其实一行就在日志里。
7. **环境级的间歇失败，处置是「改编排」而不是「改代码」；而且要把绕法写进长期记录。**
   `nproc = 24` 不是缺陷、LNK1104 也不是缺陷，但**不写下来，下一轮还会再花一轮去重新发现**。
   本轮的收益是两条**可复用**的处置：**`CARGO_BUILD_JOBS=2~4`** 与
   **「先把全部测试二进制预链接（两条命令，`--exclude nai-xtask` 会漏后者）再跑门控」**。
8. **沙箱/宿主的报错要读到「宿主文件」那一行。** 门控里那条 `not ok 14` 的错误正文写的是
   `[safe-delete] 操作失败 … trash 操作 … 系统找不到指定的文件`，栈里是
   `node-safe-delete-shim.cjs`——**它一眼就是宿主行为**，不是仓库的断言逻辑。
   同时别忘了**用 xtask 自己的失败行做交叉验证**（唯一的 `xtask:` 失败行只有 clippy ⇒ node 步是过的）。
