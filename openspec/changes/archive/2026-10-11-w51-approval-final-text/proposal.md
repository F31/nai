## Why

`tests/registry.yaml` 的 **O02**（`high-risk confirmation & approval bound`）有一条**自登记的未满足项**：

> 设计 §662（O02 验收）与 §425（T-14）要求「审批按钮显示最终文本」，当前**无法满足**——
> `utterances` 表只保存 `text_hash`（`migrations/sqlite/0001-foundation.sql:10`），审批列表接口只返回
> 哈希/元信息引用/期限/证据计数，界面如实说明「本载荷不含最终文本、身份为候选哈希」。

这条不是 UI 缺失，而是**审批语义**缺失：审批人此刻是在**批准一个哈希**。审批绑定的意义是「人对将要播报的
内容负责」，而在看不到内容时，两次点击二次确认保护的是一个操作员无法核对的字符串。技能/工具产出的候选
越往后越多，这条缺口越晚补越贵。

本轮补的是**仓库内可控的那一段**：存储侧保存候选可展示文本、控制面把它随列表返回、桌面把它显示出来。
它**不**等于 O02 达标——真实候选来源、GUI 渲染验收、生产 root 接线仍缺，`status` 按规格如实判定。

## What Changes

- **新增迁移** `migrations/sqlite/0006-utterance-display-text.sql`：`utterances` 增列 `display_text`；
  `migration.rs` 版本 5→6 并纳入 `SQL` 数组。
- **`crates/nai-storage/src/store.rs`**：`create_candidate` 写入 `cue.display_text`；
  `PendingApproval` 增加 `text`；`list_pending_approvals` 与 `read_approval` 选出该列。
- **`crates/nai-control-api/src/lib.rs`**：`PendingApprovalView` 增加 `text`，列表载荷多返回 `display_text`。
- **`schemas/openapi/control.yaml`**：`ResourceItem` 增加**可选** `display_text`（契约变更，需按生成器重建
  `crates/nai-contracts/src/generated/control_http.rs` 与 `packages/contracts-ts/src/generated/http.ts`）。
- **`apps/desktop/src/console.ts`**：载荷带文本 ⇒ 显示文本；缺文本 ⇒ 保留具名缺口说明；
  **绝不**把哈希当文本展示。`parseApprovals` 对 `display_text` 做形状校验。
- **`schemas/config/backup.schema.json`**：`db_schema_version` 常量 5→6（迁移契约随版本）。
- 文档：新增 ADR；就地订正 `docs/control.md`、`docs/ui.md`；`tests/registry.yaml` O02 notes 更新。
- 不做：不建真实候选来源/证据详情页/手机审核/生产 root 接线；不引入 DOM 库；不新增依赖与 xtask 命令。

## Capabilities

### Modified Capabilities

- `approval-workflow`：把「列表项不返回候选载荷明文」订正为「返回**展示文本**但身份仍是候选哈希」，
  并新增一条需求：展示文本是**只读展示**，不得被当作身份，缺文本时必须具名说明。

## Impact

- 新增：`migrations/sqlite/0006-utterance-display-text.sql`、`openspec/specs/approval-workflow/spec.md`（经归档吸收）、
  `docs/adr/0045-approval-display-text-boundary.md`、`docs/reports/2026-10-11-w51-approval-final-text.md`。
- 修改：`crates/nai-storage/{migration.rs,store.rs,src/tests/knowledge_publication.rs}`、
  `crates/nai-control-api/src/lib.rs`、`schemas/openapi/control.yaml`、`schemas/config/backup.schema.json`、
  `crates/nai-contracts/src/generated/control_http.rs`（生成物）、`packages/contracts-ts/src/generated/http.ts`（生成物）、
  `apps/desktop/src/console.ts`、`tests/ui/approval-surface.test.mjs`、`docs/control.md`、`docs/ui.md`、
  `docs/development-plan.md`、`AGENTS.md`、`tests/registry.yaml`。
- **零新增依赖**：仍 18 包；存储只用 rusqlite/serde，控制面无新 crates，前端不引 DOM 库。
- **迁移风险**：`verify()` 对已应用迁移的校验和按文件独立，故**不得**改 `0001`，只新增 `0006`；
  旧库经 `migrate()` 追加补列。`backup.schema.json` 的常量须与 `migration::VERSION` 同步，否则备份清单校验失败。
- **验证**：`check-deps`、`check --registry`、`check --profile portable`、`pnpm test:ui`、
  `cargo xtask schema --check`（本机受环境限制以重建产物佐证，见 tasks 第 5 节）、
  `openspec validate --all --strict --archived`；CI 观测 `check --profile full` 两平台。
