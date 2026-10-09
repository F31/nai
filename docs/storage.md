# 本地存储、备份与凭据

## 工程入口

```bash
cargo xtask test --suite storage
cargo xtask check --profile portable
```

前者真实执行 nai-storage/nai-security 测试，后者另含全部 workspace 边界/fmt/clippy/测试。
Windows 原生运行同一 suite 时会执行系统 Credential Manager 测试；非 Windows 后端返回
`CAPABILITY_NOT_READY`（Unavailable），不回退文件或 mock。

## owner 与作用域

`DataPaths::for_current_user` 使用 OS ProjectDirs 本地用户路径；`DataPaths::at` 用于明确的
部署路径/测试。创建 db/knowledge/assets/models/cache/backups 等目录；拒绝链接、重解析点及显式 UNC。
SQLite 必须在同机本地盘；路径检查不能自动认出所有映射盘/NFS，也不替代宿主权限认证。

`Store::open` 获取非阻塞 owner 文件锁并打开唯一连接。私有连接不传给插件。
owner 管理接口（建 workspace、授权、备份/恢复）只供可信装配层；普通方法检查 actor/workspace。
未来 IPC 认证后提供真实 principal，不能用模型自报身份调用这些方法。

`Operation::new` 的最长期限30秒，可通过克隆共享取消标记。
SQLite bounded busy/progress handler 和备份循环复检期限；同步 OS 调用不能强制抢占。
引擎未来应在专用有界工作队列调用 owner，P0 静音不等待该队列。

## 迁移与事实

SQL 源位于 `migrations/sqlite/0001-foundation.sql` 至 `0005-decision-log.sql`，当前 schema 5。
基础表、知识准备/发布副本、决策日志及索引/触发器由事务迁移建立；打开时检查版本、checksum 与实际 DDL。
未知未来 schema、缺表/改 DDL、未版本化旧数据库均拒绝，不伪报健康。

事实使用 D02 的 AuthoritativeFact/Money：CAS 修订、不可改历史、精确 minor units/decimal 字符串。
读取执行权限和有效期过滤，配置读写使用同源离线 JSON Schema。
事实更新和权限撤销同事务失效旧候选/审批/缓存并记录 `invalidation_outbox`。
存储审批复检 hash/session/epoch、事实时效和当前批准权限；这不是 Guard 或媒体播放授权。
outbox 的实际运行/媒体消费者已由 D06/D09 落地。

## 不可变内容与快照

`publish_artifact` 仅接受 knowledge/assets/models 类型，在允许根下使用 SHA-256 文件名，
完整文件写入后才登记数据库；失败可留下未引用文件，不把未登记文件当作有效知识。
原文、许可和素材/模型清单需经此入口登记。D09 已有已解析文本导入和发布闭环，详见 [知识指南](knowledge.md)；模型包管理与写实管线（W22）未开始。

`backup` 用 SQLite online backup，复制快照本身的登记清单而非活跃库的另一个版本。
限制：每文件16MiB，最多512项，总256MiB；这是初始上限，未来调整需匹配配置和测试。
数据库/文件哈希、schema、运行版本、契约目录 hash 和敏感范围存于 `manifest.json`。
备份声明从 migration::VERSION 取实际 schema，当前生成契约要求 db_schema_version=5；不把新数据库伪标为旧版本。
密钥本体不在该数据根或备份内；记录的 credential_ref 不是可恢复的密钥。
快照未加密/签名，不对未知来源保证真实性；保护其中的原文/事实/审计。

`restore` 只向全新不存在的目标目录恢复。验证数据后旁路暂存，再提交目录；原运行目录不变。
旧会话停止，epoch 提高，审批/待播/缓存失效，设备和权限全撤销；重新配对/授权后才可读取与准备。
旧 source 的后备份新事实和审计不被覆盖。失败暂存不是活动版本；若最终打开失败则保留新目录供诊断，
不删除可能已被使用的数据。程序激活指针、维护窗口和提交后回退由升级器（W15）完成。

恢复测试验证可读取原文/配置/事实并创建新演练会话记录，不等于已完成真实 engine preflight 或直播。

## 决策日志（R07 前置）

`decision_log` 是只追加哈希链：同 workspace 内每条 `prev_hash` 等于前一条 `entry_hash`，创世为
64 个 0；`entry_hash` 覆盖 prev_hash/kind/criticality/session/payload_hash/时间/trace_id。触发器拒绝
一切 UPDATE/DELETE，恢复流程不触碰该表；`verify_decision_chain` 逐条重算并报告首个不一致位置。

`append_decision` 按类别分级（ADR 0015 勘误）：critical（approval/cue/cancellation/duty_lease）
独立事务先持久化，失败原样报错，不先冲刷遥测；telemetry（event/observation/decision/guard/synthesis/playback_ack/expiry/usage）
失败进入容量 1024 的有界缓冲，满则丢弃最旧并累加 dropped 计数。后续遥测写入时将旧缓冲与新条目按序在同一事务重新链接，
任一失败完整回滚，提交成功才清缓冲；新遥测不能越过失败的缓冲独立落盘。`flush_decisions` 也使用整批事务。
critical 可先于降级缓冲提交，seq 是持久化顺序，created_at_ms 保留原记录时间，不宣称跨分级的事件时间全序。
缓冲统计经 `decision_buffer_stats` 暴露给诊断；持久失败的缓冲头仍可能阻止后续遥测，容量规则和丢弃计数继续可见，关键写入不受该缓冲头依赖。

playback_ack 类别已定义但**无生产端**：无 output-keeper 与真实播放 ACK，R07 因此标 implemented 而非
tested；桩 Provider 回放一致性与 ACK 生产端属 W17。哈希链只保证逻辑完整性，不防拥有写库权限者整体重写。

## 系统密钥与软件 broker

`CredentialRef::parse` 只接受 `credential:<id>`；`Secret` 不提供明文 Debug，内存使用 Zeroizing。
`NativeVault` 使用 workspace namespace 和 Windows 当前用户的系统凭据服务。
`CredentialBroker` 由可信 owner 提供/撤销 principal 的引用 allowlist，每次读取复检，过期/取消请求拒绝。
配置前端只能提交安全后端入口，不能获得 owner 或读取 API；D14 接入时落实 Tauri/角色约束。

Windows 测试仅创建唯一 test namespace 条目并清理。Linux 测试替身仅 `#[cfg(test)]`。
当前已交叉检查 Windows 代码，但真实 Windows 读写、同用户第三方进程隔离仍未验收。
