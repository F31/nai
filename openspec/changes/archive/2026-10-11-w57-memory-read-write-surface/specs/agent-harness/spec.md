# agent-harness 变更（w57）

## ADDED Requirements

### Requirement: A memory write carries its source, and only played approved content becomes a fact

每次记忆写入 SHALL 声明**写入来源**（已批准且已播完／已批准未播出／工具返回／外部智能体返回／评论原文／模型草稿）、
作用域、来源标识、时间，以及「叙述还是权威事实」的标注。只有**已批准且已播完**的内容可写为事实；
工具返回、外部智能体返回、评论原文与模型草稿 MUST NOT 写成事实或承诺，MUST 被**具名拒绝**。
只有**语义记忆**可承载权威事实，且其 MUST 带 §6 的发布引用。缺作用域、来源标识或时间的写入 MUST 被具名拒绝。

#### Scenario: A tool result, peer return, comment or model draft is offered as a fact

- **WHEN** 试图把工具返回、外部智能体返回、评论原文或模型草稿写成场次记忆中的事实或承诺
- **THEN** 写入被具名拒绝，并指向 §6 的导入与发布流程

#### Scenario: An approved but unplayed candidate is offered as a fact

- **WHEN** 某候选已批准但**尚未播完**却被写为事实
- **THEN** 写入被具名拒绝，MUST NOT 落库

#### Scenario: An authoritative fact lacks a publication reference, or sits outside semantic memory

- **WHEN** 某记录标为权威事实却没有 §6 的发布引用，或该记录不属于语义记忆
- **THEN** 写入被具名拒绝

#### Scenario: A write omits its scope, source reference or timestamp

- **WHEN** 写入缺少作用域、来源标识或时间中的任一项
- **THEN** 写入被具名拒绝，MUST NOT 以默认值补齐

### Requirement: Memory reads filter by scope before time, and every refusal is named

读取 SHALL **先按作用域**（workspace／session／观众作用域）过滤，**再按时效**过滤；过期、被撤销、权限不足的条目
MUST NOT 进入上下文，MUST NOT 用「近似条目」顶替。观众长期记忆 **默认关闭**；其读取 MUST 有授权引用并记录用途，
缺任一项 MUST 以「无记忆」继续，MUST NOT 以推断补齐。记忆段 SHALL 有独立配额，超配额 MUST 按
「**场次记忆 > 工作记忆 > 观众长期记忆**」的固定优先级裁剪。每一次拒绝（越作用域、过期、撤销、权限不足、
近似顶替、超配额）MUST 具名并保留记录引用，MUST NOT 静默丢弃。

#### Scenario: A read crosses scopes

- **WHEN** 某会话读取另一个 workspace、另一个会话或另一个观众作用域的记忆
- **THEN** 该条目被具名拒绝且不进入上下文，MUST NOT 返回近似条目顶替

#### Scenario: Expired, revoked or under-permitted records

- **WHEN** 命中记录已过期、已被撤销，或调用方权限不足
- **THEN** 该记录 MUST NOT 进入上下文，并以各自的名字出现在拒绝清单里

#### Scenario: Long-term viewer memory is off by default

- **WHEN** 未显式启用观众长期记忆，或读取时缺少授权引用或用途记录
- **THEN** 读取以「无记忆」继续并具名拒绝，MUST NOT 写画像、MUST NOT 以推断补齐

#### Scenario: The budget is exceeded

- **WHEN** 作用域与时效过滤后的条目超出记忆段配额
- **THEN** 按「场次 > 工作 > 观众长期记忆」裁剪，被裁条目 MUST 以名字出现在拒绝清单里，MUST NOT 静默截断

### Requirement: Memory forgetting is verifiable and claims no more than the purge achieved

TTL 到期、事实失效、权限撤销与用户请求四类触发的遗忘 SHALL 共用同一条删除链：**先阻断检索与读取，再异步清除副本**。
删除 MUST 可验证：读取侧 MUST 先失效，清理侧 MUST 给出完成状态。存在第三方留存时，MUST 在使用前明示该政策。
未完成清除却宣称已完全擦除、或存在第三方留存却宣称已完全擦除、或阻断态未给出留存说明，MUST 均被**具名拒绝**。

#### Scenario: The read side is blocked after the purge

- **WHEN** 某次遗忘的读取侧失效发生在清除之后，或从未发生
- **THEN** 拒绝，MUST NOT 记为已完成删除

#### Scenario: An erasure claim outruns the purge

- **WHEN** 清除未完成（或仍有第三方留存）却宣称已完全擦除
- **THEN** 拒绝该宣称并具名指出原因

#### Scenario: A third-party retention is not disclosed

- **WHEN** 清除受阻于第三方留存却没有给出留存说明
- **THEN** 拒绝，MUST NOT 让「说不清」被读成「已清完」

### Requirement: The memory surface exposes exactly the four operations of memory.access@1

`memory.access@1` 已由能力目录标为 `frozen`（与基线 §20.6 的「保持 Draft」不一致，**以目录为准**）。
记忆访问 SHALL 以四项操作 `write` / `read` / `forget` / `scopes` 表达，四项以外的操作名 MUST 被**具名拒绝**，
MUST NOT 被当作未知操作静默接受。四项操作的语义 SHALL 落进一致性套件，覆盖作用域越权的拒绝、过期与撤销的拒绝、
以及删除的可验证性。

#### Scenario: An operation outside the four is offered

- **WHEN** 请求携带四项之外的操作名
- **THEN** 具名拒绝，MUST NOT 静默接受或猜测其意图

#### Scenario: The four operations are exercised together

- **WHEN** 一致性套件依次执行 write、read、forget 与 scopes 的作用域完整性判据
- **THEN** 每一项都必须能在**不依赖真实存储**的前提下被构造与判定
