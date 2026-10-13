# agent-harness 变更（w89）

## ADDED Requirements

### Requirement: A closed viewer-memory capability is closed on the write face too, from one answering point

「观众长期记忆默认关闭」SHALL 由**唯一一处**判定，记忆的读取面、写入面与请求准入面 MUST 共用它，
MUST NOT 各自复写这个开关。未启用时，观众长期记忆的**写入** MUST 被具名拒绝，且拒因与读取面**相同**，
MUST NOT 出现「读取被拒、写入却被接受」的半覆盖。该判定的位置 SHALL 先于**来源与标注**的判定，
与读取面的「结构必填 → 能力开关 → 授权」同序。

#### Scenario: A viewer record is written while the capability is closed

- **WHEN** 未显式启用观众长期记忆，却试图写入一条观众长期记忆记录
- **THEN** 写入被具名拒绝，其名字与读取面的拒因**相同**，MUST NOT 被接受

#### Scenario: The capability is closed and the source is untrusted at the same time

- **WHEN** 一条候选同时命中「观众长期记忆未启用」与「来源不可信」
- **THEN** 报的是**能力未启用**——证明能力判定先于来源判定，且该顺序不是一句注释

#### Scenario: The capability is open and the candidate is compliant

- **WHEN** 观众长期记忆已启用，且候选的来源、作用域、来源标识与时间均已齐备
- **THEN** 写入通过；且该开关 MUST NOT 拒绝非观众类别的写入
