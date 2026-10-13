# ADR 0075：被测面的集合必须由解析算出并交叉核验

- **日期**：2026-10-14
- **状态**：已接受（Corresponding change：`openspec/changes/2026-10-14-w97-gate-coverage`）
- **关联**：ADR 0073（CI 执行面的闭世界）、ADR 0074（本机命令面的闭世界）、W90 的成因闭集纪律、
  W92 的空态纪律、W93 的「同一件事 MUST NOT 两处定义」

## 背景

「本机门控到底测了哪些 Rust 包」完全由**实参字面量**决定：

```rust
run(root, &["test", "--workspace", "--locked", "--exclude", HARNESS])?;  // 覆盖 18 个包
run(root, &["test", "--locked", "--package", HARNESS, "--lib"])?;        // 覆盖  1 个包
["test", "--suite", "storage"] => run(root, &["test", "--locked",
                                              "--package", "nai-security"]),
```

而 ADR 0073 那 8 条 suite step 的 `localEvidence` **全部填了同一个 marker**
（`["test", "--workspace", "--locked", "--exclude", HARNESS]`），其判据只做
`gate.get(file).includes(marker)`。

⇒ 一条 `--workspace` 的证据回答的是「**全 workspace 被跑了**」，**没有一条**回答
「**这个 suite 自己的包集被跑了**」。**用超集冒充专属。**

三个 Mutation（改真文件 + 真跑）确认它是一条真缝：

| 编号 | 改动 | 结果 |
| --- | --- | --- |
| M1 | 把门控的 `--workspace --exclude HARNESS` 换成 `-p nai-storage` | **9 个 suite 全部失去覆盖、17 个成员不再被测**，零信号 |
| M2 | `--suite storage` 的一个 `-p` 改成幽灵包名 | 静态判据不发现；跑到那条 suite 才红（而本机门控**不跑**它） |
| M3 | 往 `members` 加一个新成员 | 「它的测试有没有被跑」无人回答 |

第三层：CI 执行面写着 10 条 `cargo xtask …`，而 ADR 0073 只认它们「**形状**像一条命令」
（`JUDGE_COMMAND_SHAPES`），没有回 ADR 0074 的 dispatch 台账核实它们存在。
**相邻三层各自闭合，层与层之间没有接线。**

## 决定

### 1. 判定结果必须是**集合的包含关系**，不是字符串的逐字存在

把 `["test", …]` 实参数组解析成一个覆盖集合：`--workspace`（可经 `--exclude` 挖洞）
与 `-p/--package` 两种形态（`COVERAGE_KINDS` 闭集）。此后判据回答的是「**谁包含谁**」。

`--locked`／`--lib`／`--tests` **不参与集合计算**：它们约束的是**目标类型**，不是**包**。

### 2. Inclusion 必须由**单一条**门控命令满足，不许用并集拼

若允许「若干条命令的并集 ⊇ P(suite)」，把 `--workspace` 悄悄拆成 `-p a`、`-p b`、`-p c`
三条时每一条都能通过 ⇒ 覆盖率会在「看起来还在」的形态下被切走。
已由 `tests/contracts/gate-coverage.test.mjs` 的
「coverage by a union of commands is not coverage」用例钉住。

### 3. 包集永远从源码算出，登记表不写第二份

`TEST_SUITES` 只声明 **`driver`**：`inline`（包集在 arm 体里）或 `delegated`
（包集在被调函数体里，如 `schema::contracts`）。⇒ W93 的纪律适用：**同一件事 MUST NOT 两处定义**。
`TEST_SUITES` 不承担包集，只承担「从哪算」——它与 dispatch 派生的 suite 集合做**双向**闭世界。

### 4. `-p` 的每个值 MUST 落在一个真实成员的包名上；标识符项 MUST 先解析

`HARNESS` 是源码 const（`env!("CARGO_PKG_NAME")`，实测 `nai-xtask`），
由 `xtask/Cargo.toml` 的 `name` **派生**（不写死）。解析不了 ⇒ **具名失败**；
把它当成「没有 `--exclude`」会让 workspace 覆盖多算一个包，从而把缝隙掩盖成绿的。

### 5. 空态 MUST 被显式断言（W92）

driver 被标错形态时（例如 `contracts` 其实是 `schema::contracts(root)` 却标成 inline），
它的体里没有 `["test", …]` ⇒ 包集算成 **∅** ⇒ inclusion 对空集**恒真**而没有信号。
⇒ 加两条：`MUST 至少解析出一条 test 命令`、`包集 MUST 非空`。

### 6. 注释不是实参

实现一个 `stripRustComments`，并且**显式跟踪字符串状态**（`"https://x"` 里的 `//` 不是注释）。
不剥离注释时，一句注释里举了个 `[…]` 形状的例子就会被当成实参去解析
——**把散文当成代码**，与 W91「同名 ≠ 同一个」同源。

### 7. 跨面：CI 的每条 xtask 命令 MUST 在 dispatch 在册

复用 `xtask-commands.mjs` 的 `dispatchSignatures`（**不写第二套解析**），
把 CI 文本里的 `cargo xtask <…>` 剥掉 shell 尾缀（`2>&1`、`| tee …`、`&&`、`;`）后与之对账。

## 不做的事

- **不判「每条 suite 命令 MUST 有 CI step」**：`test --suite upgrade` 当前在 CI 执行面没有具名
  step，而 KNOWLEDGE 与 workflow 的四次提交历史里**都没有**关于它的记载。给它配一个闭集成因
  就是**给未知事实贴标签**（W90）⇒ 本轮只把这件事作为**新发现的事实**登记进报告 §八遗留。
- **不验「某个包里真的有 `#[test]`」**：`--workspace` 对没有测试函数的包也是合法覆盖。
- **不验 CI 的运行结果**，只回答「这条命令是不是真的存在」。
- **不改任何被测对象的实参**：本轮只补判据与登记。

## 后果

- `scripts/contracts/gate-coverage.mjs`（新）+ `tests/contracts/gate-coverage.test.mjs`（新，**24 用例**）。
- Node 套件 **7 → 8**；`test-reachability` 站点 **9 → 10**；
  `xtask/src/lib.rs` **+1 行登记（含注释 6 行）**，Rust **零行为改动**。
- 三条改前全绿的漂移现在会红：门控列表被拆分（M1）、包名漂移（M2）、成员失去覆盖（M3）。
