## 1. 规格与设计

- [x] 1.1 建立 OpenSpec 变更 `w42-ci-complete-profile`，新增 `engineering-foundation` 差量需求（2 条需求 / 3 个场景），`validate --strict` 通过。
- [x] 1.2 落设计决策 D1–D5（两 job 统一 `full`、删除被覆盖的根 `pnpm typecheck`/`build`、保留 8 个 `test --suite`、保留 `windows-local` profile、CI 的可验证边界）。
- [x] 1.3 把缺口写成可核对的发现：`foundation.yml` 不含 `test:ui`/`frontend-gate`/`profile full`；根 `pnpm --recursive typecheck|build` 的遍历结果与 `full` 前端步骤逐包对应。

## 2. CI 配置

- [x] 2.1 `.github/workflows/foundation.yml`：两个 job 统一执行 `cargo xtask check --profile full`，删除根 `pnpm typecheck` / `pnpm build` 两步。
- [x] 2.2 保留 `schema --check`、8 个 `test --suite`、`tests/contracts/schema.test.mjs`、`spec:validate` 与 `openspec validate --archived` 步骤；不新增 job、不改 matrix/timeout。
- [x] 2.3 在 workflow 内以注释说明为何 Windows 也走 `full`（`full` ⊇ `windows-local` 的步骤，平台要求由 `runs-on` 满足）。

## 3. 实测与验证

- [x] 3.1 Node 解析 `foundation.yml` 做结构断言 **13/13 通过**：两个 job 均含 `check --profile full`；不含 `pnpm typecheck`/`pnpm build`；不含 `--profile portable`/`--profile windows-local` 的步骤；矩阵与 timeout 未变；8 个 suite 与 openspec 校验步骤保留。
- [x] 3.2 本机回归：`check-deps` 18 包、`check --registry` 通过、`check --secrets` 714 文件零命中、`check --profile full` **EXIT=0**（82 目标 / 695 passed / 0 failed，与 W41 一致）、`openspec validate --all --strict` 37 项通过。
- [x] 3.3 如实记录边界：CI 的实际执行**未经本机观测**（无 runner、无 `act`），本轮只验证了 workflow 结构与本机 `full` 通过。

## 4. 文档与记录

- [x] 4.1 新增 `docs/reports/2026-10-11-w42-ci-complete-profile.md`（含 D5 的边界披露与结构断言的可复核命令）。
- [x] 4.2 更新 `AGENTS.md`、`docs/development-plan.md`、`tests/registry.yaml` 的 D01 notes（**未改任何 `status`**）。

## 5. 闭环

- [x] 5.1 勾选本 tasks，`openspec archive w42-ci-complete-profile -y`，`validate --all --strict --archived` 通过。
- [x] 5.2 一次 commit + push；更新项目记忆。
