# W108 报告：成因 MUST 付代价——仓库内凭据，或从闭集逐出

- 轮次：W108
- 日期：2026-10-14
- 设计：[ADR 0086](../adr/0086-cause-payment.md)
- 变更：`openspec/changes/2026-10-14-w108-cause-payment/`（归档后位于 `archive/` 下）
- 落地：`scripts/contracts/cause-payment.mjs`（新建）、`scripts/contracts/xtask-commands.mjs`、
  `scripts/contracts/workflow-steps.mjs`、`tests/contracts/xtask-commands.test.mjs`、
  `tests/contracts/workflow.test.mjs`、`tests/contracts/{step-config,step-target,workflow-triggers}.test.mjs`

## 一、目标

W95／W96 给「为什么这一步／这条命令不在本机门控闭包里」这件事建了**闭集**：workflow step 侧是
`CI_ONLY_REASONS`，`cargo xtask` 命令侧是 `STANDALONE_REASONS`，各自 4 个取值。判据对闭集**之内**
的每一格只做一次 `includes()`：

```js
if (!CI_ONLY_REASONS.includes(entry.reason)) { … }
```

本轮的目标：**让「成因」这句话本身也要付代价**——每一条成因 MUST 给出一条**可从仓库自证**的凭据
（一串在仓库里逐字存在的文本），否则它只是一句**无从查证的话**。

## 二、缝在哪

### 2.1 直接证据（**现测**，不是推断）

1. 判据只做一次 `includes()`：它答「这个成因**在不在**表里」，**不答**「这个成因**凭什么成立**」。
   代码位置：`scripts/contracts/workflow-steps.mjs`（step 侧）、`scripts/contracts/xtask-commands.mjs`
   （命令侧）。
2. **两处闭集各有一格零实例**（命令层 `host-subprocess`／step 层 `host-subprocess`）。
3. 两处测试只宣布闭集**非空**：
   `assert.ok(CI_ONLY_REASONS.length > 0, 'the cause table must not be empty')`。

于是两处空位：

- **成因本身没有仓库内的凭据**。`superseded-by-workspace-run` 断言「门控跑的是全 workspace 测试，
  覆盖同一批包」——这句话若哪天不再成立（那条 `--workspace` 调用被换成显式包列表），
   成因格照旧成立、判据照旧全绿。
- **成因格不要求有实例**。某一格若被**合法清空**，格子仍在常量里，它的规则**对空集恒真**。

这是本仓的母题之一（W78 的规则表、W95 的 `CONDITION_SHAPES`）：**规则表没有实例 ⇒ 规则恒真**。

### 2.2 触发：一条被预言的失效真的发生了

ADR 0073 的「遗留 1」逐字写着：

> **不验 `ci-only` 的成因是否仍然成立**：例如 `schema --check` 哪天在本机变得可跑，判据不会有
> 信号（`reason` 仍写着 `host-subprocess`）。要发现它得靠人重新实测宿主。

**2026-10-14 侦察实测**：`cargo xtask schema --check` **跑通了**（退出码 `0`，输出
`Rust/TS/C++/JSON Schema/OpenAPI generation checked with no drift`），而注册表里那条登记**逐字未变**
（`reason: 'host-subprocess'`），**全部判据零信号**。

⇒ 一条**会随仓库自身进展而过期**的声明，没有任何判据盯着它。这是**遗忘的形状**
（§6.17「解析得开 ≠ 它现在还悬空着」的同族）。

⚠️ **这次实测只是侦察见证，MUST NOT 进任何断言输入**（铁律二）。本轮**没有**因为「本机能跑了」
而把 `schema --check` 接进 `engineering_checks`——见 §四与 §十。

### 2.3 缝型归属

| 维度 | 本轮 |
|---|---|
| 缝的类型 | **声明没有凭据**（不是「没有执行体」） |
| 与 W95–W107 的关系 | 前十三轮找的是「判定面存在、执行体不存在」；本轮找的是「**成因存在、凭据不存在**」 |
| 为什么以前没发现 | 判据对闭集之内的每一格**只验成员资格**，不验**成员资格凭什么成立** |

## 三、设计（详见 [ADR 0086](../adr/0086-cause-payment.md)）

### 决策 1：成因 MUST 付代价

两处各建一张**兑付规格表**，每格 **≥1** 条凭据：

```
{ <reason>: { evidence: [ {file, marker, why}, … ] } }
```

两处**共用**一套核验（`scripts/contracts/cause-payment.mjs::reasonPaymentErrors`），六条断言：

