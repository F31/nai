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

### Requirement: Remote delete contract enforces input bounds and never claims third-party erasure
远端删除 SHALL 以 `RemoteGateway::delete` 传输契约执行：删除目标 id 列表 SHALL 非空、≤16、且每个 id 非空，否则 SHALL 以 `Unauthorized` 拒绝且不触网。远端回执 SHALL 原样透传，系统 SHALL NEVER 将 `RemoteReceiptStatus` 的任何变体解释为「第三方副本已擦除」的证明——`RemoteReceiptStatus` 的有界集合（accepted/pending/deleted/retained）SHALL 不含任何「已擦除」语义。删除前的本地 tombstone 由存储层 `remote_delete_targets` 负责（§24.4），传输契约假定其已存在。

#### Scenario: Malformed delete targets are rejected without network
- **WHEN** 删除目标为空列表、超过 16 个、或含空 id
- **THEN** `RemoteGateway::delete` 返回 `Unauthorized` 且不向远端 `/delete` 端点发起请求

#### Scenario: Remote receipt is passed through and never claims erasure
- **WHEN** 远端返回 `RemoteReceipt{status: deleted, ...}`
- **THEN** 网关原样返回该回执（`request_id` / `policy_ref` / `status` 透传），且 `RemoteReceiptStatus` 的穷尽匹配仅含 accepted/pending/deleted/retained 四态，无任何「第三方已擦除」语义

### Requirement: Remote gateway construction rejects invalid policy
`RemoteGateway::new` SHALL 拒绝 `source_id` / `policy_ref` / `permission_revision` 任一为空、`allowed_kb_ids` 为空、或 `unknown_revision_ttl_ms` 为 0 或 >5000 的 policy，返回 `Invalid` 且不触网。

#### Scenario: Malformed policy is rejected at gateway construction
- **WHEN** policy 的 `source_id` / `policy_ref` / `permission_revision` 为空、`allowed_kb_ids` 为空、或 `unknown_revision_ttl_ms` 为 0 或 >5000
- **THEN** `RemoteGateway::new` 返回 `Invalid` 且不发起任何网络请求

### Requirement: Remote knowledge client construction rejects invalid args
`RemoteKnowledgeClient::new` SHALL 拒绝空 endpoint、endpoint 长度 >2048、`timeout` 为零或 >600 秒、`max_retries` >10，返回 `Invalid` 且不建立 http client（不触网）。

#### Scenario: Malformed client args are rejected at client construction
- **WHEN** `endpoint` 为空 / 长度 >2048、`timeout` 为零 / >600 秒、或 `max_retries` >10
- **THEN** `RemoteKnowledgeClient::new` 返回 `Invalid` 且不建立 http client

### Requirement: Remote sync rejects malformed documents without network
`RemoteGateway::sync` SHALL 仅接受非空、≤16 个、且每个文档 `content` 长度 ≤65536 字节的文档集；否则 SHALL 以 `Unauthorized` 拒绝且不向远端 `/sync` 端点发起请求。

#### Scenario: Malformed sync documents are rejected without network
- **WHEN** `sync` 文档集为空、超过 16 个、或任一文档 `content` 长度 >65536 字节
- **THEN** `RemoteGateway::sync` 返回 `Unauthorized` 且不向远端 `/sync` 端点发起请求

### Requirement: Remote capabilities requires a search or answer first operation
`RemoteGateway::capabilities` SHALL 仅当 policy 首个 `allowed_operations` 为 `Search` 或 `Answer` 时才查询远端 capabilities，否则 SHALL 以 `Unauthorized` 拒绝且不触网。

#### Scenario: Capabilities requires a search or answer first operation
- **WHEN** policy 首个 `allowed_operations` 不是 `Search` 或 `Answer`（如 `Sync` / `Delete`）
- **THEN** `RemoteGateway::capabilities` 返回 `Unauthorized` 且不查询远端 capabilities

### Requirement: Remote rerank short-circuits empty base without invoking reranker
`rerank_evidence` SHALL 当 `base` 为空时立即返回 `RetrievalEmpty` 状态与空证据集，不进入 `spawn_blocking`、不调用 reranker 闭包。

