## Context

W41 交付 `cargo xtask check --profile full` 时，**有意**把「是否在流水线里调 `full`」留作另行决定
（`docs/reports/2026-10-11-w41-frontend-gate-profile.md` 的「未做边界」段）。本轮做那个决定。

事实（已核实）：

- `.github/workflows/foundation.yml` 只有 `portable`（Linux）/`windows-local`（Windows）两条 profile 分支，
  外加两个 job 各跑一次的根 `pnpm typecheck` 与 `pnpm build`。
- 根 `typecheck` = `pnpm --recursive typecheck`、`build` = `pnpm --recursive build`（`package.json`），
  遍历的包正是 `packages/contracts-ts` 与 `apps/desktop`。
- `scripts/frontend-gate.mjs` 的 `STEPS` 依次为：`packages/contracts-ts typecheck` → `apps/desktop typecheck` →
  `packages/contracts-ts build` → `apps/desktop build` → `apps/desktop test` → 根 `test:ui`。
- 流水线**完全没有** `apps/desktop test` 与根 `test:ui`。

## Goals / Non-Goals

**Goals**

- 让前端 workspace 的类型检查、构建与测试**每次提交都实际执行**，且命令定义保持**单一真源**。
- 两个平台行为一致，消除「Linux 跑 A、Windows 跑 B」的隐性差异。
- CI 覆盖面成为**规格承诺**，不能无声回退。

**Non-Goals**

- 不新增 job、不改 matrix/timeout、不动 Rust 源码。
- 不清理既有的 8 个 `test --suite` 步骤（与 `cargo test --workspace` 重复，见 D3）。
- 不引入新的前端检查项（不改任何 `package.json` 的 `scripts`）。
- 不声称 CI 结果构成浏览器渲染、人工 UI 或任何产品验收。

## Decisions

### D1：两个 job 统一 `full`，而不是只改 Linux

`full` 与 `windows-local` 的关系可判定：两者都调用同一份 `engineering_checks`，差别只在
`windows-local` 额外要求 `cfg!(target_os = "windows")`，而 `full` 另加 `frontend::check`。
在 Windows runner 上，**`full` ⊇ `windows-local` 的步骤集合**——平台要求由 `runs-on: windows-latest` 天然满足，
不存在「因为走了 full 就丢掉了 Windows 侧检查」的情况。

统一命令的收益是消除两平台漂移：若只改 Linux，Windows 侧仍只跑 `typecheck`/`build`，
「前端测试在哪个平台跑」会变成一个需要记住的例外。

### D2：删除根 `pnpm typecheck` / `pnpm build`

`pnpm --recursive <script>` 的遍历结果与 `full` 前端步骤里的逐包调用**逐一对应**（见 Context）。
保留它们不会增加覆盖，只会让「前端怎么检查」在 CI 里出现**第二处定义**——与 W41 已写进规格的
「命令 SHALL 取自各包 `package.json` 的 `scripts`，MUST NOT 由门控另行定义一套可能与之漂移的检查」相抵。
删除是让该原则在 CI 侧也成立，而不是削弱覆盖。

### D3：保留 8 个 `test --suite` 步骤

它们与 `portable`（进而 `full`）内部的 `cargo test --workspace` 重复，但把结果**分段**呈现，
让失败的 suite 一眼可辨。这是既有设计，本轮不扩大范围去动它——把「CI 覆盖面」与「CI 步骤去重」两件事分开做。

### D4：`windows-local` profile 保留在工具中

CI 不再调用它，不代表它无用：它表达的是「这份检查必须在原生 Windows 上跑」这一**工具侧**约束，
仍由 `xtask/tests/cli.rs` 的断言（非 Windows 请求 windows-local 必须被拒绝）覆盖。
规格中 `Explicit build and certification boundaries` 对它的要求不因本轮改变。

### D5：CI 的可验证边界必须如实披露

本机没有 GitHub Actions runner，`nektos/act` 也不在依赖里。因此本轮**能**验证的是：

- `foundation.yml` 的**结构**（用 Node 解析 YAML 做断言：含 `check --profile full`、不含被删步骤、不含两条旧分支）；
- `cargo xtask check --profile full` 在**本机**通过（W41 已实测；本轮零源码改动，故仍成立）。

**不能**验证的是流水线在各 runner 上的实际运行。报告与 `registry` notes 一律按此措辞，
MUST NOT 写成「CI 已验证通过」。

## Risks / Trade-offs

- **Windows runner 上首次跑前端门控**：本机（Windows）在 W41 已实测 `full` 通过，而 CI 的 `node_modules`
  由 pnpm 在 Windows 上安装，比本机（Linux 容器安装后退化到根层手工副本）**更完整**，风险反而更低。
  若仍失败，回退只需改一个文件。
- **流水线时长**：`full` 比 `portable` 多出前端六步（本机实测约十余秒量级），相对既有的 Rust 编译与 8 个 suite 不显著；
  `timeout-minutes: 20` 不变。
- **`pnpm typecheck`/`build` 删除后失去的日志分段**：由前端门控的逐步输出（`[frontend] ...`）替代，
  失败时给出包名与 script 名，比根命令的递归输出更具体。

## Rollback

单文件改动：把 `foundation.yml` 中统一的 `check --profile full` 还原为 `portable`（Linux）/
`windows-local`（Windows）两条分支并加回两步根命令即可。规格差量随之作废（已归档的变更不追改，
按仓库惯例在报告中记明）。
