## Why

`openspec/specs/engineering-foundation/spec.md` 的 **Reproducible workspace** 需求从 D01 起就写着：工程 SHALL「支持锁定依赖的编译、
**类型检查和前端构建**」，其 Scenario 也要求「workspace 编译、测试和**前端检查**可执行」。但任务工具里**没有任何入口执行前端检查**：
`cargo xtask check --profile portable` 的分支只做六件事——`check-deps`、`check --registry`、`fmt --all --check`、
`clippy --workspace --all-targets`、`cargo test --workspace --exclude nai-xtask`、`cargo test -p nai-xtask --lib`
（`xtask/src/lib.rs:134-174`），前端一行都没有。

前端检查其实**一直存在**，只是从未被门控覆盖。仓库有两个前端包，各自在 `package.json` 里定义了检查命令：

| 包 | 命令 | 位置 |
| --- | --- | --- |
| `@nai/contracts` | `typecheck: tsc --noEmit`、`build: tsc` | `packages/contracts-ts/package.json` |
| `@nai/desktop` | `typecheck: tsc --noEmit`、`build: tsc --noEmit && vite build`、`test` | `apps/desktop/package.json` |
| 根 | `typecheck: pnpm --recursive typecheck`、`build: pnpm --recursive build`、`test:ui` | `package.json` |

而历次记录里，它们**始终与 portable 并列**写成人工结果：「…portable、contracts十三类消息互通、schema无漂移与**前端检查**通过」（`AGENTS.md:105`
等多处）、W40 的「两处 `tsc` EXIT=0；desktop 5/5、`test:ui` 15/15」（`AGENTS.md:606`）。并列的写法本身就说明：**这些是有人记得时才有的一次人工检查**。

这就是缺口 2 的准确性质：**不是「前端没有检查」，而是「前端检查没有门控入口」**。后果是 `check --profile portable` 跑绿
**完全不能排除**前端已破——类型错误、构建失败、`tests/ui` 回归都可能与绿色门控共存，直到下一次有人手动去跑。规格早已声明的
「前端检查可执行」也因此**没有任何一次自动执行来兑现它**。

## What Changes

- 新增 **`cargo xtask check --profile full`**：执行 `portable` 的**全部**步骤，并额外执行前端 workspace 的类型检查、构建与测试。
- **`portable` 语义不变**：仍只需 Rust 工具链即可运行，历史基线与既有报告口径不受影响。
- 前端步骤的**命令取自各 `package.json` 的 `scripts`**（单一真源），由新增的 `scripts/frontend-gate.mjs` 解析真实可执行入口后执行——
  不依赖 `pnpm` 的 `node_modules/.bin` 垫片（本机的 `.bin/tsc` 是 Linux 容器留下的 POSIX 脚本，Windows 上不可执行）。
- **缺前置一律具名失败**：缺 `node`、缺 `typescript`/`vite` 依赖、`scripts` 出现门控不认识的可执行，都以具名诊断非零退出，
  MUST NOT 静默跳过该步或报为通过。
- `xtask/src/frontend.rs` 承载「何时跑、以什么身份跑」，`scripts/frontend-gate.mjs` 承载「怎么跑前端」；两者职责分离，便于单测。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `engineering-foundation`：新增四条需求——完整工程 profile（`full`）SHALL 覆盖前端 workspace 的类型检查、构建与测试且命令取自 `scripts`；
  前端步骤的失败与前置缺失 MUST 具名报告且 MUST NOT 被跳过或降级；门控不认识的 `scripts` 内容 MUST 被拒绝而非猜测执行；
  前端检查通过 MUST NOT 被报告为渲染、人工 UI 或任何产品验收的达标。

## Impact

- 新增：`xtask/src/frontend.rs`、`scripts/frontend-gate.mjs`。
- 修改：`xtask/src/lib.rs`（抽出工程 profile 步骤、新增 `full` 分支、usage 文本）、`package.json`（便于人工调用的脚本别名）、
  `openspec/specs/engineering-foundation/spec.md`（经归档吸收）、`AGENTS.md`、`docs/development-plan.md`、`tests/registry.yaml`（D01 notes）。
- **不新增 workspace 包**（仍 18 个），`check-deps` 的方向规则不被触碰；`xtask` 不新增 crate 依赖（前端门控经外部进程调用）。
- **不触碰任何验收状态**：本轮是工程基础覆盖面的扩展，不是任何验收项的达标；registry 的 `status` 一个都不改。
- **不做**：CI 流水线配置、DOM 渲染测试（仍不引入 DOM 库）、真实浏览器或人工 UI 验收、前端依赖的重新安装（门控只检查**已安装**的依赖，
  不代跑 `install`）。
