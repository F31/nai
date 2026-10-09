# ADR 0004：本地存储与凭据 owner

- 日期：2026-10-07
- 状态：Accepted
- 对应：D03；V3.5 第 14、24 章

## 决策

1. nai-storage 使用 rusqlite 0.40.2 的 bundled/backup/hooks，关闭默认 WASM 依赖。
   SQLite 与配置都在用户本地数据目录；不启动额外 DB 服务。
2. 单连接 owner，数据库连接保持私有；文件 try_lock 拒绝第二写者。
   外键启用，WAL + synchronous FULL、50ms busy timeout、1000 VM 指令 progress handler。
   Operation 用本地单调期限和取消标记，最长 30s；备份页循环另行检查。
   这些为可配置实现起点，不是硬实时/磁盘时延认证；P0 不进入存储或凭据锁。
3. 两份 SQL 建立 34 个表，涵盖 V3.5 全部表目录及权限/事实历史、失效 outbox、登记文件。
   checksum + user_version + 实际 DDL 对照检测漂移，事务迁移失败回滚。
   Schema 2 当前为备份组合；未来迁移/回退由维护窗口及 D17 编排。
4. SQLite 有符号 INTEGER 保存 epoch/时间/revision；超出 i64 范围明确拒绝，绝不截断 uint64。
   生成 Fact/Money 保存为 Protobuf BLOB；金额不转浮点。事实版本由 CAS 分配十进制字符串。
5. Rust config 验证使用同源 JSON Schema（jsonschema 0.58.6，禁用 HTTP/file resolve），
   读取和写入都校验；配置及 URL 不接受明文凭据。原文、许可和模型/素材元数据以有界 CAS 文件登记。
6. owner 管理方法仅可信装配层可调用；普通读写检查 workspace/actor 权限。
   principal 认证和原生进程权限不是数据库字符串证明，D05 必须再执行真实身份/文件/宿主边界。
7. 候选哈希 `sha256-canonical-v1` 的当前规范：克隆生成 Cue，清空 candidate_hash/approval_ref，
   hash_algorithm 固定该名称，以 Rust BTreeMap Protobuf 编码后 SHA-256。
   此存储绑定只证明已解码对象一致；实际 Guard、风险分类、签发和播放授权仍由 D06/D10 执行。
8. 事实变更/权限撤销同事务更新候选、审批、缓存有效状态与 outbox；权限撤销保守地撤销该 workspace 旧状态。
   outbox 保留未来运行消费者入口；本轮无媒体静音、内存缓存或跨进程撤销执行。
9. 备份由 SQLite online backup 获取 WAL 提交，按快照里的登记表复制不可变文件。
   每文件最大16MiB、最多512文件、总256MiB，清单记录哈希和版本。
   大权重/索引不默认备份；权威原文、许可及素材/模型清单必须通过登记 API 纳入。
   未登记文件不声称包含；后续导入/安装流程必须遵守该边界。
10. restore 只允许不存在的目标目录，先检查哈希、schema/DDL/外键/版本，再在旁路暂存新目录。
    会话停机/提高 epoch，旧审批/候选/缓存失效，设备和权限全部撤销。
    原目录与后备份新写入不变；不提供覆盖已有数据或提交后静默回滚。
    清单是完整性元数据，不是发布签名或加密；仅接受可信备份来源并重新授权。
11. nai-security 的 Operation 用于安全/存储调用共享的取消期限检查。
    keyring 3.6.3 windows-native 使用 Windows Credential Manager，workspace namespace 隔离逻辑引用。
    非 Windows 明确 CapabilityNotReady/Unavailable，不启用 keyring 的 mock/default 后端。
    Secret 使用 zeroize 1.9.1 且 Debug 隐藏本体；当前 broker allowlist 每次读取复检。
12. 系统调用是协作边界：前后/循环检查不代表能强制中断任意原生函数。
    Credential Manager 不能隔离同用户恶意进程，broker 不是 OS 沙箱。
    同用户不可信插件仍需 D05 受限身份/AppContainer 等真实约束；本轮不豁免。

## 验证与范围

12 项 SQLite/恢复测试与4项安全测试在 Linux/WSL 通过，安全 crate 的 Windows GNU 分支及测试代码交叉检查通过。
Windows 系统凭据读写未执行，标 Blocked；MSVC、文件 ACL/重解析点和设备/SDK 认证未实测。
新增原生库和 Schema 校验依赖，不新增云主机、常驻服务、Python 推理或付费模型请求。
