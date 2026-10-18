# Proposal: W143 — 三条「没有判定面」的尾巴（N4/N5/N6）

## Why

W141 第八节把三件事留给后续轮次，本轮收口三件中的两件半。

**N4**：写入面有四个取值，判据只用裸字面量钉住了 `12` 与 `14`，`1`（待批准）与 `6`（已批准）
一条都没有 —— 而这两个恰是**活着**的两个：另两个只由级联失效与拒绝写入。常量被改错时无人会红。

**N5**：`target-check/` 是本机的第二份构建缓存（12G，W112 起），一直没进 `.gitignore`
⇒ 24,796 条常驻在 `git ls-files --others` 的补集里，既是两次暂存事故的源头，也让
「未跟踪也要登记」这条判据在本机与 CI 上形状不同。三处注释/文档复述了这个前提，会一起过期。

**N6**：`openspec archive` 那句「Why section should not exceed 1000 characters」反复出现，
从没有人裁定它算不算门控。本轮实测给出答案：`validate --all --strict` 根本不读 `proposal.md`
（1414 字符的探针变更仍 `44 passed / 0 failed`）；唯一的调用点在 archive 里，且上游注明
non-blocking；存量 137 份可解析的归档 proposal 里有 32 份超限，最长 2611。

三件事的共同点是**没有判定面**：真问题不是「改哪一行」，而是「该不该被守、不守的代价说清楚了没有」。

## What Changes

- `crates/nai-storage/src/tests/status_domain.rs`：新增 1 条用例（两条从存储读回的字面断言），
  并把三处重复的样本前置收成 `seed_candidate` / `set_status` / `stored_status` 三个夹具。
- `.gitignore`：加 `/target-check/`（带理由的三行注释）。
- `tests/contracts/pending-registration.test.mjs`：就地订正那段注释（旧前提 + 一处笔误）。
- `docs/development-plan.md`：两处就地订正；`docs/reports/2026-10-17-w135-*.md`：追加勘误段
  （已发布的历史记录不就地改）。
- `docs/adr/0117-repository-boundary-ignores-and-why-limit.md`：N5 与 N6 的裁定与代价。
- `openspec/specs/approval-workflow`：加一条 Requirement —— 登记落待批准、批准推进为已批准，
  两个取值是**落盘契约**。

## Impact

- 行为零变化：不迁移、不改 schema、不改任何生产语句。
- 凭据扫描域少 24,796 条路径（都是构建产物），**有意为之**，理由与代价写在 ADR 0117。
- 判据面 +2 条断言；`pending-registration` 的补集在本机与 CI 上都是 0 条，且仍**刻意不钉**
  `scanned > 0`（0 是期望状态）。
