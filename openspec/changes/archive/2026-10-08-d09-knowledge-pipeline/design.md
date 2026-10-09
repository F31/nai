# 设计

新增 `crates/nai-knowledge-core`（layer=shared，依赖 contracts/security/tokio/reqwest，dev tiny_http）。
storage 使用 bundled SQLite 内置 FTS5 与迁移 0003（FTS5 虚拟表 + 带外键的向量行）。rusqlite 0.40.2 无 `fts5` feature。shared 层允许 http，
不引入 Python sidecar；向量先提供正确有界精确检索，HNSW 留待已测规模。

## 词法与分块

中文切词：CJK 连续区间生成 unigram + bigram token，ASCII 按词界；token 长度/数量有上限，不解析非文本。
分块：按预算字符数切分，保留 source_span（行/偏移）、product_scope、acl_ref；空文本拒绝（需 OCR 的明确标记）。

## 向量空间

`VectorIndex` identity = {embedding_provider, model_version, dimension, normalization, chunker_version, content_hash}；
`nai-contracts` 复用 EmbeddingResult。normalize（L2/cosine）后暴力 top-k（候选有界），
不同 identity 不混合。真实 embedding 经 D08 Provider；账户无余额时真实向量生成 Blocked，测试用 fixture 向量。

## 本地检索

`retrieve_local`：ACL/workspace/产品/时间过滤（facts 用 valid 范围，chunks 用 doc_revision valid 范围）
→ 精确 facts 精确匹配 → 中文 FTS MATCH（OR 词法 token）→ 向量候选 → 融合排名（facts 优先，FTS/向量按
rank 去重合并）。Evidence 携带 source_span/revision/valid_until/authority/permission_revision；
Rerank 超时丢弃未完成结果，保留已完成合法证据。状态：有证据 complete，仅部分合法 partial，
无候选 empty，超时 timeout，服务不可用 unavailable，未授权 unauthorized。

## 远端契约

`RemoteKnowledgeClient`：GET/POST `capabilities`、`health`、`search`，可选 `answer/sync/delete`。
复用 D08 的有界策略：401/403 不重试→unauthorized；429 Retry-After 限期限内→仍失败 unavailable；
5xx/连接指数退避+jitter 初值 1 次；deadline/cancel 丢弃迟到数据。能力标志
retrieval_with_evidence/answer_with_evidence/answer_only；answer_only 仅人工参考。
凭证经 `Secret`，redact；策略/ACL 由 root 注入，不信任远端自报授权。

## 发布与撤销

`publish_snapshot`：manifest_hash（全部 chunk token/向量/事实引用的规范化哈希）与 fact_revision CAS，
同事务写 knowledge_snapshots + vector_index_manifests + 索引引用；失败不产生部分快照。
`revoke_knowledge`：写 tombstone/revocation 使待播与检索引用失效，清 FTS/向量行（本地），
第三方远端删除标记为不可用/未知保留，不宣称完整擦除。

## 验证

knowledge suite：knowledge-core 纯算法（切词/分块/向量/融合/状态）与远端 mock（capabilities/health/
search/answer/sync/delete、401/403/429/5xx/期限/取消、answer_only）。
storage：FTS 中文召回、向量身份隔离、导入上限/空拒绝、CAS 发布、撤销过滤、empty/timeout 区分。
portable 全量、契约生成无漂移、前端构建、OpenSpec strict。
真实远端知识 API：无有效远端端点/凭据（账户无余额），Blocked，不伪造 Passed。
Windows GNU 交叉检查受 D06/D08 工具链限制，如实记录。

## 2026-10-08 实施检查点（不改变上述目标）

基础算法、检索过滤、存储撤销与 mock 必需端点已实测。发布当前只具备快照元数据 CAS，
缺少 stage/index/publish 可见性、事实版本 CAS 和冻结会话绑定。远端正式 OpenAPI 与生成消费者、
answer/sync/delete 的能力门控、上传/留存政策、rerank 超时处理和重导入旧待播证据失效未完成。
这些为内部实现任务，不标外部 Blocked；D09 在满足规格前保持活动、不归档。

## 本轮发布闭环设计

