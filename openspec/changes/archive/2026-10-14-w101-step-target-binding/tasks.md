# w101 任务

## 1. 判据模块

- [x] `scripts/contracts/step-target.mjs`：`TARGET_SHAPES`、`targetsOf(run)`、`stepTargetErrors`
- [x] `targetsOf` 的 `xtask` 那一支**复用** `ciXtaskCommands`（MUST NOT 第二份剥壳逻辑）
- [x] R1 绑定双向：解析出的目标集合 == 声明的目标集合
- [x] R2 角色与目标一致：`infra` / `diagnostic` MUST NOT 声明目标，且解析出零个目标
- [x] R3 形状闭世界：`shape` 域外 ⇒ 具名失败
- [x] 缺输入 / 空注册表 / 不可解析 ⇒ 具名失败，MUST NOT 静默降级

## 2. 在册数据与接线

- [x] `WORKFLOW_STEPS` 的 13 个判定条目补 `target`（gate 1 + suite 8 + 契约 1 + schema 1 + node-test 1 + pnpm 2）
- [x] `infra`（7）与 `diagnostic`（1）**不加**字段
- [x] `tests/contracts/gate-coverage.test.mjs` 那条钉具体串的断言**订正为形状级**
- [x] 登记进 `xtask/src/lib.rs::engineering_checks`
- [x] 登记进 `scripts/contracts/test-reachability.mjs` 的 `TEST_REGISTRATION_SITES`（13 → 14）
- [x] 同步 `tests/registry.test.mjs` 的逐字站点清单（13 → 14）

## 3. 测试

- [x] `tests/contracts/step-target.test.mjs`：合成世界（三形状各自的解析规则）+ 真实仓库
- [x] 逐条注入：两个方向的对账、`infra`/`diagnostic` 两个方向、域外形状、缺输入、空注册表
- [x] 反向不红：合法的目标声明（含扩展名/多文件/多目标）MUST NOT 红
- [x] 非空洞守卫：闭集非空；三形状的解析规则各自被合成世界覆盖

## 4. 验证

- [x] W1–W4 重跑：**由零信号变红**（改真文件 + 真跑 9 套件 + 逐字节还原 + `sha256` 一致）
- [x] 新增边界注入：清空目标 / 域外形状 / `infra` 塞判定 / 缺输入，逐条具名
- [x] 边界实测：与 W95 / W96 / W97 / W99 / W100 互不顶替
- [x] 全量门控 `cargo xtask check --profile full` 通过

## 5. 文档与收尾

- [x] `docs/adr/0079-step-target-binding.md`
- [x] `.workbuddy/memory/KNOWLEDGE.md` 新增 §6.24，并更新 §10 计数索引
- [x] `AGENTS.md`、`docs/development-plan.md` 追加 W101 段
- [x] `docs/reports/2026-10-14-w101-step-target-binding.md`

## 6. 每轮闭环（流程项，**不自我勾选**）

归档与提交按 `AGENTS.md` §0 的闭环执行：门控通过 → tasks 归档前全勾 → `archive --yes`（系统日期戳）
→ 两条 validate 都跑 → 一次 commit + push → 单次低频 CI 观测 → 回填报告（回填提交自身不回填）。
