# ADR 0086：成因 MUST 付代价——可从仓库自证的凭据，或从闭集逐出

- 状态：已接受
- 日期：2026-10-14
- 轮次：W108
- 关联：[ADR 0073](0073-workflow-step-closure.md)（步层闭世界——本 ADR 关闭它的**遗留 1**）、
  [ADR 0078](0078-step-configuration.md)（执行配置面——本 ADR 收缩它的成因枚举）、
  [ADR 0041](0041-engineering-profile-frontend-boundary.md)（「portable 只需 Rust 工具链」）、
  [ADR 0043](0043-sbom-generation-and-scope.md)（`deliberate-on-demand` 的裁决记录）

## 背景

W95 给 CI workflow 的 step 集建了**闭世界**，`ci-only` 的每条 MUST 给出成因且成因 MUST 取自
`CI_ONLY_REASONS`；W96 给 `cargo xtask` 的命令面建了同形的一层，`standalone` 的成因取自
`STANDALONE_REASONS`。判据对闭集**之内**的每一格只做一次 `includes()`：

```js
if (!CI_ONLY_REASONS.includes(entry.reason)) { … }   // scripts/contracts/workflow-steps.mjs
```

它答「这个成因**在不在**表里」，**不答**「这个成因**凭什么成立**」。留下两处空位：

1. **成因本身没有仓库内的凭据。** `superseded-by-workspace-run` 断言「门控跑的是全 workspace
   测试，覆盖同一批包」——这句话若哪天不再成立（那条 `--workspace` 调用被换成显式包列表），
   成因格照旧成立、判据照旧全绿。
2. **成因格不要求有实例。** 两处测试都只宣布闭集**非空**
   （`assert.ok(CI_ONLY_REASONS.length > 0, 'the cause table must not be empty')`）。
   某一格若被**合法清空**，格子仍在常量里，它的规则**对空集恒真**。

这是本仓的母题之一（W78 的规则表、W95 的 `CONDITION_SHAPES`）：**规则表没有实例 ⇒ 规则恒真**。

### 触发：一条被预言的失效真的发生了

ADR 0073 的「遗留 1」逐字写着：

> **不验 `ci-only` 的成因是否仍然成立**：例如 `schema --check` 哪天在本机变得可跑，判据不会有
> 信号（`reason` 仍写着 `host-subprocess`）。要发现它得靠人重新实测宿主。

2026-10-14 实测 `cargo xtask schema --check` **跑通了**（退出码 `0`，输出
`Rust/TS/C++/JSON Schema/OpenAPI generation checked with no drift`），而注册表里那条登记**逐字未变**
（`reason: 'host-subprocess'`），**全部判据零信号**。一条**会随仓库自身进展而过期**的声明，
没有任何判据盯着它——**遗忘**的形状。

## 决策

### 1 成因 MUST 付代价

两处各建一张**兑付规格表**（成因闭集本就不同 ⇒ 两张表不是「同一件事的第二处定义」）：

```
{ <reason>: { evidence: [ {file, marker, why}, … ] } }
```

每格 **≥1 条**凭据。六个校验函数共享的核验（`scripts/contracts/cause-payment.mjs`）：

| # | 断言 |
|---|---|
| 1 | 兑付表的键集 MUST **恰等于**成因闭集（**双向**：欠付 / 空规格各自具名） |
| 2 | 每条凭据的 `marker` MUST **逐字存在**于 `file` 的内容里 |
| 3 | `file` MUST 已被 git 跟踪（能读到的工作树文件不是仓库事实，W75） |
| 4 | `file` MUST NOT 落在 `scripts/contracts/` 下（表自己写的凭据对表恒真） |
| 5 | 每个成因取值 MUST ≥1 在册条目（没有实例的规则对空集恒真） |
| 6 | `why` MUST 非空（把「这条凭据回答了哪个问题」写到明面上） |

⚠️ **刻意不加**「逐条遍历在册条目的成因、必须有规格」：既有断言已保证
`entry.reason ∈ 成因闭集`，加上第 1 条 ⇒ `entry.reason ∈ 键集` 是**纯集合论蕴含**。
**能由结构蕴含的 MUST NOT 再具名一次。**

### 2 逐出 `host-subprocess`

两处闭集 4 → 3。**理由是关键的**：

- ❌ **不实理由**：「实测 `schema --check` 现在本机可跑了，所以它不成立。」——这**引用了一次宿主
  瞬态**，换一台机器（或这台机器的下一次会话）结论就变 ⇒ 判据不可复现（**铁律二**）。
