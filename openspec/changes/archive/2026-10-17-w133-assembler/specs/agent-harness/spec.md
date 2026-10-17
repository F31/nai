## ADDED Requirements

### Requirement: Assembly order is imposed, not echoed
装配器 SHALL 按 §41.1 的固定序列输出段，MUST NOT 回显调用方的输入顺序。顺序存在的理由是「约束不被挤掉」，而回显恰好丢掉这一点。输入顺序与 §41.1 相反、或按段名排序时，输出 SHALL 仍为 §41.1 的顺序。

#### Scenario: Candidates arrive in reverse
- **WHEN** 调用方按 `tool_results`、`evidence`、`system_policy` 的顺序给出候选
- **THEN** 计划里这三段的顺序是 `system_policy`、`evidence`、`tool_results`

#### Scenario: The assembler sorts by name instead
- **WHEN** 装配器把段按名字排序（字母序）而非按 §41.1 的顺序
- **THEN** 不满足本需求；九段齐全的输入下顺序断言 MUST 变红

### Requirement: A segment with no candidates is absent, not empty
调用方未提供候选的段 SHALL 不出现在计划里，MUST NOT 以空切片补齐。§41.1 让 1–3 段必需，而「必需却缺席」由判定面具名拒绝；补一个空切片会让那条拒绝在真实路径上**没有对象可拒**。

#### Scenario: Only the policy segment is offered
- **WHEN** 调用方只给出 `system_policy` 的候选
- **THEN** 计划里只有这一段；补齐成九段、或补一个空切片的实现不满足本需求

#### Scenario: Padding hides the missing segment
- **WHEN** 装配器为缺席的段补空切片
- **THEN** 判定面再也看不到「不可裁段缺失」⇒ 不满足本需求

### Requirement: In-segment cropping runs from the tail and records both halves
段内超预算时 SHALL 从**尾部**裁，并 SHALL 同时记录被裁项与原因。只记原因不记被裁项、从头部裁、或不记录裁剪，都不满足本需求。存活部分的占用（tokens / chars / items）SHALL 只计存活项。

#### Scenario: Items over the call site's cap
- **WHEN** 一段给了三条候选而调用方的 items 上限是二
- **THEN** 保留前两条，被裁的第三条按 id 具名，原因非空

#### Scenario: Cropping takes the head
- **WHEN** 保留的是尾部而非头部
- **THEN** 不满足本需求；保留集合的 id 断言 MUST 变红

#### Scenario: Usage reports what was offered
- **WHEN** 占用报的是全部候选的合计而非存活项
- **THEN** 不满足本需求；裁剪后的 tokens / chars / items 断言 MUST 变红

### Requirement: The uncroppable segments are never cropped
§41.1 的 1–3 段 SHALL NOT 被裁剪，无论调用方给的预算如何。由此产生的超预算段 SHALL 原样输出，交由判定面拒绝——装配器 MUST NOT 为了让它合格而裁剪，也 MUST NOT 替它改预算。

#### Scenario: A floor segment exceeds its own cap
- **WHEN** `system_policy` 给了三条候选而 items 上限是一
- **THEN** 三条全部保留、无裁剪记录，且判定面判它不可调用

### Requirement: An undefined segment name is refused by name
§41.1 未定义的段名 SHALL 是具名拒绝，MUST NOT 被静默丢弃。丢弃会让判定面的 `UnknownSegment` 从产生侧**不可达**，并把「我不认识它」变成「没有东西需要认识」。

#### Scenario: A near-miss name is offered
- **WHEN** 调用方给了一个 §41.1 没定义的名字（例如 `system_prompt`）
- **THEN** 装配器返回 `assemble.unknown_segment`，不产出计划

### Requirement: The assembler never supplies a budget of its own
调用方未给预算的段 SHALL 以「无预算」原样输出，装配器 MUST NOT 代编默认预算。§41.2 的初值须由 §47.2 黄金集回放标定并记入 ADR；在这里发明一个数字等于把 §41.2 答第二次。该段是否可用由判定面答一次。

#### Scenario: The call site omits the budget
- **WHEN** 某段没有 `SegmentBudget`
- **THEN** 计划里该段的预算仍为空，且判定面报 `SegmentBudgetMissing`
