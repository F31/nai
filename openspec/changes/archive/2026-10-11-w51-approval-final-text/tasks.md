# W51 任务

## 1. 存储侧保存候选展示文本

- [x] 1.1 新增 `migrations/sqlite/0006-utterance-display-text.sql`（`ALTER TABLE utterances ADD COLUMN display_text TEXT NOT NULL DEFAULT ''`）
- [x] 1.2 `crates/nai-storage/src/migration.rs`：`VERSION` 5→6、`SQL` 数组纳入 0006
- [x] 1.3 `store.rs`：`record_candidate` 以具名列写入 `cue.display_text`
- [x] 1.4 `store.rs`：`PendingApproval` 增 `display_text`；`list_pending_approvals` 与 `read_approval` 选出该列
- [x] 1.5 `schemas/config/backup.schema.json` 的 `db_schema_version` 常量 5→6；`tests/knowledge_publication.rs` 断言同步
- [x] 1.6 存储测试：候选入库后列表与单读都能拿到文本；旧行为（无文本）不伪造内容

## 2. 控制面契约与生成物

- [x] 2.1 `crates/nai-control-api/src/lib.rs`：`PendingApprovalView` 增 `text`；列表载荷增 `display_text`
- [x] 2.2 `schemas/openapi/control.yaml`：`ResourceItem` 增可选 `display_text`（写明「仅供显示、身份仍是 id」）
- [x] 2.3 按生成器重建 `crates/nai-contracts/src/generated/control_http.rs` 与 `packages/contracts-ts/src/generated/http.ts`
- [x] 2.4 控制面测试：列表项含 `display_text` 且不含语音文本/证据明细

## 3. 桌面呈现层

- [x] 3.1 `apps/desktop/src/console.ts`：有文本显示文本；无文本保留具名缺口；绝不把哈希当文本
- [x] 3.2 `parseApprovals` 校验 `display_text` 形状（可选、字符串）
- [x] 3.3 `tests/ui/approval-surface.test.mjs`：断言上述三种情形

## 4. 验证

- [x] 4.1 `cargo xtask check-deps`（18 包）
- [x] 4.2 `cargo xtask check --registry`
- [x] 4.3 `cargo xtask check --profile portable`
- [x] 4.4 `pnpm test:ui`
- [x] 4.5 `openspec validate w51-approval-final-text --strict`

## 5. 规格、登记、报告与归档

- [x] 5.1 规格 delta：`approval-workflow` 修改 1 条 + 新增 1 条（归档吸收）
- [x] 5.2 `docs/adr/0045-approval-display-text-boundary.md`
- [x] 5.3 就地订正 `docs/control.md`、`docs/ui.md`；更新 `docs/development-plan.md`、`AGENTS.md`
- [x] 5.4 `tests/registry.yaml`：O02 notes 更新（状态按规格如实判定，不越过证据升级）
- [x] 5.5 写 `docs/reports/2026-10-11-w51-approval-final-text.md`
- [x] 5.6 `validate --all --strict` → `openspec archive -y` → `validate --all --strict --archived`
- [x] 5.7 一次 commit + push，并匿名复核该次运行结论
