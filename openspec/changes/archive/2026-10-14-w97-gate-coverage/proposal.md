# w97：被测面的集合必须由解析算出并交叉核验——门控跑哪些包、每个 suite 跑哪些包、CI 调用哪些命令，三者之间必须有接线

## Why

本机门控 `cargo xtask check --profile full` 到底**测了哪些 Rust 包**，由一串实参决定：

```rust
run(root, &["test", "--workspace", "--locked", "--exclude", HARNESS])?;
run(root, &["test", "--locked", "--package", HARNESS, "--lib"])?;
```

而 9 条 `test --suite <X>` 各自也由一串实参决定自己测哪些包：

```rust
["test", "--suite", "storage"] => run(root, &["test", "--locked",
                                             "--package", "nai-storage",
                                             "--package", "nai-security"]),
```

**这两组集合之间没有任何判据**。W95 的 `workflow-steps.mjs` 要求每个 CI step 声称「本机也跑」时必须
给出 `{file, marker}`，但它**只验 marker 在 `xtask/src/lib.rs` 里逐字存在**——于是 8 条 suite step
全部填了同一条：

```js
localEvidence: {file: 'xtask/src/lib.rs', marker: '["test", "--workspace", "--locked", "--exclude", HARNESS]'}
```

一条 `--workspace` 的证据回答的是「**全 workspace 被跑了**」，**没有一条**回答「**这个 suite 自己的
包集被跑了**」。用超集冒充专属——这是同一个陷阱的**第六次**出现（`repositoryFiles` → `readTrackedText`
→ W94 两张事实表 → W95 `gateSources` → W96 门控闭包区域 → 本轮：包含关系必须被**算出来**，
不许由一个「恰好存在」的字符串顶替）。

实测（三个 Mutation，改真文件、真跑）：

| Mutation | 改动 | 当前工艺的结果 |
| --- | --- | --- |
| M1 | 把门控的 `--workspace --exclude HARNESS` 改成 `-p nai-storage` | **9 个 suite 全部失去覆盖 + 17 个成员不再被测，零信号** |
| M2 | 把 `--suite storage` 的一个 `-p` 改成不存在的包名 | 要到跑那条 suite 才会红；本机门控与静态判据**都不会发现** |
| M3 | 往 `Cargo.toml` 的 `members` 加一个新成员 | 它的 Rust 测试是否被跑到，**无人回答** |

第三层：CI 的 workflow 里写着 `cargo xtask test --suite storage`、`check --profile full`、
`schema --check`。W95 只认「这一行**形状**像一条 xtask 命令」（`JUDGE_COMMAND_SHAPES`），
**没有回到 W96 的 dispatch 台账核实这条命令真的存在**。⇒ 相邻两层各自闭合，层与层之间没有接线。

## What Changes

1. **新增 `scripts/contracts/gate-coverage.mjs`**：把「被测面」从字符串提升到**解析出来的集合**，五条判据：
   - **V1 闭世界（suite 表 ↔ dispatch）**：从 `execute()` 派生出的 `test --suite X` 集合，与独立声明的
     `TEST_SUITES` 登记表**双向**一致（新增/删除/改名两侧都被点名）。
   - **V2 每个 suite 的包集 MUST 被解析得出**：`inline`（arm 体内联 `run(root, &["test", …])`）与
     `delegated`（调 `schema::contracts` ⇒ **穿透**进它的函数体读 `-p`）两种形态；不可解析 ⇒ 具名失败。
   - **V3 `-p` 的每个值 MUST 是真包名**：每个 `-p <pkg>` MUST 落在一个 workspace 成员的真实包名上
     （`--exclude HARNESS` 这类标识符项先经 `identifiers` 解析；解析不了 ⇒ 失败，**不静默降级**）。
   - **V4 每个 suite 的包集 MUST 被门控里**某一条** test 命令的覆盖集包含**：不许用**并集**凑
     （正是 M1 的守卫）。
   - **V5 每个 workspace 成员 MUST 被门控 test 命令的并集覆盖**（M3 的守卫）。
   - **V6 CI 的每条 `cargo xtask <签名>` MUST 在 W96 的 dispatch 台账里在册**（跨面接线；
     复用 `xtask-commands.mjs` 的 `dispatchSignatures`，**不重复**第二套解析）。
2. **新增 `tests/contracts/gate-coverage.test.mjs`**：承载判据，并登记进
   `xtask/src/lib.rs::engineering_checks` 与 `test-reachability.mjs` 的站点表。
3. **规格**：`openspec/specs/engineering-foundation/spec.md` 新增 1 条 Requirement（ADDED）。

## Impact

- 零 Rust **行为**改动（`xtask/src/lib.rs` 只加一行登记），零新依赖，零包增减。
- Node 套件由 **7 个 → 8 个**。
- **不做**「每条 suite 命令 MUST 有 CI step」这条对账：`test --suite upgrade` 当前在 CI 执行面
  没有具名步骤，而仓库里**没有任何记载**说明为什么（已查 KNOWLEDGE 与 workflow 提交历史）。
  给它配一个成因等于**给未知事实贴标签**（W90：一个未知成因是一个标签，不是一个理由）
  ⇒ 本轮只把它作为**新发现的事实**登记进报告 §八遗留，不写成判据。
- **明确不做**：不验「CI 真的跑了某条命令」（要读运行结果）；不验「某个包里有没有测试函数」
  （`--workspace` 对没有测试的包也是合法覆盖）；不改任何被测对象的实参。