#### Scenario: Empty base short-circuits without invoking reranker
- **WHEN** `rerank_evidence` 以空 `base` 调用（reranker 为会 panic 的探针）
- **THEN** 返回 `status == RetrievalEmpty` 且 `evidence` 为空，且 reranker 闭包未被调用（panic 未触发）

### Requirement: Remote rerank deduplicates repeated evidence IDs
`rerank_evidence` SHALL 当 reranker 返回的序列含重复 `evidence_id` 时仅保留每个 id 的首次出现，丢弃后续重复项。

#### Scenario: Repeated evidence IDs are deduplicated
- **WHEN** `base` 含重复 `evidence_id`（如 `[a, a, b]`）且 reranker 为恒等函数
- **THEN** 返回证据集长度为去重后数量（2）且顺序为 `[a, b]`，无重复 id 透传

### Requirement: Remote rerank maps reranker failure to Invalid
`rerank_evidence` SHALL 当 reranker 闭包 panic（导致 `spawn_blocking` 返回 `JoinError`）时返回 `RemoteError::Invalid`，不静默降级为 partial 或 empty。

#### Scenario: Reranker failure maps to Invalid
- **WHEN** reranker 闭包 panic（模拟重排故障）
- **THEN** `rerank_evidence` 返回 `Err(RemoteError::Invalid)`，而非 `partial` / `empty` 假阴性

### Requirement: Remote search drops evidence with empty ID
`RemoteKnowledgeClient::search` 经 `to_evidence` SHALL 丢弃 `evidence_id` 为空的远端证据项，不将其纳入检索结果。

#### Scenario: Evidence with empty ID is dropped from search results
- **WHEN** 远端 `/search` 返回含 `evidence_id` 为空的证据项（如 `[{evidence_id:"good"}, {evidence_id:""}]`）
- **THEN** `search` 结果只含非空 id 项（`len() == 1` 且 `evidence_id == "good"`）

### Requirement: Remote error Unauthorized maps to RetrievalUnauthorized
`RemoteError::status()` SHALL 把 `RemoteError::Unauthorized` 映射为 `RetrievalStatus::RetrievalUnauthorized`。

#### Scenario: Unauthorized maps to RetrievalUnauthorized
- **WHEN** 取 `RemoteError::Unauthorized.status()`
- **THEN** 返回 `RetrievalStatus::RetrievalUnauthorized`

### Requirement: Remote error Unavailable maps to RetrievalUnavailable
`RemoteError::status()` SHALL 把 `RemoteError::Unavailable` 映射为 `RetrievalStatus::RetrievalUnavailable`。

#### Scenario: Unavailable maps to RetrievalUnavailable
- **WHEN** 取 `RemoteError::Unavailable.status()`
- **THEN** 返回 `RetrievalStatus::RetrievalUnavailable`

### Requirement: Remote error Invalid collapses to RetrievalUnavailable
`RemoteError::status()` SHALL 把 `RemoteError::Invalid` 映射为 `RetrievalStatus::RetrievalUnavailable`（非显然坍缩：不存在独立的 `RetrievalInvalid` 分支）。

#### Scenario: Invalid collapses to RetrievalUnavailable
- **WHEN** 取 `RemoteError::Invalid.status()`
- **THEN** 返回 `RetrievalStatus::RetrievalUnavailable`

### Requirement: Remote error Timeout and Cancelled collapse to RetrievalTimeout
`RemoteError::status()` SHALL 把 `RemoteError::Timeout` 与 `RemoteError::Cancelled` 都映射为 `RetrievalStatus::RetrievalTimeout`（两个不同语义的错误坍缩进同一状态）。

#### Scenario: Timeout and Cancelled collapse to RetrievalTimeout
- **WHEN** 取 `RemoteError::Timeout.status()` 与 `RemoteError::Cancelled.status()`
- **THEN** 两者都返回 `RetrievalStatus::RetrievalTimeout`

### Requirement: Remote search truncates evidence to top_k preserving order
远端检索结果 SHALL 截断为不超过 `top_k` 项，且保留远端返回序列的前 `top_k` 项及其相对顺序。

