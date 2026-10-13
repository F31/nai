# W97 轮次报告：被测面的集合必须由解析算出并交叉核验

- 日期：2026-10-14
- 变更：`openspec/changes/2026-10-14-w97-gate-coverage/`
- ADR：[0075](../adr/0075-gate-coverage.md)

## 一、母题

W95 管「CI 侧执行面」（`foundation.yml` 的 step 集合），W96 管「本机门控的入口面」
（`execute()` 的 24 条臂）。两层各自的闭世界**都成立了**——但它们之间**没有接线**，
以及各自内部的「**被测面**」也没有被算出来。

具体缝：「门控测哪些 Rust 包」与「每个 `test --suite` 测哪些包」，都由一串**实参**决定：

```rust
run(root, &["test", "--workspace", "--locked", "--exclude", HARNESS])?;  // 覆盖 18 个包
run(root, &["test", "--locked", "--package", HARNESS, "--lib"])?;        // 覆盖  1 个包
["test", "--suite", "storage"] => run(root, &["test", "--locked",
                                              "--package", "nai-security"]),
```

而 W95 那 8 条 suite step 的 `localEvidence` **全部填了同一条 marker**：

```js
{file: 'xtask/src/lib.rs', marker: '["test", "--workspace", "--locked", "--exclude", HARNESS]'}
```

一条 `--workspace` 的证据回答的是「**全 workspace 被跑了**」，**没有一条**回答
「**这个 suite 自己的包集被跑了**」——**用超集冒充专属**。这是同一个陷阱的**第六次**出现：

| 轮次 | 缺口 | 处置 |
| --- | --- | --- |
| W75 | 「来源存在」不验 ⇒ 假契约 | 注入事实（`repositoryFiles`） |
| W86 | 同上，另一层 | 注入事实（`readTrackedText`） |
| W94 | 「前置是否已达成」查不到 | 注入两张**事实表**，判据自己算 |
| W95 | 「本机也跑」没有凭据 | 注入 `gateSources`，marker 须逐字存在 |
| W96 | marker 可填 arm 自己的文本 ⇒ 自我满足 | **门控闭包区域**，由解析得出判定域 |
| **W97** | 一条超集 marker 一次签 8 个名 | **解析出包集合，做包含关系计算** |

第三层：CI 执行面写着 **10 条** `cargo xtask …`，而 W95 只认它们「**形状**像一条命令」
（`JUDGE_COMMAND_SHAPES`），没有回 W96 的 dispatch 台账核实它们真的存在。

## 二、核实（先测后设计）

### 2.1 基线（三个纯 JS 探针，解析两侧）

```
members 19（含 xtask）        packages 19（每个成员自己的 Cargo.toml 的 name）
gate 测试命令 2
  test --workspace --locked --exclude nai-xtask   → 18 包   （HARNESS 由 xtask/Cargo.toml 派生）
  test --locked --package nai-xtask --lib         →  1 包
  union = 19 ⇒ 每个成员都被跑到

suite（9 条，形态 / 包集）
  contracts  delegated → schema::contracts@xtask/src/schema.rs   nai-contracts
  knowledge  inline    nai-knowledge-core, nai-storage
  provider   inline    nai-provider-core
  scheduler  inline    nai-budget-core, nai-kernel, nai-storage
  session    inline    nai-kernel, nai-media-core, nai-storage
  kernel     inline    nai-kernel, nai-plugin-sdk
  hosts      inline    nai-plugin-hosts, nai-plugin-sdk
  storage    inline    nai-security, nai-storage
  upgrade    inline    nai-storage, nai-upgrade-core
```

⇒ **当前是干净的**：每个 suite 的包集都被**单条**门控命令覆盖，每个成员都在并集里。

### 2.2 三个 Mutation（改真文件 → 真跑 → 从备份拷回 → `sha256` 逐字校验）

| 编号 | 改动 | 结果 |
| --- | --- | --- |
| **M1** | 把门控的 `--workspace --exclude HARNESS` 换成 `-p nai-storage` | **9 个 suite 全部失去覆盖 + 17 个成员不再被测，零信号** |
| **M2** | `--suite storage` 的一个 `-p` 改成幽灵包名 | 本机门控不发现；跑到那条 suite 才红（而本机门控**不跑**它） |
| **M3** | 往 `members` 增加一个新成员 | 「它的测试有没有被跑」无人回答 |

**M3 的如实更正**：进一步实测发现，`--workspace` 形态对成员集合是**自适应**的——
`cargo test --workspace` 确实会跑到新成员 ⇒ 判据**不**该报警。真缺口在
「门控改用**显式**列表并漏掉某个成员」那一天。**这轮我先是期望错，然后是判据没错**
（MEMORY §6：「实测的差异」≠「正确的设计」）。

