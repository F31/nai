## ADDED Requirements

### Requirement: The configuration text bound is inclusive and refused by name
一次配置写入的序列化文本 SHALL 受一个声明上限约束；恰为该上限的文本 MUST 被接受，多一字节 MUST 以 `resource_exhausted` 具名拒绝，MUST NOT 被静默截断。

#### Scenario: At the declared ceiling is accepted and one byte over is refused
- **WHEN** 保存一份序列化后恰为声明上限的配置
- **THEN** 写入被接受
- **WHEN** 保存一份比上限多一字节的配置
- **THEN** 写入以 `resource_exhausted` 具名拒绝

### Requirement: The fact encoding bound is measured on the bytes that are stored
一份权威事实 SHALL 在完成发布改写（版本 / 发布者 / 权限版本）**之后**按编码字节数受上限约束；恰在界上 MUST 被接纳，越界 MUST 以 `resource_exhausted` 具名拒绝且不留下任何行。

#### Scenario: The bound falls on the stored encoding, not the caller's input
- **WHEN** 发布一份改写后编码恰为声明上限的事实
- **THEN** 落库的字节数等于该上限
- **WHEN** 发布一份改写后编码超过上限的事实
- **THEN** 以 `resource_exhausted` 具名拒绝，且库中不留下半条记录

### Requirement: The candidate dependency bound is separately attributable from the candidate payload bound
候选记录 SHALL 对「依赖条数」与「候选编码字节」分别给出**可区分**的具名拒绝；恰好踩在两条上限上的候选 MUST 被接纳。

#### Scenario: Each of the two conditions has its own refusal
- **WHEN** 提交一条依赖条数越界、编码远低于上限的候选
- **THEN** 拒绝理由指向依赖条数上限
- **WHEN** 提交一条编码越界、依赖条数为零的候选
- **THEN** 拒绝理由指向候选编码上限
- **WHEN** 提交一条依赖条数与编码都恰好踩在上限上的候选
- **THEN** 写入被接受

### Requirement: The pending-candidate capacity counts unapproved work
一个会话被接纳的**未获批准**候选数 SHALL 受一个声明上限约束；恰为该上限的那一条 MUST 被接纳，其后一条 MUST 以 `resource_exhausted` 具名拒绝。

#### Scenario: The bound-th candidate is admitted and the next one is refused
- **WHEN** 连续提交恰好达到该上限的候选
- **THEN** 每一条都被接纳
- **WHEN** 再提交一条候选
- **THEN** 以 `resource_exhausted` 具名拒绝，且不留下行

### Requirement: The snapshot document selection bound is attributable to the count
一次快照 SHALL 对所选文档数给出上限；空选集与越界 MUST 各自被具名拒绝，且越界那一侧的拒绝 MUST 只可能来自条数。

#### Scenario: An empty selection and an oversized selection are refused, at the ceiling is accepted
- **WHEN** 请求一个空的文档选集
- **THEN** 以 `invalid_argument` 具名拒绝
- **WHEN** 选集包含一个不存在的文档
- **THEN** 以指向「文档缺失或重复」的具名拒绝拒绝
- **WHEN** 选集恰为声明上限且每个文档都存在
- **THEN** 快照被接受
- **WHEN** 选集比上限多一个成员
- **THEN** 以 `invalid_argument` 具名拒绝

### Requirement: The artifact count bound counts distinct paths
导出快照时登记的托管文件数 SHALL 按**不重复路径**计数并受一个声明上限约束；恰为上限时导出 MUST 成功，多一条路径时 MUST 以 `resource_exhausted` 具名拒绝。

#### Scenario: The ceiling exports and one more path is refused
- **WHEN** 托管文件的不重复路径数恰为声明上限
- **THEN** 导出被接受
- **WHEN** 再多登记一条不同的路径
- **THEN** 导出以 `resource_exhausted` 具名拒绝

### Requirement: The revocation batch bound counts distinct document revisions
一次撤销 SHALL 受「它触碰的不重复 `(document, revision)` 数」上限约束，而**不是**所选文档数；恰为上限的批次 MUST 被放行，多一个不重复版本对 MUST 以 `resource_exhausted` 具名拒绝且不落下任何撤销记录。

#### Scenario: A batch at the ceiling is admitted and one more revision pair is refused
- **WHEN** 撤销批次触碰的不重复版本对恰为声明上限
- **THEN** 撤销被放行
- **WHEN** 同一批文档再多铸出一个版本（使版本对越界）
- **THEN** 撤销以 `resource_exhausted` 具名拒绝，且不增加任何撤销记录
