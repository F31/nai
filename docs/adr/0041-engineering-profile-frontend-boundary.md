# ADR 0041：工程 profile 与前端门控的边界

- 状态：接受
- 日期：2026-10-11
- 相关：[ADR 0001](0001-workspace-layout-and-naming.md)、`openspec/specs/engineering-foundation/spec.md`、`xtask/src/frontend.rs`、`scripts/frontend-gate.mjs`

## 背景

`engineering-foundation` 的 **Reproducible workspace** 需求从 D01 起就写着工程 SHALL「支持锁定依赖的编译、
**类型检查和前端构建**」，Scenario 也要求「workspace 编译、测试和**前端检查**可执行」。但任务工具里没有这个入口：
`cargo xtask check --profile portable` 的分支只做六件事（`check-deps`、`check --registry`、`fmt --all --check`、
`clippy --workspace --all-targets`、`cargo test --workspace --exclude nai-xtask`、`cargo test -p nai-xtask --lib`），
前端一行都没有。

而前端检查**一直存在**，只是从未被门控覆盖：仓库有两个前端包，`@nai/contracts` 与 `@nai/desktop` 各自在
`package.json` 里定义了 `typecheck`/`build`，桌面包另有 `test`，根上还有 `test:ui`（`tests/ui`）。
历次记录里它们**始终与 portable 并列**写成人工结果（「…portable、contracts 十三类消息互通、schema 无漂移与
**前端检查**通过」、「两处 `tsc` EXIT=0；desktop 5/5、`test:ui` 15/15」）——并列的写法本身就说明：
**这些是有人记得时才有的一次人工检查**。

缺口的准确性质因此不是「前端没有检查」，而是「前端检查没有门控入口」：`portable` 跑绿**完全不能排除**前端已破。

## 决策

### 1. 新增 `check --profile full`，`portable` 原样不动

`portable` 的名字与规格都在承诺「只要 Rust 工具链就能跑」。把前端步骤塞进去会让「刚 clone、尚未 install」
的环境立刻变红——那既不是 Rust 破了也不是前端破了，只是依赖没装；同时历次报告的 `portable` 数字口径会被改写。

`full` = `portable` 的**全部**步骤 + 前端门控。两者共用同一段实现（`engineering_checks`），
因此「`full` 包含 `portable` 的全部步骤」是**代码结构事实**，而不是一份需要同步维护的清单。

### 2. 「前端怎么跑」交给 Node 脚本，不在 Rust 里调 `pnpm`

`xtask` 当然可以 `Command::new("pnpm")`，但那会在本机**必然失败，且失败原因与前端无关**：本仓库的 `node_modules`
由 Linux 容器安装，`node_modules/.bin/*` 是一千字节左右的**无扩展名 POSIX 垫片**，Windows 上既不能直接执行也不是
可识别命令；`pnpm` 本身也不保证在 PATH 上。让门控依赖「`pnpm` 在 PATH 且垫片与本机 ABI 匹配」，等于把
**主机环境事实**混进门控结论。

而「前端该怎么检查」天然是 JS 生态的问题：读 `package.json`、解析 npm 包入口、展开 glob。所以职责一分为二：

- `xtask/src/frontend.rs`：**何时跑**（`full` 的最后一步）与**失败怎么报**；
- `scripts/frontend-gate.mjs`：**怎么跑**，不依赖 `pnpm`，也不依赖 `.bin` 垫片。

`node` 取 `NODE` 环境变量、缺省 PATH 上的 `node`，与既有 `run()` 取 `CARGO` 的写法一致。

### 3. 命令取自 `package.json` 的 `scripts`，门控不定义第二套

门控不该另立一套「前端该怎么检查」——那会和 `package.json` 漂移，且漂移时**门控仍然绿**。脚本从各包 `scripts`
读原文，只做一件事：把可执行名解析成真实入口。规则有边界且确定：

