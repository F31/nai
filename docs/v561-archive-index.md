# V5.6.1 旧基线归档索引

日期：2026-11-04。目的：明确哪些历史设计与验收不再参与 V5.6.1 发布门。

## 旧验收

| 资产 | 状态 | 说明 |
|---|---|---|
| `tests/registry.yaml` | 只读归档 | V4.8，164 ACC/62 INV；不再作为 V5.6.1 来源 |
| `tests/legacy/v311/` | 历史 | V3.11 旧语料 |
| `tests/legacy/v43/` | 历史 | V4.3 快照 |
| `schemas/tests/registry.schema.json` | 历史 | V4.8 schema；V5.6.1 使用 `schemas/tests/v561-registry.schema.json` |
| `tests/registry.ARCHIVED` | 标记 | 指向 `tests/v561/registry.yaml` |

## 旧设计

保持历史参考，不参与 V5.6.1 执行与通过：

- `docs/digital-host-agent-design-V4.1..V4.9/V5.1/V5.2.md`
- `docs/数字人直播工作台-V5.4/V5.5/V5.6设计方案.md`
- `docs/v48-development-plan.md`
- `docs/v52-*.md`
- V5.6 审阅报告：`docs/reports/2026-11-04-v56-review-and-recommendations.md`（作为 V5.6.1 输入）

## 旧 OpenSpec 变更

- 已归档变更保留在 `openspec/changes/archive/`；不删除、不改写。
- 仍处于 active 的旧变更（如 `v52-personal-live-workbench`、`v49-realistic-avatar` 等）不自动成为 V5.6.1 通过依据。
- V5.6.1 的执行契约只有 `openspec/changes/v561-flashhead-single-engine/`。
- 任何把旧变更纳入 V5.6.1 的决定必须新增 ADR/规格映射，不能默认继承。

## 旧代码

- 已删除：`apps/render-worker`、`plugins/platform-unavailable`、`crates/nai-plugin-hosts`、`crates/nai-runtime`。
- 已删除第二批：`apps/engine`、`crates/nai-control-api`、`nai-control-ipc`、`nai-knowledge-core`、`nai-storage`、`nai-security`、`nai-upgrade-core`、`nai-budget-core`、`nai-provider-core`。
- workspace 从 26 个包降到 18 个包；旧 Rust 树不再参与默认构建。
- 剩余旧模块：`adapters/storage-sqlite` 的 workspace/role/approval/budget 模块、旧 `scripts/contracts/*` 锚点、可选 `audition-worker`/`human-mic-worker`；见 `docs/architecture/v561-code-reuse-and-removal-map.md`。

## 旧数据/许可

- 旧数据库不自动迁移；旧配置/协议/ABI 不保证兼容。
- 旧用户资产只通过手动导入（肖像、参考音频、知识文档）。
- 旧许可结论不自动沿用；V5.6.1 使用 `docs/v561-license-gate.md` 重新核查。