| # | 断言 |
|---|---|
| 1 | 兑付表的键集 MUST **恰等于**成因闭集（**双向**：欠付 / 空规格各自具名） |
| 2 | 每条凭据的 `marker` MUST **逐字存在**于 `file` 的**内容**里 |
| 3 | `file` MUST 已被 git 跟踪（**能读到**的工作树文件不是**仓库事实**，W75） |
| 4 | `file` MUST NOT 落在 `scripts/contracts/` 下（表自己写的凭据对表恒真） |
| 5 | 每个成因取值 MUST ≥1 在册条目（没有实例的规则对空集恒真） |
| 6 | `why` MUST 非空（把「这条凭据回答了哪个问题」写到明面上） |

### 决策 2：刻意**不加**第七条

原设计有 7 条：第七条是「逐条遍历在册条目的成因、必须有规格」。**删掉**，因为它是**纯集合论蕴含**：
既有断言已保证 `entry.reason ∈ 成因闭集`，加上第 1 条 ⇒ `entry.reason ∈ 键集`。
**能由结构蕴含的 MUST NOT 再具名一次。**

反过来说第 5 条（格非空洞）**不被蕴含**：既有断言只从条目**指向**闭集，**推不出**
「闭集每个取值都有在册条目」。

### 决策 3：逐出 `host-subprocess`（两处闭集 4 → 3）

**理由是决定性的**：

- ❌ **不实理由**：「实测 `schema --check` 现在本机可跑了，所以它不成立。」——这**引用了一次宿主
  瞬态**，换一台机器（或这台机器的下一次会话）结论就变 ⇒ 判据不可复现（**铁律二**）。
- ✅ **本 ADR 的理由**：「它的成因是**宿主/机器事实**（能不能派生子进程），而宿主瞬态 MUST NOT
  成为判定输入 ⇒ 它**不可能**有凭据 ⇒ 它作为在册成因里的一格是一个**永久无法兑付的空格**。」

正确理由**不引用任何发生过的瞬态**：即使本机从未跑通 `schema --check`，逐出结论**相同**。

### 决策 4：两条实例按不随宿主变化的事实重分类

| 层 | 实例 | 新成因 |
|---|---|---|
| 命令层 | `schema --check` | `requires-compiled-artifact` |
| step 层 | `Check generated contracts without modifying consumers` | `compiled-artifact` |

依据（`xtask/src/schema.rs` 实测）：`generate()` 在编译 proto 前**硬校验** `protoc` 版本
（`libprotoc 3.21.12`，不符即 `Err("generation requires exactly protoc 3.21.12")`）。
portable 门控的前提是「**只需 Rust 工具链**」（ADR 0041；ADR 0043 复述）⇒ 无论宿主装没装 protoc，
`schema --check` 都 MUST NOT 并入 `engineering_checks`，与 `test --suite contracts` **同因**。

### 决策 5：规格里的成因**计数**改为按类别分派

活规格有一句「成因是其余**三因**之一却带了平台限定条件」。计数是易漂移的
（**易漂移的计数 MUST NOT 进活文档**）⇒ 本次改为「成因**不是** `platform-toolchain` 却带了平台限定
条件」。去计数之后，因闭集收缩而需要的订正**一次性**完成。

### 决策 6：不强制凭据成对

原设想要求凭据成对（门控源 + 它引向的仓库文本）。放弃：**强制成对会逼出凑数的第二条**，
而「为凑数而补一条看起来相关的文本」正是本项目禁止的**装饰**。形状定为「至少一条」。

## 四、可做性算术：哪些成因**可做**、哪些**不可能有凭据**

| 成因 | 事因属于 | 凭据 |
|---|---|---|
| `superseded-by-workspace-run` | **仓库**（门控那条 `--workspace` 调用在） | ✅ 有 |
| `requires-compiled-artifact` | **跨仓库契约**（生成物由 protoc 编译产出） | ✅ 有 |
| `compiled-artifact` | 同上 | ✅ 有 |
| `platform-toolchain` | 同上（`tests/contracts/cpp` 需要 C++ 工具链） | ✅ 有 |
| `deliberate-on-demand` | **具名裁决**（ADR 0043 有文档依据） | ✅ 有 |
| `not-in-profile` | **仓库**（`package.json` 的 `spec:validate` 脚本） | ✅ 有 |
| `host-subprocess` | **宿主/机器** | ❌ **不可能有** ⇒ 逐出 |

⇒ 「可做性」的判据是：**这条成因能不能由仓库自身说清**，而**不是**「本机现在跑不跑得动」。