#### Scenario: Evidence is truncated to the first top_k in order
- **WHEN** 远端 `/search` 返回 5 个有效证据项且以 `top_k = 2` 调用检索
- **THEN** 结果恰为前 2 个证据项且顺序不变（`[e0, e1]`）

### Requirement: Remote search applies top_k after dropping empty-ID evidence
远端检索结果 SHALL 在丢弃 `evidence_id` 为空的证据项**之后**再按 `top_k` 截断，即 `top_k` 统计的是有效证据项数量而非原始回包项数。

#### Scenario: top_k counts valid evidence after dropping empty IDs
- **WHEN** 远端 `/search` 返回 `[e0, "", e1, e2]`（含 1 个空 id 项）且以 `top_k = 2` 调用检索
- **THEN** 结果为 `[e0, e1]`（长度 2，空 id 项不占用 `top_k` 名额）

### Requirement: Remote search with zero top_k yields empty evidence
远端检索结果在 `top_k = 0` 时 SHALL 返回空证据集合，不得静默视为「不截断」。

#### Scenario: top_k zero truncates to empty
- **WHEN** 以 `top_k = 0` 调用检索（远端返回非空证据）
- **THEN** 结果证据集为空

### Requirement: Remote search with top_k exceeding reply keeps all evidence
远端检索结果在 `top_k` 大于有效证据项数时 SHALL 保留全部有效证据项，不得补齐占位项、不得报错。

#### Scenario: top_k greater than evidence count keeps all
- **WHEN** 远端 `/search` 返回 5 个有效证据项且以 `top_k = 10` 调用检索
- **THEN** 结果含全部 5 个证据项且顺序不变

### Requirement: Remote search maps partial body status to RetrievalPartial
远端检索回包 `status` 为 `partial` 时，`parse_reply` SHALL 把结果状态映射为 `RetrievalStatus::RetrievalPartial`。

#### Scenario: partial body status maps to RetrievalPartial
- **WHEN** 远端 `/search` 返回 HTTP 200 且 body `status` 为 `"partial"`
- **THEN** 检索结果状态为 `RetrievalStatus::RetrievalPartial`

### Requirement: Remote search maps timeout body status to RetrievalTimeout
远端检索回包 `status` 为 `timeout` 时，`parse_reply` SHALL 把结果状态映射为 `RetrievalStatus::RetrievalTimeout`。

#### Scenario: timeout body status maps to RetrievalTimeout
- **WHEN** 远端 `/search` 返回 HTTP 200 且 body `status` 为 `"timeout"`
- **THEN** 检索结果状态为 `RetrievalStatus::RetrievalTimeout`

### Requirement: Remote search maps unavailable body status to RetrievalUnavailable
远端检索回包 `status` 为 `unavailable` 时，`parse_reply` SHALL 把结果状态映射为 `RetrievalStatus::RetrievalUnavailable`。

#### Scenario: unavailable body status maps to RetrievalUnavailable
- **WHEN** 远端 `/search` 返回 HTTP 200 且 body `status` 为 `"unavailable"`
- **THEN** 检索结果状态为 `RetrievalStatus::RetrievalUnavailable`

### Requirement: Remote search maps unauthorized body status to RetrievalUnauthorized
远端检索回包 `status` 为 `unauthorized` 时，`parse_reply` SHALL 把结果状态映射为 `RetrievalStatus::RetrievalUnauthorized`。

#### Scenario: unauthorized body status maps to RetrievalUnauthorized
- **WHEN** 远端 `/search` 返回 HTTP 200 且 body `status` 为 `"unauthorized"`
- **THEN** 检索结果状态为 `RetrievalStatus::RetrievalUnauthorized`

### Requirement: Remote request sends Bearer authorization header when API key present
远端客户端在配置了 `api_key` 时 SHALL 为每个出站请求携带 `Authorization: Bearer <key>` 请求头，其值为 `Bearer ` 后接凭证原文。

#### Scenario: API key is sent as an exact Bearer header
- **WHEN** 以 `api_key = Some("test-api-key")` 构造远端客户端并发出检索请求
- **THEN** 观测到的出站请求头含 `Authorization`，其值逐字等于 `Bearer test-api-key`

