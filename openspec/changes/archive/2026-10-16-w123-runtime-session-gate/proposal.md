# Proposal: W123 — 装配根接上真实会话，回答升级激活门（R10 接线）

## Why
R10（开播与回滚不丢写入）记录的缺口是「**真实会话注册表（装配根）未接线**」：`nai-upgrade-core::gates::ActiveSessionGate`
只有 rehearsal 实现（`RehearsalSessions`，一个内存 `AtomicUsize`），装配根从未用自己的真实会话回答它。

本轮把**装配根**接上：`nai-runtime::Composition` 持有单会话装配的真实 `Session`，由它实现 `ActiveSessionGate`。

## What Changes
1. `crates/nai-runtime/src/composition.rs`：为 `Composition` 实现 `nai_upgrade_core::ActiveSessionGate`。
2. `crates/nai-runtime/Cargo.toml`：新增 `nai-upgrade-core` 依赖（层次规则允许 runtime → shared）。
3. `crates/nai-runtime/tests/composition.rs`：新增 2 条判据（真实 lifecycle 双向 + busy 锁 fail-closed）。

## Modified Capabilities
- `runtime-composition`：装配根 MUST 用**自己持有的真实会话**回答升级激活门；不可观测时 MUST fail closed。

## Impact
- 生产代码：`Composition` 新增一个 trait 实现（只读，不改既有行为路径）；新增一个依赖。
- registry：`R10` **维持 `implemented`**（本轮接的是「能答 gate」，运行时仍无升级激活调用点），notes 记进展与剩余缺口。
- 无契约字段改动、无迁移、无 schema 改动。
