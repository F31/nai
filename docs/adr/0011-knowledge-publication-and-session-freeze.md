# ADR 0011：知识发布可见性与冻结会话引用

- 日期：2026-10-08
- 状态：Accepted
- 基线：V3.5 第 21、24、25 章；活动变更 `d09-knowledge-pipeline`

## 决策

原有 documents/chunks/FTS/vector 表作为准备区。迁移 0004 引入 snapshot-scoped 文档、分块、
词法/向量索引、事实副本与唯一活动指针。普通检索不直接读取准备区。
`stage_snapshot` 捕获明确选择的文档、workspace 事实 fingerprint、权限版本与预期活动 ID；
`index_snapshot` 完成词法/向量副本并生成服务器 manifest；`publish_snapshot` 复算后原子切换。
manifest 绑定有序、带类型/长度和算法版本的字段摘要，含来源、有效期、ACL、token、向量 bytes/identity 和事实。
文档原始 content_hash 与 document revision 分离；revision 绑定解析版本、作用域、有效期和分块，
避免只改 ACL/有效期却继续复用旧证据版本。

发布事务同时执行 manifest、当前事实/权限、活动指针与源 revision CAS，旧依赖失效、审批撤销、
保守缓存失效、outbox 与审计。任一失败全部回滚。重启不会自动发布 staging。
当前 facts CAS 以 workspace 全集合为一致性边界，范围保守但不漏版本变化。

## 冻结与撤销

session 一次绑定一个当前活动已发布 ID；引用写入已有 SessionSnapshot.knowledge_snapshot_ref，
不增加私有跨语言消息。发布新快照不重写绑定；新会话选择新活动版本。
会话读取旧副本时仍复检 ACL、有效期、文档撤销和事实当前 revision，失效不能替换为最新版。
候选记录使用绑定快照校验来源与 fact dependencies，记录 doc_revision；审批/预播检查复核失效状态。

版本替换/移除仅对相应 kb_scope 写 revision tombstone，避免同一文档在另一个知识库的合法引用被误伤。
显式文档/产品撤销写 workspace-wide `*` tombstone，再清本地索引和待播审批；第三方删除另属远端任务。
已发布副本不因重导入而修改，显式撤销允许清除对应 FTS/vector 副本，保留文档、事实和审计。

## 迁移与恢复

0001–0003 不改写；v3 元数据“published”不会获得新活动指针，需重新 stage/index/publish。
备份 JSON Schema 的 db_schema_version 改为 4，先生成契约再改备份消费者，实际版本取 migration::VERSION。
此运行版本不接受伪称 schema 2 的新备份；旧备份兼容/程序激活由独立升级流程明确处理。
恢复保留知识与历史冻结引用，仍撤销播放权限、审批并提高 session epoch。

## 成本与验证

不新增服务。每快照最多 256 个文档、4096 块；文本、token、向量和 facts 分别有 16 MiB 上限。
副本增加 SQLite 存储成本；超限明确拒绝，不承诺百万规模延迟。
见 [发布实测报告](../reports/2026-10-08-d09-knowledge-publication.md)。该报告验证存储逻辑，不替代 D10 守门或 D11 媒体认证。