### Requirement: Remote request omits authorization header when no API key
远端客户端在未配置 `api_key` 时 SHALL 不携带 `Authorization` 请求头，不得凭空构造凭证头。

#### Scenario: No API key means no Authorization header
- **WHEN** 以 `api_key = None` 构造远端客户端并发出检索请求
- **THEN** 观测到的出站请求**不含** `Authorization` 头

### Requirement: Remote request normalizes trailing slash in endpoint
远端客户端 SHALL 在拼接请求 URL 前去掉 endpoint 结尾的斜杠，使最终路径不含重复斜杠。

#### Scenario: Single trailing slash in endpoint is trimmed
- **WHEN** endpoint 以单个 `/` 结尾（如 `http://127.0.0.1:{port}/`）并发出检索请求
- **THEN** 观测到的请求路径为 `/search`（而非 `//search`）

### Requirement: Remote request normalizes repeated trailing slashes in endpoint
远端客户端 SHALL 去掉 endpoint 结尾的**全部**斜杠，而非仅去掉一个。

#### Scenario: Repeated trailing slashes in endpoint are trimmed
- **WHEN** endpoint 以多个 `/` 结尾（如 `http://127.0.0.1:{port}///`）并发出检索请求
- **THEN** 观测到的请求路径为 `/search`（而非 `///search`）

### Requirement: Remote search returns unavailable when retry backoff exceeds remaining budget
远端客户端在重试退避时长不小于当前 `Operation` 剩余预算时 SHALL 立即返回 unavailable，不得等待退避耗尽后被超时中断。

#### Scenario: Backoff that does not fit the remaining budget yields unavailable at once
- **WHEN** 远端返回 429 且 `Retry-After` 大于当前 `Operation` 的剩余预算
- **THEN** 检索结果为 `RetrievalStatus::RetrievalUnavailable`，且立即返回（不耗时等待退避）

### Requirement: Remote search stops retrying after the retry budget is exhausted
远端客户端在尝试次数达到 `max_retries` 上界后 SHALL 停止重试并返回 unavailable，尝试次数 MUST 有界。

#### Scenario: Retry budget exhaustion yields unavailable with a bounded attempt count
- **WHEN** `max_retries = 1` 且远端持续返回可重试状态（429，退避为 0）
- **THEN** 检索结果为 `RetrievalStatus::RetrievalUnavailable`，且尝试次数恰为 2（`max_retries + 1`）

### Requirement: Remote search retries server errors with exponential backoff and ignores Retry-After
远端客户端对 5xx 响应 SHALL 使用指数退避并**忽略** `Retry-After` 头（`Retry-After` 仅用于 429）。

#### Scenario: A 5xx with a large Retry-After is still retried
- **WHEN** 远端首次返回 503 且带 `Retry-After: 100`，随后返回 200，且 `Operation` 剩余预算充足
- **THEN** 客户端仍会重试并最终成功，尝试次数为 2（而非因 100s 退避而直接放弃）

### Requirement: Remote search rejects a response body beyond the read cap
远端客户端 SHALL 拒绝超过 8 MiB 读取上限的响应体并返回 invalid，不得无上限缓冲。

#### Scenario: A response body over 8 MiB is rejected as invalid
- **WHEN** 远端返回一个合法但超过 8 MiB 的响应体
- **THEN** 检索结果为 `Err(RemoteError::Invalid)`（而非被完整缓冲后正常解析）

### Requirement: The published snapshot bound is the sum over the selected documents
快照发布 SHALL 按**所选文档的合计**判定分块数与分块字节的上限；两份各自合法的文档合起来越界时 MUST 被拒绝，且计数越界与字节越界 MUST 是**可区分**的具名拒绝。

#### Scenario: Two admissible documents whose sum crosses a bound
- **WHEN** 合集内每份文档单独看都在上限内
- **THEN** 该合集仍可被拒绝
- **WHEN** 越界来自分块数而字节远低于上限
- **THEN** 拒绝理由指向计数上限
- **WHEN** 越界来自合计字节而分块数远低于上限
- **THEN** 拒绝理由指向字节上限
