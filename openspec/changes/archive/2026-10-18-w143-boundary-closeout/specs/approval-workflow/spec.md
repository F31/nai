# approval-workflow Specification

## ADDED Requirements

### Requirement: A recorded candidate is stored as pending and a fresh approval advances it to approved

登记候选 SHALL 把该行的 `utterances.status` 落成**待批准**的声明值；批准 SHALL 把**同一条**
候选推进成**已批准**的声明值。两个取值 MUST 是**落盘契约**：它们是既有数据库里已经存在的值，
改变必须配迁移，MUST NOT 只改一处常量了事。判定 MUST 从存储读回，MUST NOT 采信调用方给出的值。

#### Scenario: Recording a candidate stores the pending value

- **WHEN** 经生产的 `record_candidate` 登记一条候选
- **THEN** 从存储读回该行的 `status` MUST 是待批准的声明值

#### Scenario: Approving the same candidate advances it

- **WHEN** 对同一条候选调用 `approve` 并成功
- **THEN** 从存储读回它的 `status` MUST 是已批准的声明值，且 MUST NOT 仍是待批准

#### Scenario: The stored numbers are pinned literally

- **WHEN** 有人改动待批准或已批准的**数字常量**
- **THEN** 判据 MUST 变红 —— 断言钉的是**存储里的值**，不是「常量等于常量」
