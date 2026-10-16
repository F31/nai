# Proposal: W122 — `tested` 的证据可核验（registry 状态回填第一批）

## Why
按「从改 registry 状态入手」的裁定推进 R1：15 条 `implemented` 中逐条核实证据链，只把**真有可核验证据**的升级。

**立项判据（实测）**：`admission-registry` 早已声明「`tested` 仅可用于**已有归档实测证据**的项目，且证据 MUST
指向**真实报告**与测试计数」，但 `tests/registry.test.mjs` **没有任何判据读取 `acceptance[].evidence`** ——
`grep -n "evidence" tests/registry.test.mjs` 只命中三处（均为 `landing` 的内容锚点，W93），`acceptance[].evidence`
**零载体**。那句话是一句散文。

同时，15 条的逐条核实结论（实测，非摘要）：
- **A13 可升 `tested`**：命题是**静态结构事实**（矩阵 ↔ 源码扫描 ↔ SDK 枚举标签一致）；`tests/platform/platform-matrix.test.mjs`
  22 项断言经 `engineering_checks` 在 portable 与 full 门控内**实跑**，本机门控实测 TAP 汇总 `1..22 / # pass 22 / 0 fail`；
  notes 的「未升级」只说明 W55 自己没动状态，**不是**能力缺口；无外部依赖。
- **A03 不升**：notes 既有裁定指明剩余缺口是**外部真值**（真实官方接口、调试版伴侣 PoC、§9.3 平台准入决定），
  本仓无从提供可核验证据 ⇒ 按 spec「未完成或外部阻塞的目标 MUST 保留为 implemented」维持原状。

## What Changes
1. **registry 回填**：`A13` `implemented → tested`（并把升级依据写入 notes）；`A03` 追加本轮复核留痕（状态不变，说明为何不升）。
2. **把「证据可核验」从散文变成可执行判据**：`tests/registry.test.mjs` 新增 4 项，遍历 registry **自己的键**、
   覆盖**全部在册 `tested` 对象**：
   - `every tested acceptance carries a report that exists, is tracked, and lives under reports/`（三方向一起判）；
   - `the evidence-carrier scan is closed over the whole tested set`（闭世界：不静默跳过任何 tested 项 + id 唯一 + 非空洞下限）；
   - `guard: a tested acceptance with an empty evidence field is rejected`（注入：空 evidence ⇒ 具名失败）；
   - `guard: evidence outside reports/, or untracked, is rejected`（注入：`docs/knowledge.md` ⇒ `not-under-reports`；
     `reports/…不存在.md` ⇒ `not-tracked`）。

## Modified Capabilities
- `admission-registry`（准入注册表）：为已声明的「`tested` 证据可追溯」补上载体——存在的三方向（非空 / 落在归档报告目录 / 是仓库成员）各成一条可判定断言，并注入验证。

## Impact
- 无生产代码改动；无契约字段改动、无迁移、无 schema 改动。
- registry 状态分布：`implemented` 28 → 27、`tested` 10 → 11（恰一条迁移）；`pending/blocked/not_applicable` 不变。
- **语义不变量**：`MET_STATUSES = ['implemented','tested']` ⇒ 本次升级**不改变** met/unmet 归类，对账类守卫不受影响。
- 测试计数：`tests/registry.test.mjs` 52 → 56（W122 +4）。
- spec：新增 3 条 Requirement（各 1 Scenario）。
