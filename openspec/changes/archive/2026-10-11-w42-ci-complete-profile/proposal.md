## Why

W41 新增了 `cargo xtask check --profile full`——它在 `portable` 的全部步骤之后额外执行前端 workspace 的类型检查、构建与测试。
但**流水线从未调用它**。

`.github/workflows/foundation.yml` 的现状是单一 job、matrix `[ubuntu-24.04, windows-latest]`，工程检查分两条分支：

- Linux：`cargo xtask check --profile portable`
- Windows：`cargo xtask check --profile windows-local`
- 两个 job 各自再跑一次根 `pnpm typecheck`（= `pnpm --recursive typecheck`）与 `pnpm build`（= `pnpm --recursive build`）

该文件全文**不含** `test:ui`、`frontend-gate`、`profile full`，也不含 `apps/desktop` 的 `test`。于是：

- `apps/desktop` 的 `test`（`apps/desktop/tests/control.test.mjs`，5 项）**在 CI 中从不运行**；
- 根的 `test:ui`（`tests/ui/`，15 项，W38 首次建立）**在 CI 中从不运行**。

后果与 W41 的缺口同构：**流水线跑绿不能排除这 20 项已回归**。W41 把前端检查从「人工」提升到「有门控入口」，
但入口在 CI 里**没有接线**——`full` 的存在只解决了「有人记得可以跑」，没有解决「每次提交都会跑」。

规格层面：`engineering-foundation` 的 `Explicit build and certification boundaries` 有一条 `Scenario: CI foundation checks`，
但它只约束**结论诚实性**——「仅报告已运行的工程结果，不将其视为设备、GPU、官方 SDK 或安装验收」——**不指名 CI 执行哪个 profile**。
所以 CI 接入是执行侧选择；本轮把这个选择落定，并把它钉进规格，避免日后无声退化为「只覆盖 Rust 侧」。

## What Changes

- 两个 CI job **统一执行 `cargo xtask check --profile full`**（替换 `portable` / `windows-local` 两条分支步骤）。
- **删除被 `full` 完全覆盖**的两步根命令 `pnpm typecheck` / `pnpm build`——`pnpm --recursive` 遍历的正是 `full` 前端步骤里的两个包，
  保留它们等于让同一检查在 CI 里存在两份定义。
- 不新增 job、不改 matrix、不改 timeout，保留既有的 `schema --check` 与 8 个 `test --suite` 步骤。
- `--profile windows-local` 在**工具中保留**（本机与未来用途），只是 CI 不再使用它：
  `full` 在 Windows 上是它的**严格超集**——两者跑同一份 `engineering_checks`，`full` 另加前端 workspace。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `engineering-foundation`：新增两条需求——CI SHALL 执行完整工程 profile（`full`），其步骤覆盖 `portable` 全部与前端 workspace；
  CI MUST NOT 用「只覆盖 Rust 侧的 profile + 另行定义的独立前端命令」来代替，前端命令的定义 SHALL 与门控同源（单一真源）。

## Impact

- 修改：`.github/workflows/foundation.yml`（**单文件，可整体回退**）。
- 修改：`openspec/specs/engineering-foundation/spec.md`（经归档吸收）、`tests/registry.yaml`（D01 notes）、`AGENTS.md`、
  `docs/development-plan.md`；新增 `docs/reports/2026-10-11-w42-ci-complete-profile.md`。
- **零源码改动**：不改 Rust、不改前端包、不改 `scripts/frontend-gate.mjs`；workspace 包数不变（仍 18），`check-deps` 方向规则不被触碰。
- **不触碰任何验收状态**：registry 的 `status` 一个都不改——本轮是流水线覆盖面，不是任何验收项的达标。
- **边界**：CI 在 GitHub Actions 上执行，**本机没有 runner**。因此本轮能验证的是 workflow 的**结构**与 `full` **在本机的通过**，
  **不能**观测流水线的实际运行；报告如实标注这一边界，不声称「CI 已验证通过」。
