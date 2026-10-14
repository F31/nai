# Design — W108 成因 MUST 付代价：可自证的凭据，或逐出

## 1 缝在哪里

`ci-only` / `standalone` 两类条目各自 MUST 给一个成因，成因 MUST 取自闭集。闭集**建立**在
W95（step 层）与 W96（命令层）。但闭集**之内**的每一格，今天是这样被验的：

```js
// scripts/contracts/workflow-steps.mjs:369
if (typeof entry.reason !== 'string' || !CI_ONLY_REASONS.includes(entry.reason)) { … }
```

一次 `includes()`。它答的是「这个成因**在不在**表里」，**不答**「这个成因**凭什么成立**」。
两处空位（皆实测）：

| 空位 | 实测量 |
|---|---|
| 成因本身没有仓库内的凭据 | 两处常量是**唯一**的书写处；`grep -rn 'CI_ONLY_REASONS\|STANDALONE_REASONS'` 命中 4 个 `.mjs`（2 定义 + 2 消费），**零**凭据字段 |
| 成因格不要求有实例 | `tests/contracts/workflow.test.mjs:147` 与 `tests/contracts/xtask-commands.test.mjs:187` 都只 `assert.ok(…length > 0, 'the cause table must not be empty')` |

第二条的形状值得单说：它验的是「**闭集非空**」，不是「**每一格非空**」。一条
`deliberate-on-demand` 若在某轮被全部改归别因，格子仍在常量里，它的规则**对空集恒真**
（W78 母题：规则表没有实例 ⇒ 规则恒真）。

## 2 触发：ADR 0073 遗留 1 预言的失效

ADR 0073（W95）「如实披露」第 2 条与「遗留」第 1 条，逐字：

> **不验 `ci-only` 的成因是否仍然成立**：例如 `schema --check` 哪天在本机变得可跑，判据不会有信号
> （`reason` 仍写着 `host-subprocess`）。要发现它得靠人重新实测宿主。

2026-10-14 实测：

```
$ cargo xtask schema --check
catalog checked: 33 capabilities (27 frozen, 6 draft); 112 protobuf messages
TS codecs generated for 10 protobuf sources
Rust/TS/C++/JSON Schema/OpenAPI generation checked with no drift
$ echo $?   # 0
```

而 `scripts/contracts/xtask-commands.mjs:126-130` 里那条登记**逐字未变**：

```js
{ signature: 'schema --check', kind: 'standalone', reason: 'host-subprocess' },
```

⇒ 一条**被预言会失效**的声明**真的失效了**，而**全部判据零信号**。本仓纪律：
「**一条声明若会随仓库自身进展而过期，判据 MUST 配一条会过期的断言，否则它就是遗忘**」。
这正是那个遗忘。

## 3 可做性判定：哪些成因能付，哪些付不出

**可做性算术**（本轮的核心）：

| 成因 | 它成立靠什么 | 能不能从仓库自证 |
|---|---|---|
| `superseded-by-workspace-run` | 门控里那条 `--workspace` 调用**在** | **能**——凭据就是那条调用 |
| `requires-compiled-artifact` / `compiled-artifact` | 仓库里有文本**要求** protoc / 编译产物 | **能**——凭据就是那段要求 |
| `platform-toolchain` | 执行体里**有** `cmake` 建的 C++ 子工程 | **能**——凭据就是那段构建 |
| `not-in-profile` | 那些脚本**真实存在**，只是没接进 profile | **能**——凭据就是脚本定义 |
| `deliberate-on-demand` | 有一条**具名的裁决**说「不并入」 | **能**——凭据就是那条裁决 |
| **`host-subprocess`** | **宿主/机器能不能派生子进程** | **不能**——见 §4 |

⇒ 五格「因**仓库**之事」或「因**跨仓库契约**之事」⇒ 有凭据、可做。
一格「因**宿主/机器**之事」⇒ 不可能有凭据。**这一格必须逐出，不是给它配一条假凭据。**

## 4 逐出 `host-subprocess`：理由是「不可能有凭据」

**理由的措辞是关键的**，因为它决定本设计符不符合铁律二：

- ❌ **错误理由**：「实测 `schema --check` 现在本机可跑了，所以 `host-subprocess` 不成立。」
  ——这**引用了一次宿主瞬态**，换一台机器（或这台机器的下一次会话）结论就变 ⇒ 判据不可复现。
- ✅ **正确理由**：「它的成因是**宿主事实**，而宿主瞬态 MUST NOT 成为判定输入（铁律二）
  ⇒ 它**不可能**有凭据 ⇒ 它作为在册成因里的一格是一个**永久无法兑付的空格**。」

