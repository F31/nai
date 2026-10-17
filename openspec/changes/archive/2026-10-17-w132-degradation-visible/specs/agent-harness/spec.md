## ADDED Requirements

### Requirement: A judged degradation is rendered into one notice that copies the judgement
一条已判定的降级 SHALL 被渲染成**恰好一条**界面可读的通知，SHALL 带有层标签、具名错误码、§12.2 落点级与一句**可行动**说明。通知的每个字段 SHALL **照抄判定输出**，渲染器 MUST NOT 从别处重算 code / level / 说明。渲染 SHALL **不去重也不合并**：同一层失败两次是两个降级事件。六层的可行动说明 SHALL 互异——一个对所有层通用的「已降级」是「可见但什么也没说」。空判定集 SHALL 报 `0`。

#### Scenario: The renderer substitutes a generic note
- **WHEN** 渲染器把说明换成对所有层相同的通用串
- **THEN** 不满足本需求；六层的说明 MUST 逐层不同

#### Scenario: Two failures of the same layer
- **WHEN** 同一层失败两次并被判定成两条降级
- **THEN** 渲染出**两条**通知；按层去重或只渲染第一条的实现不满足本需求

#### Scenario: A field is recomputed rather than copied
- **WHEN** 通知的 code、落点级或说明与判定输出不一致（例如写的是该层的**首选级**而非回退后的级）
- **THEN** 不满足本需求；在首选级恒等于回退级（全部可用）的输入下两种写法无差别，故 MUST 用首选级不可用的输入来判

### Requirement: The degradation view is decoded from the chain and never buffered
降级 SHALL 以 `degradation` 条目落进只追加的哈希链，且 SHALL 被计为 **critical**：telemetry 写失败会落进遥测缓冲，而一条能悄悄消失的降级就是静默降级。界面视图 SHALL **只**由链上解码得出，MUST NOT 由调用方给出。读回时不可解析的行 SHALL 是**具名失败**，MUST NOT 被静默跳过；空链 SHALL 报 `0`；视图的域 SHALL 恰为 `degradation` 那一种，其它种类 MUST NOT 混进来。无失败 SHALL 不落盘。

#### Scenario: A row that cannot be decoded
- **WHEN** 链上一条 `degradation` 行的载荷不是本写路径写过的形状
- **THEN** 读回具名失败并指名缺什么；`unwrap_or_default()` 式跳过的实现不满足本需求

#### Scenario: A degradation would be buffered
- **WHEN** `degradation` 被计为 telemetry 且一次写入失败
- **THEN** 它落进遥测缓冲而界面永远看不到 ⇒ 不满足本需求；该种类 MUST 是 critical

#### Scenario: Another kind carries a decodable payload
- **WHEN** 链上另有一种类的行带着**可解码**的通知载荷
- **THEN** 它 MUST NOT 出现在降级视图里；只按载荷形状收行的实现不满足本需求

#### Scenario: Nothing degraded
- **WHEN** 本次调用没有任何层失败
- **THEN** 视图为空且不落盘；判定照跑，MUST NOT 把「未检查」写成「检查过为零」

### Requirement: The free-form rule is decided on the runtime path and writes nothing
判定 SHALL 在生产入口里**先于写入**执行：先算出每一层的落点，再判 INV-32 后半段。当本次调用存在任一降级且上下文已失去策略或人设约束时 SHALL **具名拒绝**并 MUST **一个字节都不落盘**——命名了错误码却仍继续生成、或边拒绝边落盘，都不满足本需求。拒绝 SHALL 携带「本来会显示什么」以便检查，但那部分明确**不落盘**。无降级时该规则不适用。

#### Scenario: A degraded call lost its policy constraint
- **WHEN** 存在降级且装配后上下文失去策略约束
- **THEN** 具名拒绝且链上零新增；只落盘不判、或判了照落的实现都不满足本需求

#### Scenario: A degraded call lost its persona
- **WHEN** 存在降级且上下文失去人设
- **THEN** 以**另一个**具名违规拒绝；两类违规 MUST 分别具名，MUST NOT 互相顶替

#### Scenario: The caller supplies the verdict instead of the facts
- **WHEN** 入口接收「落到哪一级」而不是「哪些层失败了、还剩什么可用」
- **THEN** 不满足本需求——判定 MUST 由本入口在生产路径上算出，MUST NOT 由调用方注入
