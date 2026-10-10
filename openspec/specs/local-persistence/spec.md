# local-persistence Specification

## Purpose
为 nai 本地工作站提供会话、配置、权威事实和审计的统一持久化边界，使数据版本、权限撤销和故障恢复可验证，且不依赖外部数据库服务或插件直接访问共享数据库。

## Requirements

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

### Requirement: Portable durable confirmation
备份与恢复在确认「已写字节落盘」时 SHALL 采用两个平台都能成功的方式。持久化确认 MUST NOT 依赖只读句柄（Windows `FlushFileBuffers` 会拒绝只读句柄），且该确认 MUST 由单处实现供各调用点共用。

#### Scenario: Backup confirms durability on either platform
- **WHEN** 在 Windows 或 Linux 上创建备份并确认其字节落盘
- **THEN** 确认成功，不因句柄的读写模式被拒绝

#### Scenario: Restore shares the same confirmation
- **WHEN** 恢复一个已校验的快照
- **THEN** 恢复路径与备份路径使用同一套持久化确认实现，不出现两个平台知识副本

### Requirement: Classified portable error reasons
存储错误 SHALL 以可移植的类别区分失败原因（文件系统类别、SQLite 结果类别）。错误 MUST NOT 被压成覆盖全部情形的单一笼统原因，MUST NOT 携带 OS 本地化文本。

#### Scenario: Filesystem classes are distinguishable
- **WHEN** 分别发生「路径不存在」与「被拒绝访问」
- **THEN** 两者的 reason 可区分，报告无需开 debug 构建即可判读

#### Scenario: SQLite classes are distinguishable
- **WHEN** SQLite 返回不同的结果类别（例如忙、损坏、不可打开、磁盘满）
- **THEN** reason 反映相应类别，而不是统一的通用文案

#### Scenario: Reason stays stable across hosts
- **WHEN** 同一失败在另一台机器或另一语言的宿主上复现
- **THEN** reason 保持一致，可与先前的报告直接比对