## 五、实现

1. **新建** `scripts/contracts/cause-payment.mjs`：`reasonPaymentErrors({label, where, closedSet,
   payments, reasons, sources, tracked})`。四个前置守卫（闭集非空 / 兑付表是对象 /
   `sources instanceof Map` / `tracked instanceof Set`）各返回单条**具名**错误。
   摆在 `scripts/contracts/` 是安全的：`tests/contracts/npm-scripts.test.mjs` 的扫描显式排除该目录，
   `TEST_EXTENSIONS = ['.test.mjs']` ⇒ 不会有「新 `.mjs` 未在册」的红。
2. `scripts/contracts/xtask-commands.mjs`：`STANDALONE_REASONS` 4 → 3；新增
   `STANDALONE_REASON_PAYMENTS`（3 格）；`commandErrors(...)` 加**第 4 个必填参数** `reasonInput`；
   `schema --check` 条目的 `reason` 由 `host-subprocess` 改 `requires-compiled-artifact`。
3. `scripts/contracts/workflow-steps.mjs`：`CI_ONLY_REASONS` 4 → 3；新增
   `CI_ONLY_REASON_PAYMENTS`（3 格）；`workflowStepErrors(...)` 加**第 5 个必填参数** `reasonInput`；
   step `Check generated contracts without modifying consumers` 的 `reason` 由 `host-subprocess`
   改 `compiled-artifact`。
4. **兑付表的可选注入位** `reasonInput.payments`（缺省用模块常量）：**没有它，「少一格／多一格／
   凭据指错」这几种缺陷无法被测试构造**（纪律：守卫都要注入验证）。
5. **`scripts/contracts/step-config.mjs` 一行不改**——立证：`git diff --stat
   scripts/contracts/step-config.mjs` **空输出**。它按闭集 `includes()` 分派，收缩**自动**正确。

**零迁移、零 Rust 改动**：本轮不改任何 `.rs`、`migrations/**`、`schemas/**`、生成物、
`migration::VERSION`、registry、验收 `status`。

## 六、测试与非空洞守卫

「先测后设计」的实证（**含一次自查纠错**）：第一次改 `xtask-commands.mjs` 时**同时**改了闭集
⇒ 违反纪律。**纠错**：把闭集**恢复为 4 值**，先跑一次让判据在**最原始的数据**上真红——
命令层 `pass 15 / fail 2`，两条红的**原因唯一**：

```
the cause "host-subprocess" is in the closed set but no payment says what it stands on
```

step 层同样：`pass 10 / fail 3`，原因亦唯一。⇒ **判据自己推出了那个设计决定**
（那一格永远无法兑付）。**然后**才执行收缩。

**扩面 MUST 带回归断言**：补齐全部调用点的新参数后首次跑，除「该红的那条」外**逐项全绿**
（命令层 16/17、step 层 12/13）⇒ 既有用例的结论**与扩张前逐项相同**。

**输入面扩张的连带面（实测）**：`commandErrors` 命令层 **15** 处调用点、`workflowStepErrors`
step 层 **15** 处调用点，**另加 3 个「正交性守卫」测试文件**
（`step-config.test.mjs`／`step-target.test.mjs`／`workflow-triggers.test.mjs`）也调用同一判据。
未传新参数会红（`the cause evidence sources were not provided as a map, so a receipt cannot be
told from a claim`）⇒ 按纪律**补齐前提**，**MUST NOT** 用短路关掉判据。

**「格非空洞」按整张注册表算**，不按单个 host 的 `entries` ⇒ 宁可重复不可漏报
（按 host 算会把「另一个文件里有实例」变**假阳性**）。

## 七、非空洞注入验证

方法：真改文件 + 真跑 + 逐字节还原。基准 = 过完 `cargo fmt --all`（**零改动** ⇒ 基准 == 当前工作区
状态），`sha256sum` 备份两个生产 `.mjs` + `xtask/src/lib.rs`。每组红数以 `node --test` 的
`pass/fail` **现测**为准；命中数 = 该组专有错误消息在输出里的出现次数。

