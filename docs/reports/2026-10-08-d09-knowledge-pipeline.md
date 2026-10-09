# D09 知识链路基础阶段实测报告

本文件保留基础轮次的历史结论；发布/冻结/CAS 与远端收尾完成后见
[完整实测报告](2026-10-08-d09-knowledge.md)（D09 已归档）。

- 日期：2026-10-08
- 环境：Linux/WSL；Rust 1.96.0；项目锁定 pnpm/OpenSpec
- 变更：`openspec/changes/d09-knowledge-pipeline/`
- 结论：基础算法、存储检索与 mock 传输已实现并实测；D09 整体进行中，未归档。

## 实现与修复

接线 knowledge 模块/公开类型，添加迁移 0003（FTS5、向量外键）与 knowledge suite/CI。
修复不存在的 rusqlite fts5 feature（bundled SQLite 已编译启用）、迁移测试硬编码版本、
OptionalExtension/借用/错误类型编译问题、FTS5 别名 MATCH、NULL 有效期被过滤、撤销 UPDATE 条件错误。
工作区与产品/知识库作用域分离；内容 hash revision 与来源 span 随证据返回。
增加产品/文档/分块 ACL 与时间过滤，向量身份不可变、缺失 chunk 回滚、重导入清旧向量。
撤销写 invalidation outbox 并取消已经关联的待播记录、撤销审批。
中文切词改流式有界处理；分块修复循环中重复全文字符计数的二次复杂度，并限制 max_chunks。
远端在途发送/分块读取轮询取消与期限，响应累积上限 8 MiB；400 不重试，未知可核验性保持 false。

## 实测结果

| 命令 | 结果 | 范围 |
|---|---|---|
| `cargo xtask test --suite knowledge` | Passed：37 项 | knowledge-core 11 + remote mock 5 + storage 21 |
| `cargo xtask check --profile portable` | Passed：139 项 Rust 测试 | 11 包依赖边界、fmt、Clippy `-D warnings`、全 workspace |
| `cargo xtask schema --check` | Passed | 27 capabilities / 107 messages，Rust/TS/C++/JSON Schema/OpenAPI 生成无漂移 |
| `pnpm typecheck` | Passed | desktop 与 contracts-ts |
| `pnpm build` | Passed | desktop 与 contracts-ts |
| `OPENSPEC_TELEMETRY=0 pnpm exec openspec validate d09-knowledge-pipeline --strict --no-interactive` | Passed | 活动变更工件格式 |
| `OPENSPEC_TELEMETRY=0 pnpm spec:validate` | Passed：11 项 | 10 个主规格 + 1 个活动变更 |
| `OPENSPEC_TELEMETRY=0 pnpm exec openspec validate --archived --no-interactive` | Passed：8 项 | 既有 D01–D08 归档；D09 未归档 |

测试覆盖中文召回与 span/revision、空/超限导入及未写入、ACL/产品/有效期过滤、向量身份冲突与维度、
批次失败回滚、快照 ID 元数据 CAS、撤销检索和待播、取消、401 不重试、400 不重试、429/500、empty/timeout 区分。
初次 portable 因新增代码 Clippy 检查失败，修复迭代/转换/参数数量/字符串长度后通过。

本轮 workspace 实际输出合计 139；前轮文档中的 122 不作为本轮测试计数依据。

## 状态边界

- **已实现/实测**：上述库与事务基础、算法、37 项 suite 与 portable 检查。
- **未实现**：完整 stage/index/publish 可见性、事实版本 CAS、冻结会话绑定、重导入旧待播证据失效、
  远端正式 OpenAPI/生成消费者、可选 answer/sync/delete 能力门控、留存/上传政策接线、rerank 超时保留证据。
- **Blocked**：真实远端缺有效端点/凭据；真实付费 embedding 沿用 D08 账户无余额证据，本轮未发付费调用。
- **未执行**：Windows 原生安装/设备/凭据/SDK/GPU 认证；已有 MinGW 工具链缺失未在本轮重测。
- **NotApplicable**：本轮无模型权重/媒体设备任务，不将其标为认证通过。

快照元数据 CAS 测试不能代替原子发布可见性测试；mock 不能代替真实远端验证。
全部 D09 需求验证前保留活动变更与未勾选任务，不提前归档。
