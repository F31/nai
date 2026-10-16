# Design: W123 — 装配根接上真实会话

## 语义决策（用户裁定：A）
`active()` = **在播**会话数 ⇒ `lifecycle ∈ {Running, Draining}`。其余（Created/Preparing/Ready/ReadyDegraded/Stopped/Faulted）不算。

**为何不取「一切未停止的会话」**：本装配是单会话的，`prepare` 后会话即处于 `Created`。若把未停止都算活跃，
激活将**永久被 defer** ⇒ 门恒挡、实际不可用（是死的，而不是谨慎的）。

## 决策
1. **读真实会话，不缓存**：核心刻意不持有会话状态（缓存会成为第二份会过期的真相）；装配根每次读自己持有的 `Session`。
2. **不可观测 ⇒ fail closed**：会话锁 busy/poisoned 时 **返回错误**，MUST NOT 返回 `0`。返回 0 等于谎称「无人活跃」，
   会让版本切换在**开播中**发生——这正是该门存在的唯一理由。按「不可解析 ⇒ 失败并分类，不静默降级」。
3. **测试用握手造竞争**（既有 `hold_session` 辅助），不用 sleep 赌调度。
4. **判据落在既有测试文件**内（避免新增测试文件触发 test-reachability 登记）。

## 明确不做
- 不在运行时新建升级激活调用点（那是另一轮；本轮到此为止，因而 R10 不升 `tested`）。
- 不改 `Operation` 预算、不改会话生命周期机。