| 组 | 注入内容 | 红数 | 命中消息 ×次 |
|---|---|---|---|
| I1 | 某格凭据 `marker` 换成不存在的串 | `15/3` | `receipt is a claim, not a fact` ×6 |
| I2 | 凭据 `file` 换成未被跟踪的路径 | `15/3` | `the repository does not track` ×6；`was not provided for judgement` ×**0** |
| I3 | 凭据 `file` 换成 `scripts/contracts/workflow-steps.mjs`（自引） | `15/3` | `true by construction` ×6 |
| I4 | 某格**零实例** | `14/4` | `vacuously true` ×6 |
| I5 | 兑付表**多**一个闭集外的键 | `15/3` | `declares a payment but is not in the cause table` ×6 |
| I6 | 兑付表**少**一个键 | `15/3` | `no payment says what it stands on` ×6 |
| I7 | 删掉凭据的 `why` | `15/3` | `does not say what it answers` ×6 |
| I8 | **删掉门控里那条 `--workspace` 调用** | `15/3` | `receipt is a claim, not a fact` ×6 |
| I9 | step 层 `platform-toolchain` 凭据 `marker` 换成不存在的串 | `11/3` | 同族 ×6 |
| I10 | step 层某格零实例 | `10/4` | `vacuously true` ×6 |

**I8 是本设计最强的兑付**：它的被审对象不是凭据表，而是**被凭据指向的那条仓库事实**
（`xtask/src/lib.rs` 里 `["test", "--workspace", "--locked", "--exclude", HARNESS]`）。
把那条调用从门控里删掉 ⇒ 凭据的 `marker` 在文件里**不再存在** ⇒ 红。
这正是「**会随仓库自身进展而过期**」这一失效模式的判据形状。

**排干扰（W93 教训）**：探针值 `no-such-receipt-at-all`／`no-such-step-receipt`／
`w108-surplus-cause`／`w108-surplus-step` 在注入前先 `grep -rlF` 确认**全部零命中**
（W93 曾因选了一个**真实存在**的串而得到假阴性）。`pass=0 fail=1` 是「**注入自身坏了**」的特征信号
⇒ 本轮**未出现**该形态。

**逐字节还原**：每组跑完 `cp` 备份拷回，末次 `sha256sum -c` **三行全 OK**；注入临时目录逐个删除，
`git status --porcelain` 只剩预期改动。

### 7.1 本轮的方法论产出：I2 暴露了一处**真测试缺陷**

首次跑 I2 得 `pass 3 / fail 15`，而 W108 专有消息**零命中**——表现为「测试自己坏了」而不是
「判据在响」。**根因**：测试的输入构造在读不到凭据文件时抛 `ENOENT`，把**判据该报的缺陷**
伪装成**测试框架的错误**。

**修法**：让输入构造**读不到不抛**（该文件不进 `sources`），由**判据**去具名报告。
修完 I2 重跑 = `15/3`、命中 **6** 处、`was not provided for judgement` **0** 次
⇒ **顺序正确**（先报「未跟踪」，不误报「未提供」）；五个测试文件的输入构造同步加固。

⇒ 一般化：**「测试的输入构造」也是判据链的一环**，它的失败会**盖住**判据该说的话；
注入验证的读数 MUST 先问「这条红是被审对象在响，还是夹具在响」。

## 八、门控与 validate

### 8.1 全量门控

`cargo xtask check --profile full` ⇒ **`EXIT=0`**，计数**现测**（口径：`grep -c '^test result:'`）：

Rust `^test result:` **95**（`Running` **78** 含 1 行伪目标 + `Doc-tests` **18**）/ **914 passed / 0 failed**；
Node TAP 根 **13** / `# tests` **319** / `# fail` **0**；日志内 `error[`／`LNK1104`／`out of memory`
命中 **0 / 0 / 0**。

`Running` 自洽：**78 − 1**（伪目标行
``Running `E:/nai-build-target\debug\nai-xtask.exe check --profile full` ``）**+ 18** = `test result:` **95**。

**归档后复跑门控**（`openspec/specs/**` 已被归档改动）⇒ **`EXIT=0`**，计数**逐项相同**
（95 / 78 / 18 / 914 / 0；Node 13 / 319 / 0；`error[`／`LNK1104`／`out of memory` 均 **0**）。

**对照 W107**（95 目标 / 914 passed、13 / 317）⇒ Rust **逐项相同**（本轮**零 Rust 改动**）、
Node **+2**（恰为两个既有套件各 +1：命令层 17 → 18、workflow 13 → 14）⇒ **计数变化逐项对得上**，
不是「口径变了」。

### 8.2 本轮的非一次即绿（**MUST 记住的两条宿主坑**）

