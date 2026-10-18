# Tasks: W141 — utterance 取值域

## 1. 勘定

- [x] 复核 W139 第 5 结论的计数（`NOT IN(11..15)` 6 / `NOT IN(12..15)` 3 / `BETWEEN 1 AND 5` 3 / `SET status=N` 9）
- [x] 补测 W139 未列的 3 处：`approvals` 撤回语句里读取 `status=12`
- [x] 确认幽灵值（`11`、`13`、`15`、`2..=5`、`7..=10`）零写入者
- [x] 确认 `nai-storage` 的测试组织与判据落点（`src/tests/` + `src/tests.rs` 的 `mod`）

## 2. 执行体

- [x] `crates/nai-storage/src/utterance_status.rs`：具名取值 + 三条带 + `StatusClass` 五值闭世界 + `classify()` + `usable()`
- [x] `usable_values()` 由 `usable()` **全域**派生（扫 `[0, 不可用带上界]`），使 `usable()` 有生产消费者
- [x] 生产源码的 status 裸字面量全部改为派生（`store.rs` / `knowledge.rs` / `publication.rs`）
- [x] 可用性由黑名单改白名单

## 3. 判据与注入

- [x] `src/tests/status_domain.rs`（4 条）：扫描器自检 / 零裸字面量 / SQL 与 Rust 逐值一致 / 白名单行为 / 级联保护带行为
- [x] 模块内 3 条：闭世界与反方向 / 白名单 / 两带一致且不相交
- [x] 注入 1（可用性退回黑名单）⇒ 4 条红；首跑暴露**盲区**（`usable_values` 区间太窄）⇒ 修派生链后复测仍红
- [x] 注入 2（保护带缩一格）⇒ 2 条红
- [x] 注入 3（绕过声明写裸字面量）⇒ 扫描判据红且可归因
- [x] 注入 4（`usable_guard` 绕开派生链）⇒ 3 条红且可归因
- [x] 逐字节还原（`sha256sum -c` 3/3 OK）

## 4. 文档与状态

- [x] `openspec/changes/w141-utterance-status-domain/`（proposal / specs / tasks）
- [x] `docs/adr/0115-utterance-status-domain.md`
- [x] `docs/reports/2026-10-18-w141-utterance-status-domain.md`
- [x] `docs/development-plan.md` 同步真实状态

## 5. 闭环

- [x] `cargo fmt --all --check` / clippy `-D warnings` / `cargo xtask check --profile full`
- [x] 两条 openspec validate；归档
- [x] 一次 commit + push；两平台 CI 观测；回填报告 §六 / §七