1. 按 `&&` 拆成子命令序列；
2. 子命令第一个 token 必须是**已知可执行**（`node`/`tsc`/`vite`），其余一律具名拒绝，不猜、不落到 PATH；
3. 所有 JS 工具都经**当前 `node`** 运行其 JS 入口（`typescript/lib/tsc.js`、`vite/bin/vite.js`）——
   这是绕开 `.bin` 垫片的关键；
4. 参数里的 `*` 由脚本自行展开，**展开为空即失败**；
5. `|`、`>`、`||` 等 shell 语法一律拒绝（门控不实现 shell，也不近似执行它看不懂的命令）。

### 4. 缺前置必须具名失败，绝不静默跳过

三条硬规则，对应工程基础既有的「未执行 MUST NOT 报为通过」：

| 情况 | 具名标记 |
| --- | --- |
| 运行时无 `node` | `FRONTEND_TOOLCHAIN_MISSING` |
| `typescript`/`vite` 因依赖未安装而不可解析 | `FRONTEND_DEPS_MISSING` |
| `scripts` 出现不认识的可执行或 shell 语法 | `FRONTEND_SCRIPT_UNSUPPORTED` |
| 某步以非零状态结束 | `FRONTEND_STEP_FAILED`（含包与 script 名） |

绝不把「没跑」写成「通过」，绝不把跳过的步数计入总结，绝不在缺依赖时降级成替代检查。

### 5. 步骤顺序：类型 → 构建 → 测试，失败即停

`packages/contracts-ts` typecheck → `apps/desktop` typecheck → `packages/contracts-ts` build →
`apps/desktop` build（`tsc --noEmit && vite build`）→ `apps/desktop` test → 根 `test:ui`。
任一步失败即停：前置不成立时继续跑只会产生噪声。清单在脚本的 `STEPS` 里具名存在，Rust 侧不复制。

### 6. 通过不等于产品验收

类型检查、构建、单测通过只说明**工程基础可执行**。MUST NOT 被读成浏览器渲染、人工 UI 或任何 `O` 系列验收达标。

## 否决方案

| 方案 | 否决理由 |
| --- | --- |
| 把前端并入 `portable` | 破坏「只需 Rust 工具链」的承诺；未 install 的环境会变红；改写历史基线口径 |
| 只在文档里要求人工跑前端检查 | 这正是现状：盲区是「有人记得才有」，不是「没有检查」 |
| 在 Rust 里 `Command::new("pnpm")` | 把主机环境事实（垫片 ABI、pnpm 是否在 PATH）混进门控结论 |
| 门控硬编码 `tsc --noEmit && vite build` | 与 `package.json` 漂移，且漂移时门控仍绿 |
| 缺依赖时跳过前端步并报告通过 | 跳过等于盲区，且是「未执行报为通过」 |
| glob 交给 shell 展开 | 门控不经 shell；且空匹配会被静默读成通过 |
| 门控代跑 `pnpm install` | 安装会改 `node_modules`，不是检查该做的事 |
| 引入 DOM 库以便测渲染 | 超出门控范围；渲染验收是另一件事（W38 既定边界） |

## 后果

- `cargo xtask check --profile full` 成为「完整工程基础」的单一入口；`portable` 的历史口径与适用场景不变。
- 前端检查命令的真源仍在 `package.json`；门控只负责解析与报告。
- **主机关联事实**：本机 `apps/desktop/node_modules/{vite,typescript,react}` 是指向 `.pnpm/` 的、Windows 不可解析的
  符号链接，故前端依赖解析退化到仓库根层的手工副本；这些副本此前缺 `@rollup/rollup-win32-x64-msvc` 与
  `@esbuild/win32-x64`，已按既有做法补齐。这是**本机环境**状态，不是仓库内容；`pnpm install` 后可能需要重补。
- 遗留：不配 CI（是否在流水线里调 `full` 另行决定）；不测渲染；不代跑安装。
