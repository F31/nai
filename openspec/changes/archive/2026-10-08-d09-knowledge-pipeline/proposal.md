# D09 知识完整链路

## Why
V3.5 第 25 章要求导入→解析→分块→索引→发布的原子管线、中文词法/向量候选与融合排名、远端 knowledge API 契约，且本地语义索引在 Rust 目标机跑通、无需 Python sidecar。

## What Changes
- `nai-knowledge-core`：中文切词、文档分块、向量空间身份与 top-k、融合排名、远端 knowledge 客户端。
- storage：FTS5 索引、向量行、导入/发布/撤销/本地检索 API（迁移 0003）。
- 本地检索先按 ACL/workspace/产品/时间过滤，再精确 facts + 中文 FTS + 向量候选融合；Rerank 超时只保留完成合法证据。
- 真实远端知识调用受账户/凭据约束，如实记录；mock 覆盖完整契约与状态。

## Capabilities
### New Capabilities
- `knowledge-pipeline`: 导入、中文 FTS、向量空间、远端契约与原子发布。

## Impact
新增 knowledge-core（shared，依赖 contracts/security），storage 增 FTS5 与 0003 迁移；
新增 knowledge suite。无 Python sidecar；向量先提供正确精确检索用于验证，百万规模 HNSW 留待已测规模实现。
真实 embedding/远端 API 消费受余额约束，不在本轮伪造。
