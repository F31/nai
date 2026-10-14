## ADDED Requirements

### Requirement: A rebuildable assembly reaches the hash chain as its own fragments

装配根 SHALL 提供一处把一次上下文装配交给只追加哈希链的入口，且该入口 MUST 先经 `agent-harness` 的
装配判定：只有判定为**可重建**的装配才追加一条 `assembly` 类条目。追加的载荷 MUST **恰为**判定放行的
那三份材料——按记录**进入顺序**排列的片段（每项带其 `id`、来源标识 `source_ref` 与**字面正文**）、
模型版本、提示词版本；正文 MUST NOT 退化成指向当时内存的指针。追加 SHALL 复用装配根既有的单 owner
决策写入路径与逐次调用预算，MUST NOT 另开第二条写入路径。追加之后链 SHALL 校验通过。

#### Scenario: A rebuildable assembly is appended with its fragment bodies

- **WHEN** 一次装配经判定为可重建
- **THEN** 决策日志追加一条 `assembly` 条目，其载荷按进入顺序含每个被引用片段的标识、来源标识与**正文**，
  且哈希链校验通过

#### Scenario: A fragment body that only lives in memory is refused, not logged as a pointer

- **WHEN** 某被引用片段的正文不在交给判定的材料里（只存在于当时的内存）
- **THEN** 该装配被判为不可重建，入口具名拒绝且不追加；把正文落成指针（或 `null`）后追加的实现
  不满足本需求

### Requirement: A non-rebuildable assembly is named and leaves the chain untouched

判定为**不可重建**的装配 MUST NOT 触发任何追加：入口 MUST 返回一处**具名拒绝**，逐条列出判定给出的
拒绝名，并附上判定的三类问题标识（引用了却不在日志里的 id、正文只在当时的 id、正文含明文凭据的 id）
供调用方查看。入口的返回 MUST 可区分「已落日志」与「已拒绝」。把拒绝实现为静默 `Ok`（既不追加也不具名）
不满足本需求。

#### Scenario: The log carries a fragment the record never entered in that order

- **WHEN** 记录声明的进入顺序与日志自身携带的顺序不一致，且被引用片段齐全
- **THEN** 入口返回具名拒绝（含 `rebuild.order_differs`），且决策日志**未新增**任何条目

#### Scenario: A referenced fragment is absent from the log

- **WHEN** 记录引用的某个片段在日志里根本没有条目
- **THEN** 入口返回具名拒绝（含 `rebuild.fragment_not_logged`），且决策日志未新增任何条目

#### Scenario: A refusal does not masquerade as success

- **WHEN** 一次装配被判为不可重建
- **THEN** 入口的返回必须与「已落日志」可区分；以 `Ok` + 空载荷静默通过的实现不满足本需求

### Requirement: An assembly is rebuilt from the persisted chain alone

装配根 SHALL 提供一处**只读**入口，从只追加哈希链中读回 `assembly` 条目并重排其片段序列，判定依据 MUST
来自链本身而非调用方另给的切片；重排顺序 MUST 取**日志自身的顺序**，MUST NOT 先按记录顺序排序再比较。
重建 SHALL 复用 `agent-harness` 的装配判定，MUST NOT 另写第二份判定逻辑。读回 SHALL NOT 追加、修改或
重排链。

#### Scenario: A logged assembly is reproduced from the chain alone

- **WHEN** 一次可重建装配已追加进链，随后用仅携带标识与版本的记录从链重建
- **THEN** 重建结果可重建，且其片段 (标识, 来源标识) 序列与追加时的进入顺序一致

#### Scenario: Rebuilding reads the chain, not a caller-supplied slice

- **WHEN** 链中不存在任何 `assembly` 条目，而调用方持有一个非空记录
- **THEN** 重建报告记录引用的片段**全部**不在日志里，MUST NOT 因调用方能提供片段而判为可重建
