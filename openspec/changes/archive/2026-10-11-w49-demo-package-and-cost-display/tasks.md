# W49 任务

## 1. O01 判定机制（`crates/nai-kernel/src/demo.rs`）

- [x] 1.1 描述符：`DemoPackage`、`DemoAvatar`、`KnowledgeSeed`、`DemoScenario`、`ProviderPreset`、`ConnectivityTest`、`ProbeKind`、`CostLine`、`CostDeclaration`、`DrillRecord`
- [x] 1.2 `audit_demo_package`：**十六条**稳定标记，逐条报出并具名
- [x] 1.3 `DemoReadiness` 三态 + `readiness()`（`Defective` 优先；未声明/未演练 ⇒ `Blocked`）
- [x] 1.4 `crates/nai-kernel/src/lib.rs` 挂上 `pub mod demo;`
- [x] 1.5 `crates/nai-kernel/tests/demo.rs`（19 项）：每条规则可构造、干净描述符审计为空、「恰为这些标记」的并集断言
- [x] 1.6 模块头注释写明设计章节、判据与**不判什么**（不联网、不测量耗时）

## 2. O05 成本显示（`apps/desktop/src/upgrade.ts`）

- [x] 2.1 严格解析：缺字段/未知字段/非法金额/空原因 ⇒ 具名拒绝
- [x] 2.2 `costRows()`：已知 ⇒ 金额；未知 ⇒ 「未知（原因）」——**绝不** 0
- [x] 2.3 `totalPresentation()`：全已知 ⇒ 合计；含未知 ⇒ 下界 + 未知计数，**绝不**称合计
- [x] 2.4 `tests/ui/upgrade-cost.test.mjs`（19 项）：断言上述性质与严格解析

## 3. 验证

- [x] 3.1 `cargo xtask check-deps`（18 包）
- [x] 3.2 `cargo xtask check --registry`
- [x] 3.3 `cargo xtask check --profile portable`（83 目标、725 passed / 0 failed，+19 = 本轮 demo 单测）
- [x] 3.4 `pnpm test:ui`（`tests/ui` 34/34）与 `scripts/frontend-gate.mjs`（六步全通过）
- [x] 3.5 `openspec validate w49-demo-package-and-cost-display --strict`

## 4. 规格、登记、报告与归档

- [x] 4.1 新能力 `demo-package`（9 条需求，归档吸收进 `openspec/specs/demo-package/spec.md`）
- [x] 4.2 `local-control-surface` 新增「升级成本逐项列示」需求（归档吸收）
- [x] 4.3 `docs/adr/0044-demo-package-readiness-boundary.md`
- [x] 4.4 新增 `docs/demo-package.md`；更新 `docs/ui.md`、`docs/development-plan.md`、`AGENTS.md`
- [x] 4.5 `tests/registry.yaml`：O01/O05 的 notes 追加、**状态保持 `pending`**
- [x] 4.6 写 `docs/reports/2026-10-11-w49-o01-o05-in-repo-closeout.md`
- [x] 4.7 `validate --all --strict` → `openspec archive -y` → `validate --all --strict --archived`
- [x] 4.8 一次 commit + push，并匿名复核该次运行结论