正确理由**不引用任何发生过的瞬态**：即使本机从未跑通 `schema --check`，逐出结论**相同**
（因为「能不能派生子进程」永远不是仓库能自证的事）。⇒ 结论对宿主变化**免疫**。

并且它与既有裁决自洽：ADR 0078 决策 5 已把 `host-subprocess` 明文归入「与 runner 的 OS 无关」
那一侧——一条「与平台无关」的成因若在闭集里零实例，会与「规则表每格 MUST ≥1 实例」冲突。

### 4.1 两条 `host-subprocess` 实例的去处：重分类为「需要编译产物」

`host-subprocess` 逐出后，它的两条实例（同一条命令的两个层面）必须重新归类。
**不是**随手塞进一格，而是按**今日实测的**事实重新归类：

| 层 | 实例 | 新成因 |
|---|---|---|
| 命令层 | `schema --check` | `requires-compiled-artifact` |
| step 层 | `Check generated contracts without modifying consumers` | `compiled-artifact` |

理由（`xtask/src/schema.rs:207-217` 实测）：

```rust
let protoc = std::env::var_os("PROTOC").unwrap_or_else(|| "protoc".into());
let version = Command::new(&protoc).arg("--version").output()…?;
if !version.status.success()
    || String::from_utf8_lossy(&version.stdout).trim() != "libprotoc 3.21.12"
{
    return Err("generation requires exactly protoc 3.21.12".into());
}
```

即：**它硬校验 `protoc` 3.21.12**。而 portable 门控的前提是「**只需 Rust 工具链**」
（ADR 0041 裁决、ADR 0043 第 116 行复述）。⇒ 无论宿主装没装 protoc，
`schema --check` 都 MUST NOT 并入 `engineering_checks`——与 `test --suite contracts` 同因。

**这个理由不随宿主变化**：本机**恰巧**装了 protoc，不代表 portable 门控可以**依赖**它。
（这也解释了为什么「本机能跑」与「该不该进门控」是两件事——见 §9.2。）

## 5 凭据的形状与逐格取值

### 5.1 形状

```js
{ <reason>: { evidence: [ {file, marker, why}, … ] } }
```

- 每格 **≥1 条**凭据；每条三字段。
- `file`：凭据所在的仓库路径。
- `marker`：**逐字**存在的子串。
- `why`：这条凭据**回答了什么**（非空）。把人的判断写到明面上，供人审——判据验不了它，
  但它让「凭据与成因的对应」成为**可读的**而不是隐含的。

### 5.2 逐格取值（全部经 `grep -F -c` 实测唯一）

**命令层** `STANDALONE_REASON_PAYMENTS`：

| 成因 | 在册实例数 | `file` | `marker` | 唯一性实测 |
|---|---|---|---|---|
| `requires-compiled-artifact` | 2 | `xtask/src/schema.rs` | `generation requires exactly protoc 3.21.12` | `1` |
| `superseded-by-workspace-run` | 8 | `xtask/src/lib.rs` | `["test", "--workspace", "--locked", "--exclude", HARNESS]` | `1` |
| `deliberate-on-demand` | 3 | `xtask/src/golden.rs` | `pub fn check(` | `1` |
| 〃 | 〃 | `docs/adr/0043-sbom-generation-and-scope.md` | `` 暂不并入 `engineering_checks` `` | `1` |

**step 层** `CI_ONLY_REASON_PAYMENTS`：

| 成因 | 在册实例数 | `file` | `marker` | 唯一性实测 |
|---|---|---|---|---|
| `compiled-artifact` | 1 | `tests/contracts/descriptor.test.mjs` | `needs protoc, so these belong to the contracts suite` | `1` |
| `platform-toolchain` | 1 | `xtask/src/schema.rs` | `"tests/contracts/cpp"` | `1` |
| `not-in-profile` | 2 | `package.json` | `"spec:validate": "openspec validate --all --strict --no-interactive"` | `1` |
| 〃 | 〃 | `package.json` | `"@fission-ai/openspec"` | `1` |

三点说明：

1. **`superseded-by-workspace-run` 的凭据正是那条 `--workspace` 调用本身**。它也是 step 层五条
   `local` 步的 `localEvidence` —— 两处引用**同一个仓库事实**，但回答的是两个不同的问题
   （「本机门控真的跑它」vs「门控跑全 workspace 测试覆盖同一批包，只是不走这个入口」）。
   这不是「同一件事两处定义」：两处的**断言**不同，只是**证据源**相同。而这条证据一旦被破坏，
   本仓会**立刻静默牺牲包**（W97 已实测）——它是最值得配凭据的一条。
