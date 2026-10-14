## ADDED Requirements

### Requirement: An allowed promotion into supervised automatic reaches the hash chain as its enablement record

装配根 SHALL 提供一处把一次爬坡晋升交给只追加哈希链的入口，且该入口 MUST 先经 `agent-harness` 的晋升
判定：只有判定**放行**且判定产出了一份启用记录时，才追加一条 `approval` 类条目。追加的载荷 MUST
**恰为**判定放行的那份材料——机制名、起始阶段、目标阶段，以及启用记录自带的那组字段；
MUST NOT 夹带判定未审过的字段（尤其请求携带的那两份回归类别集）。追加 SHALL 复用装配根既有的单 owner
决策写入路径与逐次调用预算，MUST NOT 另开第二条写入路径。追加之后链 SHALL 校验通过。
**该条目按 `approval` 类落账**（critical 类）⇒ 写入失败 MUST 原样上报，MUST NOT 静默进遥测缓冲：
启用记录是「要么落地、要么被报告」的决定，不是可丢遥测。

#### Scenario: An allowed promotion into supervised automatic is appended

- **WHEN** 一次晋升到受监督自动经判定放行，且启用记录齐备
- **THEN** 决策日志追加一条 `approval` 条目，其载荷含机制、起始阶段与目标阶段，且哈希链校验通过

#### Scenario: The appended payload is the judged material, not the request's regression classes

- **WHEN** 一次放行的晋升，其请求携带了受影响的回归类别集与已通过类别集
- **THEN** 追加载荷**不含**这两份类别集；把它们一并落账的实现不满足本需求

#### Scenario: A promotion below the gated stage is allowed without a record

- **WHEN** 一次晋升的目标阶段在受监督自动**之前**，且确为前进
- **THEN** 入口返回「放行但无需记录」这一具名结局，且决策日志**未新增**任何条目

### Requirement: A refused promotion is named and leaves the chain untouched

判定为**拒绝**的晋升 MUST NOT 触发任何追加：入口 MUST 返回一处**具名拒绝**，逐条列出判定给出的拒绝名，
并附上判定给出的**全部**缺失回归类别与值内含明文凭据的字段名供调用方查看。入口的返回 MUST 可区分
「已落记录」「放行但无需记录」「已拒绝」三种结局。把「放行但无需记录」折进拒绝、把拒绝折进
「已落记录」、或把拒绝折叠成静默 `Ok` 的实现，均不满足本需求。拒绝路径 MUST NOT 追加任何条目，
且 MUST NOT 因其被返回而把凭据字面量写入链。

#### Scenario: A refused promotion appends nothing

- **WHEN** 一次晋升被判定拒绝（受影响的回归类别未全部通过、或启用记录值内含明文凭据）
- **THEN** 入口返回具名拒绝，且决策日志**未新增**任何条目

#### Scenario: Every missing regression category is named, not just the first

- **WHEN** 晋升声明的受影响类别里有多个未通过
- **THEN** 具名拒绝列出**全部**缺失类别；只报第一个的实现不满足本需求

#### Scenario: A refusal does not masquerade as success

- **WHEN** 一次晋升被判定拒绝
- **THEN** 入口的返回必须与「已落记录」可区分；以 `Ok` + 空载荷静默通过的实现不满足本需求
