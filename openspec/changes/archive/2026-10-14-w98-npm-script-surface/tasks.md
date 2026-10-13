# w98 任务

## 1. 判据模块

- [x] `scripts/contracts/npm-scripts.mjs`：`packageScripts`、`driverSteps`、`driverExecutables`、
      `driverShellSyntax`、`pnpmReferences`、`workspaceGlobs`、`stripSlashComments`、`stripYamlComments`、
      `scriptErrors`
- [x] V1 `(包目录, 脚本名)` ↔ `SCRIPT_DISPOSITIONS` 双向闭世界
- [x] V2 引用可解析（`run` / `--filter P` / `--recursive`）+ **自引用不算消费者**
- [x] V3 `invoked` 的 `{file, marker}` 在**剥离注释后**逐字存在；`driver` 形态验「真是 STEPS 一项」
- [x] V4 `orphan` 成因落闭集 `ORPHAN_CAUSES`，且三格各自付代价（判定域两两不同）
- [x] V5 驱动表每项落该包 `scripts`，首 token 在 `KNOWN_EXECUTABLES` 内、无 shell 语法
- [x] V6 成因闭集非空

## 2. 接线与测试

- [x] `tests/contracts/npm-scripts.test.mjs`（34 个用例：基线干净 + 注入 + 非空洞 + 不可解析必失败）
- [x] 登记进 `xtask/src/lib.rs::engineering_checks`
- [x] 登记进 `scripts/contracts/test-reachability.mjs` 的 `TEST_REGISTRATION_SITES`（10 → 11）
- [x] 同步 `tests/registry.test.mjs` 的逐字站点清单

## 3. 验证

- [x] P1–P15 注入对真实文件执行，逐条产生具名错误，`sha256` 逐字还原（备份拷回，**不用 `git checkout`**）
- [x] 边界实测：与 W95（`WORKFLOW_STEPS`）、W96（`XTASK_COMMANDS`）、W97（`TEST_SUITES`）、
      W84（站点）四者互不顶替（4/2/4/3/2）
- [x] M1 反例如实记录：CI 那行文本被 W95 的 marker 抓到，但语义不同
- [x] 两条判据缺陷（贪婪正则吞第二条命令、`packages` 为 null）由测试抓出并修
- [x] 全量门控 `cargo xtask check --profile full` 通过

## 4. 文档与收尾

- [x] `docs/adr/0076-npm-script-surface.md`
- [x] `.workbuddy/memory/KNOWLEDGE.md` 新增 §6.21，并更新 §10 计数索引
- [x] `AGENTS.md`、`docs/development-plan.md` 追加 W98 段
- [x] `docs/reports/2026-10-14-w98-npm-script-surface.md`
- [x] OpenSpec 归档（tasks 全勾 → `archive --yes` → 两条 validate）
- [x] 一次 commit + push + 单次低频 CI 观测