2. **`platform-toolchain` 的凭据指向执行体**（`xtask/src/schema.rs` 里 `cmake -S tests/contracts/cpp`），
   **不是** workflow 里那个 `Install Linux C++ protobuf runtime` 步。理由：后者是**被审宿主**自身的
   另一个步，用它做凭据会让两个在册条目互为证据；前者跨到执行体，是真正的外部兑付。
3. **`not-in-profile` 的两条凭据覆盖它的两条实例**：`spec:validate` 脚本的定义（实例 1 能跑）
   与 `@fission-ai/openspec` 依赖（实例 2 用的 CLI 装了）。这个「覆盖」是**人的判断**，
   判据验不了——见 §9.1。

### 5.3 为什么用「一组凭据」而不是「强制成对」

原设想要求凭据**成对**（门控源 + 它引向的仓库文本）。放弃的理由：**强制成对会逼出凑数的第二条**。
`not-in-profile` 的两条实例各自只有一条自然凭据；为了凑成对而补一条「看起来相关」的文本，
正是本仓禁止的**装饰**。⇒ 形状定为「**至少一条**」，条数由成因自己决定（实测 1–2 条）。

## 6 判据的六条

两个校验函数（`commandErrors` / `workflowStepErrors`）各自新增：

| # | 断言 | 方向 |
|---|---|---|
| 1 | 兑付表的键集 MUST 恰等于成因闭集 | **双向**（各报一次） |
| 2 | 每条凭据的 `marker` MUST 逐字存在于 `file` 的内容里 | 读文件 |
| 3 | `file` MUST 已被 git 跟踪 | 仓库成员（W75 的 `readTrackedPaths`） |
| 4 | `file` MUST NOT 落在 `scripts/contracts/` 下 | 防自引 |
| 5 | 每个成因取值 MUST ≥1 在册条目 | 格非空洞 |
| 6 | `why` MUST 非空 | 人可审 |

### 6.1 为什么**没有**「逐条遍历在册条目」这一条

第一版设计有 7 条，第 1 条是「每个在册条目的成因 MUST 在兑付表里有规格（逐条遍历、非抽样）」。
**它被蕴含，因此删掉**：

```
既有断言：entry.reason ∈ CI_ONLY_REASONS      (workflow-steps.mjs:369)
新断言 1：keys(CI_ONLY_REASON_PAYMENTS) == CI_ONLY_REASONS
────────────────────────────────────────────────────────────
蕴含：    entry.reason ∈ keys(CI_ONLY_REASON_PAYMENTS)
```

纯集合论。（`standalone` 一侧同形。）**能由结构蕴含的 MUST NOT 再具名一次**——多写一条不会
多一个信号，只会多一处将来会漂移的定义。

