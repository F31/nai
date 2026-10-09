# knowledge-pipeline Specification

## Purpose
为 nai 提供本地优先、Rust 可运行的知识链路：有界导入与原子发布、中文词法/向量候选与融合排名、远端 knowledge API 契约，使证据可追溯且失效可撤销。

## Requirements

### Requirement: Bounded import with atomic publication
系统 SHALL 对导入大小/分块/句段计数设上限，解析为空或缺失 OCR 不得当已入库；发布 SHALL 使用 manifest 哈希与 fact revision 的 CAS 事务，任一阶段失败 SHALL 不留下部分索引。切块 SHALL 保留来源 span 与产品/ACL 作用域。

#### Scenario: Rejected oversized and empty import
- **WHEN** 文档超限或解析文本为空
- **THEN** 拒绝并记录，不写入文档/索引，不把空文本当已入库

#### Scenario: Snapshot CAS publication
- **WHEN** 发布时 manifest 哈希或 fact revision 与预期不一致
- **THEN** 发布失败且不产生部分可见快照

#### Scenario: Staged index stays invisible after restart
- **WHEN** 导入或索引已提交但发布尚未完成，或发布事务失败后重新打开存储
- **THEN** 普通检索只读取上一已发布活动快照，分阶段数据不进入结果

#### Scenario: Frozen session never follows a new active snapshot
- **WHEN** 会话绑定已发布快照后同一知识库发布新快照
- **THEN** 会话保留不可变绑定，新会话可绑定新版本；被撤销或被替代的旧文档证据失效而不偷偷切换绑定

#### Scenario: Fact or active pointer changes during indexing
- **WHEN** staging 后事实版本、权限版本或活动快照指针改变
- **THEN** 发布 CAS 失败，保留原活动快照与审计，不把过期 staging 直接激活

### Requirement: Chinese lexical and vector candidates
系统 SHALL 以明确算法在 Rust 中生成中文词法候选（中文切词）并索引；向量索引 SHALL 以 dimension/metric/normalization/模型快照/切块版本构成 identity，不同模型空间隔离。向量候选 SHALL 为有界精确检索用于验证，百万规模不得未测承诺低延迟。

#### Scenario: Chinese query recall
- **WHEN** 查询含中文且原文以中文字符连续出现
- **THEN** 词法候选可召回并携带来源 span 与 revision

#### Scenario: Mixed index identities isolated
- **WHEN** 两个不同 embedding 模型空间被写入同一工作区
- **THEN** 各自 identity 隔离，查询只使用匹配 identity 的索引

### Requirement: Filtered local retrieval and honest status
本地检索 SHALL 先按服务端 ACL、workspace、产品/场景与时间过滤，再融合精确 facts、中文词法与向量候选并按排名返回有界证据；Rerank 超时 SHALL 只保留已完成合法证据，不返回误导性空结果。检索状态 SHALL 区分 complete/partial/empty/timeout/unavailable/unauthorized，不得混淆 empty 与 timeout。

#### Scenario: Empty vs timeout
- **WHEN** 检索无候选或检索超时
- **THEN** 分别返回 empty 与 timeout，不由调用方臆断为空

### Requirement: Remote knowledge contract and revocation
远端 SHALL 提供 capabilities/health/search，可选 answer/sync/delete；能力标志区分 retrieval_with_evidence/answer_with_evidence/answer_only。调用 SHALL 有期限/取消/有界重试，401/403 不重试，429 遵循 Retry-After 且不超过期限；answer_only SHALL 仅作人工参考。删除 SHALL 先发布 tombstone/revocation 过滤所有待播，再清理本地索引与可删除远端副本；无法删除的第三方留存须在使用前明确政策。

#### Scenario: Remote unauthorized and rate limited
- **WHEN** 远端返回 401/403 或 429
- **THEN** 401/403 不重试并映射 unauthorized，429 在期限内重试后仍失败则 unavailable，不误报为 empty

#### Scenario: Local policy gates every optional operation
- **WHEN** answer/sync/delete 未声明、上传未授权、留存政策不兼容或本地授权已过期/撤销
- **THEN** 拒绝相应调用，未知能力不当授权；受限文档不能进入远端请求

#### Scenario: Answer-only stays human reference
- **WHEN** 远端仅返回 answer_only 或证据缺失可核验版本
- **THEN** 返回明确人工参考处置，不伪造 revision，也不授予自动外发许可

#### Scenario: Rerank budget expires
- **WHEN** 合法检索证据已完成，但可选 rerank 自身预算耗尽或返回未知证据 ID
- **THEN** 保留已完成证据为 partial，不注入未知证据或返回误导性 empty；整体任务取消仍停止处理

#### Scenario: Revocation filters pending output
- **WHEN** 产品/文档被撤销
- **THEN** 本地检索与待播证据不再引用该内容，且不宣称第三方已完整擦除
