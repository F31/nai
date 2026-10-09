# D09 发布闭环与冻结会话知识实测报告

本文件保留发布轮次结论；远端正式契约、政策门控、answer_only 与 rerank 完成后见
[完整实测报告](2026-10-08-d09-knowledge.md)（D09 已归档）。

- 日期：2026-10-08
- 环境：Linux/WSL，Rust 1.96.0，锁定 pnpm/OpenSpec/protoc/libprotobuf
- 活动变更：`openspec/changes/d09-knowledge-pipeline/`
- 本轮结论：stage/index/publish 可见性、真实 CAS、冻结引用、旧证据失效与一致备份已实现并实测。
- D09 整体仍进行中，远端正式协议与 rerank 剩余任务未完成，不归档。

## 契约优先与实现

先补 OpenSpec 场景、设计和任务，新增迁移 0004；备份 JSON Schema db_schema_version 改为 4，
执行 `cargo xtask schema` 后修改消费者。SessionSnapshot 复用已有 knowledge_snapshot_ref，无私有跨语言消息。
新增 `publication.rs` 与 snapshot-scoped 文档、块、FTS、vector、fact 副本、活动指针、build CAS 和冻结绑定。
原 import/index 表变准备区，不能直接进入普通检索。stage/index/发布分离；manifest 为服务器有序字段摘要。
原始内容 hash 与覆盖 parser/ACL/有效期/块的 revision 分开，避免只改有效期却沿用旧证据版本。
publish 同事务核对事实/权限/活动指针/源 revision，并提交活动切换、旧依赖失效、缓存失效、outbox 与审计。
会话只读取冻结 ID，失效的文档/facts 不替换为最新值；候选依赖和审批也复检。
kb-scoped 替换/移除不误伤另一库，显式撤销跨该 workspace 全库生效。

## 新增 12 项 Rust 实测

1. v3→v4 迁移保留原数据但不激活旧 metadata-only published 记录。
2. stage/index 不可见；取消发布与重启后仍读旧活动版本，可显式恢复发布。
3. 发布最后阶段注入 SQL 错误，活动指针、tombstone 与审批全部回滚。
4. facts、权限变化与两个竞争发布分别触发真实 CAS 冲突。
5. 调用者自报 hash、被篡改块、源 revision 改变或缺失词法索引不能激活。
6. 会话绑定不可替换/改作用域；旧文档失效后不跟随新活动版本。
7. 缺向量 seal 回滚 FTS；准备区重导入不破坏已发布向量；缺所请求模型仍保留合法词法为 partial。
8. 选定文档隔离不同 kb_scope/workspace。
9. 文档实际到期后已有审批与新审批均拒绝。
10. 一致备份声明 schema 4；恢复可重新授权读取知识，但旧审批/输出权限与 epoch 不恢复。
11. 同文档另一库合法引用不受单库移除影响；显式撤销使两库依赖失效。
12. 冻结事实不切换到新 live revision，不能给旧绑定候选附最新事实。

## 已执行检查

| 命令 | 结果 |
|---|---|
| `cargo xtask schema` | Passed：同源生成，无手改产物 |
| `cargo xtask test --suite knowledge` | Passed：49 项（11 算法 + 5 remote mock + 33 storage） |
| `cargo xtask check --profile portable` | Passed：151 项 Rust 测试，11 包依赖边界、fmt、Clippy `-D warnings` |
| `cargo xtask test --suite contracts` | Passed：生成无漂移、Rust 6 项、Node 10 项、Rust/TS/C++ 双向互通 9 类消息 |
| `pnpm typecheck` / `pnpm build` | Passed：desktop 与 contracts-ts |
| `OPENSPEC_TELEMETRY=0 pnpm exec openspec validate d09-knowledge-pipeline --strict --no-interactive` | Passed：本轮扩展场景、设计与任务 |
| `OPENSPEC_TELEMETRY=0 pnpm spec:validate` | Passed：10 主规格 + 1 活动变更 |
| `OPENSPEC_TELEMETRY=0 pnpm exec openspec validate --archived --no-interactive` | Passed：既有 8 归档；D09 保持活动 |

上一基础轮次为 139 项 Rust/37 项 knowledge；本轮新增 12 项存储测试后为 151/49。
上述为真实运行的本地逻辑检查；注入错误和向量 fixture 仅在 tests，不标真实集成通过。

## 状态与边界

- **已实现/实测**：本轮发布、冻结、CAS、依赖失效、迁移与备份恢复。
- **未实现**：远端正式 OpenAPI/生成消费者、可选 answer/sync/delete 与上传/留存政策门控、rerank 超时处理；D09 未归档。
- **Blocked**：真实远端缺有效端点/凭据；真实付费 embedding 仍受 D08 无余额证据约束，本轮未发付费调用。
- **未执行**：Windows 原生安装/设备/SDK/GPU、真实媒体播放与完整 T/E/U 认证；未重复此前 MinGW 缺失检查。
- **NotApplicable**：本轮不执行模型权重制作与 G0–G3，不把未启用测试视为通过。

普通 API 只能使用当前已发布快照；已发布副本不由重导入改写。显式撤销可清除对应 FTS/vector 副本，保留事实/原文/审计。
facts fingerprint 暂以 workspace 全集合做保守 CAS。容量拒绝明确，不承诺百万规模精确扫描延迟。
schema 4 当前运行包拒绝旧版本备份声明；旧备份兼容与发布维护窗口由独立升级流程落实，不静默改写旧备份。
