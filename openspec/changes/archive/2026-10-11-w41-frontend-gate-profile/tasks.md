## 1. 规格与设计

- [x] 1.1 建立 OpenSpec 变更 `w41-frontend-gate-profile`，新增 `engineering-foundation` 差量需求（4 条需求 / 6 个场景），`validate --strict` 通过。
- [x] 1.2 落设计决策 D1–D6（新增 `full` 而非并入 `portable`、前端委托 Node 脚本而非 Rust 调 `pnpm`、命令取自 `scripts` 单一真源、缺前置具名失败、步骤集合与顺序、边界与不做）。
- [x] 1.3 把缺口的性质写成据可核对的发现（`portable` 分支只有六步；前端命令散落三个 `package.json`；历次记录把前端检查与 portable 并列写成人工结果），各附路径与行号。

## 2. xtask：完整工程 profile

- [x] 2.1 `xtask/src/lib.rs` 把工程检查的六步抽成 `engineering_checks`，`portable` 与 `full` 共用同一份实现。
- [x] 2.2 新增 `xtask/src/frontend.rs`：`check(root)` 以 `NODE`（缺省 `node`）发起 `scripts/frontend-gate.mjs`，缺解释器与非零退出都翻译成具名诊断；前端**步骤清单只在脚本侧**，Rust 不复制一份。
- [x] 2.3 `lib.rs` 新增 `["check", "--profile", "full"]` 分支（共用检查 + `frontend::check`）与 usage 文本；未知 profile 仍被拒绝。
- [x] 2.4 `xtask/src/frontend.rs` 单测：不存在的解释器以 `FRONTEND_TOOLCHAIN_MISSING` **具名失败**而非被跳过；`xtask/tests/cli.rs` 既有断言（未知 profile 非零退出）保持通过（xtask lib **40 项**全绿）。

## 3. 前端门控脚本

- [x] 3.1 新增 `scripts/frontend-gate.mjs`：从各包 `package.json` 的 `scripts` 取命令原文，按 `&&` 拆成子命令序列。
- [x] 3.2 只允许已知可执行（`node`/`tsc`/`vite`），其余具名拒绝；一律经**当前 node** 运行其 JS 入口（`typescript/lib/tsc.js`、`vite/bin/vite.js`，后者经包根拼路径——其 `exports` 不暴露该文件），不依赖 `.bin` 垫片与 `pnpm`。
- [x] 3.3 缺 `node`、缺依赖、门控不认识的 shell 语法（`|`/`>`/`||`）一律具名非零退出；glob 由脚本自行展开且**空匹配即失败**；任一步失败即停，后续步骤不再执行。
- [x] 3.4 根 `package.json` 增补 `check:frontend` 脚本别名，便于人工单独调用同一入口。

## 4. 实测

- [x] 4.1 `cargo xtask check --profile full` 通过（**EXIT=0**，2m59s）：前端六步全过（两包 typecheck、两包 build——`vite v7.1.9` 30 modules / 3.32s、desktop `1..5`、`test:ui` `1..15`）；Rust 侧 **82 目标**（65 `Running` + 17 `Doc-tests`）、**695 passed / 0 failed**。
- [x] 4.2 反向自检三条，均随后还原（`git diff` 无残留）：注入类型错误 ⇒ `FRONTEND_STEP_FAILED` 且后续步骤停止；`scripts` 注入 `eslint .` ⇒ `FRONTEND_SCRIPT_UNSUPPORTED`；注入 `tsc --noEmit | cat` ⇒ 同标记并指名 token。缺解释器的路径另由 `frontend.rs` 单测覆盖。
- [x] 4.3 `check --profile portable` 语义未变（**EXIT=0**，82 目标、695 passed / 0 failed；与 W40 的 694 之差恰为本轮新增的 `frontend.rs` 单测），工作区无污染（`dist/` 仍被忽略）。

## 5. 文档与记录

- [x] 5.1 新增 `docs/reports/2026-10-11-w41-frontend-gate-profile.md` 与 `docs/adr/0041-engineering-profile-frontend-boundary.md`。
- [x] 5.2 更新 `AGENTS.md`、`docs/development-plan.md`、`tests/registry.yaml` 的 D01 notes（只补记事实，**未改任何 `status`**）。

## 6. 闭环

- [x] 6.1 勾选本 tasks，`openspec archive w41-frontend-gate-profile -y`，`validate --all --strict --archived` 通过。
- [x] 6.2 复跑全量门控（`check-deps` 18 包、`--registry`、`--secrets` 710 文件零命中、`--profile portable`、`--profile full`）并记录结果。
