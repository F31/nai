# W41 报告：完整工程 profile（前端门控）

- 日期：2026-10-11
- 变更：`w41-frontend-gate-profile`
- ADR：[0041](../adr/0041-engineering-profile-frontend-boundary.md)
- 相关规格：`engineering-foundation`

## 1. 缺口的准确性质

`openspec/specs/engineering-foundation/spec.md` 的 **Reproducible workspace** 从 D01 起就要求工程 SHALL
「支持锁定依赖的编译、**类型检查和前端构建**」，Scenario 也要求「workspace 编译、测试和**前端检查**可执行」。
但任务工具里没有这个入口：`cargo xtask check --profile portable` 只做六件事
（`check-deps`、`check --registry`、`fmt --all --check`、`clippy --workspace --all-targets`、
`cargo test --workspace --exclude nai-xtask`、`cargo test -p nai-xtask --lib`），前端一行都没有。

前端检查**一直存在**，只是从未被门控覆盖：

| 包 | 命令 | 位置 |
| --- | --- | --- |
| `@nai/contracts` | `typecheck: tsc --noEmit`、`build: tsc` | `packages/contracts-ts/package.json` |
| `@nai/desktop` | `typecheck: tsc --noEmit`、`build: tsc --noEmit && vite build`、`test` | `apps/desktop/package.json` |
| 根 | `typecheck`、`build`（各自 `--recursive`）、`test:ui` | `package.json` |

历次记录里它们**始终与 portable 并列**写成人工结果（`AGENTS.md` 多处：「…portable、……与**前端检查**通过」；
W40：「两处 `tsc` EXIT=0；desktop 5/5、`test:ui` 15/15」）——并列的写法本身就说明：**这些是有人记得时才有的一次人工检查**。

> 因此缺口不是「前端没有检查」，而是「前端检查**没有门控入口**」：`portable` 跑绿完全不能排除前端已破。

## 2. 做了什么

- 新增 **`cargo xtask check --profile full`** = `portable` 的**全部**步骤 + 前端门控。
- `xtask/src/lib.rs`：把六步抽成 `engineering_checks`，`portable`/`windows-local`/`full` 共用；
  「`full` 包含 `portable` 的全部步骤」是**代码结构事实**，不是需要同步维护的清单。
- `xtask/src/frontend.rs`（新）：发起 `scripts/frontend-gate.mjs`（`NODE` 覆盖、缺省 `node`），
  缺解释器与非零退出都翻译成具名诊断。
- `scripts/frontend-gate.mjs`（新，「前端怎么跑」的单一真源）：从各包 `package.json` 的 `scripts` 读命令，
  只把可执行名解析成真实入口；已知可执行仅 `node`/`tsc`/`vite`，其余与未实现的 shell 语法一律具名拒绝。
- 根 `package.json` 增补 `check:frontend`，便于人工单独调用同一入口。
- **`portable` 语义不变**：仍只需 Rust 工具链即可运行。

## 3. 实测

### 3.1 `cargo xtask check --profile full` —— EXIT=0（2m59s）

六个前端步骤的实际输出：

| # | 步骤 | 结果 |
| --- | --- | --- |
| 1 | `packages/contracts-ts` → typecheck | 通过 |
| 2 | `apps/desktop` → typecheck | 通过 |
| 3 | `packages/contracts-ts` → build | 通过 |
| 4 | `apps/desktop` → build | `vite v7.1.9`，30 modules transformed，3.32s |
| 5 | `apps/desktop` → test | `1..5`（5 项） |
| 6 | 根 → test:ui | `1..15`（15 项） |

Rust 侧（与 portable 同一份实现）：

- 测试目标 **82**（`Running <test target>` **65** + `Doc-tests` **17**）
- **695 passed / 0 failed**（W40 基线 694，+1 即本轮 `frontend.rs` 的新单测）
- 计数口径与 W38 起的勘误一致：`Running \`…nai-xtask.exe check --profile full\`` 是 cargo 打印的**包装行**，
  不是测试目标，数封装行就是记账错误。

### 3.2 非空洞自检（三条，均随后还原）

生成器只产合法输入会让门控「通过而什么都没证明」。三条反向自检证明了前端步骤**真的在拒绝**：

| 自检 | 注入 | 期望 | 实测 |
| --- | --- | --- | --- |
| A | `apps/desktop/src/__gate_probe.ts`（类型错误） | 门控失败 | `error TS2322` → `FRONTEND_STEP_FAILED`（指名包与 script），后续步骤停止 |
| B | `contracts-ts` 的 `typecheck` 改为 `eslint .` | 具名拒绝 | `FRONTEND_SCRIPT_UNSUPPORTED: 门控不认识可执行 \`eslint\`（已知：node, tsc, vite）` |
| C | `contracts-ts` 的 `typecheck` 改为 `tsc --noEmit \| cat` | 具名拒绝 | `FRONTEND_SCRIPT_UNSUPPORTED: … 含门控不实现的 shell 语法 \`\|\`（token \`\|\`）` |

另有实例化单测覆盖「缺解释器」：`run(OsStr::new("nai-definitely-not-a-program"), …)` 必须返回含
`FRONTEND_TOOLCHAIN_MISSING` 的错误，而**不是**被跳过（`xtask/src/frontend.rs` 的 `#[cfg(test)]`）。

自检后 `packages/contracts-ts/package.json` 与 `apps/desktop/src/` 均按原样恢复（`git diff` 无残留），
重跑门控回到 EXIT=0。

## 4. 主机关联事实（不是仓库内容）

本机 `apps/desktop/node_modules/{vite,typescript,react}` 是指向 `.pnpm/` 的、**Windows 不可解析**的符号链接
（`fs.realpathSync` 报 `EACCES`），只有 `@nai/contracts` 是 JUNCTION。因此前端依赖解析**退化到仓库根层的手工副本**，
而这些副本此前缺 Windows 原生包，实测两处并已按既有做法补齐：

| 缺什么 | 补齐来源 |
| --- | --- |
| `node_modules/@rollup/rollup-win32-x64-msvc` | `.pnpm/rollup@4.64.0/node_modules/@rollup/` |
| `node_modules/@esbuild/win32-x64` | `.pnpm/esbuild@0.25.12/node_modules/@esbuild/` |

这是**本机环境**状态（`node_modules/` 不被跟踪），不是仓库缺陷；重新 `pnpm install` 后可能需要重补。
门控本身不代跑 `install`，缺依赖时以 `FRONTEND_DEPS_MISSING` 具名失败。

## 5. 未做边界

- **不配 CI**：本轮只让门控**有能力**覆盖前端；是否在流水线里调 `full` 另行决定。
- **不测渲染**：仍不引入 DOM 库；`tests/ui` 断言的是无 DOM 的纯逻辑（W38 既定边界）。
- **不代跑 `install`**：门控只检查**已安装**的依赖。
- **不改任何验收状态**：`tests/registry.yaml` 的 `status` 一个都没动；D01 的 notes 只补记「前端检查已并入 `full`」。
- **前端门控通过不等于产品验收**：类型检查、构建、单测通过只说明工程基础可执行，MUST NOT 被读成
  浏览器/人工 UI 验收或任何 `O` 系列验收达标。
