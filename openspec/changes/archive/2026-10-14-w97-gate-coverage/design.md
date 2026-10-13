# w97 设计

## 一、问题形状

「本机门控到底测了什么」当前完全由**实参字面量**决定，而没有任何判据把这些字面量算成集合：

```
check --profile full
  └─ engineering_checks(root)
       ├─ run(root, &["test", "--workspace", "--locked", "--exclude", HARNESS])   ← 覆盖 18 个包
       └─ run(root, &["test", "--locked", "--package", HARNESS, "--lib"])         ← 覆盖  1 个包
                                                                    并集 = 19 = members 全集

test --suite storage   => run(root, &["test", …, "--package", "nai-storage",
                                                "--package", "nai-security"])     ← {2 个包}
test --suite contracts => schema::contracts(root)                                  ← {nai-contracts}（穿透）
```

两条 Gallus 线之间**没有任何对账**。W95 那 8 条 `localEvidence` 全填了同一个 marker：

```js
{file: 'xtask/src/lib.rs', marker: '["test", "--workspace", "--locked", "--exclude", HARNESS]'}
```

⇒ 「本机也跑」的证据是**超集**，不是**专属**。W95 的 `workflowStepErrors` 只做
`gate.get(file).includes(marker)` ⇒ 八份证据一次性全绿，而**没有任何一份**回答它自己那条 step 的问题。
这是 MEMORY §6「**每条来源 MUST 有独有覆盖**」在本层的缺口，也是同一个陷阱的**第六次**出现：

| 轮次 | 缺口 | 处置 |
| --- | --- | --- |
| W75 | 「来源存在」不验 ⇒ 假契约 | 注入事实（`repositoryFiles`） |
| W86 | 同上，另一层 | 注入事实（`readTrackedText`） |
| W94 | 「前置 hypothetically 达成」查不到 | 注入两张**事实表**，判据自己算 |
| W95 | 「本机也跑」没有凭据 | 注入 `gateSources`，marker 须逐字存在 |
| W96 | marker 可填 arm 自己的文本 ⇒ 自我满足 | **门控闭包区域**，由解析得出判定域 |
| **W97** | 一条超集 marker 一次签 8 个名 ⇒ 谁的覆盖都没被证明 | **解析出包集合，做包含关系计算** |

## 二、证据（三个 Mutation，改真文件 + 真跑）

.baseline（`members` **19**、包 **19**、suite **9**、门控 test 命令 **2**）：

```
suite        形态        包集
contracts    delegated   nai-contracts
knowledge    inline      nai-knowledge-core, nai-storage
provider     inline      nai-provider-core
scheduler    inline      nai-budget-core, nai-kernel, nai-storage
session      inline      nai-kernel, nai-media-core, nai-storage
kernel       inline      nai-kernel, nai-plugin-sdk
hosts        inline      nai-plugin-hosts, nai-plugin-sdk
storage      inline      nai-security, nai-storage
upgrade      inline      nai-storage, nai-upgrade-core

gate: test --workspace --locked --exclude nai-xtask   → 18 包
gate: test --locked --package nai-xtask --lib         →  1 包
union = 19 ⇒ 每个成员都被跑到
```

⇒ **当前是干净的**：每个 suite 的包集都被**单条**门控命令覆盖，每个成员都在并集里。

| Mutation | 改动 | 结果 |
| --- | --- | --- |
| **M1** | 把 `--workspace --exclude HARNESS` 换成 `-p nai-storage` | **9 个 suite 全部失去覆盖**，且 **17 个成员不再被任何门控跑到** —— 零信号 |
| **M2** | `--suite storage` 的一个 `-p` 改成幽灵包名 | 静态判据不发现；跑到那条 suite 才红（而本机门控**不跑**它） |
| **M3** | `members` 增加一个新成员 | 「它的测试有没有被跑」无人回答 |

第三层实测：CI 执行面共 **10 条** xtask 命令（`check --profile full`、`schema --check`、
8 个 `test --suite`），全部能解析成签名；**但没有任何判据**回 W96 的 dispatch 台账核实它们存在。

