# Tasks: W143 — 三条「没有判定面」的尾巴（N4/N5/N6）

## 1. 勘定

- [x] 现测全仓 utterance status 的裸数字断言：`tests.rs:485` = 12、`approval_workflow.rs:202` = 14、`migration.rs:555` = 12；`1` 与 `6` **零条**
- [x] 确认 `1` 与 `6` 正是写入面的另两个取值（`record_candidate` 的 `INSERT` 用 `STATUS_PENDING`、`approve` 的 `UPDATE` 用 `STATUS_APPROVED`）
- [x] 现测补集：24,797 条，其中 `target-check/` 24,796 条
- [x] 用 1414 字符 Why 的探针变更实测 `validate --all --strict` = `44 passed / 0 failed`（探针已删）
- [x] 定位唯一调用点：`archive.js:1153` 的 `validateChange()`，在 `if (!json)` 内、上游注明 non-blocking
- [x] 用 upstream `ChangeParser` 逐条量 139 份归档 proposal：137 可解析 / 2 失败 / 32 超 1000 / 最长 2611

## 2. 判据（N4）

- [x] `status_domain.rs` 新增 `record_candidate_stores_one_and_approve_writes_six`：两条从存储读回（1、6）的字面断言
- [x] 把三处重复的前置收成 `seed_candidate` / `set_status` / `stored_status`，`attempt_approval` 与 `cascade_outcome` 改用它
- [x] 注入 1：`STATUS_PENDING: 1 → 2` ⇒ **只有**新用例红（`left: 2 / right: 1`）
- [x] 注入 2：`STATUS_APPROVED: 6 → 5` ⇒ **只有**新用例红（`left: 5 / right: 6`）
- [x] 逐字回滚：`git diff --stat` 只剩 `status_domain.rs`；`cargo test -p nai-storage --lib` 95/0

## 3. 忽略集（N5）

- [x] `.gitignore` 加 `/target-check/` + 理由注释
- [x] 复测补集：24,797 → **0**；`git status` 不再出现 `?? target-check/`
- [x] 订正 `pending-registration.test.mjs` 的注释（旧前提 + 「与 `scanned > 0` 成对」这处笔误）
- [x] 订正 `docs/development-plan.md` 两处（活载体，就地）
- [x] W135 报告追加勘误段（已发布记录，不就地改）
- [x] `node --test tests/contracts/*.test.mjs tests/registry.test.mjs tests/platform/*.test.mjs` = 293/0
- [x] `cargo test -p nai-xtask --lib secrets` = 16/0（含 `the_workspace_itself_is_clean` 与 `the_scan_domain_...excludes_ignored_state`）

## 4. 裁定与文档（N6）

- [x] `docs/adr/0117-repository-boundary-ignores-and-why-limit.md`：N5 与 N6 的决定、否决的替代、具名代价
- [x] ADR 里如实写明「`validateChange()` 有一个调用方，且上游刻意不阻塞」（此前记作「无调用方」，本轮实测订正）
- [x] 本 proposal 的 Why 段 ≤ 1000 字符（样本，不是判据）
- [x] `docs/reports/2026-10-18-w143-boundary-closeout.md`
- [x] `docs/development-plan.md` 追加 W143 段

## 5. 闭环

- [x] `cargo fmt --all --check` / clippy `-D warnings` / `cargo xtask check --profile full`
- [x] 两条 openspec validate；归档
- [x] 一次 commit + push（CI 两平台观测与报告 §六 / §七 的回填见回填提交，`docs-only`）
