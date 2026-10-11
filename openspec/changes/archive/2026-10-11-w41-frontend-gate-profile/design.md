## 决策

### D1 —— 新增 `full` profile，而不是把前端并入 `portable`

`portable` 的名字与既有规格都在承诺一件事：**只要 Rust 工具链就能跑**（`engineering-foundation` 的 Scenario 明说「不要求 GPU、SDK
或自建云服务」，历次报告也把它当最小基座）。把前端步骤直接塞进 `portable` 会让「刚 clone、尚未 `install`」的环境**立刻变红**——
而那既不是前端破了，也不是 Rust 破了，只是依赖还没装；同时历次报告的 `portable` 数字口径会被改写。

因此：`portable` 原样不动，另立 `full` = `portable` 的全部步骤 **加上**前端。两者共用同一段工程检查实现（抽成函数），
`full` 只是「再往前走一步」，不复制、不分叉。

代价是「一个命令跑全部」不再由 `portable` 承担，而是由 `full` 承担；这需要文档说清楚提交/CI 该跑哪一条。

### D2 —— 前端门控委托 Node 脚本，而不是在 Rust 里调 `pnpm`

`xtask` 是 Rust。它当然可以用 `Command::new("pnpm")`，但那样会在本机**必然失败**，且失败原因与前端无关：本仓库的 `node_modules`
是 Linux 容器安装的，`apps/desktop/node_modules/.bin/tsc` 是一个 1000 字节的 **POSIX shell 脚本**，Windows 上无法执行；
`pnpm` 本身也不在 PATH 上（只有 `corepack`）。让门控依赖「`pnpm` 在 PATH 且 `.bin` 垫片与本机 ABI 匹配」，等于把**主机环境事实**
混进门控结论。

而「前端跑什么」天然是 JS 生态的问题：它需要读 `package.json`、解析 npm 包入口、跨平台定位 `node`。所以：

- `xtask/src/frontend.rs` 只负责**何时跑**（`full` profile 的最后一步）与**把子进程的失败翻译成具名诊断**；
- `scripts/frontend-gate.mjs` 负责**怎么跑**，它不依赖 `pnpm`，也不依赖 `.bin` 垫片。

`node` 通过 `NODE` 环境变量覆盖、否则取 PATH 上的 `node`，与既有 `run()` 取 `CARGO` 的写法一致。

### D3 —— 命令取自 `package.json` 的 `scripts`，不硬编码

门控不该定义第二套「前端该怎么检查」——那会和 `package.json` 漂移，且漂移时**门控仍然绿**。因此脚本从各包
`package.json` 的 `scripts` 里读原文（`typecheck`/`build`/`test`、根的 `typecheck`/`build`/`test:ui`），只做一件事：
把可执行名解析成真实入口。**改 `scripts` 就会改门控跑的东西**，单一真源。

解析规则是确定性的、有边界的：

1. `scripts` 按 `&&` 拆成子命令序列（本仓库的 scripts 都是这种形式）；
2. 每个子命令的第一个 token 必须是**已知可执行**（`node`、`tsc`、`vite`）——未知则可执行名一律具名失败，不猜、不落到 PATH；
3. 所有 JS 工具都经**当前 `node`** 运行其 JS 入口（`typescript/lib/tsc.js`、`vite/bin/vite.js`），而不是把可执行名交给 PATH
   —— npm 的 `.bin` 垫片是无扩展名的 POSIX 脚本，Windows 上既不能直接执行也不是可识别命令；
4. 其余 token 原样作为参数传递；参数里的 `*`（如 `tests/ui/*.test.mjs`）由脚本**自行展开**，不交给 shell，
   且**展开结果为空即失败**——「0 个测试文件被跑到」绝不能被读成「测试通过」。

出现在 scripts 里的 `|`、`>`、`||` 等 shell 语法一律**拒绝**并具名说明——门控不实现 shell，也不近似执行它看不懂的命令。
这样「门控覆盖的命令」与「`pnpm run` 跑的命令」在**已知集合内**永远一致。

### D4 —— 缺前置必须具名失败，绝不静默跳过

三条硬规则，对应工程基础既有的文化（「未执行 MUST NOT 报为通过」）：

- **缺 `node`**：`FRONTEND_TOOLCHAIN_MISSING`，指出需要什么、怎么满足，非零退出；
- **缺 `typescript`/`vite` 依赖**（`node_modules` 未安装）：`FRONTEND_DEPS_MISSING`，指出缺哪个包、哪个包目录解析失败；
- **`scripts` 里有门控不认识的可执行**：`FRONTEND_SCRIPT_UNSUPPORTED`，指出是哪个包、哪个 script、哪个 token。

绝不做的事：把「没跑」写成「通过」、把跳过的步数写进总结、在缺依赖时降级成 `tsc` 的替代检查。

### D5 —— 步骤集合与顺序

`full` 的前端部分按**先类型、再构建、后测试**推进，任一步失败即停（后续步骤不再执行——前置不成立时继续跑只会产生噪声）：

| # | 工作目录 | script | 说明 |
| --- | --- | --- | --- |
| 1 | `packages/contracts-ts` | `typecheck` | 契约包类型检查 |
| 2 | `apps/desktop` | `typecheck` | 桌面包类型检查 |
| 3 | `packages/contracts-ts` | `build` | 契约包 emit（产物在未跟踪的 `dist/`，不污染工作区） |
| 4 | `apps/desktop` | `build` | `tsc --noEmit && vite build` |
| 5 | `apps/desktop` | `test` | 桌面客户端单测（5 项） |
| 6 | 根 | `test:ui` | `tests/ui`（15 项） |

步骤清单在 `scripts/frontend-gate.mjs` 的 `STEPS` 里具名存在（包目录 + script 名）——Rust 侧**不复制**这份清单，
`full` 只多做一件事：在共用工程检查之后追加 `frontend::check`。粒度归 JS 侧，因为「前端有哪些步骤」属于
「前端怎么跑」的知识；Rust 侧负责的是「什么时候跑」与「失败怎么报」。两侧因此各只有一份真源。

### D6 —— 边界与不做

- **不做 CI 配置**：本轮只让门控**有能力**覆盖前端，是否在流水线里调 `full` 属另行决定，不在本轮范围。
- **不测渲染**：仍不引入 DOM 库；`tests/ui` 断言的是无 DOM 的纯逻辑（W38 的既定边界）。
- **不代跑 `install`**：门控只检查**已安装**的依赖；缺失即失败，不替开发者执行安装（安装会改 `node_modules`，不是检查该做的事）。
- **不改任何验收状态**：`tests/registry.yaml` 的 `status` 一个都不动；D01 的 notes 只补记「前端检查已并入 `full`」这一事实。
- **前端检查的通过不等于产品验收**：类型检查、构建、单测通过只说明工程基础可执行，MUST NOT 被读成浏览器/人工 UI 验收或任何 `O` 系列验收达标。
