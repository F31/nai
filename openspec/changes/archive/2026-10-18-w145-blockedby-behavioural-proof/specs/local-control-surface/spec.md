# Spec delta: W145 — `blockedBy` 的取值由路由器自己作证

## ADDED Requirements

### Requirement: An entry's reason for minting no job id is proved by what the router does, not by a source anchor

每个在册项的 `blockedBy` 取值 SHALL 与路由器对该操作的**实际处理**一致：`no_handler` 的操作
MUST 落到兜底 `404 CAPABILITY_ABSENT`；`refuses_without_installer` 的操作 MUST 答
`503 CAPABILITY_NOT_READY` 且响应体 MUST NOT 携带 `job_id`。路由器 MUST NOT 对任一在册项答 `202`。
出现判据没有行为期望的类别 MUST 具名失败。

#### Scenario: A route declared absent gains a branch

- **WHEN** 某条 `no_handler` 的操作在路由器里获得了分支（不再落兜底 404）
- **THEN** 执行侧判据具名失败并给出该操作的实际状态码；MUST NOT 因为「锚点那句话还在」而放行

#### Scenario: A refusing route starts minting a job id

- **WHEN** 某条 `refuses_without_installer` 的操作不再拒绝，或拒绝时带上了 `job_id`
- **THEN** 执行侧判据具名失败；一个指向没人写的 journal 行的 id MUST NOT 被当成可观测面

#### Scenario: An absent route answers with the wrong absence

- **WHEN** 某条 `no_handler` 的操作答 `404` 但错误码不是 `CAPABILITY_ABSENT`（例如「所命名的对象不存在」）
- **THEN** 执行侧判据具名失败；「这条根本不在」与「这个名字查不到」MUST NOT 被混为一谈

#### Scenario: A class with no behavioural expectation

- **WHEN** 登记表使用了一个判据尚未给出响应形状的类别（今天是 `kind`）
- **THEN** 执行侧判据具名失败并说明「新类别 MUST 连同它承诺的响应形状一起到达」；
  MUST NOT 把它当作「暂无期望」而跳过

### Requirement: The registry the contract side judges is the same bytes the execution side probes

作业面登记表 SHALL 是一个可被两侧结构化读取的**单一来源**。契约侧判据与执行侧探针 MUST 读同一份
文件；两侧各持一份数据 MUST 具名失败。执行侧嵌入该文件的路径 MUST 逐字出现在契约侧判据所登记的
消费者锚点里。

#### Scenario: The execution side embeds another file

- **WHEN** 执行侧 `include_str!` 的路径不再是登记表的路径
- **THEN** 契约侧判据具名失败并给出期望的逐字锚点；「有个 `include_str!`」MUST NOT 被当作「读的是同一份」

#### Scenario: The registry is not in the tracked tree

- **WHEN** 登记表文件未被版本控制跟踪
- **THEN** 契约侧判据具名失败；干净 checkout 上两侧读不同的树 MUST NOT 被读成「已一致」

#### Scenario: The registry is absent rather than empty

- **WHEN** 判据被调用时没有传入登记表
- **THEN** 判据具名失败并指出缺失的是登记表；一个缺席的登记表 MUST NOT 被当成空登记表
