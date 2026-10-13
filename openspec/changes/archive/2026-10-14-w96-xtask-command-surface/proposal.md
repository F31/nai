# w96：`cargo xtask` 的每一条命令必须具名在册，且 usage 与 dispatch 必须对账

## Why

`xtask/src/lib.rs` 的 `execute()` 是本机门控的**入口面**：一个
`match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice()` 的 **24 条臂**。
这个集合**没有任何在册台账**，而它就住在同一个文件里、与 `check --profile full` 的门控闭包
相隔几十行。同一个文件里的用法说明字符串（`Usage: cargo xtask …`）是命令集合的
**第二次书写**，两处**没有任何机制**保证它们一致。

三个探针实证（本轮实测，改前/改后都真跑）：

| 探针 | 改动 | 本机 4 个 Node 套件（registry 52 / schema 30 / platform 22 / workflow 13） |
| --- | --- | --- |
| P1 加法 | 加一条臂 `["check", "--audit"] => Ok(())` | **全绿、零信号** |
| P2 减法 | 删掉 `["check", "--secrets"] => secrets::check(root)` | **全绿**——一条判定命令**静默消失** |
| P3 漂移 | usage 字符串追加 `\| check --audit` | **全绿**——usage 与 dispatch 分家，无人发现 |

`grep` 证实：**没有任何测试**读 `Usage:` 字符串或 match 臂（`grep -rn "Usage:" tests/` 为空）。
⇒ 24 条命令与它们的用法声明**只存在于代码里**，没有第二双眼睛。

同族的既有先例：W84 给「**测试文件**有没有被接上电」补了闭世界，W86 给「**OpenAPI 文档集**」补，
W95 给「**CI workflow 的 step**」补。本轮给「**`execute()` 的命令面**」补——同一手法，
**新的一层对象**。

## What Changes

1. **新增 `scripts/contracts/xtask-commands.mjs`**：
   - **命令面闭世界**：`dispatchSignatures(text)` 从 `execute()` 的 match 臂解析出命令的
     **签名**集合，每条 MUST 有且只有一条登记，在册的每条 MUST 仍在 dispatch 里（**双向**）。
     判定单元 MUST 是**签名**（空格连接的顶层字面量序列），MUST NOT 是裸命令名——W91 的教训：
     「同名」≠「同一个」。`check --golden` 与 `check --golden --dir` 是**两条**命令，签名区分它们。
   - **usage 双向对账**：`usageSignatures(text)` 从 `Usage:` 字符串展开出命令集合。
     `usage ⊆ dispatch`（usage 不得声称一条不存在的命令）；
     `dispatch \ usage ⊆ HELP_SIGNATURES`（除三个 help 类别外，每条命令 MUST 被 usage 列出）。
   - **四类闭集** `COMMAND_KINDS`：
     `gate`（执行体在**本机门控闭包**里被执行）、`standalone`（是判定命令，但**不在**门控闭包里）、
     `artifact`（生成产物、**不产生判定结论**）、`help`（不产生动作的入口）。
   - **`gate` 必须付代价**：每条 SHALL 引出一处**真的存在**的门控落点 `{file, marker}`，
     且该 marker MUST 在 **`engineering_checks` 的函数体 ∪ `check --profile …` 各 arm 的函数体**
     内**逐字存在**——否则「本机门控真的跑它」是一句空声明（W75「已声明 ≠ 已执行」）。
   - **`standalone` 必须付代价**：每条 SHALL 给出**成因**，成因 SHALL 取自一个**闭集**
     （`host-subprocess`／`requires-compiled-artifact`／`superseded-by-workspace-run`／
     `deliberate-on-demand`）。域外成因 ⇒ 具名失败（W90：一个未知成因是一个标签，不是一个理由）。
   - **双向**：`gate` MUST 带 `evidence`、`standalone` MUST NOT 带 `evidence` 且 MUST 带 `reason`
     （触发条件只由「字段存在」决定，W83）。
2. **新增 `tests/contracts/xtask-commands.test.mjs`**：承载上述判据，并**在 `xtask/src/lib.rs` 的
   `engineering_checks` 登记**——同时**在 `scripts/contracts/test-reachability.mjs` 的
   `TEST_REGISTRATION_SITES` 登记站点**（否则 W84 的闭世界会立刻报「这个测试文件没人跑」，
   这正是它该有的行为）。
3. **规格**：`openspec/specs/engineering-foundation/spec.md` 新增 1 条 Requirement（ADDED）。

## Impact

- 零 Rust **行为**改动（`xtask/src/lib.rs` 只加一行登记，不改任何一条 arm、不改 usage 一个字符）；
  零新依赖；零 workspace 包增减。
- 新增 1 个 Node 测试文件 ⇒ 门控目标数不变，**Node 套件由 6 个变 7 个**。
- **一处与 W95 的边界**：`workflow-steps.mjs` 断言的是「**CI workflow 的 step** 有没有被分类在册」；
  本模块断言的是「**xtask 的 CLI 命令**有没有被分类在册」。两者共享宿主
  （`xtask/src/lib.rs` 与 `.github/workflows/foundation.yml` 互相调用），但**断言的单元不同**，
  MUST NOT 互相顶替（本轮报告的 §五 逐条核对）。
- **明确不做**：不验「CI 真的跑了某条命令」（那要读运行结果）；不验 `standalone` 的成因是否成立
  （语义，仍由人读，与 ADR 0054 决策 2 同族的边界）；不改 `execute()` 与 usage 的任何一个字符。
