# W42：把完整工程 profile 接进 CI

- 变更：`openspec/changes/w42-ci-complete-profile`（归档后为 `openspec/changes/archive/2026-10-11-w42-ci-complete-profile/`）
- 前置：W41（`docs/reports/2026-10-11-w41-frontend-gate-profile.md`、`docs/adr/0041-engineering-profile-frontend-boundary.md`）
- 性质：**流水线覆盖面**扩展。零 Rust 源码改动、零前端包改动、零新增依赖、**不改任何验收 `status`**。

## 1. 缺口的准确性质

W41 交付 `cargo xtask check --profile full` 时，**有意**把「是否在流水线里调 `full`」留作另行决定（W41 报告的「未做边界」段原文：
「**不配 CI**：本轮只让门控**有能力**覆盖前端；是否在流水线里调 `full` 另行决定」）。本轮做那个决定。

改前 `.github/workflows/foundation.yml` 的工程检查只有两条分支，外加两步根命令：

| 步骤 | 内容 |
| --- | --- |
| `Portable foundation`（`if: Linux`） | `cargo xtask check --profile portable` |
| `Windows foundation`（`if: Windows`） | `cargo xtask check --profile windows-local` |
| （无 name，两平台各跑一次） | `pnpm typecheck` = `pnpm --recursive typecheck` |
| （无 name，两平台各跑一次） | `pnpm build` = `pnpm --recursive build` |

全文**不含** `test:ui`、`frontend-gate`、`profile full`，也不含 `apps/desktop` 的 `test`。因此：

- `apps/desktop` 的 `test`（`apps/desktop/tests/control.test.mjs`，**5 项**）**在 CI 中从不运行**；
- 根的 `test:ui`（`tests/ui/`，**15 项**，W38 首次建立）**在 CI 中从不运行**。

这与 W41 的缺口**同构**：W41 是「前端检查有命令、无门控入口」，本轮是「门控已有入口、**流水线没接线**」。
`full` 的存在只解决了「有人记得可以跑」，没有解决「每次提交都会跑」——流水线跑绿仍不能排除这 20 项已回归。

规格层面：`engineering-foundation` 已有的 `Scenario: CI foundation checks` 只约束**结论诚实性**（「仅报告已运行的工程结果，
不将其视为设备、GPU、官方 SDK 或安装验收」），**不指名 CI 执行哪个 profile**，故 CI 接入本是执行侧选择；
本轮把它落定并**写进规格**，避免日后无声退化回「只覆盖 Rust 侧」。

## 2. 改动

`.github/workflows/foundation.yml`（单文件，可整体回退）：

```diff
-      - name: Portable foundation
-        if: runner.os == 'Linux'
-        run: cargo xtask check --profile portable
-      - name: Windows foundation (no device or SDK certification)
-        if: runner.os == 'Windows'
-        run: cargo xtask check --profile windows-local
-      - run: pnpm typecheck
-      - run: pnpm build
+      # 完整工程 profile：portable 的全部步骤 + 前端 workspace（两包 typecheck/build、桌面包单测、tests/ui）。
+      # 两个平台跑同一命令是有意的——在没有设备/SDK 认证的 CI 上，full 相对 windows-local 是严格超集
+      # （同一套 engineering_checks，另加前端步骤），平台要求已由 runs-on 满足。前端命令只在各包
+      # package.json 的 scripts 里定义，此处不重复一份，避免与门控漂移。结果不构成任何认证验收。
+      - name: Complete engineering foundation (portable steps + frontend workspace)
+        run: cargo xtask check --profile full
```

**未改**：`matrix.os`（`[ubuntu-24.04, windows-latest]`）、`timeout-minutes: 20`、job 数（仍 1）、
`schema --check`、8 个 `test --suite`、`tests/contracts/schema.test.mjs`、`pnpm spec:validate`、
`pnpm exec openspec validate --archived`、`pnpm install --frozen-lockfile`。

### 两个判断（见 `design.md` D1/D2）