迁移 0004 新建 snapshot-scoped 文档/分块/FTS/向量/事实副本、build 预期版本、活动指针、文档 revision 撤销记录与冻结 session 绑定。
现有 import/index 表改作有界准备区，不在普通检索中直接可见，旧 0001–0003 不改写。
`stage_snapshot` 在事务内复制明确选择的文档/分块与事实，捕获预期活动 ID、事实 fingerprint 和权限版本。
`index_snapshot` 将匹配捕获 revision 的向量与完整 identity 复制进快照；词法-only 也必须显式完成该阶段。
manifest 使用带版本的有序字段哈希，绑定 snapshot/workspace/kb、来源/ACL/有效期、分块/token、向量 identity/字节与事实版本，不接受调用者自报完整性。
`publish_snapshot` 在同一 IMMEDIATE 事务内复算 manifest，核对 captured/current facts 与权限、expected_active，切换活动指针并失效被替换文档的旧待播证据；失败全部回滚。
已发布快照副本不因重导入被修改；取消或重启后 staging 仍不可见。普通 retrieve_local 只选活动已发布快照。
`bind_session_knowledge` 绑定当前活动已发布 ID，一会话一次，SessionSnapshot 复用已有 knowledge_snapshot_ref，无新跨语言消息。
会话检索只读绑定 ID，并复检 live ACL/有效期/revocation/事实版本；不跟随新的活动指针。
候选依赖必须来源于绑定快照，记录 doc_revision；审批复检到期/撤销，不授权已失效旧证据。
本轮验证覆盖 staging/索引/发布/重启、活动指针与事实/权限 CAS、KB/workspace 隔离、冻结/替换/撤销与候选审批。

实施结果：发布闭环与冻结绑定已实测；doc revision 绑定元数据/分块，替换 tombstone 限 kb_scope，显式撤销覆盖 workspace 全库。
v3 数据库升级不激活旧 metadata-only 记录。备份 schema 4 已先改 JSON Schema 并生成消费者。
剩余实现为远端正式协议/能力/留存政策与可选 rerank，不再将本轮已验证发布功能列为未实现。

## 远端收尾设计

新增独立 `schemas/openapi/remote-knowledge.yaml`，定义 nai adapter profile 的 POST capabilities/health/search/answer/sync/delete，
与桌面 control API 分开；不假定供应商原生协议一致。生成 TS types、Rust serde DTO、离线 JSON Schema validators 和 endpoint constants，生成代码不手改。
本地策略在 trusted composition root 注入并绑定 source、允许 KB/操作、query/document 上传、最大留存、删除要求、授权过期与未知版本短 TTL。
每次调用复检本地策略/撤销；capabilities/留存声明只作能力约束，不当权限。未知/缺失能力拒绝；answer_only 保持 HumanReference，其他答案仍需守门。
响应严格校验有界 DTO，不 filter_map 静默丢弃坏证据；来源 ID 使用本地 source scope，未知版本不伪造并限制 TTL。
search/answer 有界只读重试；sync/delete 默认不自动重试，避免未知提交重复。删除结果是远端回执而不是第三方已完整擦除的承诺。
storage 提供本地 tombstone 后再远端 delete 的协调入口，远端失败不回滚本地失效。
可选 rerank 只重排已合法证据 ID，不接受新内容；子预算超时/异常保留 base 为 partial，整个 Operation 取消/超时返回停止。

## 2026-10-08 远端收尾实施结果

正式 remote-knowledge.yaml 已生成 Rust DTO/TS types/内嵌 JSON Schema/端点常量，无手改产物；
`RemoteGateway` 在 trusted composition root 注入 `RemotePolicy`，每次调用复检过期、允许操作、KB 白名单与上传/留存/删除政策；
answer_only 显式返回 human reference 标志；sync/delete 单次尝试并返回回执；rerank 只重排已知证据 ID。
`Store::remote_delete_targets` 提供本地 tombstone 后的 kb-scoped 远端删除目标。
mock 覆盖 gating/answer_only/单次提交/receipt/rerank 预算与未知 ID 丢弃；真实远端缺端点/凭据，Blocked。
