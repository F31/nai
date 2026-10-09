# Tasks

## 1. 规格与校验
- [x] 1.1 完成 proposal/design/tasks/specs 并通过 `pnpm exec openspec validate w02-w03-decision-log-duty-lease --strict --no-interactive`。

## 2. 决策日志（R07 前置）
- [x] 2.1 新增 `migrations/sqlite/0005-decision-log.sql`（表、索引、只追加触发器）。
- [x] 2.2 nai-storage 新增 decision 模块：`append_decision`（critical/telemetry 分级、有界缓冲、满丢弃最旧）、`verify_decision_chain`（含首个不一致 seq）、缓冲统计查询。
- [x] 2.3 测试：链校验通过/篡改检出（含位置）/触发器拒改删/缓冲分级与容量/workspace 链隔离/缓冲冲刷连续性/超大载荷拒绝。
- [x] 2.4 备份契约同步：`schemas/config/backup.schema.json` 的 db_schema_version 4→5，`cargo xtask schema` 重生成 TS，契约测试与备份断言同步。

## 3. 值守租约（INV-08）
- [x] 3.1 nai-kernel 新增 duty 模块与 Session 集成：confirm_duty、note_heartbeat（不续期）、duty_valid、auto_reply_permitted、enforce_duty（600ms 撤权）、change_mode 值守前置。
- [x] 3.2 测试：到期边界、心跳不续期、到期停自动、600ms 撤权与幂等、仅显式确认可续、无值守拒切 Auto、无租约无需处置。

## 4. 收尾
- [x] 4.1 更新 tests/registry.yaml（R07→implemented 注明缺口；S01 补值守证据）、docs/storage.md、docs/session.md、docs/development-plan.md（含 V3.6 切片工作项表）。
- [x] 4.2 运行 `cargo test --workspace --offline`（232 项全绿）、`cargo xtask test --suite storage|kernel|contracts`、`cargo xtask check --registry`、`cargo xtask check-deps`、`cargo xtask check --profile portable`、`cargo xtask schema --check`。
- [x] 4.3 写 docs/reports/2026-10-08-w02-w03-decision-log-duty-lease.md，更新 AGENTS.md，归档变更并 `pnpm exec openspec validate --archived --no-interactive`。

## Workflow follow-up

- 主规格 `decision-log-integrity`、`duty-lease` 归档生效。
- 后续：output-keeper（W05）/render-worker（W06）接播放 ACK 与真实租约静音；composition root 接决策日志与值守事件写入；
  进程级强杀故障演练（R03 缺口）与桩 Provider 回放（W17）另行开卡。
