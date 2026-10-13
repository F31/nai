## ADDED Requirements

### Requirement: Every CI workflow step is declared with its local coverage

CI 工作流的每一个 step SHALL 在一个在册登记表里有且只有一条登记，并 SHALL 归入一个闭集类别之一：
`infra`（环境准备，不产生判定）、`local`（本机门控同样执行）、`ci-only`（只有 CI 执行）、
`diagnostic`（仅在失败时跑的诊断通道）。`local` 的每条 SHALL 引出一处本机门控落点并证明它存在；
`ci-only` 的每条 SHALL 给出成因而成因 SHALL 取自一个闭集。新增、删除或改名一个 step，
SHALL 使检查失败并具名指出该 step。

#### Scenario: An unregistered step fails by name

- **WHEN** 往 workflow 插入一个没有登记的 step，或删掉一个已登记的判定 step
- **THEN** 检查失败，且错误里具名指出该 step 的身份

#### Scenario: Claiming local coverage has to pay for itself

- **WHEN** 一个 step 声明 `local`，但它引出的门控落点在该文件里不存在
- **THEN** 检查失败并具名该 step——「本机也跑」不是可以空口说的话

#### Scenario: A reason outside the closed set is a label, not a reason

- **WHEN** 一个 `ci-only` 的 step 给出的成因不在闭集里，或它同时又挂了一处本机落点
- **THEN** 检查失败并具名该 step