| # | 现象 | 判据 | 处置 |
|---|---|---|---|
| 1 | `rustc` **挂死**：33 分钟零输出、累计 CPU 仅 **2.4 s**、日志 mtime 冻结 | 进程**无进展**（不是慢，是阻塞） | 终止后重跑 ⇒ 第二次**正常推进** |
| 2 | `LNK1104` 反复命中 `nai-engine` 的**测试链接目标**，且**每次点名的 exe 都不同**（`pipeline-696b…exe` → `engine-976f…exe`） | **逐目标能过**（`--no-run` 预热一次即过）；`Get-Process` 过滤 `E:/nai-build-target/*` **零残留**（排除「上一个产物进程占着 exe」） | **重试、绝不改代码** |

⇒ 两条都是**环境级**的：与「本机红 ≠ CI 红」同族。

### 8.3 OpenSpec validate

变更级 `openspec validate 2026-10-14-w108-cause-payment --strict` ⇒ **`is valid`**。

归档前（**现测**）：`--specs --strict` **43 passed / 0 failed**、`--all --strict` **44 / 0**、
`--changes` **1 / 0**。

归档后：`--archived` **106 passed / 0 failed**（W107 为 105 ⇒ **+1**）、`--all --strict` **43 / 0**。

归档 `engineering-foundation` ⇒ **`+1 added` / `~1 modified` / `-0` / `→0`**；活规格已就地更新
（新需求 `Every registered cause is paid for by evidence the repository can show`；被 MODIFIED 的需求里
「其余**三因**之一」已改为「成因**不是** `platform-toolchain`」）。

⚠️ 归档给了一条**非阻塞警告** `CHANGE_WHY_TOO_LONG`（`proposal.md` 的 Why 段超 1000 字符），
而 `--all --strict --archived` 仍 **0 failed** ⇒ 该警告**不**使严格校验失败（与 W83 同形）。

## 九、CI（**回填**）

⟦CI⟧

## 十、如实披露、遗留与边界

### 10.1 如实披露

1. **判据验的是「凭据存在」，不是「凭据与成因语义一致」。** 它**不能**验「这串文本消失了该成因就不再
   成立」——后者是**人的判断**，且成因是三合一的（仓库事实 + 跨仓库契约 + 触发方式）。本轮兑付面
   **只覆盖第一项**。`why` 字段与 ADR 0086 是披露的载体：**判据读 `marker`，人读 `why`。**
2. **同一个执行体在两层的成因不同。** `test --suite contracts` 在命令层归 `requires-compiled-artifact`，
   在 step 层归 `platform-toolchain`；`schema --check` 亦然。两层回答的是「**谁的范围**」
   （命令面 / workflow 步面）。这是**已存在的**事实，本轮**如实登记、不改变**它。
3. **本机这次能跑 `schema --check` 只作侦察见证，MUST NOT 出现在任何断言输入里**（铁律二）。
   本轮**没有**把它接进 `engineering_checks`——它的工具链前提（`protoc`）超出 portable 门控的既有
   不变式（ADR 0041），且那是**另一件事**（接线）。
4. **「格非空洞」按整张注册表算**，不按单个 host 的条目算。将来若有多个 workflow 文件，同一句话会在
   每个 host 上各报一次——**宁可重复，不可漏报**（按 host 算会让「另一个文件里有实例」变成假阳性）。
5. **I2 暴露了一处测试缺陷**（见 §7.1）：输入构造的异常会**盖住**判据该说的话。

### 10.2 遗留

1. **凭据与成因的语义对应不由判据审**（披露 1）。将来若某条凭据被「保持存在但语义已变」
   （例如同一句话被挪到完全不同的语境），判据不会红。
2. **两处闭集的交集为空**是当前的事实，不是被断言的属性。将来若两层真的共享一个成因名（同义），
   需要一条「同名必须同义」的断言——现在没有对象，MUST NOT 提前造。
3. **`infra` 的形状启发式**（ADR 0073 遗留 2）**未动**。
4. ADR 0073 遗留 3/4/5（步版本号进身份、四类非空、W90–W94 的其余遗留）**均未动**。

### 10.3 边界（本轮**不做**的事）

- **不把 `schema --check` 接进 `engineering_checks`**（见披露 3）。
- **不给 `host-subprocess` 编一条凭据**——它不可能有凭据，凑一条就是**装饰**。
- **不把 `host-subprocess` 改归 `deliberate-on-demand`**：那是用「有意」掩盖一条**前提约束**，
  把成因当措辞用。
- **不让凭据落在 `scripts/contracts/**`**：表自己写的凭据对表恒真。
- **不把「格非空洞」并进「双向」那一条**：欠付与空规格是两个方向，合并后只剩一个信号。
