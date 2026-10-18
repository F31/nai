# Proposal: W148 — 预构建处方的补集：块外不得有第二份

## Why

W147 立了「预构建处方 ↔ 门控测试步」的双向对账，但它的判定面只覆盖**标记块内**。
W147 报告 §八把剩下的缺口具名登记为两条：块外的散文与围栏不受判据约束；`testCommandsOf`
没有一条「不许被过滤」的守卫。

本轮先把第二条量成数字（两次注入，逐字节还原）：

| 注入形状 | 结果 |
|---|---|
| 黑名单 `if (values.includes('--exclude')) continue;` | 25 pass / 6 fail |
| 白名单 `if (!values.includes('--locked')) continue;` | **31 pass / 0 fail（零信号）** |

零信号的原因是**夹具是一致性样本**：`gate-coverage.test.mjs` 里每一条 `["test", …]` 夹具都带
`--locked`，白名单过滤在它们身上是**恒等变换**。故 W147 §八第 3 条的措辞要改写：不是「没有判据」，
而是「判据对『只保留某类旗标』这一形态零信号」。

第 2 条则量清了域：`docs/` 下 50 份活文档、2096 行围栏内容里，围栏内的预构建处方只有
`engineering.md` 标记块那两条；而 `docs/adr/0121` 与 `docs/reports/2026-10-18-w147-*`
里另有 5 条副本——那是**已发布历史记录**，不可就地订正，故必须按路径段排除在域外。

## What Changes

- `scripts/contracts/gate-coverage.mjs`：新增 `isLiveDocument`（域的唯一出处）、`fencedLines`
  （围栏解析，不配对即抛）、`prebuildPrescription`（只认带 `--no-run` 的命令）、
  `strayPrebuildErrors`（多重集补集），并把补集**接进** `prebuildRecipeErrors`。
- `tests/contracts/gate-coverage.test.mjs`：`load()` 从 `.git/index` 派生活文档集；+3 条用例
  （31 → 34）：补集基线 + 非空洞三件、围栏副本/散文/不平衡围栏三种形态、四种旗标剖面的
  「N 进 N 出」判别性样本。
- `docs/engineering.md` L53–57 那句补一句「块外不得再出现处方」；ADR 0122；轮次报告；
  `docs/development-plan.md` W148 段。

## Impact

- **门控零改动**：`xtask/src/lib.rs` 一字节不改，不新增子命令 ⇒ 不撞逐字钉住的 24 条签名清单。
- **判据域为零新依赖**：活文档集来自既有 `git-index.mjs`（W75），不遍历工作区。
- 历史记录（`docs/adr/`、`docs/reports/`）不进域，理由由一条断言**付代价**：测试实测那边确实
  有 5 条副本，故排除不是装饰；反向注入（把历史纳入域）实测 4 条用例变红。
