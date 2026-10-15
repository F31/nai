## ADDED Requirements

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