⚠️ 但「**格非空洞**」(#5) **不**被蕴含：既有断言只从条目**指向**闭集，推不出「闭集每个取值
都**有**在册条目」。⇒ 它必须独立存在。这一对（#1 双向 / #5 格非空洞）是不可合并的两个方向：
**欠付**（有人声明、没人付）与**空规格**（有人付、没对象）。

**为什么 #1 的两个方向各自具名**：它们挡住的东西不同。#1 的方向一挡「一条在册条目的成因
**没人兑付**」；方向二挡「一个兑付规格**没有对象**」。合并成一个信号会让两类缺陷共用一句话。

**为什么 #2 与 #3 都要**：#2 挡「凭据是一句不存在的话」；#3 挡「凭据住在一个**存在但未被提交**的
文件里」（`target/` 下的产物、临时探针）。后者**能读到内容**却**不是仓库事实**——正是 W75
立「仓库成员断言」要挡的那类。

**为什么 #4 必要**：凭据若指向 `scripts/contracts/**`（被审注册表自身），就变成
「**这条成因的凭据是由这条成因所在的表写的**」——恒真。防自我满足。

## 7 输入面必须扩张：既有输入不足以判定

`commandErrors(text, registered, gateSources)` 拿到的是：宿主全文 + 被审表 + 门控源 map。
凭据核验还需要两样**它现在没有**的东西：

| 需要 | 为什么不能省 |
|---|---|
| 每条凭据文件的**全文** `Map<path, text>` | #3 要把它读出来比对；文件读不到 ⇒ **具名失败**，MUST NOT 静默跳过 |
| **已跟踪路径集** `Set<string>` | #4 要区分「仓库内容」与「工作树里的游离文件」 |

⇒ 两个函数各加一个**必填**参数 `reasonInput = {sources, tracked}`。缺 `sources` 或缺
`tracked`（不是 `Map` / 不是 `Set`）⇒ **具名失败**。

这条扩张是**刻意的**：本仓纪律「**扩面 MUST 带回归断言（旧口径逐项不变）**」
⇒ 既有 20 余条用例的调用点全部补上新参数，且**逐项结论 MUST 与扩张前相同**。

## 8 两张表不是「同一件事的第二处定义」

两处成因闭集**本就不同**：

| | 命令层 | step 层 |
|---|---|---|
| 闭集 | `requires-compiled-artifact` / `superseded-by-workspace-run` / `deliberate-on-demand` | `compiled-artifact` / `platform-toolchain` / `not-in-profile` |
| 判定单元 | 命令的**签名** | 步的**身份** |
| 回答的问题 | 「这条命令为什么不在**本机门控闭包**里」 | 「这一步为什么**只有 CI** 跑」 |

**交集是空的**（逐出前也只共享 `host-subprocess` 一个名字，而它的语义在两层亦不同：
命令层指「执行体要派生子进程」，step 层指「这一步要派生子进程」）。⇒ 两张兑付表是
**两件事各自的规格**，不是同一件事写了两遍。

**并且**同一个执行体在两层的成因确实不同（实测）：`test --suite contracts` 在**命令层**归
`requires-compiled-artifact`（依赖 protoc），而它在 **step 层**归 `platform-toolchain`
（CI 上要 `Install Linux C++ protobuf runtime` 那一步）。这**不是**不一致——两层回答的是
「谁的范围」（命令面 / workflow 步面）。本轮**如实披露**它，**不改变**它。

## 9 如实披露

### 9.1 判据验的是「凭据存在」，不是「凭据与成因语义一致」

#2 只能验「这串文本在那个文件里」。它**不能**验「这串文本消失了该成因就不再成立」——
后者是**人的判断**，且成因是三合一的（仓库事实 + 跨仓库契约 + 触发方式）。
⇒ 本轮的兑付面**只覆盖第一项**。`why` 字段与 ADR 是本披露的载体：**人读 `why`，判据读 `marker`。**

### 9.2 「本机能跑」与「该不该进门控」是两件事

本轮实测 `schema --check` 本机可跑，但**没有**把它接进 `engineering_checks`：
它的工具链前提（protoc）超出 portable 门控「只需 Rust 工具链」。这条区分是**结构性的**，
不是本轮的选择。⚠️ 本机的这次实测只作**侦察见证**，MUST NOT 出现在任何断言输入里（铁律二）。

### 9.3 逐出不是「消灭了一格」

`host-subprocess` 从两处闭集消失，**没有**让任何判定**变弱**：它的两条实例都改归了一格
**本来就有凭据**的成因。收缩后两处闭集**每格仍 ≥1 实例**（命令层 2/8/3，step 层 1/1/2，皆实测）。

## 10 替代方案与取舍

1. **只给成因加凭据，保留 `host-subprocess`** —— 不行：它不可能有凭据，只能配一条**假凭据**
   （例如引 ADR 0078 里那句「与 runner 的 OS 无关」），那是装饰。
2. **把 `host-subprocess` 改归 `deliberate-on-demand`（「有意按需」）** —— 不行：不实。
   `schema --check` 不在 profile 里**不是**「有意按需」的选择，而是**工具链前提**所致；
   用「有意」掩盖一条前提约束，是把成因当措辞用。
3. **用「本机跑一次试试」判成因真假** —— 不行：宿主瞬态 MUST NOT 成为判定输入（铁律二），
   换一台机器对同一仓库会得出不同分类。
4. **凭据强制成对** —— 不行：会逼出凑数的第二条（§5.3）。
5. **让凭据落在 `scripts/contracts/**`** —— 不行：自我满足（§6 #5）。
6. **把「格非空洞」并进「双向」一条断言** —— 不行：欠付与空规格是两个方向，合并后
   只剩一个信号（§6）。
7. **判据用「凭据缺失 ⇒ 跳过核验」的可选参数** —— 不行：静默降级。输入缺失 MUST 具名失败，
   否则判据在被削弱时保持沉默（本仓既有形状）。
8. **顺手把 `schema --check` 接进 `engineering_checks`** —— 不行：会让 portable 门控多一个
   protoc 前置，破坏 ADR 0041 的既有不变式；且那是**另一件事**（接线），不属本轮。