> 附带发现（**不写成判据**，见 §六）：`test --suite upgrade` 在 CI 执行面没有具名 step，
> 而 KNOWLEDGE 与 workflow 的四次提交历史里**都没有**关于它的记载。

## 三、设计决定

### 决定 1：判定单元是**包集合**，不是「marker 逐字存在」

`testCoverage(items)` 把一条 `["test", …]` 实参解析成一个集合：

| 形状 | 覆盖集 |
| --- | --- |
| `test --workspace` | 全部成员包名 |
| `test --workspace --exclude X` | 全部成员包名 \ {X} |
| `test -p a -p b` / `--package` | {a, b} |
| 赤裸 `cargo test` | 全部成员包名（保守方向） |

`--locked`／`--lib`／`--tests` 等旗标不参与集合计算（它们约束的是**目标类型**，不是**包**）。

⇒ **判据从此回答「包含的哪一边」**：V4 要求「存在**一条**门控命令，其覆盖集 ⊇ 该 suite 的包集」。

### 决定 2：不许用**并集**顶替 V4

若允许「∃ 若干条命令，其覆盖集的并集 ⊇ P(suite)」，那么有人把 `--workspace` 拆成
`-p a`、`-p b`、`-p c` 三条命令时，三条都能各自通过 ⇒ 覆盖率会在「看起来还在」的形态下
被悄悄切走（这正是 M1 想抓住的那一类漂移，只是切得更细）。⇒ **必须是单条**。

### 决定 3：`delegated` 形态必须**穿透**，且「driver」是静态登记表的一部分

`["test","--suite","contracts"] => schema::contracts(root)` —— 包集不在 arm 体里，
在 `xtask/src/schema.rs::contracts` 的函数体里（`&["test","--package","nai-contracts","--locked"]`）。

- 登记表逐条写 `driver: 'inline' | 'delegated'`（`SUITE_DRIVERS` 闭集）；
- `delegated` MUST 带 `file` 与 `fn`，判据**从被 asserting 对象的真源**读出它的 `-p`；
- **不可定位 ⇒ 具名失败**，绝不静默降级为空集（空集会让 V4 恒真）。

这是 W93「同一件事 MUST NOT 两处定义 ⇒ 调用第一处」的应用：**不在登记表里复写一份包集**，
包集永远从源码算出来。

### 决定 4：`--exclude HARNESS` 的标识符项必须先解析

`HARNESS` 是源码里的 `const`（= `env!("CARGO_PKG_NAME")`，实测值 `nai-xtask`）。数组项既可能是
字符串字面量也可能是标识符 ⇒ `topLevelItems` MUST 处理两种，并用一个 `identifiers: Map` 解析。
默认从 `xtask/Cargo.toml` 的 `name` 派生 `HARNESS`（**真源**），允许额外覆盖。
**解析不了 ⇒ 失败并分类**，不静默当作「没排除」（后者会让 workspace 覆盖多算一个包，进而把
缝隙掩盖成绿的）。

### 决定 5：闭世界在 **suite 表 ↔ dispatch** 这一层也要成立

「有哪些 suite」的**独立声明**是 `TEST_SUITES`；它由 W96 的 `dispatchSignatures()` 派生出的
`test --suite X` 集合做双向对账 ⇒ 新增/删除/改名两侧都被点名。**复用** W96 的解析器，
**不写**第二套 dispatch 解析（同一件事 MUST NOT 两处定义）。

### 决定 6：所有输入必须显式注入，缺一项即具名失败

入参对象：

```js
{
  rootToml,      // string  —— workspace members 的真源
  memberTomls,   // Map<path, text>  —— 每个成员的 Cargo.toml（包名）
  harnessText,   // string  —— xtask/src/lib.rs
  gateSources,   // Map<path, text>  —— 至少含 xtask/src/lib.rs 与 xtask/src/schema.rs
  workflowText,  // string  —— CI 执行面
  suiteRegistry, // ReadonlyArray —— TEST_SUITES（默认）
  identifiers,   // Map<string,string> —— 源码标识符 → 值（默认派生 HARNESS）
}
```

