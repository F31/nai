# nai 知识链路（D09 完成）

## 算法与传输入口

- `nai-knowledge-core::tokenize`：中文 unigram/bigram 与混合词 token，流式处理、最多 4096 项。
- `split_document`：线性分块，保留行列 span、product_scope、acl_ref，最多 4096 块。
- `vector::VectorSpace`：维度/归一化校验、cosine 排名和有界 top-k；`rank::fuse` facts 优先并去重。
- `RemoteKnowledgeClient`：严格 DTO 校验的 capabilities/health/search/answer/sync/delete 传输。

## 发布闭环

`Store::import_document` 接受已解析文本与分块，写入准备区，**普通检索不可见**。
空文本拒绝，内容/分块内容合计与 token 总量分别最多 16 MiB，单块 token 最多 4096。
workspace_id 与 product_scope 独立。原始 content_hash 为 SHA-256；document revision 还绑定 parser、
source、ACL、有效期与分块内容，改元数据也会生成新 revision。
重导入更新准备区并清理其旧向量，不修改已发布副本。

```rust
store.import_document(&op, actor, &parsed_document)?;
store.stage_snapshot(
    &op, actor, workspace_id, snapshot_id, kb_scope,
    &reviewed_document_ids, None, previous_active_id,
)?;
let sealed = store.index_snapshot(&op, actor, workspace_id, snapshot_id)?;
store.publish_snapshot(&op, actor, &sealed)?;
```

上例为词法-only。需要向量时，经允许的 D08 API 生成并 `index_vectors` 写入准备区，stage 时传 index_ref。
向量 provider/model/dimension/normalization/chunker/content_hash identity 不可用同 ID 改写；
维度最多 8192、非有限值/全零拒绝，缺失 chunk 外键使批次回滚。
seal 要求向量覆盖所有选定块且与捕获 revision 匹配；失败不留下部分 FTS/向量副本。

stage 捕获选定文档、facts、权限版本与 previous_active_id；每快照最多 256 文档、4096 块，
文本、token、向量和 facts 各最多 16 MiB。stage/index 成功仍不可见，重启也不自动发布。
服务器 manifest 绑定文档/来源/ACL/有效期、块/token、完整向量 identity/bytes 和事实版本。
publish 同事务核对 manifest、workspace 当前事实 fingerprint、权限、活动指针和源 revision，
切换活动快照、失效旧依赖并写审计；失败保持原版本。重复发布旧已发布 ID 不回切活动指针。

## 会话、检索与候选

- `retrieve_local`：只读该 workspace/kb_scope 当前已发布快照；没有活动版本为 unavailable。
- `bind_session_knowledge`：一次绑定当前活动已发布版本，返回已有 SessionSnapshot.knowledge_snapshot_ref。
- `retrieve_session_knowledge`：只读冻结 ID；新的发布不重写绑定，失效证据也不偷偷换成新版本。
- `record_candidate`：校验引用来自冻结快照，记录 doc_revision；不能把最新 facts 塞入旧绑定。
- `approve` / `approval_valid`：复检文档期限、tombstone、session/epoch/hash、事实与审批权限。

SQL/向量候选均复检产品、文档/分块 ACL 和时间；facts 必须仍匹配当前 revision。
Evidence 带原文、来源 span、revision、有效期、产品和 permission_revision；facts 带 RevisionRef。
正常完成有证据为 complete，无合法候选为 empty；缺失所请求向量但已有合法词法/facts 时为 partial，保留结果。
本地取消/期限通过 StorageError 返回，不能误转换为 empty。

替换/移除写 kb-scoped revision tombstone，失效关联待播/审批并保守清缓存；同文档另一知识库仍可合法引用。
显式 `revoke_knowledge` 写 workspace-wide tombstone，再清索引与关联待播审批。
这不代表真实媒体进程已静音或第三方副本已擦除。

## 远端适配与本地政策门控

