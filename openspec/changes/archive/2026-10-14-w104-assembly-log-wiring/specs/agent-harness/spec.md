## MODIFIED Requirements

### Requirement: Every assembled fragment is sourced, scoped and reconstructible

每个进入上下文的片段 SHALL 携带来源类型（系统策略/场景/事件/知识证据/记忆/工具结果/外部智能体）、来源标识、作用域、时效（如适用），
以及**数据或指令**的标注。观众文本、检索片段、工具返回与外部智能体返回 MUST 标为数据，其内部出现的「忽略规则」「已批准」等措辞
MUST NOT 取得指令权。装配 SHALL 落决策日志（片段标识、各段预算占用、裁剪与原因、模型与提示词版本），且 MUST NOT 含明文凭据。
装配 SHALL 可由决策日志的片段引用重建，MUST NOT 依赖当时的内存状态。
来源标识 MUST NOT 为空白，作用域 MUST 携带（「时效」原文自带「如适用」，故其缺失 MUST NOT 被读作违规）。
重建 MUST **只**消费日志：日志缺该片段、或该片段的正文不在日志里而只存在于当时的内存，MUST 分别具名拒绝；
被引用片段在日志中的顺序与记录中的进入顺序不一致时，MUST 具名拒绝。判定的是「能否只靠日志重建」；
落链与运行期重建由 `runtime-composition` 承担，本需求 MUST NOT 被读作「已落链」。

#### Scenario: A fragment arrives without provenance

- **WHEN** 某片段缺少来源类型或作用域
- **THEN** 装配失败并指名该片段，MUST NOT 以空来源入上下文

#### Scenario: Rebuilding an assembly from the decision log

- **WHEN** 用决策日志中记录的片段引用重建某次调用的上下文
- **THEN** 重建结果的片段集合与各段预算占用 MUST 与原始记录一致（不要求逐字节相同，但要求同一片段集合与同一裁剪结论）

#### Scenario: An untrusted fragment is marked as an instruction

- **WHEN** 观众文本、检索片段、工具返回或外部智能体返回中的任一类被标为「指令」
- **THEN** 具名拒绝；这**四个**来源的清单 MUST 被逐项钉住，一个只判「有没有标注」的实现不满足本需求

#### Scenario: A fragment whose content was never logged

- **WHEN** 重建时某被引用片段的正文不在日志里，只能从当时的活动内存取得
- **THEN** 具名拒绝——「不依赖当时的内存」是本需求的判据，MUST NOT 用当前内存补齐后放行

#### Scenario: The log's own order is what decides

- **WHEN** 日志中被引用片段的排列顺序与记录中的进入顺序不一致
- **THEN** 具名拒绝；一个先按记录的进入顺序去排序再比较的实现会让本断言恒真，故不满足本需求
