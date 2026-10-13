# w97 任务

## 1. 判据模块

- [x] `scripts/contracts/gate-coverage.mjs`：`membersFromToml`、`packageNames`、`topLevelItems`、
      `testCoverage`、`gateTestCommands`、`suitePackages`、`ciXtaskCommands`、`coverageErrors`
- [x] V1 suite 表 ↔ dispatch 双向闭世界（复用 W96 的 `dispatchSignatures`）
- [x] V2 每个 suite 的包集可解析（`inline` 内联 / `delegated` 穿透 `schema::contracts`）
- [x] V3 `-p` 的每个值落在一个真实成员包名上
- [x] V4 每个 suite 的包集被**单条**门控 test 命令覆盖（不许用并集凑）
- [x] V5 每个 workspace 成员被门控 test 命令的**并集**覆盖
- [x] V6 CI 的每条 xtask 命令签名在 dispatch 台账里在册

## 2. 接线与测试

- [x] `tests/contracts/gate-coverage.test.mjs`（基线干净 + 注入用例 + 非空洞 + 不可解析必失败）
- [x] 登记进 `xtask/src/lib.rs::engineering_checks`
- [x] 登记进 `scripts/contracts/test-reachability.mjs` 的 `TEST_REGISTRATION_SITES`
- [x] 同步 `tests/registry.test.mjs` 的逐字站点清单

## 3. 验证

- [x] P1–P10 注入对真实文件执行，逐条产生具名错误，`sha256` 逐字还原
- [x] 边界实测：与 W95（`WORKFLOW_STEPS`）、W96（`XTASK_COMMANDS`）、W84（站点）三者互不顶替
- [x] 全量门控 `cargo xtask check --profile full` 通过

## 4. 文档与收尾

- [x] `docs/adr/0075-gate-coverage.md`
- [x] `.workbuddy/memory/KNOWLEDGE.md` 新增 §6.20，并更新 §10 计数索引
- [x] `AGENTS.md`、`docs/development-plan.md` 追加 W97 段
- [x] `docs/reports/2026-10-14-w97-gate-coverage.md`
- [x] OpenSpec 归档（tasks 全勾 → `archive --yes` → 两条 validate）
- [x] 一次 commit + push + 单次低频 CI 观测
