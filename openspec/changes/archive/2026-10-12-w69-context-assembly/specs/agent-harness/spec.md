# agent-harness 变更（w69）

## MODIFIED Requirements

### Requirement: Context is assembled in a fixed order under per-segment budgets

每次调用付费模型前，上下文 SHALL 由声明的装配步骤按**固定顺序**生成：系统策略、人设与场景、任务与输出契约、
当前事件与 Observation 摘要、已批准事实与检索证据、记忆片段、技能与工具描述、最近实际播完的句子、工具结果与外部返回。
前三个片段（策略、人设、任务契约）MUST NOT 被裁剪；预算不足时裁剪 MUST 只从后段发生，且 MUST 记录被裁项与原因。
Token 与字符 SHALL 同时计入，`remaining_budget_ms` 与 Token 预算是两个维度，MUST NOT 互相替代。
预算 SHALL **按段**分配，每个片段 MUST 有自己的配额，前三个片段还 MUST 带固定下限；除 Token 与字符外，
**条数**是第三个可判定的维度。阈值的**取值**由策略给定，MUST NOT 硬编码在本需求里，MUST NOT 采用外部观察值。
未定义段名、顺序错乱、不可裁段缺失或被裁、裁剪无原因或无被裁项、裁剪不落在可裁序列的后缀、缺配额、任一维度超限，
MUST 各自具名拒绝并指名涉及的片段。

#### Scenario: An uncroppable segment is missing

- **WHEN** 系统策略、人设或任务与输出契约任一片段缺失，或预算不足以容纳它们
- **THEN** MUST 拒绝调用该模型，MUST NOT 以「少几条策略继续生成」代替

#### Scenario: Budget pressure reaches the front segments

- **WHEN** 上下文超预算且裁剪落到前三个片段
- **THEN** 装配失败并具名报告，MUST NOT 静默裁掉策略或人设

#### Scenario: Cropping skips a front croppable segment

- **WHEN** 超预算触发的裁剪落在「已批准事实与检索证据」，而更靠后的「技能与工具描述」与「最近实际播完的句子」完好
- **THEN** 具名拒绝——「裁剪只从后段发生」意味着被裁的必须是**可裁序列的一个后缀**；
  一个只检查「有裁剪记录且记了原因」的实现会放行，故不满足本需求

#### Scenario: A crop that records neither items nor reason

- **WHEN** 某段被标记为已裁剪，但原因空白；或原因非空而被裁项为空
- **THEN** 分别具名拒绝，MUST NOT 让「说不清裁了什么」被读作已记录

#### Scenario: One total cap standing in for per-segment budgets

- **WHEN** 各片段共享一个总上限而自己不带独立配额
- **THEN** 具名拒绝，MUST NOT 以「总量仍在范围内」放行——本需求要求的是**按段**分配

#### Scenario: Only one dimension is recorded

- **WHEN** 某段只记录了 Token 用量而未记字符用量（或反之）
- **THEN** 具名拒绝——Token 与字符是两个维度，MUST NOT 互相替代

### Requirement: Every assembled fragment is sourced, scoped and reconstructible

每个进入上下文的片段 SHALL 携带来源类型（系统策略/场景/事件/知识证据/记忆/工具结果/外部智能体）、来源标识、作用域、时效（如适用），
以及**数据或指令**的标注。观众文本、检索片段、工具返回与外部智能体返回 MUST 标为数据，其内部出现的「忽略规则」「已批准」等措辞
MUST NOT 取得指令权。装配 SHALL 落决策日志（片段标识、各段预算占用、裁剪与原因、模型与提示词版本），且 MUST NOT 含明文凭据。
装配 SHALL 可由决策日志的片段引用重建，MUST NOT 依赖当时的内存状态。
来源标识 MUST NOT 为空白，作用域 MUST 携带（「时效」原文自带「如适用」，故其缺失 MUST NOT 被读作违规）。
重建 MUST **只**消费日志：日志缺该片段、或该片段的正文不在日志里而只存在于当时的内存，MUST 分别具名拒绝；
被引用片段在日志中的顺序与记录中的进入顺序不一致时，MUST 具名拒绝。

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
