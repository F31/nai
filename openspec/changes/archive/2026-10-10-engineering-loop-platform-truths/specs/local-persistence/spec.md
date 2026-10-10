## Purpose
让本地持久化的**持久化确认**与**错误词表**在两个平台上都可达成且可判别：确认字节落盘不得依赖只读句柄，失败原因不得被压成单一笼统文案。

## ADDED Requirements

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