### 2.3 第三层（CI 命令面）

CI 文本里 `cargo xtask …` 共 **10 条**（`check --profile full`、`schema --check`、8 条 `test --suite`）。
剥掉 shell 尾缀（`2>&1 | tee "$RUNNER_TEMP/nai-gate.log"`）后可归一成签名。
**没有任何判据**把它们与 dispatch 台账对账。

> 附带发现（**不写成判据**，见 §八）：`test --suite upgrade` 在 CI 执行面没有具名 step，
> 而 KNOWLEDGE 与 workflow 的四次提交历史里**都没有**关于它的记载。

## 三、判据形态（`scripts/contracts/gate-coverage.mjs`）

判定单元从「marker 逐字存在」升级为「**解析出的包集合 + 包含关系**」。

| 判据 | 内容 |
| --- | --- |
| **V1** | suite 登记表（`TEST_SUITES`，9 条）与 dispatch 派生的 suite 集合**双向**闭世界 |
| **V2** | 包集由 `driver` 从源码算出：`inline`（arm 体）/ `delegated`（穿透 `schema::contracts` 的函数体）；不可定位 ⇒ 具名失败 |
| **V3** | 每个 `-p` 值 MUST 落在真实成员包名上；`HARNESS` 由 `xtask/Cargo.toml` **派生**而非写死，解析不了 ⇒ 具名失败 |
| **V4** | 每个 suite 的包集 MUST 被门控里**单一条** test 命令覆盖——**不许用并集拼** |
| **V5** | 每个 workspace 成员 MUST 被这些 test 命令的**并集**覆盖 |
| **V6** | CI 的每条 `cargo xtask <签名>` MUST 在 dispatch 台账里在册（**复用** W96 的 `dispatchSignatures`） |

三个设计要点：

1. **`TEST_SUITES` 只声明「从哪算」，不承担包集**（W93：同一件事 MUST NOT 两处定义）——
   登记表写 `driver`，包集永远从源码解析。
2. **`--locked`／`--lib`／`--tests` 不参与集合计算**：它们约束的是**目标类型**，不是**包**。
3. **注释不是实参**：新增登记的注释里写了一句 `[…]` 形状的例子，**当场被判据当成真的实参数组**
   （`…` 无法解析 ⇒ 具名失败）⇒ 加 `stripRustComments`，且须**跟踪字符串状态**
   （`"https://x"` 里的 `//` 不是注释；用行正则会吃掉字符串尾巴）。

## 四、注入验证（真改文件 + 真跑 + `sha256` 逐字还原）

还原统一改为「**备份文件拷回**」——本轮有一次 `git checkout --` 丢了后来的注疏编辑
（index 落后于工作区所致），已登记为操作坑（KNOWLEDGE §11）。

| 编号 | 注入 | 结果 |
| --- | --- | --- |
| P1（M1） | 门控改成 `-p nai-storage` | 27 条错误，**每个 suite 逐一点名** + 17 个成员失去覆盖 |
| P2（M2） | `-p nai-security` → `nai-phantom` | `the suite "storage" tests the package "nai-phantom", which is not a package of this workspace` |
| P3（M3） | `members` 加新成员 + 门控改显式列表 | `the workspace member crates/nai-newcomer … is tested by no command in the local gate closure` |
| P4 | CI 加 `cargo xtask check --audit` | `the CI workflow calls "cargo xtask check --audit", which the harness does not dispatch` |

P1 的具名样本：

```
the suite "contracts" tests "nai-contracts", and no single command in the local gate closure covers all of it — coverage by a union of commands is not coverage
the suite "knowledge" tests "nai-knowledge-core", "nai-storage", and no single command … 
…（9 条逐一点名）
the workspace member plugins/avatar-2d (package "nai-avatar-2d") is tested by no command in the local gate closure — member added but never run is a test surface that quietly shrank
```

另有 **24 个用例内**的 F 级注入（覆盖 V1 两个方向、driver 双向、delegated 缺源\缺 fn、幽灵包名、
并集顶替、members 增加、CI 幽灵命令、shell 尾缀、缺 memberTomls、标识符不可解析、空输入、
空登记表、重复登记、注释 vs 字符串、覆盖形态消失、driver 清空）。

## 五、与 W84 / W95 / W96 的边界（四者互不顶替）

改真实表 → 跑两个套件 → 从备份拷回 → `sha256` 校验：

| 改动 | 本模块 | 对方模块 |
| --- | --- | --- |
| 删一条 `TEST_SUITES` | gate-coverage **4 红** | workflow **0 红** |
| 删一条 `WORKFLOW_STEPS` | gate-coverage **0 红** | workflow **1 红** |
| 删一条 `XTASK_COMMANDS` | gate-coverage **0 红** | xtask-commands **3 红** |
| 删新增的 `test-reachability` 站点 | gate-coverage **0 红** | registry **2 红** |

