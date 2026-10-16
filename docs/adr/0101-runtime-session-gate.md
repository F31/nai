# ADR 0101：装配根用真实会话回答升级激活门

- 状态：Accepted｜日期：2026-10-16｜归属：W123（R10 接线）
- 关联：ADR 0100（§tested 证据可核验）、`runtime-composition` spec、R10（开播与回滚不丢写入）、W15（nai-upgrade-core）

## 背景
R10 记录的缺口是「**真实会话注册表（装配根）未接线**」：`nai-upgrade-core::gates::ActiveSessionGate`
只有 rehearsal 实现（内存 `AtomicUsize`），装配根从未用自己的真实会话回答它。⇒ 「开播中只暂存不激活」这条
命题在真实运行时链路上**没有被执行过**。

## 决策
1. **由装配根回答**：`nai-runtime::Composition` 持有单会话装配的真实 `Session`，由它实现 `ActiveSessionGate`；
   核心刻意不持有会话状态，缓存计数会成为第二份会过期的真相。
2. **活跃 = 在播**：`lifecycle ∈ {Running, Draining}` 计为 1，其余为 0。**不取「一切未停止的会话」**——
   本装配 `prepare` 后会话即 `Created`，那会让激活**永久被 defer**，门恒挡即死门（不是谨慎）。
3. **不可观测 ⇒ fail closed**：会话锁 busy/poisoned 返回**错误**，MUST NOT 返回 0。返回 0 等于谎称「无人活跃」，
   会让版本切换在开播中发生——这正是该门存在的全部理由。按「不可解析 ⇒ 失败并分类，不静默降级」。
4. **依赖方向**：新增 `nai-runtime → nai-upgrade-core`（`runtime → shared` 合法；`shared` 反向被禁，见 `xtask/src/deps.rs`）。

## 后果
- 正向：门的答案是**读真实会话得来**的，且「在播挡、停播放」双向与 fail-closed 各有确定性判据
  （真实 lifecycle 走 Created→Preparing→Ready→Running→Draining→Stopped；busy 锁用握手造竞争 ⇒ Conflict）。
- **R10 不因此升 `tested`**：本轮接的是「装配根**能**答 gate」，而运行时**仍无升级激活调用点**
  （`nai-runtime` / `apps/engine` 内无 `UpgradeTransaction` 使用处）⇒ 命题尚未在真实链路上被执行。
  这是「守卫通过 ≠ 本项达标」的同一条规矩（ADR 0100）。
- 生产代码只增一个**只读** trait 实现，不改既有行为路径；无契约/迁移/schema 改动。
- 如实披露：单会话装配下 `active()` 只会返回 0 或 1；多会话装配不在本轮范围。