- ✅ **本 ADR 的理由**：「它的成因是**宿主/机器事实**（能不能派生子进程），而宿主瞬态 MUST NOT
  成为判定输入 ⇒ 它**不可能**有凭据 ⇒ 它作为在册成因里的一格是一个**永久无法兑付的空格**。」

正确理由**不引用任何发生过的瞬态**：即使本机从未跑通 `schema --check`，逐出结论**相同**。
⇒ 结论对宿主变化**免疫**。并且它与 ADR 0078 决策 5 自洽：那里已把 `host-subprocess` 明文归入
「与 runner 的 OS 无关」那一侧，而一条「与平台无关」的成因若在闭集里零实例，会与
「规则表每格 MUST ≥1 实例」冲突。

### 3 两条实例按今日实测的事实重分类

| 层 | 实例 | 新成因 |
|---|---|---|
| 命令层 | `schema --check` | `requires-compiled-artifact` |
| step 层 | `Check generated contracts without modifying consumers` | `compiled-artifact` |

依据（`xtask/src/schema.rs` 实测）：`generate()` 在编译 proto 前**硬校验** `protoc` 版本
（`libprotoc 3.21.12`，不符即 `Err("generation requires exactly protoc 3.21.12")`）。
portable 门控的前提是「**只需 Rust 工具链**」（ADR 0041；ADR 0043 复述）⇒ 无论宿主装没装 protoc，
`schema --check` 都 MUST NOT 并入 `engineering_checks`，与 `test --suite contracts` 同因。
**这个理由不随宿主变化**：本机恰巧装了 protoc，不代表门控可以**依赖**它。

### 4 规格里的成因计数改按类别分派

`openspec/specs/engineering-foundation/spec.md` 有一句「成因是其余**三因**之一却带了平台限定条件」。
计数是易漂移的（MUST NOT 进活文档）⇒ 本次把它改为「成因**不是** `platform-toolchain` 却带了平台
限定条件」。去计数之后，本起因闭集收缩而需要的订正**一次性**完成。

实现侧无需改动：`step-config.mjs` 的分派本来就是
`CI_ONLY_REASONS.includes(entry.reason) && restricted`（遍历闭集，不写死取值）⇒ 收缩**自动**正确。
（已核对立证：`git diff --stat scripts/contracts/step-config.mjs` 空输出。）

### 5 逐格凭据（全部经 `grep -F -c` 实测唯一）

命令层：`requires-compiled-artifact` ← `xtask/src/schema.rs` 的 `generation requires exactly protoc 3.21.12`；
`superseded-by-workspace-run` ← `xtask/src/lib.rs` 的 `["test", "--workspace", "--locked", "--exclude", HARNESS]`；
`deliberate-on-demand` ← `xtask/src/golden.rs` 的 `pub fn check(` **加**
`docs/adr/0043-sbom-generation-and-scope.md` 的 `` 暂不并入 `engineering_checks` ``。

step 层：`compiled-artifact` ← `tests/contracts/descriptor.test.mjs` 的
`needs protoc, so these belong to the contracts suite`；`platform-toolchain` ← `xtask/src/schema.rs` 的
`"tests/contracts/cpp"`；`not-in-profile` ← `package.json` 的 `"spec:validate": …` **加** `"@fission-ai/openspec"`。

两点刻意的选择：

- **`superseded-by-workspace-run` 的凭据就是那条 `--workspace` 调用本身**（它同时是五条 `local` 步的
  `localEvidence`）。两处引用**同一个仓库事实**，但回答两个不同的问题（「本机门控真的跑它」vs
  「门控跑全 workspace 测试覆盖同一批包，只是不走这个入口」）⇒ 不是同一件事的第二处定义。
  而这条证据一旦被破坏，本仓会**立刻静默牺牲包**（W97 已实测）——它最值得配凭据。
- **`platform-toolchain` 的凭据指向执行体**，不是 workflow 里那个装 C++ 运行时的步：
  后者是**被审宿主**里的另一条目，用它做凭据会让两个在册条目互为证据。

### 6 不强制凭据成对

原设想要求凭据成对（门控源 + 它引向的仓库文本）。放弃的理由：**强制成对会逼出凑数的第二条**——
为凑数而补一条「看起来相关」的文本，正是本项目禁止的**装饰**。形状定为「至少一条」，
条数由成因自己决定（实测 1–2 条）。

## 如实披露