**判据自己不碰磁盘**（遍历工作区是 W75 的 F2 根因）。

### 决定 7：跨面 V6 —— CI 的每条 xtask 命令 MUST 在 dispatch 台账里在册

从 workflow 文本抽出 `cargo xtask <...>` 并剥掉 shell 尾缀（`2>&1 | tee "$RUNNER_TEMP/..."`、
`&&`、`;`），规范化成**签名**，再与 `dispatchSignatures(harnessText)` 对账。
当前 CI 的 **10 条** xtask 命令（剥掉 `tee` 尾缀并归一后）是
`check --profile full`、`schema --check` 与各 `test --suite <X>`，均与 dispatch 台账逐一对上。

## 四、与 W84 / W95 / W96 的边界（三者互不顶替）

| 模块 | 断言的对象 | 单元 |
| --- | --- | --- |
| `test-reachability.mjs` (W84) | 测试**文件**有没有被接上电 | `文件名` + marker |
| `workflow-steps.mjs` (W95) | CI 的 **step** 有没有被分类在册 | `step 身份`（名字/uses/run 首行） |
| `xtask-commands.mjs` (W96) | xtask 的 **CLI 命令**有没有在册、usage 是否对账 | `命令签名` |
| `gate-coverage.mjs`（本轮） | **被测面的包集合**由门控真的覆盖；CI 命令在 dispatch 在册 | `包名的集合` + `命令签名` |

⇒ 本轮**不重复** W95 的判断作。W95 那八条 `localEvidence` 的 marker 本轮**一个字都不改**
（它属于 W95 的对象）；本轮在它下**新的一层**补一条更强的要求：**证据要答到对象自己**。
报告 §五 会逐条核对：删 `TEST_SUITES` 一条 ⇒ 本模块红、W95 不红；删 `WORKFLOW_STEPS` 一条 ⇒ 反之。

## 五、注入清单

| 编号 | 注入 | 期望 |
| --- | --- | --- |
| P1 | **M1**：把门控的 `--workspace --exclude HARNESS` 换成 `-p nai-storage` | 红（每条失去覆盖的 suite 与每个不再被跑的成员都被点名） |
| P2 | **M2**：`--suite storage` 的 `-p` 改成幽灵包名 | 红（该包名不是任何成员的包名） |
| P3 | **M3**：`members` 增加一个新成员 | 红（新成员不被门控 test 命令的并集覆盖） |
| P4 | CI 里写一条 dispatch 没有的 `cargo xtask check --audit` | 红（CI 调用了一条不存在的命令） |
| P5 | 从 `TEST_SUITES` 删一条 | 红（dispatch 派生出的 suite 无人登记） |
| P6 | 往 `TEST_SUITES` 加一条 dispatch 没有的 suite | 红（登记了一个不存在的套件） |
| P7 | `driver` 写成域外值 | 红（域外 driver 是标签不是分类） |
| P8 | `memberTomls` 少给一份 | 红（缺输入必须具名） |
| P9 | ` HARNESS` 给不出解析 | 红（未解析的标识符不得静默当作「没排除」） |
| P10 | 合法登记原样 | **不**红 |

## 六、明确不做

- **不判「每条 suite 命令 MUST 有 CI step」**：`upgrade` 缺 step 的成因在仓库里无记载，
  给它配一个闭集成因就是给未知事实贴标签（W90）⇒ 只登记为发现。
- **不验「某个包里真的有 `#[test]`」**：`--workspace` 对一个没有测试函数的包也是合法覆盖；
  有没有测试是 cargo 的事。
- **不验 CI 的运行结果**：静态判据只回答「这条命令是不是真的存在」。
- **不改动任何被测对象的实参**：本轮只补判据与登记。
