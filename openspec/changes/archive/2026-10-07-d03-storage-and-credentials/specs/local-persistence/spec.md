# Local Persistence

## Purpose

为 nai 本地工作站提供会话、配置、权威事实和审计的统一持久化边界，使数据版本、权限撤销和故障恢复可验证，且不依赖外部数据库服务或插件直接访问共享数据库。

## ADDED Requirements

### Requirement: Single owner and checked migrations
存储 SHALL 使用单写 owner、外键与关键提交持久化，覆盖 V3.5 表目录；编号迁移必须有 checksum/兼容范围，事务失败不留下半迁移，缺失/漂移或未知新版本必须拒绝打开。

#### Scenario: Migration failure or drift
- **WHEN** 迁移中途失败、已有 checksum 被修改或数据库版本超出支持范围
- **THEN** 事务回滚或打开失败，不跳过问题并宣称健康

#### Scenario: Second writer or expired operation
- **WHEN** 同一目录已有 owner，或操作已取消/过期
- **THEN** 拒绝第二写 owner 或停止操作，不无限等待

### Requirement: Scoped facts and revocation consistency
事实/配置读取 SHALL 执行 workspace/actor 权限过滤；事实发布必须使用精确值与 CAS 版本，事务内更新历史/审计和依赖旧事实的审批、候选、缓存状态及撤销记录。

#### Scenario: Concurrent revision and permission change
- **WHEN** 更新使用旧版本、跨空间权限不足，或原权限被撤销
- **THEN** 写入冲突/读取拒绝，旧批准对象不能继续被存储层判为有效

### Requirement: Complete consistent backup
备份 SHALL 使用在线一致快照包含已提交 WAL 数据，并记录 schema/版本组合、数据库与已登记文件的哈希、创建时间及敏感范围；文件数量/大小必须有界，拒绝路径穿越和链接逃逸。

#### Scenario: Backup after WAL commit
- **WHEN** 新事实已提交但尚在 WAL 中时创建备份
- **THEN** 快照仍包含该提交，数据库和文件清单均可独立校验

### Requirement: Fresh destination recovery
恢复 SHALL 先校验完整性、外键、schema 与版本兼容，仅写入全新目的目录；恢复后旧审批/候选/设备授权失效，会话不恢复自动模式且 epoch 不与旧任务相同。

#### Scenario: Existing data or tampered backup
- **WHEN** 目的目录已有数据，或数据库/文件/清单不完整
- **THEN** 恢复失败，已有事实/审计不被覆盖

#### Scenario: Clean machine recovery
- **WHEN** 有效快照恢复到全新目录
- **THEN** 配置/事实/原文可重新读取或复核，新演练会话可创建，旧授权不得自动恢复