1. **判据验的是「凭据存在」，不是「凭据与成因语义一致」。** 它**不能**验「这串文本消失了该成因就
   不再成立」——后者是**人的判断**，且成因是三合一的（仓库事实 + 跨仓库契约 + 触发方式）。
   本轮兑付面**只覆盖第一项**。`why` 字段与这份 ADR 是披露的载体：**判据读 `marker`，人读 `why`。**
2. **同一个执行体在两层的成因不同。** `test --suite contracts` 在命令层归 `requires-compiled-artifact`，
   在 step 层归 `platform-toolchain`；`schema --check` 亦然（`requires-compiled-artifact` / `compiled-artifact`）。
   两层回答的是「谁的范围」（命令面 / workflow 步面）。这是**已存在的**事实，本轮**如实登记、不改变**它。
3. **本机这次能跑 `schema --check` 只作侦察见证，MUST NOT 出现在任何断言输入里**（铁律二）。
   本轮**没有**把它接进 `engineering_checks`：它的工具链前提（protoc）超出 portable 门控的既有不变式。
4. **「格非空洞」按整张注册表算**，不按单个 host 的条目算。将来若有多个 workflow 文件，
   同一句话会在每个 host 上各报一次——**宁可重复，不可漏报**（按 host 算会让「另一个文件里有实例」
   变成假阳性）。
5. **I2 注入暴露了一处测试缺陷**：输入构造在读不到凭据文件时抛 `ENOENT`，把「判据该报的缺陷」
   伪装成「测试自己坏了」（`pass 3 / fail 15` 而专有消息**零命中**）。修法是让输入构造**不抛**，
   由**判据**去具名报告；五个测试文件的输入构造同步加固。

## 遗留

1. **凭据与成因的语义对应不由判据审**（见披露 1）。将来若某条凭据被「保持存在但语义已变」
   （例如同一句话被挪到完全不同的语境），判据不会红。
2. **两处闭集的交集为空**是当前的事实，不是被断言的属性。将来若两层真的共享一个成因名
   （同义），需要一条「同名必须同义」的断言——现在没有对象，MUST NOT 提前造。
3. **`infra` 的形状启发式**（ADR 0073 遗留 2）**未动**。
4. ADR 0073 遗留 3/4/5（步版本号进身份、四类非空、W90–W94 的其余遗留）**均未动**。

## 替代方案

1. **只给成因加凭据、保留 `host-subprocess`**：它不可能有凭据，只能配一条**假凭据**
   （例如引 ADR 0078 里那句「与 runner 的 OS 无关」）——那是装饰。**否决。**
2. **把 `host-subprocess` 改归 `deliberate-on-demand`（「有意按需」）**：**不实**。
   `schema --check` 不在 profile 里**不是**「有意按需」的选择，而是**工具链前提**所致；
   用「有意」掩盖一条前提约束，是把成因当措辞用。**否决。**
3. **用「在本机跑一次试试」判成因真假**：宿主瞬态 MUST NOT 成为判定输入（铁律二）。**否决。**
4. **凭据强制成对**：会逼出凑数的第二条（决策 6）。**否决。**
5. **让凭据落在 `scripts/contracts/**`**：表自己写的凭据对表恒真，自我满足。**否决。**
6. **把「格非空洞」并进「双向」那一条**：**欠付**（有人声明、没人付）与**空规格**（有人付、没对象）
   是两个方向，合并后只剩一个信号。**否决。**
7. **判据用「凭据缺失 ⇒ 跳过核验」的可选参数**：静默降级。输入缺失 MUST 具名失败，
   否则判据在被削弱时保持沉默。**否决。**
8. **顺手把 `schema --check` 接进 `engineering_checks`**：会让 portable 门控多一个 protoc 前置，
   破坏 ADR 0041 的既有不变式；且那是**另一件事**（接线），不属本轮。**否决。**

## 关联

- 规格：`openspec/specs/engineering-foundation/spec.md` 的新增需求
  「Every registered cause is paid for by evidence the repository can show」；
  以及被 MODIFIED 的「Every registered workflow step's execution configuration is paid for by
  a parsed platform set」（成因侧去计数）。
- 关闭：ADR 0073 遗留 1（本 ADR 的「决策 1」配上了会过期的断言）。
- 报告：[`docs/reports/2026-10-14-w108-cause-payment.md`](../reports/2026-10-14-w108-cause-payment.md)
- 变更：`openspec/changes/archive/2026-10-14-w108-cause-payment/`
- 记忆：`.workbuddy/memory/KNOWLEDGE.md` §6.33；`AGENTS.md` 的 W108 段
