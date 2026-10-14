## ADDED Requirements

### Requirement: A judged-loggable harness observation reaches the hash chain

装配根 SHALL 提供一处把 harness 观测记录交给只追加哈希链的入口，且该入口 MUST 先经
`agent-harness` 的观测判定：只有判定为**可落日志**的记录才追加一条 `observation` 类条目。
追加的载荷 MUST **恰为**判定输出的已脱敏材料（层标签、脱敏字段、脱敏观众原文三项），
MUST NOT 携带记录中任何未经判定的原始字段——runtime 是这份材料的**传输者**，不是加工者。
追加 SHALL 复用装配根既有的单 owner 决策写入路径与逐次调用预算，MUST NOT 另开第二条写入路径。
追加之后链 SHALL 校验通过。

#### Scenario: A loggable record is appended and the chain verifies

- **WHEN** 五类观测之一的一条记录经判定为可落日志
- **THEN** 决策日志追加一条 `observation` 条目，其载荷的层标签等于该记录的层，且哈希链校验通过

#### Scenario: The appended payload is the judged material, not the raw record

- **WHEN** 一条可落日志记录的某字段值超过字段上限（判定将其脱敏为截断标记）
- **THEN** 追加载荷在该字段处为**截断标记**而非原值；把 `record.fields` 原样落日志的实现不满足本需求

### Requirement: A refused harness observation is named and leaves the chain untouched

判定为**不可落日志**的记录 MUST NOT 触发任何追加：入口 MUST 返回一处**具名拒绝**，逐条列出判定给出的
拒绝名与其层标签，并附上判定输出的已脱敏材料供调用方查看；该材料 MUST NOT 因随拒绝返回而落盘。
入口的返回 MUST 可区分「已落日志」与「已拒绝」。把拒绝实现为静默 `Ok`（既不追加也不具名）不满足本需求。

#### Scenario: A record carries a plaintext credential

- **WHEN** 记录的某字段值内含明文凭据字面量
- **THEN** 入口返回具名拒绝（含 `observation.plaintext_credential`），且决策日志**未新增**任何条目

#### Scenario: A required field is missing

- **WHEN** 某一层记录缺少该层规定的必需项之一
- **THEN** 入口返回具名拒绝（含 `observation.missing_required_field`），且决策日志未新增任何条目

#### Scenario: A refusal does not masquerade as success

- **WHEN** 记录被判为不可落日志
- **THEN** 入口的返回必须与「已落日志」可区分；以 `Ok` + 空载荷静默通过的实现不满足本需求
