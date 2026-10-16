# Proposal: W128 — 运行时升级激活调用点（R10 收口）

## Why
W123 让装配根**能答**升级激活门；registry `R10` 的剩余缺口逐字写着「运行时仍无升级激活调用点
（`nai-runtime`/`apps/engine` 内无 `UpgradeTransaction` 使用处）」⇒「开播中只暂存不激活」这条要求
**从未在真实链路上被执行过**：核心有机制、装配根有门，缺的是把两者接起来的那个调用点。

## What Changes
1. 新增 `crates/nai-runtime/src/upgrade.rs`：`UpgradeActivation` 与 `Composition` 的两处入口
   （`stage_upgrade` / `activate_staged`）。装配根驱动一次 `UpgradeTransaction`，门**只在激活边界被问一次**，
   且由装配根自己持有的真实会话回答（复用 W123 的 `ActiveSessionGate` 实现）。
2. `crates/nai-runtime/src/lib.rs`：导出 `UpgradeActivation`；补 `From<UpgradeError> for RuntimeError`
   （与既有 `From<StorageError>` 同形，不把 `Conflict` 降级成内部错误）。
3. `crates/nai-runtime/tests/upgrade.rs`：新增 5 条判据（新目标，Rust 侧测试目标 78 → 79）。
4. **夹具共享化**（本轮的前置条件）：`nai-upgrade-core` 的三份逐字重复的 demo 夹具
   （`demo manifest` JSON + `CONTENT` + `facts()`，散在 `tests/{plan,gates,upgrade}.rs`）提为
   `src/rehearsal.rs` 的 `RehearsalPackage` / `demo_manifest` / `demo_facts` / `DEMO_CONTENT`。
   理由不是「去重好看」，而是**另一个 crate 够不到本 crate 的 `tests/`**：`nai-runtime` 要驱动一次**真的**
   `prepare`（`Prepared` 在核心之外无法字面构造），共享化是唯一落点；顺带把这份夹具从「三份会漂移的副本」
   变成一处定义。

## Modified Capabilities
- `runtime-composition`：装配根 MUST 提供升级的**暂存**与**激活边界**两处入口；暂存 MUST NOT 问门；
  激活 MUST 先问门、被推迟时 MUST 报一个**非错误**的推迟结局；门不可观测时 MUST 报错；
  运行后的终局 MUST 原样携带核心给出的 `Outcome`（含回滚成因与导出计数）。

## Impact
- 生产代码：`nai-runtime` 新增一个模块 + 一个 `From` 实现 + 一次 re-export（只增不改既有行为路径）。
- `nai-upgrade-core`：夹具共享化是**纯增加**（无新依赖，`serde_json`/`sha2` 已在 `[dependencies]`），
  三个测试文件的用例数逐项不变（plan 12 / gates 7 / upgrade 8）⇒ 判据未增减。
- registry：`R10` **维持 `implemented`**，notes 记本轮进展与剩余缺口（判定依据见 design.md）。
- 无契约字段改动、无迁移、无 schema 改动、无新增 crate 依赖。