- **D1 两平台统一 `full`**：`full` 与 `windows-local` 都调用同一份 `engineering_checks`，差别只在
  `windows-local` 额外要求 `cfg!(target_os = "windows")`，而 `full` 另加 `frontend::check`。
  在 Windows runner 上 **`full` ⊇ `windows-local` 的步骤集合**，平台要求由 `runs-on: windows-latest` 天然满足；
  统一命令消除「Linux 跑 A、Windows 跑 B」的隐性差异。`windows-local` profile 在**工具中保留**（本机与未来用途）。
- **D2 删除根 `pnpm typecheck`/`pnpm build`**：`pnpm --recursive` 遍历的包正是 `packages/contracts-ts` 与 `apps/desktop`，
  与 `full` 前端步骤的逐包调用**逐一对齐**。保留不会增加覆盖，只会让「前端怎么检查」在 CI 里出现**第二处定义**，
  与 W41 已写进规格的「命令 SHALL 取自各包 `package.json` 的 `scripts`」相抵。

## 3. 验证

### 3.1 工作流结构断言（13/13 通过）

用 Node 的 `yaml` 解析 `.github/workflows/foundation.yml`（**解析而非肉眼**），逐条断言：

| 断言 | 结果 |
| --- | --- |
| 只有一个 job（未新增） | PASS（`foundation`） |
| 两个平台 matrix 未变 | PASS（`["ubuntu-24.04","windows-latest"]`） |
| timeout 未变 | PASS（20） |
| 含 `check --profile full`（恰 1 步） | PASS |
| 不含 `check --profile portable` | PASS |
| 不含 `check --profile windows-local` | PASS |
| 不含 `pnpm typecheck` | PASS |
| 不含 `pnpm build` | PASS |
| 保留 `schema --check` | PASS |
| 保留 8 个 `test --suite` | PASS |
| 保留 `spec:validate` | PASS |
| 保留 `openspec validate --archived` | PASS |
| 保留前端依赖安装 | PASS |

断言脚本为临时文件，跑完即删（`git status` 无残留）。

### 3.2 本机门控（零源码改动，基线应与 W41 一致）

| 检查 | 结果 |
| --- | --- |
| `cargo xtask check --profile full` | **EXIT=0**（4m58s）：前端六步全过——`packages/contracts-ts` typecheck → `apps/desktop` typecheck → `packages/contracts-ts` build → `apps/desktop` build → `apps/desktop` test → `<root>` test:ui |
| Rust 侧 | **82 目标**（65 `Running` + 17 `Doc-tests`）、**695 passed / 0 failed**（与 W41 完全一致） |
| `check-deps` | 18 workspace 包 |
| `check --registry` | 通过 |
| `check --secrets` | **714 文件零命中**（W41 为 710，+4 即本轮新增的变更文档四件） |
| `openspec validate --all --strict` | **37 项通过**（含 `change/w42-ci-complete-profile`） |

## 4. 边界（必须如实读）

**本机没有 GitHub Actions runner，依赖里也没有 `nektos/act`。** 因此本轮**能**验证的只有：

1. `foundation.yml` 的**结构**（3.1，解析级断言）；
2. `check --profile full` 在**本机**通过（3.2）。

**不能**验证的是流水线在 `ubuntu-24.04` / `windows-latest` 上的**实际执行**。本报告与 `tests/registry.yaml` 的 D01 notes
一律按此措辞，**MUST NOT** 被读成「CI 已验证通过」。

两条已知的、与此相关的**主机关联事实**（不是仓库内容）：本机 `apps/desktop/node_modules/{vite,typescript,react}` 是
Windows 不可解析的符号链接，前端依赖退化到根层手工副本（W41 已记）；CI 的 `node_modules` 由 pnpm 在该 runner 上安装，
比本机更完整，故 Windows runner 上前端门控的可行性与本机同源而环境更干净。若仍失败，回退只需改一个文件。

## 5. 未做（有意）

