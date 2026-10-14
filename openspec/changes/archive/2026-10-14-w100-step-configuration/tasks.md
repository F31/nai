# w100 任务

## 1. 判据模块

- [x] `scripts/contracts/workflow-steps.mjs`：`workflowStepsOf` 增补 `matrix` / `continueOnError`，
      并拆出 `workflowJobsOf`（**单一解析器、两个视图**；既有三字段语义一字不动）
- [x] `scripts/contracts/step-config.mjs`：`RUNNER_OS_VALUES`、`RUNNER_LABEL_PREFIXES`、
      `CONDITION_SHAPES`、`PLATFORM_COMMAND_SHAPES`、`platformsOf`、`platformLegsOf`、
      `stepConfigErrors`
- [x] R1 可达性：在册步的执行平台集合（`matrix` × `if:` 静态求值）MUST 非空
- [x] R2 失败语义：`local`/`ci-only` MUST NOT 带 truthy `continue-on-error`；
      `diagnostic` MUST 带 truthy `continue-on-error`
- [x] R3 成因与条件一致：`platform-toolchain` MUST 由平台条件兑付（真子集且非空）；
      其余三因 MUST NOT 被平台限定
- [x] R4 配置面闭世界：`matrix` 形状、`if:` 形状、`continue-on-error` 形状、
      runner 标签与 `runner.os` 取值——域外 ⇒ 具名失败
- [x] R5 平台专属命令：`run` 命中 `sudo`/`apt-get` ⇒ 执行平台集合 MUST ⊆ 该形状支持集且非空
- [x] 缺输入 / 不可解析 / 空注册表 ⇒ 具名失败，MUST NOT 静默降级

## 2. 接线与测试

- [x] `tests/contracts/step-config.test.mjs`（合成世界 + 真实仓库 + 逐条注入 + 非空洞 + 反向不红）
- [x] 登记进 `xtask/src/lib.rs::engineering_checks`
- [x] 登记进 `scripts/contracts/test-reachability.mjs` 的 `TEST_REGISTRATION_SITES`（12 → 13）
- [x] 同步 `tests/registry.test.mjs` 的逐字站点清单

## 3. 验证

- [x] P1–P7 注入对真实文件执行，逐条产生具名错误，`sha256` 逐字还原
      （`read_bytes`/`write_bytes` + 断言 `\r` 计数不变）
- [x] 边界实测：与 W84 / W95 / W97 / W98 / W99 互不顶替（含 W95 的 `condition` 重叠说明）
- [x] 全量门控 `cargo xtask check --profile full` 通过

## 4. 文档与收尾

- [x] `docs/adr/0078-step-configuration.md`
- [x] `.workbuddy/memory/KNOWLEDGE.md` 新增 §6.23，并更新 §10 计数索引
- [x] `AGENTS.md`、`docs/development-plan.md` 追加 W100 段
- [x] `docs/reports/2026-10-14-w100-step-configuration.md`

## 5. 每轮闭环（流程项，**不自我勾选**）

归档与提交按 `AGENTS.md` §0 的闭环执行：门控通过 → tasks 归档前全勾 → `archive --yes`（系统日期戳）
→ 两条 validate 都跑 → 一次 commit + push → 单次低频 CI 观测 → 回填报告（回填提交自身不回填）。
