# Design

## Context

见 proposal。D02 已生成 AuthoritativeFact、Approval 和状态枚举；本轮复用这些 DTO。
当前 Linux/WSL，无 Windows 凭据服务；数据库与文件目录尚不存在。

## Goals / Non-Goals

**Goals:** D03 的单机存储、真实迁移/备份恢复、事实权限一致性和系统密钥边界。

**Non-Goals:** PostgreSQL、HTTP/IPC 服务、媒体执行、RAG 索引、原生宿主沙箱和升级激活编排。

## Decisions

1. rusqlite 0.40.2（bundled/backup/hooks，关闭默认 WASM 依赖），单连接 owner + OS 文件锁。
   不暴露 Connection；普通调用带共享 Operation deadline/cancellation，SQLite progress handler 与有限 busy timeout。
   WAL + synchronous FULL，短事务；媒体 P0 不使用该 owner 或密钥锁。
2. 编号 SQL 两步迁移建立 V3.5 所有表及权限/事实历史、撤销 outbox、不可变文件登记。
   每步校验 checksum 与 user_version，在事务内提交；拒绝数据库漂移及未来版本。
   SQLite epoch/时间计数用有符号 64 位整数，超限明确拒绝，不截断协议 uint64。
3. 表/方法是数据基础，不提前实现 D06 生命周期。重开/恢复将旧会话停机并提高 epoch，
   审批/未完成候选失效；事实更新和权限撤销在同事务写入 outbox，实际播放撤销由 D06/D09 消费。
4. 配置以已校验 JSON Value 保存，使用同源 JSON Schema 的离线 Rust validator，禁用 HTTP/file resolve。
   事实保存生成 Protobuf，修订 CAS 分配为十进制字符串；原文和素材元数据使用已登记不可变内容文件。
5. 新增备份 JSON Schema，Rust 使用经校验 Value，不另写一份跨语言 DTO。
   SQLite online backup 分页检查期限；对快照自身查询文件目录并复制有界不可变文件，生成哈希清单。
   恢复先在只读快照做 integrity/foreign-key/schema 检查，暂存新数据再原子改名到不存在的目标。
   不覆盖非空目的，不宣称提交后回滚已实现；失败/中断暂存目录不被当作活动数据。
6. 默认路径用 OS ProjectDirs 的用户本地数据位置；测试提供临时路径。
   SQLite 要求同机本地盘；拒绝链接/重解析点和显式 UNC，目录锁不能替代 D05 文件/宿主权限沙箱。
7. Windows native keyring 3.6.3 windows-native（目标专有依赖），安全 API 不引入本地 unsafe。
   namespace 绑定 workspace；Secret 缓冲 zeroize，Debug 隐藏本体；broker 当前 allowlist 每次检查。
   单用户 Credential Manager 不隔离同用户恶意原生进程，D05 仍须真实受限宿主执行。
8. `cargo xtask test --suite storage` 运行两个 crate 的真实测试；Windows 的系统密钥测试仅在那里执行。

## Risks / Trade-offs

- 同步系统调用不能被任意强杀 → 前后/循环检查取消与期限，保留独立媒体静音；不承诺中断任意原生忙循环。
- 文件与 DB 不是同一事务 → 不可变文件先完整写入再登记；失败可留无引用孤儿，但不产生缺失文件的提交引用。
- 软件 ACL 不等于 OS 隔离 → 明确 owner 方法仅可信装配层，principal 不取自模型字段；后续 IPC/宿主单独验收。
- 恢复旧快照可能丢新写入 → 仅允许全新目的目录，旧目录及审计不改动，旧权限统一撤销。

## Migration Plan

首次建立 Schema 1→2，没有既有产品数据库。以后升级须先一致备份和维护窗口，由 D17 编排。
测试注入坏 SQL/checksum/截断快照；不提供不可验证的降级迁移。

## Verification

storage suite、portable、生成无漂移及完整 contracts suite、前端类型/构建、OpenSpec strict。
重点真实 SQLite 事务回滚、WAL 快照、授权/事实跨域、恢复旧审批与撤销身份、文件哈希/路径/数量、取消及唯一 owner。
Windows 可交叉编译检查安全 crate，但原生密钥读写必须有真实 Windows 结果，不能由编译替代。
