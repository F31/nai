# shared-model-migration-backup Specification

## Purpose
在共享模型迁移前保护数据库、内容历史及撤权信息，提供有完整性清单的owned备份和隔离恢复验证。准备阶段须核验权限、排空及计划摘要，未完成或损坏备份不可使用；恢复不得覆盖活动数据，也不能被描述为凭据库导出、跨机器恢复或完整生产回滚能力。

## Requirements

### Requirement: 有完整性闭包的迁移备份
系统 SHALL 在排空及预览hash复检后创建包含数据库、活跃对象历史和撤权信息的owned备份，清单完成前不得记可恢复，损坏或缺失文件拒绝验证。
#### Scenario: 备份中断或文件损坏
- **WHEN** 缺完成清单或hash不匹配
- **THEN** 不返回已验证备份，不覆盖活动数据

### Requirement: 隔离恢复验证
系统 SHALL 仅在新owned验证目录恢复，拒绝越权和覆盖既有目录，保留重启撤权，不声称外部凭据已导出或生产回滚已完成。
#### Scenario: 恢复验证
- **WHEN** 可信owner恢复完整备份到隔离目录
- **THEN** 可重开读取原配置，活动数据库与Keyring保持不变
