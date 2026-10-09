# ADR 0010：Rust 知识算法与本地索引基础

- 日期：2026-10-08
- 状态：Accepted
- 基线：V3.5 第 25 章；OpenSpec `d09-knowledge-pipeline`

## 决策

`nai-knowledge-core` 为 shared 算法/传输库，复用 contracts/security；storage 为唯一数据库 owner。
中文候选采用 CJK unigram/bigram 与有界词 token，分块保留行列来源 span。
SQLite bundled 已启用 FTS5，无需不存在的 rusqlite `fts5` feature，也不增加常驻服务。
迁移 0003 建立 FTS5 与有外键约束的向量行；保持旧迁移不变。

向量采用有界精确 cosine 检索，index_id 对应不可变 provider/model/dimension/normalization/
chunker/content_hash identity。禁止用原 index_id 改写另一模型空间；不同空间用独立 ID。
本地证据按 workspace、产品、文档/分块 ACL 和有效时间过滤；词法与向量证据共享 chunk ID 去重。
撤销事务清索引、置文档 revoked、取消已关联的待播 utterance、撤销审批并写 invalidation outbox。

## 边界与后果

Local 无需 Python、独立向量数据库、Redis/NATS 或云主机；额外维护成本为 SQLite 迁移与 Rust 库。
不承诺百万规模延迟。远端客户端当前只实测 mock 的 capabilities/health/search，未知可核验性保持 false。
快照元数据 CAS 不等于完整发布：分阶段索引、已发布快照可见性、事实版本 CAS 与冻结会话知识绑定
仍待完成。远端可选 answer/sync/delete、上传/留存政策接线与 rerank 超时保留合法结果尚未实现。
本 ADR 不缩减 V3.5，以上缺口保留在 D09 未完成任务，完成前不归档。

## 验证

见 [D09 阶段报告](../reports/2026-10-08-d09-knowledge-pipeline.md) 与 [知识指南](../knowledge.md)。

## 后续实施

上述发布缺口已由 [ADR 0011](0011-knowledge-publication-and-session-freeze.md) 与
[发布实测报告](../reports/2026-10-08-d09-knowledge-publication.md) 补齐。
远端正式契约、政策门控、answer_only 人工引用与 rerank 超时保留已实现，D09 已归档；
完整实测见 [D09 报告](../reports/2026-10-08-d09-knowledge.md)。
