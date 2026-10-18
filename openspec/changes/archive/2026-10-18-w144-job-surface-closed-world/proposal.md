# Proposal: W144 — 作业面闭世界：15 条「铸了 id 却没有状态来源」的声明怎么收口

## Why

`control.yaml` 有 **15** 个操作声明 `202 + JobAccepted`，而唯一读作业状态的
`GET /jobs/{id}` 只读 `upgrade_journal` 一张表——契约自己写着「only upgrade jobs have a
journal today」。其余 14 条铸了一个指不到任何状态的 id，客户端拿到 202 之后能做的
只有等一个不会到来的终态。`JobAccepted` 在生产侧**零生产者**，15 条路由在路由表里
**一条分支都没有**，全部落到兜底 404。

W138 把它记成「12 条」，同一份报告的 `grep -c` 又是 15。本轮用解析口径定案：15 条，
`remote-knowledge.yaml` 0 条。

收口不是实现 15 套状态机——那是 15 次迁移，且每条的状态形状至今没有人设计过。
收口是让「声明了 202 却没有状态来源」**不可能再静默**：每条 MUST 在册，在册项 MUST
恰有 `kind`（能被轮询）或具名的 `blockedBy`（今天不铸 id 的理由）。

## What Changes

- `scripts/contracts/job-surface.mjs`（新）：`JOB_SURFACES` / `JOB_KINDS` / `JOB_BLOCKERS`
  作为单一来源，加两条判据——`jobSurfaceErrors`（契约侧）与 `jobSourceErrors`（仓库侧）。
- `tests/contracts/schema.test.mjs`：4 条用例（真实文档已闭 + 每条规则一个合成样本 +
  锚点判别性 + 非空洞），合成样本 MUST NOT 从真实文档克隆（YAML 别名共享引用）。
- `schemas/openapi/control.yaml`：`JobAccepted` 补 description，写明 `accepted` 只表示
  「收到了」，能否轮询由 journal 而非响应体决定；生成物随之再生。
- `docs/adr/0118-job-surface-closed-world.md`：四个被否决的方案与三条具名残余。
- `docs/reports/2026-10-18-w144-job-surface-closed-world.md` 与 `docs/development-plan.md` 追加 W144 段。

## Impact

- **行为零变化**：不实现、不删除、不迁移、不改任何路由。`JobAccepted` 仍然零生产者。
- 判据面 +2 条函数 / +4 条用例 / +20 个注入样本，全部落在已登记的 `schema.test.mjs`，
  不新增门控目标。
- 契约面 +1 段 description（客户端可见），生成物 `control_http.rs` 与 `http.ts` 各改一处。
- 依赖成本：零新增依赖（复用 `@apidevtools/swagger-parser` 与 `readTrackedPaths`）。
