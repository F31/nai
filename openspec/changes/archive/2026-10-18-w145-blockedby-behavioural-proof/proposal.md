# Proposal: W145 — `blockedBy` 的取值必须由路由器自己作证

## Why

W144 把 15 条「声明了 `202` 却不铸 `job_id`」的作业面收成了闭世界：每条 MUST 在册，在册项 MUST
恰有 `kind`（真能被 `/jobs/{id}` 轮询）或具名的 `blockedBy`（今天不铸 id 的理由）。但
`blockedBy` 的取值只被**文本锚点**证明——锚点证明那句话还在源码里，不证明这条路由**真的没有分支**。

实测：15 条在册项里只有 **1 条**的路径字面量出现在控制面测试里（还不是作为分类证据，是被那条
拒绝用例顺手用到）；13 条 `no_handler` 从未被任何执行面判据碰过。于是「分类」与「路由器的实际
行为」之间只剩一句注释：一条 `no_handler` 的路由悄悄接上分支、一条 `refuses_without_installer`
的路由开始铸 id，两侧都还是绿的。这正是 W144 §五第 1 条自己登记的残余。

关掉它的方式不是再写一条注释：登记表挪成两侧共读的单一枢纽（`tests/job-surface.json`），执行侧
把每条的 `(method, path)` 打进真实路由器，判「分类 ⇔ 响应」；未定期望的类别 MUST 硬失败。

## What Changes

- `tests/job-surface.json`（新）：15 条分类从 `job-surface.mjs` 的字面量挪出来，成为**两侧共读**的
  单一枢纽。契约侧（Node）与执行侧（Rust）读同一份字节。
- `scripts/contracts/job-surface.mjs`：删除 `JOB_SURFACES` 字面量；新增 `JOB_REGISTRY`
  （登记表路径 + 执行侧逐字 `include_str!` 锚点）；`jobSurfaceErrors` 的 `surfaces` 参数
  **去掉默认值**（缺参即具名失败）；`jobSourceErrors` 增加「单一枢纽」规则。
- `crates/nai-control-api/src/tests.rs`：新增一条用例，逐条把 15 个 `(method, path)` 打进真实
  路由器：`no_handler` ⇒ 兜底 404 `CAPABILITY_ABSENT`；`refuses_without_installer` ⇒ 503
  `CAPABILITY_NOT_READY` 且不铸 job id；未定期望的类别 ⇒ 具名失败。
- `tests/contracts/schema.test.mjs`：+1 条用例（登记表 ⇔ 执行侧消费者同一份字节）+ 3 个注入；
  既有 4 条用例改为从盘上读登记表。
- `openspec/specs/local-control-surface/spec.md`：把活规格里两处 `JOB_SURFACES` 1:1 订正为登记表
  落点（符号已不存在）。
- `docs/adr/0119-blockedby-proved-by-router.md`、轮次报告、`docs/development-plan.md` 追加 W145 段。

## Impact

- **行为零变化**：不改任何路由、不铸任何 job id、不新增 202。`JobAccepted` 仍然零生产者。
- 判据面：Rust +1 条用例（15 次真实 HTTP 探测）；Node +1 条用例 / +2 个注入（另有 2 个真实注入在
  注入验证中执行）。不新增门控目标、不新增依赖（`serde_json` 与 `reqwest` 均已在依赖里）。
- 契约面零改动：`control.yaml` 与生成物都不动（`schema --check` 应仍是 no drift）。
- 一处**符号迁移**：`JOB_SURFACES` → `tests/job-surface.json`。W144 的 ADR / 报告 / 归档变更里
  出现的旧符号是**当时**的名字，历史记录不就地改。
