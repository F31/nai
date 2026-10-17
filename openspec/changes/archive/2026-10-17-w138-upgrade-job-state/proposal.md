# Proposal: W138 — O02 的另一半：把作业状态接上真实 journal

## Why

`O02`「高危命令确认与审批上限」登记 `pending`，其中 §30.1 第 4 条「升级与回滚提示」的前置在 W137
已被收窄成一句可判的话：**只缺 journal/执行状态那一半**。本轮把那半接上。开工前逐环现测：

| 环 | 状态（现测） |
| --- | --- |
| ① journal 本身 | **已有且真实**：`crates/nai-storage/src/upgrade.rs` 的 `upgrade_journal` 表、`UpgradeStep` 五态机、`record_upgrade`/`upgrade_record`；`Store` 就是 `UpgradeJournal` 的生产实现 |
| ② 事务编排 | **已有**：`UpgradeTransaction::{prepare,activate,recover}`（`nai-upgrade-core`）与 `Composition::{stage_upgrade,activate_staged}`（`nai-runtime`，带 `SessionGate`，活跃会话延后激活） |
| ③ 平台安装器 | **没有**：`impl UpgradeHost for` 全仓只有 `crates/nai-upgrade-core/src/rehearsal.rs:153` 的 `RehearsalHost`，它「holds no bytes and touches no disk」 |
| ④ 控制面 | `/jobs/{id}` 在契约里声明了 `job.read` + `*readResponses`，`handle` 里**零分支** ⇒ 落到 404 `CapabilityAbsent`；`/upgrades/execute`、`/upgrades/{id}/rollback` 复用 `*jobResponses`（202 + `job_id`），也**零分支** |

于是缺口的形状与 W137 的 `plan` **同族但更宽**：契约承诺「先 202 再轮询」，而这条链**两头都是空的**——
202 那头没有执行体铸 id，`/jobs/{id}` 那头没有读者。任何一半单独补上都只是把空洞挪个位置。

### 勘定时撞到的、必须一起登记的事实

`*jobResponses`（202 + `JobAccepted`）不是一个端点的形状，它被 **14 条** 路由复用。本轮处理其中
两条（upgrades），**其余 12 条** —— `knowledge/import`、`knowledge/snapshots/{id}/publish`、
`knowledge/evaluate`、`knowledge/documents/{id} delete`、`providers/{id} put`、`providers post`、
`avatar post`、`scene put`、`scene post`、`scene delete`、`devices post`、`backups post`、
`restore/preflight` —— 同样铸 `job_id` 而**没有**任何可轮询的状态来源。它们不在本轮范围内
（各自要先有真实的作业状态表，而不是先有 `JobView`）。这不是「顺手修」的对象，是**具名登记**的对象：
登记在 ADR 0113 与 `tests/registry.yaml`，不新造一条会当场变红的判据。

## What changes

- `schemas/openapi/control.yaml`：`/jobs/{id}` 改为 200 + `JobView` + 404，打
  `x-implementation-status: implemented-loopback`；新增 `JobView`（`additionalProperties: false`，
  `step` 五值闭集）。`/upgrades/execute`、`/upgrades/{id}/rollback` 增 `503` 声明并写明——
  平台安装器未注入，本路径**具名拒绝**，仍**不**标 `implemented-loopback`（本仓库的「已声明未实现」
  就是「没有该标记」，不新增第三个状态值）。
- `schemas/core/error.proto`：新增 `NOT_FOUND = 17`。现有 16 个码里**没有**「所命名的对象不存在」这一格，
  `/jobs/{id}` 的「没有这条事务」与「作业面未接入」是两件必须分开的事，而分开要求它们有不同的码。
  `tests/contracts/descriptor.test.mjs` 已有一条「openapi 错误枚举 == proto 枚举」的耦合断言，
  改一处不改另一处会当场红。
- `crates/nai-control-api`：`JobBackend`（照 `ApprovalBackend` 同形，持 `Arc<Mutex<Store>>`）
  + `with_jobs` 装配 + 权限表 `get /jobs/{id} => job.read` + execute/rollback 的具名拒绝分支。
- `crates/nai-storage`：`UpgradeStep::ALL`（五态的单一列举），供「契约值域 == 枚举」这条判据取用。
- `apps/desktop/src/control.ts`：`parseJobView`（严格，未知步骤**不猜**）+ `ControlClient.fetchJob`；
  `apps/desktop/src/upgrade.ts`：`JOB_STEP_LABELS` + `jobStatusNote`（未知步骤呈名为**缺口**）。
- 判据：控制面 10 条、`tests/ui/upgrade-job.test.mjs` 9 条、注入 7 组。
- 文档：ADR 0113、报告、`docs/control.md` 路由表、`docs/upgrade.md`、`docs/ui.md`、dev-plan、knowledge、
  `tests/registry.yaml` 的 O02 notes（**保持 `pending`**）。

## Impact

- 新增一条权限名 `job.read`（全局第 10 个）。`Access::new` 的 `permissions.len() > 8` 是**单个令牌**
  的条数上限，不是全局名集上限，故不影响既有夹具。
- 契约生成物两处变更（Rust `SCHEMAS_JSON`/`control_http.rs`、TS `http.ts`），本机再生并核 `--check`。
- `O02` 仍 `pending`：`/jobs/{id}` 有真实状态来源，但**没有生产者**——`/upgrades/execute` 拒绝执行，
  故 journal 里不会有控制面写入的行。`docs/ui.md` 与 registry 如实写这一点，MUST NOT 记作「作业语义已跑通」。

## Alternatives considered

- **让 `/upgrades/execute` 铸 `job_id`、写一条 `prepared` 记录再 202**：否决。`Prepared` 之后
  没有任何一步能推进（`UpgradeHost` 生产实现为零），那条记录会**永远停在 `prepared`**，
  而状态读数会让「准备好了」看起来像「正在升级」。这是把「做不到」包装成「进行中」。
- **只做 `/jobs/{id}`、不动 execute/rollback**：否决。那样 202 仍是个假承诺，且 `job_id` 无人铸造，
  `/jobs/{id}` 成了没有生产者的读者——「已接线 ≠ 有调用方」，缺口只是换了个位置。
- **新增第三个 `x-implementation-status` 值（如 `declared-not-implemented`）**：否决。
  `operationErrors` 的判据是 `expected = declaredSet.has(key) ? IMPLEMENTED_STATUS : undefined`，
  即「未在已实现表里 ⇒ MUST 没有该标记」。加第三个值必须先放宽这条闭世界，用削弱判据换取表达力，
  而既有表达已经够用。
- **把 12 条 202 路由的 `job_id` 一起改成具名拒绝**：否决。它们各自缺的是**自己的状态表**，
  本轮没有为它们设计状态形状；一起改等于用同一条拒绝掩盖 12 个不同的缺口。
