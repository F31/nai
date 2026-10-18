# Tasks: W142 — utterance 取值域下沉到 schema（N1）

## 1. 勘定

- [x] 复核 `migrations/sqlite/0001-foundation.sql` 里该列的形状（`status INTEGER NOT NULL DEFAULT 1`，无 CHECK）
- [x] 现测引用 `utterances` 的子表：只有 `evidence_links`（`(workspace_id,utterance_id)`）
- [x] 确认 `Store::open` 在 `migrate` **之前**打开 `foreign_keys` ⇒ 重建落在外键生效的状态下
- [x] 实测三条重建变体：无 defer ⇒ `DROP TABLE` 失败；`defer ON` 但缺 `OFF` ⇒ `COMMIT` 失败（此时 `foreign_key_check` 为空）；`PRAGMA foreign_keys` 在事务内是 no-op
- [x] 现测六处成套的承载物（`VERSION` / `SQL` / schema const / Rust 契约样本 / Node 样本含上一版 / TS 生成物）

## 2. 承载物

- [x] `migrations/sqlite/0012-utterance-status-domain.sql`：整表重建 + `CHECK(status IN(1,2,3,4,5,6,11,12,13,14,15))` + 两个 `defer_foreign_keys` pragma
- [x] `migration::VERSION` 11 → 12、`SQL` 数组加 `include_str!`
- [x] `schemas/config/backup.schema.json` 的 `db_schema_version` → 12
- [x] `crates/nai-storage/src/tests/knowledge_publication.rs` 的契约样本断言 → 12
- [x] `tests/contracts/schema.test.mjs` 的样本 → 12 且「上一版 MUST 被拒」→ 11
- [x] `packages/contracts-ts/src/generated/config/backup.ts` → 12

## 3. 判据与注入

- [x] `crates/nai-storage/src/tests/status_schema.rs`（3 条）：派生集合钉死 + 补集 / 存储里的 CHECK 子句逐字一致 / 逐值接受⟺已声明
- [x] `migration.rs` 升级用例（1 条）：带子行的库升级后行逐字保留、子行仍在、外键完好、域外被拒、域内可写
- [x] W141 的 `a_candidate_whose_status_was_never_declared_is_not_approvable` 修前提并改名为 `..._is_not_usable_...`（域内不可用的 11/12/13/14/15 仍在原处走真实 `Store::approve`）
- [x] 注入 1（SQL 列表放宽到 `0,7..=10`）⇒ 3 条红
- [x] 注入 2（删 `defer_foreign_keys=ON`）⇒ 1 条红（只有带子行的升级用例）
- [x] 注入 3（删 `defer_foreign_keys=OFF`）⇒ 1 条红（同一条用例，独立一次注入）
- [x] 注入 4（把待批准带上界 5→7，即「改了域不写迁移」）⇒ 3 条红
- [x] 逐字节还原（`sha256sum -c` 4/4 OK），且注入前基线是 fmt-clean 的最终字节

## 4. 文档与状态

- [x] `openspec/changes/w142-utterance-status-check/`（proposal / specs / tasks）
- [x] `docs/adr/0116-utterance-status-check.md`
- [x] `docs/reports/2026-10-18-w142-utterance-status-check.md`
- [x] `docs/development-plan.md` 同步真实状态

## 5. 闭环

- [x] `cargo fmt --all --check` / clippy `-D warnings` / `cargo xtask check --profile full`
- [x] 两条 openspec validate；归档
- [x] 一次 commit + push（CI 两平台观测与报告 §六 / §七 的回填见回填提交，`docs-only`）
