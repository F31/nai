# Tasks: W145 — `blockedBy` 的取值由路由器自己作证

## 1. 勘定

- [x] 量路由器实际分支集：权限链 10 个条件 / 13 条路由形状（`health`、`runtime/status`、`sessions/{id}`、`mute`、`takeover`、`approvals`、`jobs/*`、`approve|reject`、`memory/read`、`memory/forget`、`upgrades/plan`、`upgrades/execute`、`upgrades/{id}/rollback`）
- [x] 量在册项与执行面的距离：15 条里**只有 1 条**路径字面量出现在 `crates/nai-control-api/src/tests.rs`，且那条不是分类证据
- [x] 确认 13 条 `no_handler` 零执行面判据；2 条 `refuses_without_installer` 由 `execute_and_rollback_refuse_by_name_and_mint_no_job_id` 单独覆盖
- [x] 确认「分类」与「行为」之间唯一的联系是 `JOB_BLOCKERS[*].anchor` 的文本锚点

## 2. 承载物

- [x] 新建 `tests/job-surface.json`：15 条分类成为两侧共读的单一枢纽
- [x] `job-surface.mjs` 删除 `JOB_SURFACES` 字面量，改为 `JOB_REGISTRY`（路径 + 执行侧逐字锚点）
- [x] `jobSurfaceErrors` 的 `surfaces` 去掉默认值：缺参即具名失败，MUST NOT 退化为「空登记表」
- [x] 错误文案里 `JOB_SURFACES` 统一为 `the job-surface registry`（符号已不存在）

## 3. 判据

- [x] Rust：`the_router_treats_every_registered_job_surface_as_declared` —— `include_str!` 同一份登记表，逐条打真实 HTTP 请求
- [x] Rust：三类形状各自可辨（404 `CAPABILITY_ABSENT` / 503 `CAPABILITY_NOT_READY` + 无 `job_id` / 未定期望的类别硬失败）
- [x] Rust：`{id}` 之外的路径参数具名失败（MUST NOT 悄悄打一条客户端不会发的路径）
- [x] Node：`jobSourceErrors` 增加「单一枢纽」三条（登记表被跟踪 / 消费者被跟踪 / 逐字嵌入）
- [x] Node：`schema.test.mjs` +1 条用例，既有 4 条改为从盘上读登记表

## 4. 注入验证

- [x] I1 `upgrades/execute` 分类翻成 `no_handler` ⇒ 期望 404 得 503，红
- [x] I2 `knowledge/import` 翻成 `refuses_without_installer` ⇒ 期望 503 得 404，红
- [x] I3 未定期望的类别 `deferred` ⇒ 硬失败，红
- [x] I4 把真有分支的 `/upgrades/plan` 登记成 `no_handler` ⇒ 期望 404 得 400，红
- [x] J1 执行侧改指 `job-surface-alt.json` ⇒ 真实仓库那条红
- [x] J2 登记表路径改指 `tests/other-surface.json` ⇒ 真实仓库那条红
- [x] 六次注入全部逐字节还原（sha256 一致），临时备份删除

## 5. 文档与闭环

- [x] ADR 0119（四个决定 + 四个被否决方案 + 四条具名残余）
- [x] `openspec/changes/w145-blockedby-behavioural-proof`（proposal / spec delta / tasks）
- [x] 活规格里两处 `JOB_SURFACES` 1:1 订正
- [x] 轮次报告与 `docs/development-plan.md` W145 段
- [x] `cargo fmt --all --check` → `check --profile full` → 两条 validate → 归档 → 提交 → CI 观测 → 回填