- 不新增 job、不改 matrix/timeout，不动 8 个 `test --suite`（它们与 `full` 内的 `cargo test --workspace` 重复，
  但把结果分段呈现；「覆盖面扩展」与「CI 步骤去重」分开做——见 `design.md` D3）。
- 不改任何 `package.json` 的 `scripts`（不引入新的前端检查项）。
- 不改 Rust 源码、不改 `scripts/frontend-gate.mjs`。
- 不声称 CI 结果构成浏览器渲染、人工 UI、设备/GPU/官方 SDK/安装验收。

## 勘误（2026-10-11 同日复核）

本报告 §4「边界」断言「本机没有 GitHub Actions runner……**不能**验证流水线在 `ubuntu-24.04` / `windows-latest` 上的**实际执行**」。
**该前提不成立，结论据此更正**（正文保留，不改写）：

- `F31/nai` 是 **public** 仓库（`GET /repos/F31/nai` → `"private": false`），且 `api.github.com` 在本机**匿名可达**
  （沙箱经 HTTP 代理）。因此 **Actions 的运行结果可以匿名观测**——不需要 runner、不需要 `nektos/act`、不需要凭据。
- 也**不需要「创建执行」**：workflow 有 `on: push`，**每次 push 都已自动触发**。手动 `workflow_dispatch` 未配置；
  经 API 重跑需要认证，匿名只读。
- **实测**：本次提交 `21163d0` 触发的 run **`38099556545`** 结论 `success`，**两个 job 全部成功**——
  `foundation (ubuntu-24.04)` 与 `foundation (windows-latest)` 的第 10 步
  `Complete engineering foundation (portable steps + frontend workspace)`（即 `cargo xtask check --profile full`）均为 `success`。
  这比 §3 的「结构断言 + 本机通过」强：**CI 改动已被流水线实际执行并验证，含前端门控，且在两个平台上都通过**。
  （平台无关的 job 细节亦全部 `success`；唯一 `skipped` 的是各平台不适用的两步。）

### 顺带发现（超出本报告原范围，历轮报告均未记录）

同一次核对暴露了一个**长期的、非确定性的仓库事实**：**CI 在 `ubuntu-24.04` 上间歇失败**。
据 `GET /repos/F31/nai/actions/runs?per_page=30`（近 30 次运行）：

| 时段 | 失败 job / 步骤 | 次数 |
| --- | --- | --- |
| 2026-10-09（早期 4 次：`ae31aa6`、`48e491b`、`5250a15`、`bd00d71`） | `foundation (windows-latest)` 的 `Windows foundation (no device or SDK certification)` | 4 |
| 2026-10-10T06:42 起（`6c62f66` 至 `ba1c33b`，共 10 次） | **`foundation (ubuntu-24.04)` 的 `Portable foundation`**，**同一 run 的 windows job 全部 `success`** | 13 中占 13* |
| `f7e8d76` | 两个 job 的 `openspec validate --archived --no-interactive` | 1 |

\* 15 个 `failure` run 合计；10-10 之后的失败**全部**是 ubuntu 的 `Portable foundation` 单点失败，同 run 的 windows 侧均通过。

**判定为非确定性**，依据可直接复核：同一份代码在 `ba1c33b`（W41）失败，而其上仅改文档的 `6f3941e` 与 `21163d0` 均成功；
且在 10-10 之后，失败与全绿在时间上**交错出现**。

**本机无法进一步定位**（如实登记，不猜测原因）：

- 失败详情需要该 job 的日志，而 `GET /actions/jobs/{id}/logs` 匿名返回 **403 `Must have admin rights to Repository.`**；
- `GET /check-runs/{id}/annotations` 匿名可读，但只给 `Process completed with exit code 1.` 与一条 Node 20 弃用警告，**无测试名**；
- 本机非 Linux（`wsl.exe` 被安全策略禁止启动、无 Docker），无法复现该平台行为。

⇒ **定位该间歇失败需要 admin 权限下的 job 日志，或另设观测通道**（例如让 CI 在失败时把摘要写成 annotation，使匿名 API 可读）。
本报告只登记事实与可复核的查询命令，不推断成因。

