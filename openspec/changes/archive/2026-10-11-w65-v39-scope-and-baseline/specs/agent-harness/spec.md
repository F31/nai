# agent-harness 变更（w65）

## ADDED Requirements

### Requirement: Harness basics stay in the first release, the rest are capability packs

harness 的**基础纪律** MUST 留在首版门槛（R1）内：上下文装配与分段预算（H01、H02）、记忆写入门槛与作用域读取（H03、H04）、
观测入决策日志（H15）、逐层具名降级（H17），以及既有的模式爬坡门控（H18）。其余 SHALL 属 `capability_pack`：
意图识别（H05、H06）、语义缓存（H07、H08）、多步工具循环（H09、H10）、MCP（H11、H12）、A2A（H13、H14）、
harness 黄金集（H16）。分界判据 MUST 是「**一个确定性的上下文装配函数能不能满足它？**」。
降级 MUST NOT 删除该能力的契约边界、信任域归属与不变量（**INV-34**），MUST NOT 因降级使既有 `§NNN` 引用失效。

#### Scenario: A basic discipline is demoted to a capability pack

- **WHEN** 某一项被当作「可选子系统」后置（例如记忆写入门槛、逐层具名降级或模式爬坡门控）
- **THEN** 该后置 MUST 被拒绝：没有它默认组合**无法**正确调用模型，它不是增强项；分界判据 MUST 被重新适用

#### Scenario: A capability pack is enabled

- **WHEN** 一个能力包被安装或在默认组合中被启用
- **THEN** 它 MUST 先通过自身验收、MUST 走同一条升级与迁移图（MUST NOT 出现第二条安装路径），
  且 MUST NOT 改变默认组合的行为、成本或既有降级顺序；§5.9 的模式爬坡对它同样适用

#### Scenario: A demotion deletes a boundary

- **WHEN** 某个被降级的能力，其契约边界、信任域归属或不变量在降级过程中被删去，或使既有 `§NNN` 引用失效
- **THEN** 该改动 MUST 被拒绝——降级是**排期**动作，不是**设计**动作（INV-34）