⇒ 四者分别管「被测面的包集合」／「workflow 的 step」／「`execute()` 的 CLI 命令」／
「测试文件的可达性」。本轮**不改动** W95 那八条 `localEvidence`（那是 W95 的对象）。

## 六、门控与 validate

### 6.1 Rust

`xtask/src/lib.rs` **+1 行登记**（含注释 6 行），**Rust 零行为改动**——
不改任何被测实参，不加/不改任何一条 arm。新增的 24 个用例住在 Node 侧。

### 6.2 Node

套件 **7 → 8**（新增 `tests/contracts/gate-coverage.test.mjs` **24 个用例**）；
portable 侧 **182** = registry 52 + schema 30 + platform 22 + workflow 13 + xtask-commands 17 + **24**。
`test-reachability` 站点 **9 → 10**；`registry.test.mjs` 逐字清单同步（**如期变红，是它该有的行为**）。

### 6.3 全量门控 `cargo xtask check --profile full`（EXIT=0，2m56s）

| 项 | 结果 |
| --- | --- |
| Rust 目标（`test result` 行数） | **91** |
| Rust 用例 | **862 passed / 0 failed** |
| `LNK1104` | **0** |
| Node 套件（**8 个**） | registry **52** / schema **30** / platform **22** / workflow **13** / xtask-commands **17** / **gate-coverage 24（本轮新增）** / desktop **5** / ui **52** |

- Rust 侧与 W96 **逐项相同**（91 / 862）——本轮 Rust **零行为改动**，只加一行登记（含注释 6 行）。
- portable 侧合计 **182** passed（52 + 30 + 22 + 13 + 17 + 24）。

### 6.4 归档与两条 validate

| 项 | 结果 |
| --- | --- |
| `openspec archive 2026-10-14-w97-gate-coverage --yes` | ✓ Complete；`engineering-foundation` **+1 added** |
| 非阻塞警告 | **本轮确实有** `Why section should not exceed 1000 characters`（现测，不是照抄上一轮） |
| `openspec validate --all --strict`（归档后） | **43 passed / 0 failed** |
| `openspec validate --all --strict --archived` | **95 passed / 0 failed**（W96 为 94，+1） |

（归档后 MUST 立刻跑 `--archived`：tasks 未勾时 `archive --yes` **只给非阻塞警告**，
失败只出现在这一条上——W82 实测 `9 incomplete tasks`。）

## 七、CI

（推送后的单次低频查询结果回填于此：唯一查询参数破坏缓存 + `Cache-Control: no-cache` +
回读 `head_sha` == 推送 SHA。）

## 八、遗留

1. **`test --suite upgrade` 在 CI 执行面没有具名 step**，而仓库里**没有任何记载**说明原因
   （已查 KNOWLEDGE、workflow 的 4 次提交历史与 `git log -S`）。本轮**不判**这条：
   给它配一个闭集成因就是**给未知事实贴标签**（W90）。
2. **不验「签名的取值」**：V6 认的是命令签名，不认运行时实参的值。
3. **不验「某个包里真的有 `#[test]` 函数」**：`--workspace` 对没有测试函数的包也是合法覆盖。
4. **`workspace` 形态对成员集合自适应** ⇒ V5 只在「改用显式列表」时才有信号（见 §2.2 的如实更正）。
5. **AGENTS.md 里 W96 段的归档路径已就地订正**（原文写 `openspec/changes/2026-10-14-w96-…`，
   归档后真路径在 `archive/` 下）——活文档就地订正，并在此登记。
6. **W90 §八遗留 1／3／4／5、W91 的跨语言键级消费、W92 的基线收录、W93 的锚点唯一性、
   W94 的 `needs_external` 审计、W95 的 `ci-only` 成因、W96 的五项遗留**均**未动**。

## 九、如实披露

1. **不断言 CI 的执行结果**——那要读运行结果，不是静态判据的事（除非 §七 有运行级证据）。
2. **不验「某个 suite 真的跑了它的包」的执行事实**：判据验的是「实参算出来的集合被包含」，
   不是「那次运行的输出」。
3. **`delegated` 只穿透一层**：`schema::contracts` 若再往下调，包集会看不到（当前它直接写 `-p`）。
4. **`bracketBodies` 不计字符串里的括号**：宿主情形已被 `stripRustComments` 缓解，
   但 `.rs` 里若有含 `[`/`]` 的字符串路径仍可能被当成数组——当前 **`xtask/src/lib.rs` 与
   `xtask/src/schema.rs` 均无此情形**，将来出现时会先被 V3（包名不真）或「不可解析」那条拦下。
5. **本轮不改任何被测对象的实参**：补的是判据与登记。