正式协议位于 `schemas/openapi/remote-knowledge.yaml`，与桌面 control API 独立；生成 Rust DTO、TS types、
内嵌 JSON Schema 与端点常量，生成代码不手改。`RemoteGateway` 由 trusted composition root 注入
`RemotePolicy`，每次调用复检授权过期、允许操作、KB 白名单，以及 query/document 上传、第三方留存、删除要求；
capabilities/留存声明只作能力约束，不当权限。answer_only 显式返回 human-reference 标志，不自动外发。
search/answer 有界只读重试；sync/delete 单次尝试，避免未知提交重复，返回回执而非第三方已擦除的承诺。
（W113 已把此不变量钉成可测判据：`RemoteGateway::delete` 的输入边界闭世界——空集/>16/含空 id ⇒ `Unauthorized` 不触网——与回执透传枚举闭世界守卫——`RemoteReceiptStatus` 穷尽四态、新增 `erased`/`purged` 变体会编译失败；见 [ADR 0091](adr/0091-remote-retention-delete-contract.md)、[W113 报告](reports/2026-10-15-w113-remote-retention-delete-contract.md)。`max_retention_seconds` 声明但本地无强制机制，如实记为遗留。）
（W114 已把远端网关的构造期与输入边界闭世界钉成可测判据：构造期——`RemoteGateway::new` 拒绝 policy 六类非法变体、`RemoteKnowledgeClient::new` 拒绝 endpoint/timeout/max_retries 五类非法变体（均 `Invalid` 不触网）；输入边界——`RemoteGateway::sync` 文档集空/>16/单文档 content>65536 ⇒ `Unauthorized` 且显式证明仅合法调用触网，`RemoteGateway::capabilities` 仅当首个 `allowed_operations` 为 Search/Answer 才查询远端（否则 `Unauthorized` 不触网）；见 [ADR 0092](adr/0092-remote-gateway-construction-input-bounds.md)、[W114 报告](reports/2026-10-15-w114-remote-gateway-construction-input-bounds.md)。`RemoteGateway::new` 不校验 `allowed_operations`、`capabilities` 守卫依赖「首操作」约定，如实记为当前语义边界。）
（W115 已把检索证据闭世界钉成可测判据：`rerank_evidence` 空 base 直接短路返回 `RetrievalEmpty`、不进入 `spawn_blocking`（用 panic reranker 探针证明未被调用）；reranker 返回重复 `evidence_id` 时只保留首个（`seen: BTreeSet` 去重）；reranker 闭包 panic ⇒ `JoinError` ⇒ `RemoteError::Invalid`（不静默降级为 partial/empty）；`to_evidence` 丢弃 `evidence_id` 为空的远端项（被 `search` 的 `filter_map` 掉，经公共 `search` API + Mock 覆盖）；见 [ADR 0093](adr/0093-retrieval-evidence-closed-world.md)、[W115 报告](reports/2026-10-15-w115-retrieval-evidence-closed-world.md)。unknown-id drop（W114 既有）与重复 id 去重是独立闭世界维度，分别由 `known`/`seen` 两个 `BTreeSet` 负责。）
响应为 `deny_unknown_fields` 的有界 DTO，坏响应返回 Invalid，不静默变 empty。
`rerank_evidence` 只重排已合法证据 ID，未知 ID 丢弃；子预算耗尽保留 base 为 partial，整体取消/超时停止。
`Store::remote_delete_targets` 返回本地 tombstone 后的 kb-scoped 删除目标；远端失败不回滚本地失效。

## 检查与剩余任务

```bash
cargo xtask test --suite knowledge
cargo xtask check --profile portable
cargo xtask test --suite contracts
```

最新 knowledge suite **55 项通过**（算法 11、remote mock 10、storage 34）；portable **157 项 Rust 测试通过**。
发布/恢复详情见 [发布报告](reports/2026-10-08-d09-knowledge-publication.md)。
真实远端端点/凭据缺失，付费 embedding 受 D08 无余额证据约束，真实调用为 Blocked；mock 不标真实集成为 Passed。
文件格式解析器/OCR、完整导入 UI 和媒体 preplay 装配不由这些存储/网关方法冒充实现。
D09 已满足本规格并归档；真实供应商接入仍需有效凭据与独立认证。
