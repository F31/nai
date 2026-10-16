## MODIFIED Requirements

### Requirement: An assembly is rebuilt from the persisted chain alone

装配根 SHALL 提供一处**只读**入口，从只追加哈希链中读回 `assembly` 条目并重排其片段序列，判定依据 MUST
来自链本身而非调用方另给的切片；重排顺序 MUST 取**日志自身的顺序**，MUST NOT 先按记录顺序排序再比较。
重建 SHALL 复用 `agent-harness` 的装配判定，MUST NOT 另写第二份判定逻辑。读回 SHALL NOT 追加、修改或
重排链。
判定 MUST 以**条目**为单位：一个条目就是一次装配（§41.5 的四项由写路径**一起**落进一条条目），入口
MUST NOT 先把多个条目合并成一个片段池再判定。记录引用的片段标识**多重集**（含重复）与某个条目的相同
才视为该条目**承载**了这次装配；顺序不参与承载判定，承载的条目不因顺序不同而被排除。承载的条目**存在
一个**能重建即判为可重建，MUST NOT 因另一条承载条目的顺序不同而否决它。没有任何条目承载时 MUST 具名
报告 `rebuild.not_in_one_entry`，且 MUST NOT 因此把**在链上确实存在**的标识报成未落日志。

#### Scenario: A logged assembly is reproduced from the chain alone

- **WHEN** 一次可重建装配已追加进链，随后用仅携带标识与版本的记录从链重建
- **THEN** 重建结果可重建，且其片段 (标识, 来源标识) 序列与追加时的进入顺序一致

#### Scenario: Rebuilding reads the chain, not a caller-supplied slice

- **WHEN** 链中不存在任何 `assembly` 条目，而调用方持有一个非空记录
- **THEN** 重建报告记录引用的片段**全部**不在日志里，MUST NOT 因调用方能提供片段而判为可重建

#### Scenario: Halves in two entries are not this assembly

- **WHEN** 链上两个条目分别承载记录引用的一部分标识，而没有任何一个条目承载全部
- **THEN** 重建判为**不可重建**并具名 `rebuild.not_in_one_entry`
- **AND** 这些标识**在链上确实存在**，故 MUST NOT 被报成未落日志（`unlogged_ids` 为空）

#### Scenario: A carrying entry with a different order keeps its own name

- **WHEN** 链上有一个条目承载了记录的全部标识，但自身顺序与记录不同
- **THEN** 重建按具名的顺序原因拒绝，MUST NOT 被替换成 `rebuild.not_in_one_entry`
  （承载判据不计顺序，正是为了不让一条具体原因失去覆盖）

#### Scenario: An entry holding more than the record is a different assembly

- **WHEN** 记录只引用一个标识，而链上唯一的条目还持有一个记录未引用的标识
- **THEN** 该条目不承载这次装配，重建判为不可重建；MUST NOT 用它重建出一个更窄的记录
